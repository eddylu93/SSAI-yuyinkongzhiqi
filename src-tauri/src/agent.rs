use rusqlite::{params,Connection,OptionalExtension};
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::io::{BufRead,Write};
use uuid::Uuid;

pub fn setup(db:&Connection)->rusqlite::Result<()>{
    db.execute_batch("CREATE TABLE IF NOT EXISTS grants(id TEXT PRIMARY KEY,name TEXT NOT NULL,hash TEXT NOT NULL);")
}
pub fn grant(db:&Connection,name:&str)->Result<String,String>{
    if name.trim().is_empty() || name.len()>160{return Err("请输入客户端名称".into())}
    let id=Uuid::new_v4().to_string();
    let token=format!("{}{}",Uuid::new_v4().simple(),Uuid::new_v4().simple());
    let hash=format!("{:x}",Sha256::digest(token.as_bytes()));
    db.execute("INSERT INTO grants VALUES(?,?,?)",params![id,name.trim(),hash]).map_err(|e|e.to_string())?;
    let exe=std::env::current_exe().map_err(|e|e.to_string())?;
    serde_json::to_string_pretty(&json!({"mcpServers":{"au05_history":{"command":exe,"args":["--mcp"],"env":{"AU05_CLIENT_ID":id,"AU05_ACCESS_TOKEN":token}}}})).map_err(|e|e.to_string())
}
pub fn list(db:&Connection)->rusqlite::Result<Vec<Value>>{
    let mut q=db.prepare("SELECT id,name FROM grants ORDER BY name")?;
    let rows=q.query_map([],|r|Ok(json!({"id":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?})))?;
    rows.collect()
}
pub fn query(db:&Connection,id:&str,token:&str,name:&str,args:&Value)->Result<Value,String>{
    let row:Option<String>=db.query_row("SELECT body FROM settings WHERE id=1",[],|r|r.get(0)).optional().map_err(|_|"unavailable")?;
    let config:Value=serde_json::from_str(row.as_deref().unwrap_or("{}")).map_err(|_|"unavailable")?;
    let hash=format!("{:x}",Sha256::digest(token.as_bytes()));
    let granted:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM grants WHERE id=? AND hash=?)",params![id,hash],|r|r.get(0)).map_err(|_|"unavailable")?;
    if !granted || config["agent"]!=true{return Err("denied".into())}
    let mut data=Vec::new();
    match name{
        "list_transcript_apps"=>{
            let mut q=db.prepare("SELECT app,count(*) FROM history GROUP BY app ORDER BY app").map_err(|_|"unavailable")?;
            let rows=q.query_map([],|r|Ok(json!({"app":r.get::<_,String>(0)?,"count":r.get::<_,i64>(1)?}))).map_err(|_|"unavailable")?;
            for r in rows{data.push(r.map_err(|_|"unavailable")?)}
        }
        "query_transcripts"=>{
            let limit=args["limit"].as_i64().unwrap_or(20).clamp(1,100);
            let mut q=db.prepare("SELECT id,app,stamp,text FROM history WHERE (?1='' OR app=?1) AND stamp>=?2 AND stamp<=?3 AND (?4='' OR id=?4) ORDER BY stamp DESC,id LIMIT ?5 OFFSET ?6").map_err(|_|"unavailable")?;
            let rows=q.query_map(params![args["app"].as_str().unwrap_or(""),args["from"].as_i64().unwrap_or(0),args["to"].as_i64().unwrap_or(i64::MAX),args["id"].as_str().unwrap_or(""),limit,args["offset"].as_i64().unwrap_or(0).max(0)],|r|Ok(json!({"id":r.get::<_,String>(0)?,"app":r.get::<_,String>(1)?,"stamp":r.get::<_,i64>(2)?,"text":r.get::<_,String>(3)?}))).map_err(|_|"unavailable")?;
            for r in rows{data.push(r.map_err(|_|"unavailable")?)}
        }
        _=>return Err("unknown tool".into())
    }
    Ok(json!({"content":[{"type":"text","text":Value::Array(data).to_string()}]}))
}
pub fn serve(){
    let id=std::env::var("AU05_CLIENT_ID").unwrap_or_default();
    let token=std::env::var("AU05_ACCESS_TOKEN").unwrap_or_default();
    for line in std::io::stdin().lock().lines().map_while(Result::ok){
        if line.len()>65536{break}
        let Ok(req)=serde_json::from_str::<Value>(&line)else{continue};
        if req.get("id").is_none(){continue}
        let res=match req["method"].as_str(){
            Some("initialize")=>Ok(json!({"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"au05_history","version":env!("CARGO_PKG_VERSION")}})),
            Some("ping")=>Ok(json!({})),
            Some("tools/list")=>Ok(json!({"tools":[{"name":"list_transcript_apps","description":"列出已记录的应用和数量","inputSchema":{"type":"object","properties":{}}},{"name":"query_transcripts","description":"分页读取已授权的回眸","inputSchema":{"type":"object","properties":{"app":{"type":"string"},"from":{"type":"integer"},"to":{"type":"integer"},"id":{"type":"string"},"limit":{"type":"integer"},"offset":{"type":"integer"}}}}]})),
            Some("tools/call")=>request_host(&id,&token,&req["params"]),
            _=>Err("unknown method".into())
        };
        let mut reply=json!({"jsonrpc":"2.0","id":req["id"]});
        match res{Ok(r)=>reply["result"]=r,Err(msg)=>reply["error"]=json!({"code":-32000,"message":msg})}
        let mut out=std::io::stdout().lock();if writeln!(out,"{reply}").and_then(|_|out.flush()).is_err(){break}
    }
}
fn request_host(id:&str,token:&str,params:&Value)->Result<Value,String>{
    #[cfg(windows)]let mut stream=std::fs::OpenOptions::new().read(true).write(true).open(crate::bridge::endpoint()).map_err(|_|"unavailable")?;
    #[cfg(target_os="macos")]let mut stream=std::os::unix::net::UnixStream::connect(crate::bridge::endpoint()).map_err(|_|"unavailable")?;
    let request=json!({"client":id,"token":token,"name":params["name"],"args":params["arguments"]});
    writeln!(stream,"{request}").map_err(|_|"unavailable")?;
    let mut line=String::new();std::io::BufReader::new(&stream).read_line(&mut line).map_err(|_|"unavailable")?;
    let _=stream.write_all(b"ok\n");
    let value:Value=serde_json::from_str(&line).map_err(|_|"unavailable")?;
    if let Some(msg)=value["error"].as_str(){return Err(msg.into())}Ok(value["result"].clone())
}
