use crate::{audio,bridge,model::Config,runtime::{Runtime,Shared}};
use serde_json::Value;
use std::sync::{Arc,Mutex};
use tauri::{Manager,State};
use rusqlite::params;

#[tauri::command]
fn open_project(releases: bool) -> Result<(), String> {
    let repository = include_str!("../../src/repository.txt").trim();
    let url = if releases { format!("{repository}/releases/latest") } else { repository.to_owned() };
    #[cfg(windows)]
    {
        use std::ptr::{null, null_mut};
        use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
        let target: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
        let result = unsafe { ShellExecuteW(null_mut(), null(), target.as_ptr(), null(), null(), SW_SHOWNORMAL) };
        if result as isize <= 32 { return Err("无法打开默认浏览器".into()) }
    }
    #[cfg(target_os = "macos")]
    if !std::process::Command::new("/usr/bin/open").arg(url).status().map_err(|e| e.to_string())?.success() {
        return Err("无法打开默认浏览器".into());
    }
    Ok(())
}

#[tauri::command]
fn window_action(window:tauri::Window, action:String)->Result<(),String>{
    match action.as_str(){
        "hide"=>window.hide(),
        "minimize"=>window.minimize(),
        "drag"=>window.start_dragging(),
        "zoom"=>if window.is_maximized().map_err(|e|e.to_string())?{window.unmaximize()}else{window.maximize()},
        _=>return Err("未知窗口操作".into()),
    }.map_err(|e|e.to_string())
}

#[tauri::command]
fn state(desk:State<'_,Shared>)->Value{desk.lock().unwrap().snapshot()}
#[tauri::command]
async fn lighting(change:Option<crate::lighting::Change>,desk:State<'_,Shared>)->Result<crate::lighting::Lights,String>{
    let result=tauri::async_runtime::spawn_blocking(move||crate::vendor_hid::lights(change)).await.map_err(|e|e.to_string())??;
    let mut runtime=desk.lock().unwrap();
    if runtime.direct.transmitter==Some(true){runtime.direct.lights=Some((std::time::Instant::now(),result.clone()));}
    Ok(result)
}
#[tauri::command]
fn config(desk:State<'_,Shared>)->Config{desk.lock().unwrap().config.clone()}
#[tauri::command]
fn default_binding(slot:String)->Result<crate::model::Action,String>{
    if !crate::model::SLOTS.contains(&slot.as_str()){return Err("未知按键手势".into())}
    Ok(Config::default().profiles[0].bindings.get(&slot).cloned().unwrap_or(crate::model::Action::None))
}
#[tauri::command]
fn save(value:Config,desk:State<'_,Shared>)->Result<Config,String>{
    let mut d=desk.lock().unwrap();d.save(value)?;Ok(d.config.clone())
}
#[tauri::command]
fn hid_calibration(command:String, role:String,desk:State<'_,Shared>)->Result<String,String>{desk.lock().unwrap().calibrate(&command,&role)}
#[tauri::command]
fn diagnostic(value:bool,desk:State<'_,Shared>){desk.lock().unwrap().set_diagnostic(value)}
#[tauri::command]
fn cancel(desk:State<'_,Shared>){desk.lock().unwrap().cancel()}
#[tauri::command]
fn try_focus(target:Option<String>,desk:State<'_,Shared>)->Result<(),String>{desk.lock().unwrap().try_focus(target.as_deref().unwrap_or("codex"))}
#[tauri::command]
fn try_sequence(id:String,desk:State<'_,Shared>)->Result<(),String>{desk.lock().unwrap().try_sequence(id)}
#[tauri::command]
fn devices()->Result<Vec<String>,String>{audio::names()}
#[tauri::command]
async fn installed_apps()->Result<Vec<crate::installed_apps::InstalledApp>,String>{
    #[cfg(windows)]
    { tauri::async_runtime::spawn_blocking(crate::installed_apps::discover).await.map_err(|e|e.to_string())? }
    #[cfg(not(windows))]
    { Ok(Vec::new()) }
}
#[tauri::command]
async fn sample(name:String)->Result<String,String>{tauri::async_runtime::spawn_blocking(move||audio::record(name)).await.map_err(|e|e.to_string())?}
#[tauri::command]
fn stats(since:i64,desk:State<'_,Shared>)->Result<Value,String>{desk.lock().unwrap().stats(since)}
#[tauri::command]
fn ranking(since:i64,desk:State<'_,Shared>)->Result<Vec<crate::ranking::VoiceSessionUsageRecord>,String>{
    let d=desk.lock().unwrap();
    let mut q=d.db.prepare("SELECT coalesce(end_ms,stamp*1000),duration FROM usage WHERE stamp>=? AND control='K1' AND interrupted=0 AND duration>0 ORDER BY duration DESC,stamp DESC LIMIT 10").map_err(|e|e.to_string())?;
    let rows=q.query_map([since],|r|Ok(crate::ranking::VoiceSessionUsageRecord{ended_at_unix_ms:r.get(0)?,duration_seconds:r.get::<_,i64>(1)? as f64/1000.,application_name:None})).map_err(|e|e.to_string())?;
    Ok(crate::ranking::normalized_voice_session_ranking(rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?))
}
#[tauri::command]
fn clear_stats(desk:State<'_,Shared>)->Result<(),String>{desk.lock().unwrap().db.execute("DELETE FROM usage",[]).map(|_|()).map_err(|e|e.to_string())}
#[tauri::command]
fn browse_history(search:String,app:String,since:i64,until:Option<i64>,offset:u32,desk:State<'_,Shared>)->Result<Value,String>{
    browse(&desk.lock().unwrap().db,&search,&app,since,until,offset).map_err(|e|e.to_string())
}
fn browse(db:&rusqlite::Connection,search:&str,app:&str,since:i64,until:Option<i64>,offset:u32)->rusqlite::Result<Value>{
    let mut apps=db.prepare("SELECT DISTINCT app FROM history ORDER BY app")?;
    let apps=apps.query_map([],|r|r.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()?;
    let mut query=db.prepare("SELECT id,app,stamp,text FROM history WHERE instr(text,?1)>0 AND (?2='' OR app=?2) AND stamp>=?3 AND (?4 IS NULL OR stamp<?4) ORDER BY stamp DESC,id LIMIT 51 OFFSET ?5")?;
    let rows=query.query_map(params![search,app,since,until,offset],|r|Ok(serde_json::json!({"id":r.get::<_,String>(0)?,"app":r.get::<_,String>(1)?,"stamp":r.get::<_,i64>(2)?,"text":r.get::<_,String>(3)?})))?.collect::<Result<Vec<_>,_>>()?;
    Ok(serde_json::json!({"rows":rows,"apps":apps}))
}
#[tauri::command]
fn clear_history(desk:State<'_,Shared>)->Result<(),String>{desk.lock().unwrap().db.execute("DELETE FROM history",[]).map(|_|()).map_err(|e|e.to_string())}

#[cfg(test)]
mod history_tests {
    use super::browse;
    use rusqlite::{params, Connection};
    #[test]
    fn filters_before_paging_and_excludes_next_midnight() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE history(id TEXT PRIMARY KEY, app TEXT, stamp INTEGER, text TEXT)").unwrap();
        for i in 0..55 {
            db.execute("INSERT INTO history VALUES(?1,'notes',?2,'会议')",params![format!("{i:02}"),100+i]).unwrap();
        }
        db.execute_batch("INSERT INTO history VALUES('other','mail',102,'会议');INSERT INTO history VALUES('different','notes',103,'周末')").unwrap();
        let first=browse(&db,"会议","notes",100,Some(154),0).unwrap();
        assert_eq!(first["rows"].as_array().unwrap().len(),51);
        assert_eq!(first["rows"][0]["id"],"53");
        assert_eq!(first["apps"],serde_json::json!(["mail","notes"]));
        let next=browse(&db,"会议","notes",100,Some(154),50).unwrap();
        assert_eq!(next["rows"].as_array().unwrap().len(),4);
        assert_eq!(next["rows"][0]["id"],"03");
    }
}
#[tauri::command]
fn delete_entry(id:String,desk:State<'_,Shared>)->Result<(),String>{desk.lock().unwrap().db.execute("DELETE FROM history WHERE id=?",params![id]).map(|_|()).map_err(|e|e.to_string())}
#[tauri::command]
fn voice_settings()->Result<(),String>{
    #[cfg(windows)]{std::process::Command::new("explorer.exe").arg("ms-settings:sound").spawn().map_err(|e|e.to_string())?;}
    #[cfg(target_os="macos")]{std::process::Command::new("open").arg("x-apple.systempreferences:com.apple.Sound-Settings.extension").spawn().map_err(|e|e.to_string())?;}
    Ok(())
}
#[tauri::command]
fn login_start(enabled:Option<bool>)->Result<bool,String>{
    #[cfg(windows)]{
        use std::os::windows::process::CommandExt;
        let reg=std::path::PathBuf::from(std::env::var_os("SystemRoot").ok_or("找不到 Windows 目录")?).join("System32/reg.exe");
        let key=r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
        let call=|args:&[&str]|std::process::Command::new(&reg).args(args).creation_flags(0x08000000).output().map(|v|v.status.success()).map_err(|e|e.to_string());
        let current=call(&["query",key,"/v","AU05Desk"])?;
        if let Some(yes)=enabled{
            if yes==current{return Ok(current)}
            let path=format!("\"{}\"",std::env::current_exe().map_err(|e|e.to_string())?.display());
            let ok=if yes{call(&["add",key,"/v","AU05Desk","/t","REG_SZ","/d",&path,"/f"])?}else{call(&["delete",key,"/v","AU05Desk","/f"])?};
            if !ok{return Err("登录启动设置未保存，请检查当前用户权限".into())}
            return Ok(yes)
        }
        Ok(current)
    }
    #[cfg(target_os="macos")]{
        let folder=std::path::PathBuf::from(std::env::var_os("HOME").ok_or("无法找到用户目录")?).join("Library/LaunchAgents");
        let path=folder.join("app.au05desk.desktop.plist");
        if let Some(yes)=enabled {
            if yes{
                std::fs::create_dir_all(&folder).map_err(|e|e.to_string())?;
                let exe=std::env::current_exe().map_err(|e|e.to_string())?.display().to_string().replace('&',"&amp;").replace('<',"&lt;").replace('>',"&gt;");
                let temp=path.with_extension("tmp");
                std::fs::write(&temp,format!(r#"<plist version="1.0"><dict><key>Label</key><string>app.au05desk.desktop</string><key>ProgramArguments</key><array><string>{exe}</string></array><key>RunAtLoad</key><true/></dict></plist>"#)).map_err(|e|e.to_string())?;
                std::fs::rename(temp,&path).map_err(|e|e.to_string())?;
            }else if path.exists(){std::fs::remove_file(&path).map_err(|e|e.to_string())?}
        }
        Ok(path.exists())
    }
}
pub fn run(){
    use tauri::menu::{Menu,MenuItem};
    use tauri::tray::TrayIconBuilder;
    let app=tauri::Builder::default()
      .invoke_handler(tauri::generate_handler![open_project,window_action,login_start,state,lighting,config,default_binding,save,hid_calibration,diagnostic,cancel,try_focus,try_sequence,devices,installed_apps,sample,stats,ranking,clear_stats,browse_history,clear_history,delete_entry,voice_settings,grant,revoke,revoke_all,clients])
      .setup(|app|{
        let dir=crate::data_dir();
        let runtime=Runtime::open(&dir).map_err(std::io::Error::other)?;
        crate::agent::setup(&runtime.db)?;
        let desk=Arc::new(Mutex::new(runtime));app.manage(desk.clone());bridge::start(desk.clone());crate::device::start(desk.clone());
        app.manage(crate::vendor_hid::Worker::start(desk.clone()));
        let handle=app.handle().clone();
        let show=dir.join("show-window");
        std::thread::spawn(move||{
            let mut next_check=std::time::Instant::now();
            loop{
                std::thread::sleep(std::time::Duration::from_millis(15));
                desk.lock().unwrap().tick();
                if std::time::Instant::now()>=next_check {
                    next_check=std::time::Instant::now()+std::time::Duration::from_millis(500);
                    if show.is_file() && std::fs::remove_file(&show).is_ok(){
                        if let Some(w)=handle.get_webview_window("main"){let _=w.show();let _=w.unminimize();let _=w.set_focus();}
                    }
                }
            }
        });
        let open=MenuItem::with_id(app,"open","打开 SSAI-语音控制器",true,None::<&str>)?;
        let quit=MenuItem::with_id(app,"quit","退出",true,None::<&str>)?;
        let menu=Menu::with_items(app,&[&open,&quit])?;
        TrayIconBuilder::new().icon(app.default_window_icon().unwrap().clone()).tooltip("SSAI-语音控制器").menu(&menu)
            .on_menu_event(|app,event|match event.id.as_ref(){
                "open"=>if let Some(w)=app.get_webview_window("main"){let _=w.show();let _=w.set_focus();},
                "quit"=>app.exit(0),_=>{}
            }).build(app)?;
        Ok(())
      })
      .on_window_event(|w,event|if let tauri::WindowEvent::CloseRequested{api,..}=event{api.prevent_close();let _=w.hide();})
      .build(tauri::generate_context!()).expect("无法启动 SSAI-语音控制器");
    app.run(|handle,event|if let tauri::RunEvent::Exit=event{handle.state::<crate::vendor_hid::Worker>().stop();handle.state::<Shared>().lock().unwrap().cleanup()});
}

#[tauri::command]
fn clients(desk:State<'_,Shared>)->Result<Vec<Value>,String>{crate::agent::list(&desk.lock().unwrap().db).map_err(|e|e.to_string())}
#[tauri::command]
fn grant(name:String,desk:State<'_,Shared>)->Result<String,String>{
    let d=desk.lock().unwrap();if !d.config.agent{return Err("请先启用本地 Agent 访问".into())}
    crate::agent::grant(&d.db,&name)
}
#[tauri::command]
fn revoke(id:String,desk:State<'_,Shared>)->Result<(),String>{desk.lock().unwrap().db.execute("DELETE FROM grants WHERE id=?",[id]).map(|_|()).map_err(|e|e.to_string())}

#[tauri::command]
fn revoke_all(desk:State<'_,Shared>)->Result<(),String>{desk.lock().unwrap().db.execute("DELETE FROM grants",[]).map(|_|()).map_err(|e|e.to_string())}
