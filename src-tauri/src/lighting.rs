use serde::{Deserialize, Serialize};

pub const QUERY: &[u8] = &[1, 11, 0x88, 1];

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Lights {
    pub mode: u8,
    pub brightness: u8,
    pub lamps: [u8; 4],
    #[serde(rename = "workBrightness")]
    pub work_brightness: [u8; 4],
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Change {
    Mode { value: u8 },
    Brightness { value: u8 },
    Lamp { index: usize, value: u8 },
}

impl Lights {
    pub fn from_report(data: &[u8]) -> Option<Self> {
        if data.len() < 28 || data[6] > 2 || data[7] > 2 {
            return None;
        }
        let lamps = [data[8], data[13], data[18], data[23]];
        if lamps.iter().any(|v| *v > 2) {
            return None;
        }
        Some(Self {
            mode: data[6],
            brightness: data[7],
            lamps,
            work_brightness: std::array::from_fn(|i| {
                let base = 8 + i * 5;
                data[base + if lamps[i] == 2 {3} else {4}]
            }),
        })
    }
    pub fn command(&self, change: &Change) -> Result<Vec<u8>, String> {
        let mut command = vec![1, 11, 0x88, 4];
        match *change {
            Change::Mode { value } if value <= 2 => command.extend([1, 0, value]),
            Change::Brightness { value } if self.mode == 1 && value <= 2 => {
                command.extend([2, 0, 0, value])
            }
            Change::Lamp { index, value }
                if self.mode == 2 && index < 4 && value <= 2 =>
            {
                // Only change the type mask. Preserve the four existing types and all timing parameters.
                let mut payload = [0; 24];
                payload[0] = 4;
                payload[1] = index as u8;
                for (i, old) in self.lamps.iter().enumerate() {
                    payload[4 + i * 5] = *old;
                }
                payload[4 + index * 5] = value;
                command.extend(payload);
            }
            _ => return Err("当前灯光模式或固件不支持此设置".into()),
        }
        Ok(command)
    }
    pub fn matches(&self, change: &Change) -> bool {
        match *change {
            Change::Mode { value } => self.mode == value,
            Change::Brightness { value } => self.brightness == value,
            Change::Lamp { index, value } => self.lamps.get(index) == Some(&value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reads_brightness_for_the_active_effect_of_each_lamp() {
        let mut report = [0; 28];
        report[6..8].copy_from_slice(&[2, 1]);
        report[8..].copy_from_slice(&[
            2,10,2,1,0, 1,10,2,1,2, 0,10,2,2,0, 2,10,2,0,2,
        ]);
        let lights = Lights::from_report(&report).unwrap();
        assert_eq!(lights.work_brightness, [1, 2, 0, 0]);
        assert_eq!(lights.lamps, [2, 1, 0, 2]);
        assert!(Lights::from_report(&report[..27]).is_none());
    }
    #[test]
    fn lamp_write_only_changes_requested_type() {
        let s = Lights {
            mode: 2,
            brightness: 2,
            lamps: [2, 1, 1, 0],
            work_brightness: [2, 2, 2, 0],
        };
        let cmd = s.command(&Change::Lamp { index: 1, value: 0 }).unwrap();
        assert_eq!((cmd[4], cmd[5]), (4, 1));
        assert_eq!([cmd[8], cmd[13], cmd[18], cmd[23]], [2, 0, 1, 0]);
        assert!(s.command(&Change::Brightness { value: 1 }).is_err());
        assert!(s.command(&Change::Lamp { index: 4, value: 1 }).is_err());
    }
}
