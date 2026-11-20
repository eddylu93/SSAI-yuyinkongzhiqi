use crate::{
    au05_protocol::{self as protocol, Reply},
    runtime::Shared,
};
use hidapi::{HidApi, HidDevice};
use std::{
    collections::BTreeSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
const SECOND: Duration = Duration::from_secs(1);

fn open() -> Result<HidDevice, String> {
    let api = HidApi::new().map_err(|e| e.to_string())?;
    let mut matches = api.device_list().filter(|d| {
        d.vendor_id() == 0xfff1
            && d.product_id() == 0xdd
            && d.usage_page() == 0xfffc
            && d.usage() == 1
    });
    let info = matches.next().ok_or("未发现 AU05 厂商接口")?;
    if matches.next().is_some() {
        return Err("检测到多个 AU05，请仅保留一台接收器".into());
    }
    api.open_path(info.path())
        .map_err(|e| format!("AU05 厂商接口打开失败：{e}"))
}
struct Session {
    device: HidDevice,
    requested: bool,
}
impl Session {
    fn send(&self, cmd: &[u8]) -> Result<(), String> {
        let sent = self
            .device
            .write(&protocol::frame(cmd))
            .map_err(|e| e.to_string())?;
        if sent != 64 {
            return Err("接收器未接收完整命令".into());
        }
        Ok(())
    }
    fn enable(&mut self) -> Result<(), String> {
        self.send(&protocol::handshake())?;
        self.send(protocol::HEARTBEAT)?;
        self.requested = true;
        self.send(&protocol::hooks(true))
    }
    fn disable(&mut self) -> Result<(), String> {
        if self.requested {
            self.send(&protocol::hooks(false))?;
            self.requested = false;
        }
        Ok(())
    }
    fn next(&self, ms: i32) -> Result<Option<Reply>, String> {
        let mut buf = [0u8; 128];
        let n = self
            .device
            .read_timeout(&mut buf, ms)
            .map_err(|e| e.to_string())?;
        Ok(protocol::decode(&buf[..n]))
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.disable();
    }
}

pub fn lights(change: Option<crate::lighting::Change>) -> Result<crate::lighting::Lights, String> {
    static SERIAL: Mutex<()> = Mutex::new(());
    let _guard = SERIAL.lock().map_err(|_| "灯光通道不可用")?;
    let session = Session {
        device: open()?,
        requested: false,
    };
    session.send(protocol::ONLINE)?;
    let deadline = Instant::now() + SECOND;
    loop {
        if Instant::now() >= deadline {
            return Err("机身没有回应，请唤醒 AU05".into());
        }
        if let Some(Reply::Online(online)) = session.next(80)? {
            if !online {
                return Err("机身离线，请唤醒 AU05".into());
            }
            break;
        }
    }
    let read = || -> Result<crate::lighting::Lights, String> {
        session.send(crate::lighting::QUERY)?;
        let deadline = Instant::now() + SECOND;
        while Instant::now() < deadline {
            if let Some(Reply::Lights(value)) = session.next(80)? {
                return Ok(value);
            }
        }
        Err("未收到可识别的灯光参数".into())
    };
    let current = read()?;
    let Some(change) = change else {
        return Ok(current);
    };
    let command = current.command(&change)?;
    if current.matches(&change) {
        return Ok(current);
    }
    session.send(&command)?;
    for _ in 0..3 {
        thread::sleep(Duration::from_millis(100));
        let result = read()?;
        if result.matches(&change) {
            return Ok(result);
        }
    }
    Err("设备回读与请求不一致，请重新读取".into())
}

/// Read events already published by an existing session, without writing to the receiver.
pub fn observe(seconds: u64) -> Result<serde_json::Value, String> {
    if !(1..=120).contains(&seconds) {
        return Err("观察时长应为 1 至 120 秒".into());
    }
    let session = Session {
        device: open()?,
        requested: false,
    };
    let start = Instant::now();
    let mut events = Vec::new();
    let mut total = 0u64;
    let mut online = None;
    let mut battery = None;
    while start.elapsed() < Duration::from_secs(seconds) {
        let event = match session.next(80)? {
            Some(Reply::Button(control, down)) => serde_json::json!({
                "control":control,"phase": if down {"down"} else {"up"}
            }),
            Some(Reply::Turn(steps)) => serde_json::json!({"control":"DIAL","steps":steps}),
            Some(Reply::Online(v)) => {
                online = Some(v);
                continue;
            }
            Some(Reply::Battery(v)) => {
                battery = Some(v);
                continue;
            }
            _ => continue,
        };
        total += 1;
        if events.len() < 512 {
            events.push(serde_json::json!({"ms":start.elapsed().as_millis(),"event":event}));
        }
    }
    Ok(serde_json::json!({"online":online,"battery":battery,"total":total,"events":events}))
}

/// Bounded hardware diagnostic; makes no change to the stored device key table.
pub fn probe(subscribe: bool) -> Result<serde_json::Value, String> {
    let _guard = if subscribe {
        let folder = crate::data_dir();
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(folder.join("desktop.lock"))
            .map_err(|e| e.to_string())?;
        file.try_lock()
            .map_err(|_| "请通过托盘退出 SSAI-语音控制器 后再测试独立订阅")?;
        Some(file)
    } else {
        None
    };
    let mut session = Session {
        device: open()?,
        requested: false,
    };
    let mut replies = Vec::new();
    session.send(protocol::ONLINE)?;
    session.send(protocol::BATTERY)?;
    if subscribe {
        session.enable()?
    }
    let until = Instant::now() + Duration::from_secs(3);
    let mut heartbeat = Instant::now() + SECOND;
    while Instant::now() < until {
        if subscribe && Instant::now() >= heartbeat {
            session.send(protocol::HEARTBEAT)?;
            heartbeat = Instant::now() + SECOND
        }
        if let Some(reply) = session.next(80)? {
            replies.push(reply)
        }
    }
    let mut restored = false;
    if subscribe {
        session.disable()?;
        let until = Instant::now() + SECOND;
        while Instant::now() < until {
            if session.next(80)? == Some(Reply::Hooks(false)) {
                restored = true;
                break;
            }
        }
    }
    Ok(
        serde_json::json!({"interface":"FFFC:0001","replies":replies,"releaseAcknowledged":subscribe.then_some(restored)}),
    )
}

pub struct Worker {
    stop: Arc<AtomicBool>,
    thread: Mutex<Option<thread::JoinHandle<()>>>,
}
impl Worker {
    pub fn start(desk: Shared) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let worker = thread::spawn(move || {
            while !flag.load(Ordering::Acquire) {
                let result = open().and_then(|device| {
                    run(
                        Session {
                            device,
                            requested: false,
                        },
                        &desk,
                        &flag,
                    )
                });
                let mut d = desk.lock().unwrap();
                d.vendor_end();
                d.direct.battery = None;
                d.direct.transmitter = None;
                d.direct.lights = None;
                d.direct.vendor_status = result.err().unwrap_or_else(|| "设备通道已关闭".into());
                drop(d);
                for _ in 0..12 {
                    if flag.load(Ordering::Acquire) {
                        break;
                    }
                    thread::sleep(Duration::from_millis(100))
                }
            }
        });
        Self {
            stop,
            thread: Mutex::new(Some(worker)),
        }
    }
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Release);
        if let Some(t) = self.thread.lock().unwrap().take() {
            let _ = t.join();
        }
    }
}
fn run(mut session: Session, desk: &Shared, stop: &AtomicBool) -> Result<(), String> {
    let mut held = BTreeSet::new();
    let mut active = false;
    let mut next_poll = Instant::now();
    let mut next_lights = Instant::now();
    let mut next_beat = Instant::now();
    let mut alive = Instant::now();
    let mut retry = Instant::now();
    let mut battery_seen = Instant::now();
    let mut online_seen = Instant::now();
    {
        let mut d = desk.lock().unwrap();
        d.direct.vendor_status = "厂商 HID 接口已打开".into();
    }
    while !stop.load(Ordering::Acquire) {
        let now = Instant::now();
        let (enabled, body_online) = {
            let mut d = desk.lock().unwrap();
            if battery_seen.elapsed() > SECOND * 25 {
                d.direct.battery = None
            }
            if online_seen.elapsed() > SECOND * 25 {
                d.direct.transmitter = None
            }
            (d.config.vendor_controls, d.direct.transmitter)
        };
        if now >= next_poll {
            session.send(protocol::ONLINE)?;
            session.send(protocol::BATTERY)?;
            next_poll = now + SECOND * 8;
        }
        if body_online == Some(true) && now >= next_lights {
            if session.send(crate::lighting::QUERY).is_err() {
                desk.lock().unwrap().direct.lights = None;
            }
            next_lights = now + SECOND * 4;
        }
        if enabled && body_online != Some(false) && !session.requested && now >= retry {
            session.enable()?;
            alive = now;
            next_beat = now + SECOND;
            desk.lock().unwrap().direct.vendor_status = "正在等待接收器确认按键直控".into();
        }
        if session.requested && (!enabled || alive.elapsed() > SECOND * 5) {
            session.disable()?;
            active = false;
            held.clear();
            retry = now + SECOND * 4;
            let mut d = desk.lock().unwrap();
            d.vendor_end();
            d.direct.vendor_status = if enabled {
                "设备没有确认直控，稍后重试"
            } else {
                "已释放按键直控"
            }
            .into();
        }
        if session.requested && now >= next_beat {
            session.send(protocol::HEARTBEAT)?;
            next_beat = now + SECOND
        }
        let Some(reply) = session.next(80)? else {
            continue;
        };
        let mut d = desk.lock().unwrap();
        match reply {
            Reply::Battery(v) => {
                d.direct.battery = Some(v);
                battery_seen = now;
            }
            Reply::Online(v) => {
                d.direct.transmitter = Some(v);
                online_seen = now;
                if !v {
                    d.direct.lights = None;
                    next_lights = now;
                    d.direct.battery = None;
                    d.vendor_end();
                    held.clear();
                    active = false;
                    session.disable()?;
                    d.direct.vendor_status = "接收器在线，机身离线，请唤醒 AU05".into();
                } else if !session.requested {
                    d.direct.vendor_status = "AU05 机身在线".into();
                }
            }
            Reply::Lights(value) if d.direct.transmitter == Some(true) => {
                d.direct.lights = Some((now, value));
            }
            Reply::Heartbeat => {
                if active {
                    alive = now;
                }
            }
            Reply::Hooks(true) if enabled && session.requested => {
                if !active {
                    d.vendor_end();
                    held.clear();
                }
                active = true;
                alive = now;
                d.direct.vendor_active = true;
                d.direct.vendor_status = "接收器已确认按键直控".into();
            }
            Reply::Hooks(false) if active => {
                active = false;
                held.clear();
                d.vendor_end();
                session.disable()?;
                retry = now + SECOND * 4;
            }
            Reply::Button(role, down) if session.requested && enabled => {
                active = true;
                alive = now;
                d.direct.vendor_active = true;
                d.direct.transmitter = Some(true);
                online_seen = now;
                d.direct.vendor_status = "已收到机身直控事件".into();
                if if down {
                    held.insert(role)
                } else {
                    held.remove(role)
                } {
                    d.vendor_input(role, if down { "down" } else { "up" }, 0);
                }
            }
            Reply::Turn(steps) if session.requested && enabled => {
                active = true;
                alive = now;
                d.direct.vendor_active = true;
                d.direct.transmitter = Some(true);
                online_seen = now;
                d.direct.vendor_status = "已收到机身直控事件".into();
                d.vendor_input(
                    "DIAL",
                    if held.contains("DIAL") {
                        "heldTurn"
                    } else {
                        "turn"
                    },
                    steps,
                );
            }
            _ => {}
        }
    }
    session.disable()
}
