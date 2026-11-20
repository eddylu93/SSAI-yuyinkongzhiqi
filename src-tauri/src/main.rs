#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod app;
mod agent;
mod audio;
mod bridge;
mod direct;
mod input_test;
mod lighting;
mod composer;
mod installed_apps;
mod vendor_hid;
mod au05_protocol;
#[cfg(windows)] #[path="raw_windows.rs"] mod device;
#[cfg(target_os="macos")] #[path="raw_mac.rs"] mod device;
mod model;
mod platform;
mod runtime;
mod ranking;
mod speech;
fn data_dir()->std::path::PathBuf{
    #[cfg(windows)] let root=std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default());
    #[cfg(target_os="macos")] let root=std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("Library/Application Support");
    root.join("app.au05desk.desktop")
}
fn main() {
    match std::env::args().nth(1).as_deref(){
        Some("--lights")=>match vendor_hid::lights(None){Ok(v)=>println!("{}",serde_json::json!(v)),Err(e)=>{eprintln!("{e}");std::process::exit(1)}},
        Some("--observe-input")=>{
            let seconds=std::env::args().nth(2).unwrap_or_else(||"15".into()).parse::<u64>();
            let result=seconds.map_err(|_|"请输入以秒为单位的观察时长".to_owned()).and_then(vendor_hid::observe);
            match result{Ok(v)=>println!("{v}"),Err(e)=>{eprintln!("{e}");std::process::exit(1)}}
        },
        Some(arg @ ("--receiver-status"|"--test-device-session"))=>match vendor_hid::probe(arg=="--test-device-session"){Ok(v)=>println!("{v}"),Err(e)=>{eprintln!("{e}");std::process::exit(1)}},
        Some("--hid")=>match device::inspect(){Ok(list)=>println!("{}",serde_json::json!(list)),Err(e)=>{eprintln!("{e}");std::process::exit(1)}},
        Some("--mcp")=>agent::serve(),
        Some("--devices")=>println!("{}",serde_json::json!(audio::names().unwrap_or_default())),
        _=>{
            let dir=data_dir();
            std::fs::create_dir_all(&dir).expect("无法创建 SSAI-语音控制器 数据目录");
            let lock=std::fs::OpenOptions::new().read(true).write(true).create(true)
                .truncate(false).open(dir.join("desktop.lock")).expect("无法打开单实例锁");
            match lock.try_lock(){
                Ok(())=>app::run(),
                Err(std::fs::TryLockError::WouldBlock)=>{
                    let _=std::fs::write(dir.join("show-window"),[]);
                }
                Err(e)=>eprintln!("无法锁定 SSAI-语音控制器：{e}")
            }
        }
    }
}
