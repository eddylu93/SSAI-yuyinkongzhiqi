#[cfg(windows)]
mod win {
    use windows_sys::Win32::{UI::{Input::KeyboardAndMouse::*,WindowsAndMessaging::*},System::Threading::*,Foundation::*};
    use std::{mem::{size_of,zeroed},path::Path};
    fn dispatch(input:INPUT)->Result<(),String>{
        unsafe{if SendInput(1,&input,size_of::<INPUT>() as i32)==1{Ok(())}else{Err("目标窗口拒绝快捷键，请检查权限".into())}}
    }
    pub fn key(vk:u16,up:bool)->Result<(),String>{
        let mut event:INPUT=unsafe{zeroed()};
        event.r#type=INPUT_KEYBOARD;
        event.Anonymous.ki=KEYBDINPUT{wVk:vk,wScan:0,dwFlags:(if up{KEYEVENTF_KEYUP}else{0}) | (if matches!(vk,0xa3|0xa5|0x5b|0x5c|0x21..=0x28|0x2e){KEYEVENTF_EXTENDEDKEY}else{0}),time:0,dwExtraInfo:0};
        dispatch(event)
    }
    pub fn held(v:u16)->bool{unsafe{GetAsyncKeyState(v as i32)<0}}
    pub fn scroll(steps:i32)->Result<(),String>{
        let mut e:INPUT=unsafe{zeroed()};e.r#type=INPUT_MOUSE;e.Anonymous.mi=MOUSEINPUT{dx:0,dy:0,mouseData:(steps*120)as u32,dwFlags:MOUSEEVENTF_WHEEL,time:0,dwExtraInfo:0};dispatch(e)
    }
    pub fn text(s:&str)->Result<(),String>{
        for c in s.encode_utf16(){for flag in [0,KEYEVENTF_KEYUP]{
            let mut e:INPUT=unsafe{zeroed()};e.r#type=INPUT_KEYBOARD;e.Anonymous.ki=KEYBDINPUT{wVk:0,wScan:c,dwFlags:KEYEVENTF_UNICODE|flag,time:0,dwExtraInfo:0};dispatch(e)?;
        }}Ok(())
    }
    pub fn foreground()->String{unsafe{
        let mut pid=0;GetWindowThreadProcessId(GetForegroundWindow(),&mut pid);
        let p=OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION,0,pid);
        if p.is_null(){return String::new()}
        let mut buf=[0u16;1024];let mut len=buf.len() as u32;
        let ok=QueryFullProcessImageNameW(p,0,buf.as_mut_ptr(),&mut len);CloseHandle(p);
        if ok==0{return String::new()}
        Path::new(&String::from_utf16_lossy(&buf[..len as usize])).file_name().unwrap_or_default().to_string_lossy().into_owned()
    }}
}
#[cfg(windows)] pub use win::*;
#[cfg(target_os="macos")]
#[path="mac_input.rs"]
mod mac;
#[cfg(target_os="macos")] pub use mac::*;
pub fn press(keys:&[String])->Result<Vec<u16>,String>{
    let mut owned=Vec::new();
    for k in keys{
        let vk=crate::model::vk(k).ok_or("未知快捷键")?;
        if !held(vk){
            if let Err(e)=key(vk,false){release(&mut owned);return Err(e)}
            owned.push(vk);
        }
    }
    Ok(owned)
}
pub fn release(keys:&mut Vec<u16>){while let Some(k)=keys.pop(){let _=key(k,true);}}
pub fn tap(keys:&[String])->Result<(),String>{let mut owned=press(keys)?;release(&mut owned);Ok(())}
pub fn valid_launch_target(app:&str)->bool {
    #[cfg(windows)]
    if crate::installed_apps::package_id(app).is_some(){return true}
    std::path::Path::new(app).is_absolute() && app.to_ascii_lowercase().ends_with(if cfg!(windows){".exe"}else{".app"})
}
#[cfg(windows)]
#[path = "windows_launcher.rs"]
mod windows_launcher;
#[cfg(windows)]
pub use windows_launcher::Launcher;
#[cfg(windows)]
pub fn launch_is_foreground(target: &str, _: &str) -> bool { windows_launcher::is_foreground(target) }

#[cfg(target_os="macos")]
pub fn launch_is_foreground(target: &str, foreground: &str) -> bool { foreground.eq_ignore_ascii_case(target) }
#[cfg(target_os="macos")]
#[derive(Default)]
pub struct Launcher;
#[cfg(target_os="macos")]
impl Launcher {
    pub fn cancel(&mut self) {}
    pub fn waiting_for(&self, _: &str) -> bool { false }
    pub fn poll(&mut self) -> Option<(String,String)> { None }
    pub fn request(&mut self, app:&str)->Result<String,String>{
        if !valid_launch_target(app){return Err("应用启动目标无效".into())}
        let path=std::path::Path::new(app);
        if !path.is_dir(){return Err("找不到应用程序包".into())}
        let plist=path.join("Contents/Info.plist");
        let id=std::process::Command::new("/usr/libexec/PlistBuddy").args(["-c","Print CFBundleIdentifier"]).arg(plist).output().map_err(|_|"无法读取应用标识")?;
        if !id.status.success(){return Err("无法读取应用标识".into())}
        let status=std::process::Command::new("/usr/bin/open").arg(path).status().map_err(|_|"无法打开应用")?;
        if !status.success(){return Err("无法打开应用".into())}
        Ok(String::from_utf8_lossy(&id.stdout).trim().into())
    }
}
