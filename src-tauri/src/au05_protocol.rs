//! AU05 vendor input and telemetry. Wire format documented by AU05 Keys and open-vibekey.
const KEY: [u32; 4] = [0xcaa5baca, 0xbc2a8a6d, 0xca5a9eba, 0x9bb88bca];
const DELTA: u32 = 0x9e3779b9;
fn mix(x: u32, sum: u32, k0: u32, k1: u32) -> u32 {
    (x << 4).wrapping_add(k0) ^ x.wrapping_add(sum) ^ (x >> 5).wrapping_add(k1)
}
fn block(data: &[u8], encrypt: bool) -> [u8; 8] {
    let mut a = u32::from_le_bytes(data[..4].try_into().unwrap());
    let mut b = u32::from_le_bytes(data[4..8].try_into().unwrap());
    let mut sum = if encrypt { 0 } else { DELTA.wrapping_mul(32) };
    for _ in 0..32 {
        if encrypt {
            sum = sum.wrapping_add(DELTA);
            a = a.wrapping_add(mix(b, sum, KEY[0], KEY[1]));
            b = b.wrapping_add(mix(a, sum, KEY[2], KEY[3]));
        } else {
            b = b.wrapping_sub(mix(a, sum, KEY[2], KEY[3]));
            a = a.wrapping_sub(mix(b, sum, KEY[0], KEY[1]));
            sum = sum.wrapping_sub(DELTA);
        }
    }
    let mut out = [0; 8];
    out[..4].copy_from_slice(&a.to_le_bytes());
    out[4..].copy_from_slice(&b.to_le_bytes());
    out
}
pub fn frame(command: &[u8]) -> [u8; 64] {
    assert!(command.len() <= 56);
    let mut plain = [0; 64];
    plain[..command.len()].copy_from_slice(command);
    let mut cipher = [0; 64];
    for (input, output) in plain.chunks_exact(8).zip(cipher.chunks_exact_mut(8)) {
        output.copy_from_slice(&block(input, true))
    }
    let mut report = [0; 64];
    report[0] = 0x55;
    report[1..].copy_from_slice(&cipher[..63]);
    report
}
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub enum Reply {
    Lights(crate::lighting::Lights),
    Heartbeat,
    Hooks(bool),
    Online(bool),
    Battery(u8),
    BatteryStatus { percent: Option<u8>, charging: Option<bool> },
    Button(&'static str, bool),
    Turn(i32),
}
pub fn decode(report: &[u8]) -> Option<Reply> {
    let body = match report {
        [0x55, rest @ ..] if rest.len() == 63 => rest,
        _ if report.len() == 63 => report,
        _ => return None,
    };
    let mut d = [0; 16];
    d[..8].copy_from_slice(&block(&body[..8], false));
    d[8..].copy_from_slice(&block(&body[8..16], false));
    if d[..4] == [0x81, 0x0b, 0x88, 0x11] {
        let mut params = [0; 32];
        params[..16].copy_from_slice(&d);
        params[16..24].copy_from_slice(&block(&body[16..24], false));
        params[24..].copy_from_slice(&block(&body[24..32], false));
        return crate::lighting::Lights::from_report(&params).map(Reply::Lights);
    }
    let voltage = |at: usize| u16::from_le_bytes([d[at], d[at + 1]]);
    match (d[0] & 31, d[1], d[2], d[3]) {
        (6, 1, 0x23, _) => Some(Reply::Heartbeat),
        (1, 11, 0x89, 0x10) if d[4] <= 1 => Some(Reply::Hooks(d[4] == 1)),
        (6, 3, 0x0a, 0x11) if d[4] <= 1 => Some(Reply::Online(d[4] == 1)),
        (1, 1, 2, 0x11) if (2000..=5000).contains(&voltage(4)) => {
            Some(Reply::BatteryStatus {
                percent: (voltage(6) <= 100).then_some(d[6]),
                charging: match d[10] { 0 => Some(false), 1 => Some(true), _ => None },
            })
        }
        (11, 0x7b, _, _) if (2000..=5000).contains(&voltage(2)) && d[5] <= 100 => {
            Some(Reply::Battery(d[5]))
        }
        (11, 0x10, code, edge) if edge <= 1 && d[4] < 6 && d[0] == 0x8b => {
            let i = d[4] as usize;
            if [0x6f, 0x70, 0x71, 0x6e, 0x72, 0x73][i] != code {
                return None;
            }
            if i < 4 {
                Some(Reply::Button(["K1", "K2", "K3", "DIAL"][i], edge == 1))
            } else if edge == 1 {
                Some(Reply::Turn(if i == 4 { 1 } else { -1 }))
            } else {
                None
            }
        }
        _ => None,
    }
}
pub const BATTERY: &[u8] = &[1, 1, 2, 1];
pub const ONLINE: &[u8] = &[6, 3, 10, 1];
pub const HEARTBEAT: &[u8] = &[6, 1, 0x23, 0, 1];
pub fn hooks(enabled: bool) -> [u8; 5] {
    [1, 11, 0x89, 4, enabled as u8]
}
pub fn handshake() -> [u8; 9] {
    let id = uuid::Uuid::new_v4();
    let n = &id.as_bytes()[..4];
    [6, 2, 5, 1, n[0] & 15, n[0], n[1], n[2], n[3]]
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn documented_wire_vectors() {
        assert_eq!(
            &frame(BATTERY)[9..17],
            &[0x38, 0x90, 0xc4, 0x99, 0xa3, 0x60, 0xaa, 0xad]
        );
        for (i, code) in [0x6f, 0x70, 0x71, 0x6e, 0x72, 0x73].into_iter().enumerate() {
            let p = frame(&[0x8b, 0x10, code, 1, i as u8]);
            assert!(decode(&p).is_some());
            assert_eq!(decode(&p), decode(&p[1..]));
        }
        assert_eq!(
            decode(&frame(&[0x8b, 0x10, 0x6f, 0, 0])),
            Some(Reply::Button("K1", false))
        );
        assert_eq!(decode(&frame(&[0x8b, 0x10, 0x70, 1, 0])), None);
        assert_eq!(
            decode(&frame(&[0x81, 11, 0x89, 0x10, 1])),
            Some(Reply::Hooks(true))
        );
        assert_eq!(
            decode(&frame(&[0x81, 1, 2, 0x11, 0xa0, 0x0f, 80, 0])),
            Some(Reply::BatteryStatus { percent: Some(80), charging: Some(false) })
        );
        assert_eq!(
            decode(&frame(&[0x81, 1, 2, 0x11, 0xa0, 0x0f, 0xff, 0])),
            Some(Reply::BatteryStatus { percent: None, charging: Some(false) })
        );
        for (flag, expected) in [(0, Some(false)), (1, Some(true)), (0xff, None)] {
            let captured = [0x81, 1, 2, 0x11, 0x0a, 0x10, 100, 0, 0x29, 2, flag];
            assert_eq!(decode(&frame(&captured)), Some(Reply::BatteryStatus { percent: Some(100), charging: expected }));
        }
    }
}
