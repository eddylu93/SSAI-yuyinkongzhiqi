use crate::audio;
use std::{io::Write,process::{Command,Stdio},sync::mpsc::{self,Sender,Receiver},time::Duration};
use serde_json::json;
use base64::Engine;
#[cfg(windows)]
fn shell(script:&str,input:&str)->Result<String,String>{
    use std::os::windows::process::CommandExt;
    let exe=std::path::PathBuf::from(std::env::var_os("SystemRoot").ok_or("系统目录不可用")?).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let mut child=Command::new(exe).args(["-NoLogo","-NoProfile","-NonInteractive","-Command",script]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).creation_flags(0x08000000).spawn().map_err(|_|"系统识别组件未启动")?;
    if let Some(mut stdin)=child.stdin.take(){stdin.write_all(input.as_bytes()).map_err(|_|"系统组件通信失败")?;}
    let output=child.wait_with_output().map_err(|_|"系统组件通信中断")?;
    if !output.status.success(){return Err("系统识别或目标文本确认失败".into())}
    String::from_utf8(output.stdout).map_err(|_|"系统组件返回文本无效".into())
}
#[cfg(not(windows))]
fn shell(_: &str,_: &str)->Result<String,String>{Err("该系统识别器仅适用于 Windows".into())}
pub fn recognize(wav:&[u8])->Result<String,String>{
    let text=shell(include_str!("speech.ps1"),&base64::engine::general_purpose::STANDARD.encode(wav))?;
    if text.trim().is_empty() || text.chars().count()>16384{Err("没有可提交的识别文本".into())}else{Ok(text.trim().into())}
}
pub fn target(id:&str,text:&str)->Result<String,String>{shell(include_str!("target.ps1"),&json!({"id":id,"text":text}).to_string())}
pub struct Capture{stop:Sender<bool>,pub output:Receiver<Result<(Vec<u8>,String),String>>}
impl Capture{
    pub fn start(endpoint:String)->Self{
        let (stop,signal)=mpsc::channel();let (send,output)=mpsc::channel();
        std::thread::spawn(move||{
            let target=std::thread::spawn(||target("",""));
            let recorded=audio::capture(endpoint,Duration::from_secs(120),|timeout|signal.recv_timeout(timeout).unwrap_or(false));
            let id=target.join().unwrap_or_else(|_|Err("输入目标不可用".into()));
            let _=send.send(recorded.and_then(|wav|Ok((wav,id?))));
        });
        Self{stop,output}
    }
    pub fn finish(&self,keep:bool){let _=self.stop.send(keep);}
}
