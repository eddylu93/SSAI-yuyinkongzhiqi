//! Physical AU05 input only. Each mapping is learned from two identical gestures.
use crate::runtime::Runtime;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

pub const CONTROLS: [&str; 8] = [
    "K1",
    "K2",
    "K3",
    "DIAL",
    "left",
    "right",
    "heldLeft",
    "heldRight",
];
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct Pattern {
    source: String,
    first: String,
    release: Option<String>,
}
#[derive(Default)]
struct Sample {
    streams: BTreeMap<String, Vec<String>>,
    overflow: bool,
}
struct Learning {
    role: String,
    first: Option<Sample>,
    current: Sample,
    since: Instant,
}
#[derive(Default)]
pub struct Direct {
    pub vendor_active: bool,
    pub vendor_status: String,
    pub battery: Option<u8>,
    pub charging: Option<(Instant, bool)>,
    pub transmitter: Option<bool>,
    pub lights: Option<(Instant, crate::lighting::Lights)>,
    pub vendor_events: u64,
    pub sources: Vec<String>,
    pub status: String,
    pub reports: u64,
    pub last_source: String,
    mappings: BTreeMap<String, Pattern>,
    learning: Option<Learning>,
    down: BTreeSet<String>,
}
impl Direct {
    pub fn load(db: &Connection) -> Result<Self, String> {
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS au05_hid(role TEXT PRIMARY KEY, pattern TEXT NOT NULL)",
        )
        .map_err(|e| e.to_string())?;
        let mut st = db
            .prepare("SELECT role,pattern FROM au05_hid")
            .map_err(|e| e.to_string())?;
        let mut direct = Self::default();
        for row in st
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?
        {
            let (role, value) = row.map_err(|e| e.to_string())?;
            if !CONTROLS.contains(&role.as_str()) {
                continue;
            }
            direct.mappings.insert(
                role,
                serde_json::from_str(&value).map_err(|_| "设备校准数据损坏")?,
            );
        }
        Ok(direct)
    }
    pub fn snapshot(&self) -> serde_json::Value {
        let lights=self.lights.as_ref().filter(|(seen,_)|self.transmitter==Some(true) && seen.elapsed()<Duration::from_secs(12)).map(|(_,v)|v);
        let charging=self.charging.filter(|(seen,_)|self.transmitter==Some(true) && seen.elapsed()<Duration::from_secs(25)).map(|(_,v)|v);
        serde_json::json!({"vendorActive":self.vendor_active,"vendorStatus":self.vendor_status,"vendorEvents":self.vendor_events,"battery":self.battery,"charging":charging,"transmitter":self.transmitter,"lights":lights,"sources":self.sources,"status":self.status,"reports":self.reports,"lastSource":self.last_source,
            "calibrated":self.mappings.keys().collect::<Vec<_>>(),
            "learning":self.learning.as_ref().map(|s|serde_json::json!({"role":s.role,"round":if s.first.is_some(){2}else{1},"received":!s.current.streams.is_empty()}))})
    }
    pub fn reset(&mut self) {
        self.learning = None;
        self.down.clear();
    }
    pub fn expire(&mut self) -> bool {
        if self
            .learning
            .as_ref()
            .is_some_and(|s| s.since.elapsed() > Duration::from_secs(45))
        {
            self.reset();
            true
        } else {
            false
        }
    }
    fn feed(&mut self, source: &str, token: &str) -> Vec<(String, &'static str, i32)> {
        self.reports += 1;
        self.last_source = source.to_owned();
        if let Some(learn) = self.learning.as_mut() {
            let values = learn.current.streams.entry(source.into()).or_default();
            if values.last().is_none_or(|last| last != token) {
                if values.len() < 32 {
                    values.push(token.into())
                } else {
                    learn.current.overflow = true
                }
            }
            return vec![];
        }
        let mut events = Vec::new();
        for (role, p) in &self.mappings {
            if p.source != source {
                continue;
            }
            if p.release.as_deref() == Some(token) && self.down.remove(role) {
                events.push((role.clone(), "up", 0));
            }
        }
        for (role, p) in &self.mappings {
            if p.source != source || p.first != token {
                continue;
            }
            if p.release.is_some() {
                if self.down.insert(role.clone()) {
                    events.push((role.clone(), "down", 0))
                }
            } else {
                let held = role.starts_with("held") || self.down.contains("DIAL");
                let steps = if role.to_lowercase().ends_with("left") {
                    -1
                } else {
                    1
                };
                events.push(("DIAL".into(), if held { "heldTurn" } else { "turn" }, steps));
            }
        }
        events
    }
}

fn candidate(
    role: &str,
    a: &Sample,
    b: &Sample,
    existing: &BTreeMap<String, Pattern>,
) -> Result<Pattern, String> {
    if a.overflow || b.overflow {
        return Err("单次操作报告过多，请重新校准".into());
    }
    let button = matches!(role, "K1" | "K2" | "K3" | "DIAL");
    for (source, values) in &a.streams {
        if b.streams.get(source) != Some(values) || values.is_empty() || values.len() > 2 {
            continue;
        }
        if button && (values.len() != 2 || !edge_pair(&values[0], &values[1])) {
            continue;
        }
        let p = Pattern {
            source: source.clone(),
            first: values[0].clone(),
            release: if button {
                Some(values[1].clone())
            } else {
                None
            },
        };
        if existing.iter().any(|(r, m)| {
            r != role
                && m.source == p.source
                && (m.first == p.first
                    || m.release.as_ref() == Some(&p.first)
                    || p.release.as_ref() == Some(&m.first))
        }) {
            continue;
        }
        return Ok(p);
    }
    Err("两次报告不一致、缺少松开事件，或与已有控件重复。请仅操作所选控件后重试。".into())
}
fn edge_pair(first: &str, last: &str) -> bool {
    if first.starts_with("key/") {
        return first
            .strip_suffix("/0")
            .zip(last.strip_suffix("/1"))
            .is_some_and(|(a, b)| a == b);
    }
    if let Some(bit) = first
        .strip_prefix("mouse/")
        .and_then(|s| s.parse::<u8>().ok())
    {
        return bit < 10 && bit % 2 == 0 && last == format!("mouse/{}", bit + 1);
    }
    !first.starts_with("wheel/") && !first.starts_with("horizontal/") && first != last
}
impl Runtime {
    pub fn hid_connection(&mut self, sources: Vec<String>, status: String, ready: bool) {
        if self.online {
            self.cleanup();
        }
        self.direct.reset();
        self.direct.sources = sources;
        self.direct.status = status.clone();
        self.online = ready;
        self.message = status;
        self.direct.last_source.clear();
        if ready {
            for id in ["K1", "K2", "K3", "DIAL"] {
                self.bindings.insert(id.into(), id.into());
            }
        }
    }
    pub fn hid_input(&mut self, source: &str, token: &str) {
        // Telemetry and heartbeat replies must never be learned as user gestures.
        if !self.online || self.config.vendor_controls || source.starts_with("FFFC:0001/") {
            return;
        }
        for (id, phase, steps) in self.direct.feed(source, token) {
            if let Err(e) = self.input(&id, &id, phase, steps) {
                self.message = e;
            }
        }
    }
    pub fn vendor_end(&mut self) {
        if self.direct.vendor_active {
            self.set_diagnostic(self.diagnostic);
        }
        self.direct.vendor_active = false;
    }
    pub fn transmitter_online(&mut self, online: bool) {
        if !online && self.direct.transmitter != Some(false) {
            // Raw HID interfaces remain present while the paired handset sleeps.
            self.set_diagnostic(self.diagnostic);
            self.direct.vendor_active = false;
        }
        self.direct.transmitter = Some(online);
    }
    pub fn vendor_input(&mut self, role: &str, phase: &str, steps: i32) {
        self.direct.vendor_events += 1;
        let id = format!("vendor-{role}");
        self.bindings.insert(id.clone(), role.into());
        if let Err(e) = self.input(&id, role, phase, steps) {
            self.message = e
        }
    }
    pub fn calibrate(&mut self, cmd: &str, role: &str) -> Result<String, String> {
        match cmd {
            "begin" if self.online && !self.config.vendor_controls && CONTROLS.contains(&role) => {
                self.set_diagnostic(true);
                self.direct.reset();
                self.direct.learning = Some(Learning {
                    role: role.into(),
                    first: None,
                    current: Sample::default(),
                    since: Instant::now(),
                });
                Ok("操作一次，然后点击「本次完成」".into())
            }
            "cancel" => {
                self.direct.reset();
                Ok("校准已取消".into())
            }
            "finish" => {
                let mut l = self.direct.learning.take().ok_or("请先开始校准")?;
                if l.since.elapsed() > Duration::from_secs(45) {
                    return Err("校准超时，请重试".into());
                }
                if l.current.streams.is_empty() {
                    return Err("本次没有收到 AU05 报告，请唤醒设备后重试".into());
                }
                if let Some(previous) = l.first {
                    let p = candidate(&l.role, &previous, &l.current, &self.direct.mappings)?;
                    self.db
                        .execute(
                            "INSERT OR REPLACE INTO au05_hid VALUES(?1,?2)",
                            params![
                                l.role,
                                serde_json::to_string(&p).map_err(|e| e.to_string())?
                            ],
                        )
                        .map_err(|e| e.to_string())?;
                    self.direct.mappings.insert(l.role, p);
                    Ok("校准已保存。测试模式下可检查按下、松开或旋转。".into())
                } else {
                    l.first = Some(std::mem::take(&mut l.current));
                    l.since = Instant::now();
                    self.direct.learning = Some(l);
                    Ok("请重复同一操作，再点击「本次完成」".into())
                }
            }
            _ => Err("请先接入一个 AU05 接收器并选择控件".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct DeviceFixture {
        path: PathBuf,
        runtime: Option<Runtime>,
    }
    impl DeviceFixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("au05-direct-{}", uuid::Uuid::new_v4()));
            let mut d = Runtime::open(&path).unwrap();
            d.hid_connection(vec!["FFFC".into()], "ready".into(), true);
            Self {
                path,
                runtime: Some(d),
            }
        }
        fn desk(&mut self) -> &mut Runtime {
            self.runtime.as_mut().unwrap()
        }
    }
    impl Drop for DeviceFixture {
        fn drop(&mut self) {
            drop(self.runtime.take());
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
    fn learn(d: &mut Runtime, role: &str, frames: &[&str]) -> Result<(), String> {
        d.calibrate("begin", role)?;
        for _ in 0..2 {
            for f in frames {
                d.hid_input("FFFC", f)
            }
            d.calibrate("finish", role)?;
        }
        Ok(())
    }
    #[test]
    fn learn_persist_dedupe_and_disconnect() {
        let mut f = DeviceFixture::new();
        {
            let d = f.desk();
            learn(d, "K1", &["21", "00"]).unwrap();
            assert_eq!(d.event_count, 0);
            assert!(learn(d, "K2", &["21", "00"]).is_err());
            for v in ["21", "21", "00", "00"] {
                d.hid_input("FFFC", v)
            }
            assert_eq!(d.event_count, 2);
            assert_eq!(d.stats(0).unwrap(), serde_json::json!([]));
            d.hid_connection(vec![], "removed".into(), false);
            d.hid_input("FFFC", "21");
            assert_eq!(d.event_count, 2);
        }
        drop(f.runtime.take());
        let d = Runtime::open(&f.path).unwrap();
        assert!(d.direct.mappings.contains_key("K1"));
    }
    #[test]
    fn inconsistent_capture_and_incomplete_key_are_rejected() {
        let mut f = DeviceFixture::new();
        let d = f.desk();
        assert!(learn(d, "K1", &["21"]).is_err());
        d.calibrate("begin", "K1").unwrap();
        for v in ["21", "00"] {
            d.hid_input("FFFC", v)
        }
        d.calibrate("finish", "K1").unwrap();
        for v in ["22", "00"] {
            d.hid_input("FFFC", v)
        }
        assert!(d.calibrate("finish", "K1").is_err());
        assert!(d.direct.mappings.is_empty());
        assert!(!edge_pair("key/1/E/0/0", "key/2/F/0/1"));
        assert!(edge_pair("key/1/E/0/0", "key/1/E/0/1"));
    }
    #[test]
    fn rotary_uses_press_state_and_cancels_on_removal() {
        let mut f = DeviceFixture::new();
        let d = f.desk();
        learn(d, "DIAL", &["30", "00"]).unwrap();
        learn(d, "right", &["41"]).unwrap();
        assert_eq!(
            d.direct.feed("FFFC", "41"),
            vec![("DIAL".into(), "turn", 1)]
        );
        d.direct.feed("FFFC", "30");
        assert_eq!(
            d.direct.feed("FFFC", "41"),
            vec![("DIAL".into(), "heldTurn", 1)]
        );
        d.hid_connection(vec![], "removed".into(), false);
        d.hid_connection(vec!["FFFC".into()], "ready".into(), true);
        assert_eq!(
            d.direct.feed("FFFC", "41"),
            vec![("DIAL".into(), "turn", 1)]
        );
    }
}
