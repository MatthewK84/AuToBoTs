//! Startup gating. When the runtime may command the flight controller at all.
//!
//! `armed` (the production default): while the flight controller reports disarmed, the
//! runtime monitors and logs every tick and sends no setpoint and no recovery command. An arm
//! before readiness is a latched `not_ready_at_arm` fault and full fail-closed Revert at once.
//! Once armed and ready, the existing fail-closed behavior runs unchanged. A disarm goes back
//! to monitor only.
//!
//! `immediate` (bench and SITL): the arm state is ignored, but nothing is commanded until the
//! runtime is ready, or until `ready_timeout_ms` has passed without readiness.
//!
//! Ready means a flight-controller `HEARTBEAT` has been heard and the position fix is no
//! older than `ready_fix_age_ms`. Detail in `docs/startup.md`.

/// Which gate the runtime runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Armed,
    Immediate,
}

/// What the tick thread saw this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inputs {
    /// A flight-controller `HEARTBEAT` has been heard.
    pub heard: bool,
    /// The last flight-controller `HEARTBEAT` had `MAV_MODE_FLAG_SAFETY_ARMED` set.
    pub armed: bool,
    /// Heard, and the fix is fresh.
    pub ready: bool,
}

/// What the tick thread may do this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    /// Setpoints and recovery may go out. False is monitor only.
    pub act: bool,
    /// Revert whatever the spec said: the latched `not_ready_at_arm` fault.
    pub force_revert: bool,
}

#[derive(Debug, Clone)]
pub struct Gate {
    mode: Mode,
    ready_timeout_ms: u64,
    was_armed: bool,
    ever_ready: bool,
    opened: bool,
    not_ready_at_arm: bool,
}

impl Gate {
    pub fn new(mode: Mode, ready_timeout_ms: u64) -> Self {
        Self {
            mode,
            ready_timeout_ms,
            was_armed: false,
            ever_ready: false,
            opened: false,
            not_ready_at_arm: false,
        }
    }

    /// The latched arm-before-ready fault.
    pub fn faulted(&self) -> bool {
        self.not_ready_at_arm
    }

    /// One tick. Pushes the gate's reason codes.
    pub fn step(&mut self, now_ms: u64, inputs: Inputs, reasons: &mut Vec<String>) -> Decision {
        self.ever_ready |= inputs.ready;
        let timed_out = !self.ever_ready && now_ms >= self.ready_timeout_ms;
        if timed_out {
            reasons.push("ready_timeout".to_string());
        }
        let decision = match self.mode {
            Mode::Armed => self.armed_step(inputs, reasons),
            Mode::Immediate => self.immediate_step(inputs, timed_out, reasons),
        };
        if self.not_ready_at_arm {
            reasons.push("not_ready_at_arm".to_string());
        }
        decision
    }

    fn armed_step(&mut self, inputs: Inputs, reasons: &mut Vec<String>) -> Decision {
        let armed = inputs.heard && inputs.armed;
        // An arm seen before readiness, including a first heartbeat that is already armed.
        if armed && !self.was_armed && !inputs.ready {
            self.not_ready_at_arm = true;
        }
        self.was_armed = armed;
        if inputs.heard && !armed {
            reasons.push("disarmed".to_string());
        }
        Decision {
            act: armed,
            force_revert: self.not_ready_at_arm,
        }
    }

    fn immediate_step(
        &mut self,
        inputs: Inputs,
        timed_out: bool,
        reasons: &mut Vec<String>,
    ) -> Decision {
        self.opened |= inputs.ready || (inputs.heard && timed_out);
        if inputs.heard && !self.opened {
            reasons.push("not_ready".to_string());
        }
        Decision {
            act: inputs.heard && self.opened,
            force_revert: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIMEOUT: u64 = 1_000;

    fn inputs(heard: bool, armed: bool, ready: bool) -> Inputs {
        Inputs {
            heard,
            armed,
            ready,
        }
    }

    fn step(gate: &mut Gate, now: u64, i: Inputs) -> (Decision, Vec<String>) {
        let mut reasons = Vec::new();
        let decision = gate.step(now, i, &mut reasons);
        (decision, reasons)
    }

    #[test]
    fn armed_mode_is_monitor_only_while_disarmed() {
        let mut gate = Gate::new(Mode::Armed, TIMEOUT);
        for now in [0, 100, 5_000] {
            let (decision, reasons) = step(&mut gate, now, inputs(true, false, true));
            assert!(!decision.act);
            assert!(!decision.force_revert);
            assert_eq!(reasons, ["disarmed"]);
        }
    }

    #[test]
    fn arm_after_ready_acts_and_disarm_returns_to_monitor_only() {
        let mut gate = Gate::new(Mode::Armed, TIMEOUT);
        step(&mut gate, 0, inputs(true, false, true));
        let (decision, reasons) = step(&mut gate, 100, inputs(true, true, true));
        assert_eq!(
            decision,
            Decision {
                act: true,
                force_revert: false
            }
        );
        assert!(reasons.is_empty());
        // A stale fix after the arm is the spec's business, not the gate's.
        assert!(step(&mut gate, 200, inputs(true, true, false)).0.act);
        let (decision, reasons) = step(&mut gate, 300, inputs(true, false, true));
        assert!(!decision.act);
        assert_eq!(reasons, ["disarmed"]);
        assert!(!gate.faulted());
    }

    #[test]
    fn arm_before_ready_is_a_latched_fault_and_reverts_at_once() {
        let mut gate = Gate::new(Mode::Armed, TIMEOUT);
        step(&mut gate, 0, inputs(true, false, false));
        let (decision, reasons) = step(&mut gate, 100, inputs(true, true, false));
        assert_eq!(
            decision,
            Decision {
                act: true,
                force_revert: true
            }
        );
        assert_eq!(reasons, ["not_ready_at_arm"]);
        // Readiness later does not clear it.
        let (decision, _) = step(&mut gate, 200, inputs(true, true, true));
        assert!(decision.force_revert);
        // Disarmed it is still monitor only, and the fault is still logged.
        let (decision, reasons) = step(&mut gate, 300, inputs(true, false, true));
        assert!(!decision.act && decision.force_revert);
        assert_eq!(reasons, ["disarmed", "not_ready_at_arm"]);
    }

    #[test]
    fn a_first_heartbeat_that_is_already_armed_counts_as_an_arm() {
        let mut gate = Gate::new(Mode::Armed, TIMEOUT);
        assert!(step(&mut gate, 0, inputs(true, true, false)).0.force_revert);
        let mut ready = Gate::new(Mode::Armed, TIMEOUT);
        assert!(!step(&mut ready, 0, inputs(true, true, true)).0.force_revert);
    }

    #[test]
    fn nothing_is_decided_before_the_flight_controller_is_heard() {
        for mode in [Mode::Armed, Mode::Immediate] {
            let mut gate = Gate::new(mode, TIMEOUT);
            let (decision, reasons) = step(&mut gate, 0, inputs(false, false, false));
            assert!(!decision.act);
            assert!(reasons.is_empty());
        }
    }

    #[test]
    fn the_ready_timeout_is_logged_and_commands_nothing_while_disarmed() {
        let mut gate = Gate::new(Mode::Armed, TIMEOUT);
        let (_, reasons) = step(&mut gate, TIMEOUT - 1, inputs(true, false, false));
        assert_eq!(reasons, ["disarmed"]);
        let (decision, reasons) = step(&mut gate, TIMEOUT, inputs(true, false, false));
        assert!(!decision.act);
        assert_eq!(reasons, ["ready_timeout", "disarmed"]);
        // Once ready, the timeout is not logged again.
        step(&mut gate, TIMEOUT + 1, inputs(true, false, true));
        let (_, reasons) = step(&mut gate, TIMEOUT + 2, inputs(true, false, false));
        assert_eq!(reasons, ["disarmed"]);
    }

    #[test]
    fn immediate_mode_waits_for_readiness_then_ignores_the_arm_state() {
        let mut gate = Gate::new(Mode::Immediate, TIMEOUT);
        // Heartbeat before position: the boot race. Nothing is commanded.
        let (decision, reasons) = step(&mut gate, 0, inputs(true, false, false));
        assert!(!decision.act);
        assert_eq!(reasons, ["not_ready"]);
        let (decision, reasons) = step(&mut gate, 100, inputs(true, false, true));
        assert!(decision.act && !decision.force_revert);
        assert!(reasons.is_empty());
        // Ready once is enough. A later stale fix is the spec's Revert, and it is sent.
        assert!(step(&mut gate, 200, inputs(true, true, false)).0.act);
    }

    #[test]
    fn immediate_mode_acts_after_the_ready_timeout() {
        let mut gate = Gate::new(Mode::Immediate, TIMEOUT);
        assert!(
            !step(&mut gate, TIMEOUT - 1, inputs(true, false, false))
                .0
                .act
        );
        let (decision, reasons) = step(&mut gate, TIMEOUT, inputs(true, false, false));
        assert!(decision.act);
        assert_eq!(reasons, ["ready_timeout"]);
    }
}

#[cfg(test)]
mod doc_tests {
    use crate::config::{self, Startup, StartupMode};

    #[test]
    fn the_startup_note_matches_the_gate_and_the_defaults() {
        let note = include_str!("../../../docs/startup.md");
        let defaults = Startup::default();
        assert_eq!(defaults.mode, StartupMode::Armed);
        for item in [
            "## Gating".to_string(),
            "| `armed` (production default) | monitor only | monitor only, log `disarmed` | full fail-closed behavior |".to_string(),
            "| `immediate` (bench, SITL) | monitor only, log `not_ready` |".to_string(),
            "`MAV_MODE_FLAG_SAFETY_ARMED`".to_string(),
            "logs `not_ready_at_arm`".to_string(),
            "latches until process restart".to_string(),
            "A disarm goes back to monitor only.".to_string(),
            "The watchdog obeys the same gate.".to_string(),
            "no longer sends a recovery mode at boot".to_string(),
            "logs `ready_timeout`".to_string(),
            format!("| `startup.ready_timeout_ms` | {} |", defaults.ready_timeout_ms),
            format!("| `startup.ready_fix_age_ms` | {} |", defaults.ready_fix_age_ms),
        ] {
            assert!(note.contains(&item), "{item}");
        }
        let example = config::parse_str(include_str!("../../../config/example.toml")).unwrap();
        assert_eq!(example.startup, defaults);
        let requirements = include_str!("../../../docs/requirements.md");
        assert!(requirements.contains("| R-S1 | startup |"));
        assert!(requirements.contains("| R-S2 | startup |"));
        let icd = include_str!("../../../docs/icd-fc.md");
        assert!(icd.contains("`MAV_MODE_FLAG_SAFETY_ARMED` (128)"));
    }
}
