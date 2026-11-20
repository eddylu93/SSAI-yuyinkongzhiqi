//! A message-only window receives Raw Input for AU05 HID collections.
use crate::runtime::Shared;
use std::{
    collections::{BTreeMap, BTreeSet},
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
};
use windows_sys::Win32::UI::{Input::*, WindowsAndMessaging::*};

#[derive(Clone, Debug, Eq, PartialEq)]
struct Collection {
    page: u16,
    usage: u16,
    source: String,
}
type Inventory = BTreeMap<usize, Collection>;
fn product(path: &str) -> Option<&str> {
    let hardware = path.split('#').nth(1)?;
    let mut parts = hardware.split('&');
    if parts.next() != Some("VID_FFF1") || parts.next() != Some("PID_00DD") {
        return None;
    }
    Some(hardware)
}
unsafe fn inventory() -> Result<Inventory, String> {
    let size = size_of::<RAWINPUTDEVICELIST>() as u32;
    let mut count = 0;
    if GetRawInputDeviceList(null_mut(), &mut count, size) == u32::MAX {
        return Err("无法枚举系统 HID 设备".into());
    }
    let mut items: Vec<RAWINPUTDEVICELIST> = vec![zeroed(); count as usize];
    let n = GetRawInputDeviceList(items.as_mut_ptr(), &mut count, size);
    if n == u32::MAX {
        return Err("HID 设备列表变化，正在重新检测".into());
    }
    let mut found = Inventory::new();
    for item in items.into_iter().take(n as usize) {
        let mut len = 0;
        if GetRawInputDeviceInfoW(item.hDevice, RIDI_DEVICENAME, null_mut(), &mut len) == u32::MAX
            || len == 0
            || len > 8192
        {
            continue;
        }
        let mut name = vec![0u16; len as usize + 1];
        if GetRawInputDeviceInfoW(
            item.hDevice,
            RIDI_DEVICENAME,
            name.as_mut_ptr().cast(),
            &mut len,
        ) == u32::MAX
        {
            continue;
        }
        let path = String::from_utf16_lossy(
            &name[..name.iter().position(|b| *b == 0).unwrap_or(name.len())],
        )
        .to_ascii_uppercase();
        let Some(hw) = product(&path) else { continue };
        let (page, usage) = match item.dwType {
            RIM_TYPEKEYBOARD => (1, 6),
            RIM_TYPEMOUSE => (1, 2),
            RIM_TYPEHID => {
                let mut bytes = size_of::<RID_DEVICE_INFO>() as u32;
                let mut data: RID_DEVICE_INFO = zeroed();
                data.cbSize = bytes;
                if GetRawInputDeviceInfoW(
                    item.hDevice,
                    RIDI_DEVICEINFO,
                    (&mut data as *mut RID_DEVICE_INFO).cast(),
                    &mut bytes,
                ) == u32::MAX
                {
                    continue;
                }
                let info = data.Anonymous.hid;
                if (info.dwVendorId, info.dwProductId) != (0xfff1, 0xdd) {
                    continue;
                }
                (info.usUsagePage, info.usUsage)
            }
            _ => continue,
        };
        let interface = hw.split('&').skip(2).collect::<Vec<_>>().join("/");
        found.insert(
            item.hDevice as usize,
            Collection {
                page,
                usage,
                source: format!("{page:04X}:{usage:04X}/{interface}"),
            },
        );
    }
    Ok(found)
}
pub fn inspect() -> Result<Vec<String>, String> {
    Ok(unsafe { inventory()? }
        .into_values()
        .map(|c| c.source)
        .collect())
}

unsafe fn receive(raw: HRAWINPUT, known: &Inventory, desk: &Shared) {
    let hs = size_of::<RAWINPUTHEADER>() as u32;
    let mut header: RAWINPUTHEADER = zeroed();
    let mut size = hs;
    if GetRawInputData(
        raw,
        RID_HEADER,
        (&mut header as *mut RAWINPUTHEADER).cast(),
        &mut size,
        hs,
    ) != hs
    {
        return;
    }
    let Some(collection) = known.get(&(header.hDevice as usize)) else {
        return;
    };
    let mut n = 0;
    if GetRawInputData(raw, RID_INPUT, null_mut(), &mut n, hs) == u32::MAX || n < hs || n > 65536 {
        return;
    }
    // RAWINPUT requires pointer alignment. RAWHID can contain several variable-sized reports.
    let mut buffer = vec![
        0usize;
        (n as usize)
            .max(size_of::<RAWINPUT>())
            .div_ceil(size_of::<usize>())
    ];
    let got = GetRawInputData(raw, RID_INPUT, buffer.as_mut_ptr().cast(), &mut n, hs);
    if got == u32::MAX || got != n {
        return;
    }
    let event = &*buffer.as_ptr().cast::<RAWINPUT>();
    let mut d = desk.lock().unwrap();
    let mut emit = |token: String| d.hid_input(&collection.source, &token);
    match event.header.dwType {
        RIM_TYPEKEYBOARD => {
            let k = event.data.keyboard;
            if k.VKey != 255 {
                emit(format!(
                    "key/{:X}/{:X}/{:X}/{}",
                    k.MakeCode,
                    k.VKey,
                    k.Flags & 6,
                    k.Flags & 1
                ));
            }
        }
        RIM_TYPEMOUSE => {
            let b = event.data.mouse.Anonymous.Anonymous;
            for (mask, axis) in [(0x400, "wheel"), (0x800, "horizontal")] {
                if b.usButtonFlags & mask != 0 && b.usButtonData != 0 {
                    let delta = b.usButtonData as i16;
                    for _ in 0..(i32::from(delta).unsigned_abs() / 120).clamp(1, 32) {
                        emit(format!("{axis}/{}", delta.signum()));
                    }
                }
            }
            for bit in 0..10 {
                if b.usButtonFlags & (1 << bit) != 0 {
                    emit(format!("mouse/{bit}"));
                }
            }
        }
        RIM_TYPEHID => {
            let hid = std::ptr::addr_of!(event.data.hid);
            let start = std::ptr::addr_of!((*hid).bRawData).cast::<u8>();
            let offset = start.offset_from(buffer.as_ptr().cast::<u8>()) as usize;
            let width = (*hid).dwSizeHid as usize;
            let count = (*hid).dwCount as usize;
            if width == 0 || width > 1024 || count > 1024 || offset + width * count > got as usize {
                return;
            }
            for frame in std::slice::from_raw_parts(start, width * count).chunks_exact(width) {
                use std::fmt::Write;
                let mut value = String::with_capacity(width * 2);
                for byte in frame {
                    let _ = write!(value, "{byte:02X}");
                }
                emit(value);
            }
        }
        _ => {}
    }
}
unsafe fn run(desk: &Shared) -> Result<(), String> {
    let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
    let window = CreateWindowExW(
        0,
        class.as_ptr(),
        null(),
        0,
        0,
        0,
        0,
        0,
        HWND_MESSAGE,
        null_mut(),
        null_mut(),
        null(),
    );
    if window.is_null() {
        return Err("无法创建 HID 输入窗口".into());
    }
    if SetTimer(window, 1, 1000, None) == 0 {
        return Err("无法启动设备检测定时器".into());
    }
    let mut previous = None;
    let mut known = Inventory::new();
    let mut registered = BTreeSet::new();
    let mut refresh = true;
    let mut msg: MSG = zeroed();
    loop {
        if refresh {
            match inventory() {
                Ok(current) => {
                    if previous.as_ref() != Some(&current) {
                        let wanted: BTreeSet<_> =
                            current.values().map(|c| (c.page, c.usage)).collect();
                        let mut changes = Vec::new();
                        for &(page, usage) in registered.difference(&wanted) {
                            changes.push(RAWINPUTDEVICE {
                                usUsagePage: page,
                                usUsage: usage,
                                dwFlags: RIDEV_REMOVE,
                                hwndTarget: null_mut(),
                            });
                        }
                        for &(page, usage) in wanted.difference(&registered) {
                            changes.push(RAWINPUTDEVICE {
                                usUsagePage: page,
                                usUsage: usage,
                                dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
                                hwndTarget: window,
                            });
                        }
                        let ok = changes.is_empty()
                            || RegisterRawInputDevices(
                                changes.as_ptr(),
                                changes.len() as u32,
                                size_of::<RAWINPUTDEVICE>() as u32,
                            ) != 0;
                        if ok {
                            registered = wanted;
                        }
                        let sources: Vec<_> = current.values().map(|c| c.source.clone()).collect();
                        let ambiguous =
                            sources.iter().collect::<BTreeSet<_>>().len() != sources.len();
                        let ready = ok && !sources.is_empty() && !ambiguous;
                        let status = if ambiguous {
                            "检测到多个同型设备，请仅保留待校准的一台"
                        } else if !ok {
                            "HID 注册失败，正在重试"
                        } else if sources.is_empty() {
                            "等待 AU05 接收器"
                        } else {
                            "AU05 接收器已就绪，等待实体操作"
                        };
                        known = if ready {
                            current.clone()
                        } else {
                            Inventory::new()
                        };
                        desk.lock()
                            .unwrap()
                            .hid_connection(sources, status.into(), ready);
                        previous = ok.then_some(current);
                    }
                }
                Err(message) => {
                    if !known.is_empty() || previous.is_some() {
                        known.clear();
                        previous = None;
                        desk.lock().unwrap().hid_connection(vec![], message, false);
                    }
                }
            }
        }
        let outcome = GetMessageW(&mut msg, null_mut(), 0, 0);
        if outcome <= 0 {
            break;
        }
        refresh = msg.message == WM_TIMER || msg.message == WM_INPUT_DEVICE_CHANGE;
        if msg.message == WM_INPUT {
            receive(msg.lParam as HRAWINPUT, &known, desk);
        }
        DispatchMessageW(&msg);
    }
    DestroyWindow(window);
    Ok(())
}
pub fn start(desk: Shared) {
    std::thread::spawn(move || {
        if let Err(error) = unsafe { run(&desk) } {
            desk.lock().unwrap().hid_connection(vec![], error, false);
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filters_before_decoding() {
        assert!(product(r"\\?\HID#VID_FFF1&PID_00DD&MI_02&COL03#id").is_some());
        for p in [
            "x#VID_FFF1&PID_00DC",
            "x#VID_FFF1&PID_00DD0",
            "x#VID_OTHER&PID_00DD",
            "",
        ] {
            assert!(product(p).is_none());
        }
    }
}
