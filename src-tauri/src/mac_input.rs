use std::ffi::c_void;
type Event = *mut c_void;
#[link(name="ApplicationServices",kind="framework")]
extern "C" {
    fn CGEventCreateKeyboardEvent(source:Event,key:u16,down:bool)->Event;
    fn CGEventPost(tap:u32,event:Event);
    fn CGEventSetFlags(event:Event,flags:u64);
    fn CGEventSourceFlagsState(state:i32)->u64;
    fn CGEventSourceKeyState(state:i32,key:u16)->bool;
    fn CGEventKeyboardSetUnicodeString(event:Event,len:usize,text:*const u16);
    fn CGEventCreateScrollWheelEvent(source:Event,unit:u32,count:u32,scroll:i32,...)->Event;
    fn AXIsProcessTrusted()->bool;
}
#[link(name="CoreFoundation",kind="framework")]
extern "C"{fn CFRelease(v:*const c_void);}
fn mac_key(v:u16)->Option<u16>{
    let code=match v{
        65=>0,83=>1,68=>2,70=>3,72=>4,71=>5,90=>6,88=>7,67=>8,86=>9,66=>11,81=>12,87=>13,69=>14,82=>15,89=>16,84=>17,
        49=>18,50=>19,51=>20,52=>21,54=>22,53=>23,57=>25,55=>26,56=>28,48=>29,79=>31,85=>32,73=>34,80=>35,13=>36,76=>37,74=>38,75=>40,78=>45,77=>46,
        9=>48,32=>49,8=>51,27=>53,92=>54,91=>55,16|160=>56,18|164=>58,17|162=>59,161=>60,165=>61,163=>62,46=>117,
        36=>115,35=>119,33=>116,34=>121,37=>123,39=>124,40=>125,38=>126,
        112=>122,113=>120,114=>99,115=>118,116=>96,117=>97,118=>98,119=>100,120=>101,121=>109,122=>103,123=>111,124=>105,125=>107,126=>113,127=>106,128=>64,129=>79,130=>80,131=>90,
        _=>return None
    };Some(code)
}
fn post(e:Event)->Result<(),String>{unsafe{
    if e.is_null(){return Err("macOS 无法创建输入事件".into())}
    if !AXIsProcessTrusted(){CFRelease(e);return Err("请允许 SSAI-语音控制器 使用辅助功能".into())}
    CGEventPost(0,e);CFRelease(e);Ok(())
}}
pub fn key(v:u16,down_release:bool)->Result<(),String>{unsafe{
    let code=mac_key(v).ok_or("macOS 暂不支持该键值")?;
    let ev=CGEventCreateKeyboardEvent(std::ptr::null_mut(),code,!down_release);
    if !ev.is_null(){
        let flag=match code{54|55=>1<<20,56|60=>1<<17,58|61=>1<<19,59|62=>1<<18,_=>0};
        let current=CGEventSourceFlagsState(0);
        CGEventSetFlags(ev,if down_release{current & !flag}else{current | flag});
    }
    post(ev)
}}
pub fn held(v:u16)->bool{mac_key(v).is_some_and(|k|unsafe{CGEventSourceKeyState(0,k)})}
pub fn scroll(steps:i32)->Result<(),String>{unsafe{post(CGEventCreateScrollWheelEvent(std::ptr::null_mut(),1,1,steps))}}
pub fn text(s:&str)->Result<(),String>{unsafe{
    let data:Vec<u16>=s.encode_utf16().collect();if data.is_empty(){return Ok(())}
    for down in [true,false]{
        let ev=CGEventCreateKeyboardEvent(std::ptr::null_mut(),0,down);
        if !ev.is_null(){CGEventKeyboardSetUnicodeString(ev,data.len(),data.as_ptr())}
        post(ev)?;
    }
    Ok(())
}}
pub fn foreground()->String{
    let result=std::process::Command::new("/usr/bin/osascript").args(["-l","JavaScript","-e","ObjC.import('AppKit'); $.NSWorkspace.sharedWorkspace.frontmostApplication.bundleIdentifier.js"]).output();
    result.ok().filter(|v|v.status.success()).map(|v|String::from_utf8_lossy(&v.stdout).trim().to_string()).unwrap_or_default()
}
