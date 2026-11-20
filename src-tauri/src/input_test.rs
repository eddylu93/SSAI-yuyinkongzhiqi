use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputTest {
    pub pairs: BTreeMap<String, u32>,
    pub turns: BTreeMap<String, u32>,
    pressed: BTreeSet<String>,
}

impl InputTest {
    pub fn interrupt(&mut self) {
        self.pressed.clear();
    }

    pub fn record(&mut self, control: &str, phase: &str, steps: i32) {
        match phase {
            "down" => {
                self.pressed.insert(control.into());
            }
            "up" if self.pressed.remove(control) => {
                *self.pairs.entry(control.into()).or_default() += 1;
            }
            "turn" | "heldTurn" if steps != 0 => {
                let key = match (phase, steps > 0) {
                    ("heldTurn", true) => "heldRight",
                    ("heldTurn", false) => "heldLeft",
                    (_, true) => "right",
                    (_, false) => "left",
                };
                *self.turns.entry(key.into()).or_default() += steps.unsigned_abs();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_complete_presses_and_signed_rotations() {
        let mut session = InputTest::default();
        for phase in ["up", "down", "down", "up", "up", "down"] {
            session.record("K1", phase, 0);
        }
        session.interrupt();
        session.record("K1", "up", 0);
        assert_eq!(session.pairs["K1"], 1);
        for (phase, steps) in [("turn", 3), ("turn", -2), ("heldTurn", 1), ("heldTurn", -4)] {
            session.record("DIAL", phase, steps);
        }
        assert_eq!(session.turns["right"], 3);
        assert_eq!(session.turns["left"], 2);
        assert_eq!(session.turns["heldRight"], 1);
        assert_eq!(session.turns["heldLeft"], 4);
    }
}
