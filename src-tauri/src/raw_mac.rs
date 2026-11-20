use crate::runtime::Shared;
use hidapi::{HidApi, HidDevice};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::CString,
    time::{Duration, Instant},
};

fn collections(api: &HidApi) -> BTreeMap<CString, String> {
    api.device_list()
        .filter(|d| d.vendor_id() == 0xfff1 && d.product_id() == 0x00dd)
        .map(|d| {
            (
                d.path().to_owned(),
                format!(
                    "{:04X}:{:04X}/{}",
                    d.usage_page(),
                    d.usage(),
                    d.interface_number()
                ),
            )
        })
        .collect()
}
pub fn inspect() -> Result<Vec<String>, String> {
    Ok(collections(&HidApi::new().map_err(|e| e.to_string())?)
        .into_values()
        .collect())
}
pub fn start(desk: Shared) {
    std::thread::spawn(move || {
        let mut api = match HidApi::new() {
            Ok(api) => api,
            Err(e) => {
                desk.lock()
                    .unwrap()
                    .hid_connection(vec![], e.to_string(), false);
                return;
            }
        };
        let mut opened: BTreeMap<CString, (String, HidDevice)> = BTreeMap::new();
        let mut next = Instant::now();
        let mut state = None;
        loop {
            if Instant::now() >= next {
                next = Instant::now() + Duration::from_secs(1);
                if api.refresh_devices().is_err() {
                    opened.clear();
                    state = None;
                    desk.lock().unwrap().hid_connection(
                        vec![],
                        "设备枚举失败，将重新检测".into(),
                        false,
                    );
                    continue;
                }
                let wanted = collections(&api);
                let ambiguous = wanted.values().collect::<BTreeSet<_>>().len() != wanted.len();
                opened.retain(|path, _| wanted.contains_key(path) && !ambiguous);
                if !ambiguous {
                    for (path, name) in &wanted {
                        if !opened.contains_key(path) {
                            if let Ok(h) = api.open_path(path) {
                                opened.insert(path.clone(), (name.clone(), h));
                            }
                        }
                    }
                }
                let list: Vec<_> = opened.values().map(|(name, _)| name.clone()).collect();
                let description = if ambiguous {
                    "请仅保留一台 AU05"
                } else if wanted.is_empty() {
                    "等待 AU05 接收器"
                } else if list.is_empty() {
                    "系统未允许读取 AU05，请检查输入监控权限"
                } else {
                    "AU05 接收器已就绪，等待实体操作"
                };
                let current = (opened.keys().cloned().collect::<Vec<_>>(), description);
                if state.as_ref() != Some(&current) {
                    desk.lock().unwrap().hid_connection(
                        list,
                        description.into(),
                        !opened.is_empty(),
                    );
                    state = Some(current)
                }
            }
            let mut failed = false;
            for (source, h) in opened.values() {
                let mut buf = [0u8; 1024];
                match h.read_timeout(&mut buf, 0) {
                    Ok(0) => {}
                    Ok(n) => {
                        use std::fmt::Write;
                        let mut value = String::new();
                        for b in &buf[..n] {
                            let _ = write!(value, "{b:02X}");
                        }
                        desk.lock().unwrap().hid_input(source, &value);
                    }
                    Err(_) => {
                        failed = true;
                        break;
                    }
                }
            }
            if failed {
                opened.clear();
                state = None;
                desk.lock().unwrap().hid_connection(
                    vec![],
                    "AU05 读取中断，正在重新检测".into(),
                    false,
                );
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });
}
