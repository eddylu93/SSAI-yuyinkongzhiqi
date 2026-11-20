use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct InstalledApp {
    pub name: String,
    pub app: String,
}

#[cfg(windows)]
pub fn discover() -> Result<Vec<InstalledApp>, String> {
    use base64::Engine;
    use std::{os::windows::process::CommandExt, path::PathBuf, process::Command};
    let exe = PathBuf::from(std::env::var_os("SystemRoot").ok_or("找不到 Windows 目录")?)
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let script: Vec<_> = include_str!("installed_apps.ps1").encode_utf16().flat_map(u16::to_le_bytes).collect();
    let output = Command::new(exe)
        .args(["-NoProfile", "-NonInteractive", "-EncodedCommand"])
        .arg(base64::engine::general_purpose::STANDARD.encode(script))
        .creation_flags(0x08000000).output().map_err(|e| format!("读取已安装应用失败：{e}"))?;
    if !output.status.success() { return Err("Windows 应用列表读取失败，请刷新重试".into()); }
    serde_json::from_slice(&output.stdout).map_err(|_| "Windows 应用列表格式无效".into())
}

#[cfg(windows)]
pub fn package_id(app: &str) -> Option<&str> {
    let id = app.strip_prefix("shell:AppsFolder\\")?;
    let (family, entry) = id.split_once('!')?;
    let (name, publisher) = family.rsplit_once('_')?;
    let valid = |s: &str| !s.is_empty() && s.bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'.'));
    (id.len() <= 130 && valid(name) && valid(publisher) && valid(entry)).then_some(id)
}

#[cfg(all(test, windows))]
mod tests {
    use crate::{model::Action, platform::valid_launch_target};

    #[test]
    fn launch_bindings_accept_legacy_paths_and_registered_packages() {
        let legacy = serde_json::json!({"kind":"launch","app":r"C:\Tools\Example.exe"});
        let saved = serde_json::json!({"kind":"launch","app":r"shell:AppsFolder\Example_123!App","name":"Example"});
        for value in [legacy, saved] {
            let action: Action = serde_json::from_value(value.clone()).unwrap();
            if let Action::Launch { ref app, .. } = action { assert!(valid_launch_target(app)); }
            assert_eq!(serde_json::to_value(action).unwrap(), value);
        }
        for invalid in [r"shell:AppsFolder\..\other",r"shell:AppsFolder\Example_123!App -x","relative.exe"] {
            assert!(!valid_launch_target(invalid));
        }
    }
}
