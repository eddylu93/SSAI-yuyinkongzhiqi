use crate::runtime::Shared;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt,AsyncRead,AsyncWrite,AsyncWriteExt,BufReader};
pub fn start(state:Shared){
    tauri::async_runtime::spawn(async move{
        if let Err(e)=listen(state.clone()).await{state.lock().unwrap().message=format!("本地只读服务不可用：{e}");}
    });
}
#[cfg(windows)]
pub fn endpoint()->std::path::PathBuf{
    use sha2::{Sha256,Digest};
    let profile=std::env::var("USERPROFILE").unwrap_or_default().to_lowercase();
    let hash=format!("{:x}",Sha256::digest(profile.as_bytes()));
    format!(r"\\.\pipe\au05-desk-{}-reader",&hash[..16]).into()
}
#[cfg(windows)]
async fn listen(state:Shared)->std::io::Result<()>{
    use tokio::net::windows::named_pipe::ServerOptions;
    use windows_sys::Win32::{Foundation::{LocalFree,CloseHandle},Security::{TOKEN_QUERY,GetTokenInformation,TokenUser,TOKEN_USER,SECURITY_ATTRIBUTES,Authorization::{ConvertSidToStringSidW,ConvertStringSecurityDescriptorToSecurityDescriptorW}},System::Threading::{OpenProcessToken,GetCurrentProcess}};
    fn sid()->std::io::Result<String>{unsafe{
        let mut token=std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(),TOKEN_QUERY,&mut token)==0{return Err(std::io::Error::last_os_error())}
        let mut bytes=0;GetTokenInformation(token,TokenUser,std::ptr::null_mut(),0,&mut bytes);
        let mut buffer=vec![0usize;(bytes as usize+7)/8];
        let ok=GetTokenInformation(token,TokenUser,buffer.as_mut_ptr().cast(),bytes,&mut bytes);
        CloseHandle(token);if ok==0{return Err(std::io::Error::last_os_error())}
        let user=&*buffer.as_ptr().cast::<TOKEN_USER>();let mut ptr=std::ptr::null_mut();
        if ConvertSidToStringSidW(user.User.Sid,&mut ptr)==0{return Err(std::io::Error::last_os_error())}
        let mut n=0;while *ptr.add(n)!=0{n+=1}
        let value=String::from_utf16_lossy(std::slice::from_raw_parts(ptr,n));LocalFree(ptr.cast());Ok(value)
    }}
    let name=endpoint();
    let acl:Vec<u16>=format!("D:P(A;;GA;;;{})",sid()?).encode_utf16().chain(Some(0)).collect();
    let make=||unsafe{
        let mut descriptor=std::ptr::null_mut();
        if ConvertStringSecurityDescriptorToSecurityDescriptorW(acl.as_ptr(),1,&mut descriptor,std::ptr::null_mut())==0{return Err(std::io::Error::last_os_error())}
        let mut attrs=SECURITY_ATTRIBUTES{nLength:std::mem::size_of::<SECURITY_ATTRIBUTES>()as u32,lpSecurityDescriptor:descriptor,bInheritHandle:0};
        let result=ServerOptions::new().first_pipe_instance(true).reject_remote_clients(true).create_with_security_attributes_raw(&name,(&mut attrs as *mut SECURITY_ATTRIBUTES).cast());
        LocalFree(descriptor);result
    };
    let mut pipe=make()?;
    loop{
        pipe.connect().await?;
        let _=read_query(&mut pipe,state.clone()).await;
        pipe.disconnect()?;
    }
}
#[cfg(target_os="macos")]
pub fn endpoint()->std::path::PathBuf{
    std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Library/Application Support/AU05Desk").join("reader.sock")
}
#[cfg(target_os="macos")]
async fn listen(state:Shared)->std::io::Result<()>{
    use std::os::unix::fs::{PermissionsExt,FileTypeExt};
    use tokio::net::{UnixListener,UnixStream};
    let home=std::env::var_os("HOME").ok_or(std::io::ErrorKind::NotFound)?;
    let dir=std::path::PathBuf::from(home).join("Library/Application Support/AU05Desk");
    std::fs::create_dir_all(&dir)?;std::fs::set_permissions(&dir,std::fs::Permissions::from_mode(0o700))?;
    let path=endpoint();
    if let Ok(meta)=std::fs::symlink_metadata(&path){
        if !meta.file_type().is_socket(){return Err(std::io::ErrorKind::AlreadyExists.into())}
        if UnixStream::connect(&path).await.is_ok(){return Err(std::io::ErrorKind::AddrInUse.into())}
        std::fs::remove_file(&path)?;
    }
    let listener=UnixListener::bind(&path)?;
    std::fs::set_permissions(&path,std::fs::Permissions::from_mode(0o600))?;
    loop{let (stream,_)=listener.accept().await?;
        let _=read_query(stream,state.clone()).await;
    }
}

async fn read_query<S:AsyncRead+AsyncWrite+Unpin>(stream:S,state:Shared)->Result<(),()>{
    use tokio::io::AsyncReadExt;
    let mut reader=BufReader::new(stream);let mut line=String::new();
    tokio::time::timeout(Duration::from_secs(3),(&mut reader).take(16384).read_line(&mut line)).await.map_err(|_|())?.map_err(|_|())?;
    if !line.ends_with('\n'){return Err(())}
    let f:Value=serde_json::from_str(&line).map_err(|_|())?;
    let result=crate::agent::query(&state.lock().unwrap().db,f["client"].as_str().unwrap_or(""),f["token"].as_str().unwrap_or(""),f["name"].as_str().unwrap_or(""),&f["args"]);
    let reply=match result{Ok(v)=>json!({"result":v}),Err(e)=>json!({"error":e})};
    reader.get_mut().write_all(format!("{reply}\n").as_bytes()).await.map_err(|_|())?;
    reader.get_mut().flush().await.map_err(|_|())?;
    // Keep a Windows pipe connected until its peer has consumed the reply.
    let mut ack=String::new();
    let _=tokio::time::timeout(Duration::from_secs(2),reader.take(16).read_line(&mut ack)).await;
    Ok(())
}
