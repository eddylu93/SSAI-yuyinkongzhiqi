use std::collections::{HashMap, HashSet};
use std::mem::{size_of, zeroed};
use std::path::Path;
use std::ptr::{null, null_mut};
use std::time::{Duration, Instant};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED},
    Storage::Packaging::Appx::GetApplicationUserModelId,
    System::{Diagnostics::ToolHelp::*, RemoteDesktop::ProcessIdToSessionId, Threading::*},
    UI::{Input::KeyboardAndMouse::IsWindowEnabled, Shell::ShellExecuteW, WindowsAndMessaging::*},
};

const START_TIMEOUT: Duration = Duration::from_secs(8);
const POLL_INTERVAL: Duration = Duration::from_millis(200);

fn path_key(path: &str) -> String {
    if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{unc}").to_lowercase();
    }
    let path = path.strip_prefix(r"\\?\").unwrap_or(path);
    path.replace('/', "\\").to_lowercase()
}

struct Target {
    binding: String,
    key: String,
    filename: Option<String>,
    wechat: bool,
}

impl Target {
    fn new(binding: &str) -> Result<Self, String> {
        if !super::valid_launch_target(binding) {
            return Err("应用启动目标无效".into());
        }
        let (key, filename) = if let Some(id) = crate::installed_apps::package_id(binding) {
            (id.to_lowercase(), None)
        } else {
            let path = Path::new(binding);
            let full = path.canonicalize().unwrap_or_else(|_| path.to_owned());
            let name = full
                .file_name()
                .ok_or("应用路径无效")?
                .to_string_lossy()
                .to_lowercase();
            (path_key(&full.to_string_lossy()), Some(name))
        };
        let wechat = matches!(filename.as_deref(), Some("weixin.exe" | "wechat.exe"));
        Ok(Self {
            binding: binding.to_owned(),
            key,
            filename,
            wechat,
        })
    }

    fn matches_pid(&self, pid: u32) -> Option<bool> {
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if process.is_null() {
                return None;
            }
            let result = if self.filename.is_some() {
                let mut path = [0u16; 32768];
                let mut len = path.len() as u32;
                (QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut len) != 0).then(
                    || {
                        let image = String::from_utf16_lossy(&path[..len as usize]);
                        path_key(&image) == self.key
                            || Path::new(&*image)
                                .canonicalize()
                                .is_ok_and(|full| path_key(&full.to_string_lossy()) == self.key)
                    },
                )
            } else {
                let mut id = [0u16; 256];
                let mut len = id.len() as u32;
                Some(
                    GetApplicationUserModelId(process, &mut len, id.as_mut_ptr()) == 0
                        && len > 0
                        && String::from_utf16_lossy(&id[..len as usize - 1]).to_lowercase()
                            == self.key,
                )
            };
            CloseHandle(process);
            result
        }
    }

    fn running(&self) -> Result<HashSet<u32>, String> {
        let mut pids = HashSet::new();
        let mut inaccessible = false;
        unsafe {
            let mut session = 0;
            if ProcessIdToSessionId(GetCurrentProcessId(), &mut session) == 0 {
                return Err("无法查询当前 Windows 会话".into());
            }
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == INVALID_HANDLE_VALUE {
                return Err("无法读取正在运行的应用".into());
            }
            let mut entry: PROCESSENTRY32W = zeroed();
            entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
            let mut next = Process32FirstW(snapshot, &mut entry);
            while next != 0 {
                let pid = entry.th32ProcessID;
                let mut current_session = 0;
                let name = wide_text(&entry.szExeFile);
                if self
                    .filename
                    .as_ref()
                    .is_none_or(|file| name.contains('~') || name.to_lowercase() == *file)
                    && ProcessIdToSessionId(pid, &mut current_session) != 0
                    && current_session == session
                {
                    match self.matches_pid(pid) {
                        Some(true) => {
                            pids.insert(pid);
                        }
                        None if self.filename.is_some() => inaccessible = true,
                        _ => {}
                    }
                }
                next = Process32NextW(snapshot, &mut entry);
            }
            let finished = GetLastError() == ERROR_NO_MORE_FILES;
            CloseHandle(snapshot);
            if !finished {
                return Err("读取应用列表时中断，请重试".into());
            }
        }
        if inaccessible && pids.is_empty() {
            return Err("同名应用正在运行，无法确认其路径，请从任务栏切换".into());
        }
        Ok(pids)
    }

    fn start(&self) -> Result<(), String> {
        if self.filename.is_some() {
            if !Path::new(&self.binding).is_file() {
                return Err("找不到应用程序".into());
            }
            std::process::Command::new(&self.binding)
                .spawn()
                .map_err(|_| "无法启动应用")?;
        } else {
            let file: Vec<_> = self.binding.encode_utf16().chain(Some(0)).collect();
            let status = unsafe {
                ShellExecuteW(
                    null_mut(),
                    null(),
                    file.as_ptr(),
                    null(),
                    null(),
                    SW_SHOWNORMAL,
                )
            };
            if status as isize <= 32 {
                return Err("无法打开已注册应用，请刷新应用列表".into());
            }
        }
        Ok(())
    }
}

fn wide_text(buf: &[u16]) -> String {
    String::from_utf16_lossy(&buf[..buf.iter().position(|&c| c == 0).unwrap_or(buf.len())])
}

fn pid_of(hwnd: HWND) -> u32 {
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut pid);
    }
    pid
}

fn class_of(hwnd: HWND) -> String {
    let mut buf = [0; 256];
    unsafe {
        GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
    }
    wide_text(&buf)
}

unsafe extern "system" fn collect_window(hwnd: HWND, list: LPARAM) -> BOOL {
    (*(list as *mut Vec<HWND>)).push(hwnd);
    1
}

fn hosted_children(hwnd: HWND) -> Vec<HWND> {
    let mut children = Vec::new();
    if class_of(hwnd) == "ApplicationFrameWindow" {
        unsafe {
            EnumChildWindows(
                hwnd,
                Some(collect_window),
                &mut children as *mut _ as LPARAM,
            );
        }
    }
    children
}

pub fn is_foreground(binding: &str) -> bool {
    let Ok(target) = Target::new(binding) else {
        return false;
    };
    let front = unsafe { GetForegroundWindow() };
    !front.is_null()
        && (target.matches_pid(pid_of(front)) == Some(true)
            || hosted_children(front)
                .iter()
                .any(|&w| target.matches_pid(pid_of(w)) == Some(true)))
}

#[derive(Clone, Copy, PartialEq)]
struct Window {
    handle: usize,
    pid: u32,
}

fn wechat_main(hwnd: HWND, style: u32, title: &str) -> bool {
    let class = class_of(hwnd);
    if class == "WeChatMainWndForPC" || class == "mmui::MainWindow" {
        return true;
    }
    let title = title.split(['(', '（']).next().unwrap_or("").trim();
    class.starts_with("Qt") && class.ends_with("QWindowIcon") &&
        matches!(title, "微信" | "WeChat" | "Weixin") &&
        style & (WS_THICKFRAME | WS_MAXIMIZEBOX) == WS_THICKFRAME | WS_MAXIMIZEBOX &&
        // Qt also creates invisible utility windows titled Weixin. The actual
        // chat frame has a Qt child and an MMUI rendering surface.
        unsafe {
            let qt: Vec<_> = class.encode_utf16().chain(Some(0)).collect();
            let pane = FindWindowExW(hwnd, null_mut(), qt.as_ptr(), null());
            let render: Vec<_> = "MMUIRenderSubWindowHW".encode_utf16().chain(Some(0)).collect();
            !pane.is_null() && (!FindWindowExW(hwnd, null_mut(), render.as_ptr(), null()).is_null()
                || !FindWindowExW(pane, null_mut(), render.as_ptr(), null()).is_null())
        }
}

fn pick_window(
    target: &Target,
    pids: &HashSet<u32>,
    previous: Option<Window>,
) -> Result<Option<Window>, String> {
    let mut windows = Vec::new();
    unsafe {
        if EnumWindows(Some(collect_window), &mut windows as *mut _ as LPARAM) == 0 {
            return Err("无法查找应用窗口".into());
        }
        let front = GetForegroundWindow();
        let mut best = None;
        let mut best_score = 0;
        for hwnd in windows {
            let pid = pid_of(hwnd);
            if !pids.contains(&pid)
                && !(target.filename.is_none()
                    && hosted_children(hwnd)
                        .iter()
                        .any(|&child| pids.contains(&pid_of(child))))
            {
                continue;
            }
            let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
            if ex & (WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) != 0
                || !GetWindow(hwnd, GW_OWNER).is_null()
            {
                continue;
            }
            let mut cloaked: u32 = 0;
            DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED as u32, &mut cloaked as *mut _ as _, 4);
            if cloaked != 0 {
                continue;
            }
            let mut caption = [0; 256];
            GetWindowTextW(hwnd, caption.as_mut_ptr(), caption.len() as i32);
            let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
            let window = Window {
                handle: hwnd as usize,
                pid,
            };
            let visible = IsWindowVisible(hwnd) != 0;
            let known = previous == Some(window);
            let main = target.wechat && wechat_main(hwnd, style, &wide_text(&caption));
            if !visible && !(known || main) {
                continue;
            }
            if caption[0] == 0 && ex & WS_EX_APPWINDOW == 0 && !known {
                continue;
            }
            let score = 1
                + 64 * u8::from(main)
                + 16 * u8::from(visible)
                + 8 * u8::from(hwnd == front)
                + 4 * u8::from(known);
            if score > best_score {
                best = Some(window);
                best_score = score;
            }
        }
        Ok(best)
    }
}

fn activate(window: Window) -> bool {
    let hwnd = window.handle as HWND;
    unsafe {
        if pid_of(hwnd) != window.pid {
            return false;
        }
        if IsIconic(hwnd) != 0 {
            ShowWindowAsync(hwnd, SW_RESTORE);
        } else if IsWindowVisible(hwnd) == 0 {
            ShowWindowAsync(hwnd, SW_SHOW);
        }
        let popup = GetLastActivePopup(hwnd);
        let dest = if IsWindowEnabled(hwnd) == 0 && !popup.is_null() && IsWindowVisible(popup) != 0
        {
            popup
        } else {
            hwnd
        };
        if GetForegroundWindow() != dest && SetForegroundWindow(dest) == 0 {
            let front = GetWindowThreadProcessId(GetForegroundWindow(), null_mut());
            let thread = GetCurrentThreadId();
            let mut message = zeroed();
            PeekMessageW(&mut message, null_mut(), 0, 0, PM_NOREMOVE);
            let attached =
                front != 0 && front != thread && AttachThreadInput(thread, front, 1) != 0;
            if attached {
                BringWindowToTop(dest);
                SetForegroundWindow(dest);
                AttachThreadInput(thread, front, 0);
            }
        }
        GetForegroundWindow() == dest && IsIconic(hwnd) == 0 && IsWindowVisible(hwnd) != 0
    }
}

struct Pending {
    target: Target,
    deadline: Instant,
    next: Instant,
    saw_window: bool,
}

#[derive(Default)]
pub struct Launcher {
    pending: Option<Pending>,
    remembered: HashMap<String, Window>,
    starts: HashMap<String, Instant>,
}

impl Launcher {
    pub fn cancel(&mut self) {
        self.pending = None;
    }
    pub fn waiting_for(&self, binding: &str) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|p| p.target.binding == binding)
    }

    pub fn request(&mut self, binding: &str) -> Result<String, String> {
        let target = Target::new(binding)?;
        if let Some(job) = &self.pending {
            if job.target.key == target.key {
                return Ok(job.target.binding.clone());
            }
        }
        self.cancel();
        let pids = target.running()?;
        let now = Instant::now();
        self.starts
            .retain(|_, t| now.duration_since(*t) < START_TIMEOUT);
        let mut saw_window = false;
        if pids.is_empty() {
            if !self.starts.contains_key(&target.key) {
                target.start()?;
                self.starts.insert(target.key.clone(), now);
            }
        } else if let Some(window) =
            pick_window(&target, &pids, self.remembered.get(&target.key).copied())?
        {
            saw_window = true;
            if activate(window) {
                self.remembered.insert(target.key.clone(), window);
                self.starts.remove(&target.key);
                return Ok(target.binding);
            }
        }
        let identity = target.binding.clone();
        self.pending = Some(Pending {
            target,
            deadline: now + START_TIMEOUT,
            next: now + POLL_INTERVAL,
            saw_window,
        });
        Ok(identity)
    }

    pub fn poll(&mut self) -> Option<(String, String)> {
        if self.pending.as_ref()?.next > Instant::now() {
            return None;
        }
        let mut job = self.pending.take()?;
        let t = &job.target;
        let result = t
            .running()
            .and_then(|pids| pick_window(t, &pids, self.remembered.get(&t.key).copied()));
        match result {
            Err(e) => return Some((t.binding.clone(), e)),
            Ok(Some(window)) => {
                job.saw_window = true;
                if activate(window) {
                    self.remembered.insert(t.key.clone(), window);
                    self.starts.remove(&t.key);
                    return None;
                }
            }
            Ok(None) => {}
        }
        let now = Instant::now();
        if now >= job.deadline {
            let msg = if job.saw_window {
                "Windows 未能切换到目标窗口，请从任务栏打开应用"
            } else {
                "未找到可唤回的窗口，请检查应用启动状态或从系统托盘打开"
            };
            return Some((t.binding.clone(), msg.into()));
        }
        job.next = now + POLL_INTERVAL;
        self.pending = Some(job);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeat_press_joins_pending_launch_without_extending_deadline() {
        let binding = r"C:\AU05-test\delayed-start.exe";
        let mut launcher = Launcher::default();
        let target = Target::new(binding).unwrap();
        let deadline = Instant::now() + START_TIMEOUT;
        launcher.pending = Some(Pending {
            target,
            next: deadline,
            deadline,
            saw_window: false,
        });
        assert_eq!(launcher.request(&binding.to_uppercase()).unwrap(), binding);
        assert_eq!(launcher.pending.as_ref().unwrap().deadline, deadline);
    }

    // Uses an explicitly selected desktop app. Optional second pass exercises
    // restoration when the first window is hidden, without closing the app.
    #[test]
    #[ignore = "requires an interactive Windows desktop and AU05_TEST_LAUNCH_TARGET"]
    fn desktop_launch_and_reactivate() {
        let binding = std::env::var("AU05_TEST_LAUNCH_TARGET").unwrap();
        let mut launcher = Launcher::default();
        let target = Target::new(&binding).unwrap();
        let before = target.running().unwrap();
        if std::env::var_os("AU05_TEST_BACKGROUND_ONLY").is_some() {
            assert!(!before.is_empty());
            for _ in 0..4 {
                launcher.request(&binding).unwrap();
            }
            let until = Instant::now() + START_TIMEOUT + Duration::from_secs(2);
            loop {
                assert!(Instant::now() < until);
                if let Some((_, message)) = launcher.poll() {
                    assert!(message.contains("托盘"));
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            assert_eq!(target.running().unwrap(), before);
            return;
        }
        let activate_and_wait = |launcher: &mut Launcher| {
            for _ in 0..4 {
                launcher.request(&binding).unwrap();
            }
            let until = Instant::now() + START_TIMEOUT + Duration::from_secs(2);
            while launcher.waiting_for(&binding) {
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_millis(50));
                if let Some((_, error)) = launcher.poll() {
                    panic!("{error}");
                }
            }
            assert!(is_foreground(&binding));
            let w = unsafe { GetForegroundWindow() };
            (w, pid_of(w))
        };
        let original = activate_and_wait(&mut launcher);
        if !before.is_empty() {
            assert!(before.contains(&original.1));
        }
        eprintln!("foreground window {:?}, pid {}", original.0, original.1);
        if std::env::var_os("AU05_TEST_HIDE_WINDOW").is_some() {
            for show in [SW_MINIMIZE, SW_HIDE] {
                unsafe {
                    ShowWindowAsync(original.0, show);
                }
                std::thread::sleep(Duration::from_millis(250));
                assert!(unsafe { IsIconic(original.0) != 0 || IsWindowVisible(original.0) == 0 });
                assert_eq!(activate_and_wait(&mut launcher), original);
            }
        }
    }
}
