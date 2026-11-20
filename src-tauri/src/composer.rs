use std::process::Child;
use std::time::{Duration, Instant};
use std::sync::LazyLock;

#[derive(serde::Deserialize,serde::Serialize)]
#[serde(rename_all="camelCase")]
pub struct Target {
    id: String,
    pub name: String,
    processes: Vec<String>,
    start_names: Vec<String>,
    package: Option<String>,
    editor_name: String,
    editor_class: Option<String>,
    #[serde(default)] legacy: bool,
}
static TARGETS: LazyLock<Vec<Target>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../src/focus-targets.json")).expect("invalid focus target catalogue")
});
pub fn target(id: &str) -> Option<&'static Target> {
    TARGETS.iter().find(|t| t.id == id)
}

pub struct FocusJob {
    child: Child,
    started: Instant,
    pub target: &'static Target,
}
impl FocusJob {
    #[cfg(windows)]
    pub fn start(target: &'static Target) -> Result<Self, String> {
        use base64::Engine;
        use std::{
            os::windows::process::CommandExt,
            process::{Command, Stdio},
        };
        let exe = std::path::PathBuf::from(
            std::env::var_os("SystemRoot").ok_or("Windows 系统目录不可用")?,
        )
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let utf16: Vec<_> = include_str!("focus_chat.ps1")
            .trim_start_matches('\u{feff}')
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        let command = base64::engine::general_purpose::STANDARD.encode(utf16);
        let mut process = Command::new(exe);
        if target.id == "wechat" {
            process.env("AU05_WECHAT_HELPER", include_str!("wechat_composer.cs"));
        }
        let child = process.args(["-NoProfile", "-NonInteractive", "-EncodedCommand", &command])
            .env("AU05_FOCUS_TARGET", serde_json::to_string(target).map_err(|e| e.to_string())?)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|e| format!("无法启动输入框聚焦：{e}"))?;
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(child.id());
        }
        Ok(Self {
            target,
            child,
            started: Instant::now(),
        })
    }
    #[cfg(not(windows))]
    pub fn start(_: &'static Target) -> Result<Self, String> {
        Err("聊天框聚焦目前支持 Windows 桌面应用".into())
    }
    pub fn poll(&mut self) -> Option<Result<(), String>> {
        match self.child.try_wait() {
            Ok(Some(status)) => Some(match status.code() {
                Some(0) => self.verify_window(),
                Some(2) => Err(format!("未找到唯一的 {} 窗口，请先打开目标对话",self.target.name)),
                Some(3) => Err(format!("系统未允许切换到 {}",self.target.name)),
                _ => Err(format!("未找到可确认的 {} 聊天框，请先进入对话",self.target.name)),
            }),
            Ok(None) if self.started.elapsed() < Duration::from_secs(18) => None,
            _ => Some(Err(format!("{} 输入框聚焦超时或中断",self.target.name))),
        }
    }
    #[cfg(windows)]
    fn verify_window(&mut self) -> Result<(), String> {
        use std::io::Read;
        let mut result = String::new();
        if let Some(mut out) = self.child.stdout.take() {
            out.read_to_string(&mut result).map_err(|e| e.to_string())?;
        }
        let front = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() } as isize;
        if front != 0 && result.trim().parse::<isize>() == Ok(front) { Ok(()) }
        else { Err(format!("焦点已离开 {}",self.target.name)) }
    }
    #[cfg(not(windows))]
    fn verify_window(&mut self) -> Result<(), String> { Err("聊天框聚焦需要 Windows".into()) }
}
impl Drop for FocusJob {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{Action,Config};
    use std::collections::HashSet;

    #[test]
    fn focus_targets_round_trip_and_legacy_codex_still_loads() {
        let mut config = Config::default();
        let mut ids = HashSet::new();
        for entry in super::TARGETS.iter() {
            assert!(ids.insert(&entry.id));
            assert!(!entry.processes.is_empty() && !entry.editor_name.is_empty());
            let action = Action::FocusChat { target: entry.id.clone() };
            config.profiles[0].bindings.insert("DIAL.double".into(), action);
            let decoded: Config = serde_json::from_value(serde_json::to_value(&config).unwrap()).unwrap();
            assert!(decoded.validate().is_ok());
        }
        let binding = config.profiles[0].bindings.get_mut("DIAL.double").unwrap();
        *binding = serde_json::from_str(r#"{"kind":"focusCodex"}"#).unwrap();
        assert!(config.validate().is_ok());
        config.profiles[0].bindings.insert("DIAL.double".into(),Action::FocusChat{target:"missing".into()});
        assert!(config.validate().is_err());
    }
}
