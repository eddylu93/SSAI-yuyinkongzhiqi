#[cfg(windows)]
mod win {
    use windows_sys::Win32::{UI::{Input::KeyboardAndMouse::*,WindowsAndMessaging::*},System::Threading::*,Foundation::*};
    use std::{mem::{size_of,zeroed},path::Path};
    fn dispatch(input:INPUT)->Result<(),String>{
        unsafe{if SendInput(1,&input,size_of::<INPUT>() as i32)==1{Ok(())}else{Err("目标窗口拒绝快捷键，请检查权限".into())}}
    }
    pub fn key(vk:u16,up:bool)->Result<(),String>{
        dispatch(shortcut_input(vk, up))
    }
    fn shortcut_input(vk: u16, up: bool) -> INPUT {
        let mut event:INPUT=unsafe{zeroed()};
        let mouse = match vk {
            1 => Some((MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, 0)),
            2 => Some((MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, 0)),
            4 => Some((MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, 0)),
            5 | 6 => Some((MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, u32::from(vk - 4))),
            _ => None,
        };
        if let Some((down, release, data)) = mouse {
            event.r#type=INPUT_MOUSE;
            event.Anonymous.mi=MOUSEINPUT{dx:0,dy:0,mouseData:data,dwFlags:if up{release}else{down},time:0,dwExtraInfo:0};
            return event
        }
        event.r#type=INPUT_KEYBOARD;
        event.Anonymous.ki=KEYBDINPUT{wVk:vk,wScan:0,dwFlags:(if up{KEYEVENTF_KEYUP}else{0}) | (if matches!(vk,0xa3|0xa5|0x5b|0x5c|0x21..=0x28|0x2e){KEYEVENTF_EXTENDEDKEY}else{0}),time:0,dwExtraInfo:0};
        event
    }
    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn mouse_shortcuts_have_matching_press_and_release_events() {
            for (name, down, up, data) in [
                ("鼠标左键", MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, 0),
                ("鼠标中键", MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, 0),
                ("鼠标右键", MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, 0),
                ("鼠标后退键", MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, 1),
                ("鼠标前进键", MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, 2),
            ] {
                let vk = crate::model::vk(name).unwrap();
                for (released, flags) in [(false, down), (true, up)] {
                    let input = shortcut_input(vk, released);
                    assert_eq!(input.r#type, INPUT_MOUSE);
                    let mouse = unsafe { input.Anonymous.mi };
                    assert_eq!((mouse.dwFlags, mouse.mouseData), (flags, data));
                }
            }
            assert_eq!(shortcut_input(0x11, false).r#type, INPUT_KEYBOARD);
        }
    }
    pub fn held(v:u16)->bool{unsafe{GetAsyncKeyState(v as i32)<0}}
    pub fn wechat_chat_window() -> Option<isize> {
        unsafe {
            let window = GetForegroundWindow();
            let name = foreground().to_lowercase();
            if !matches!(name.as_str(), "wechat.exe" | "weixin.exe") { return None; }
            super::windows_launcher::is_wechat_main(window).then_some(window as isize)
        }
    }
    pub fn pointer_in_window(window:isize)->bool {
        unsafe {
            let mut point=zeroed();
            GetCursorPos(&mut point)!=0 && GetAncestor(WindowFromPoint(point),GA_ROOT) as isize == window
        }
    }
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
#[cfg(target_os="macos")] pub fn wechat_chat_window()->Option<isize>{None}
#[cfg(target_os="macos")] pub fn pointer_in_window(_:isize)->bool{false}
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
pub use windows_launcher::{Launcher, restore_wechat_for_focus, wechat_qt_window};
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
