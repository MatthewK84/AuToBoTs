//! One record per tick. Absent revert is Revert. A fault latches.

use crate::event::event;
use crate::monitor::BuiltMonitor;
use rta_spec::{InputRecord, Verdict};
use rtlola_interpreter::monitor::Change;
use rtlola_interpreter::Value;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TickResult {
    pub verdict: Verdict,
    pub reasons: Vec<String>,
}

#[derive(Debug, Default)]
pub struct Evaluator {
    faulted: bool,
}

impl Evaluator {
    pub fn faulted(&self) -> bool {
        self.faulted
    }

    pub fn tick(
        &mut self,
        monitor: &mut BuiltMonitor,
        record: &InputRecord,
        at_ms: u64,
    ) -> TickResult {
        if self.faulted {
            return TickResult {
                verdict: Verdict::Revert,
                reasons: vec!["fault".into()],
            };
        }
        match monitor.accept(event(record), monotonic_to_interpreter(at_ms)) {
            Err(_) => {
                self.faulted = true;
                TickResult {
                    verdict: Verdict::Revert,
                    reasons: vec!["fault".into()],
                }
            }
            Ok(verdicts) => map_changes(&verdicts.event, |id| {
                monitor.borrow_monitor().name_for_output(id).to_string()
            }),
        }
    }
}

pub fn monotonic_to_interpreter(at_ms: u64) -> Duration {
    Duration::from_millis(at_ms)
}

pub fn map_bools(revert: Option<bool>, inhibit: Option<bool>, reasons: Vec<String>) -> TickResult {
    let verdict = match (revert, inhibit) {
        (None, _) | (_, None) => Verdict::Revert,
        (Some(true), _) => Verdict::Revert,
        (Some(false), Some(true)) => Verdict::Inhibit,
        (Some(false), Some(false)) => Verdict::Pass,
    };
    TickResult { verdict, reasons }
}

fn map_changes<F>(
    changes: &[(rtlola_frontend::mir::OutputReference, Vec<Change>)],
    name_of: F,
) -> TickResult
where
    F: Fn(rtlola_frontend::mir::OutputReference) -> String,
{
    let mut revert = None;
    let mut inhibit = None;
    let mut reasons = Vec::new();
    for (id, updates) in changes {
        let name = name_of(*id);
        for update in updates {
            if let Change::Value(_, Value::Bool(value)) = update {
                if *value && !matches!(name.as_str(), "revert" | "inhibit") {
                    reasons.push(name.clone());
                }
                if name == "revert" {
                    revert = Some(*value);
                }
                if name == "inhibit" {
                    inhibit = Some(*value);
                }
            }
        }
    }
    map_bools(revert, inhibit, reasons)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotonic_millis_become_interpreter_duration() {
        assert_eq!(monotonic_to_interpreter(1_500).as_secs_f64(), 1.5);
    }

    #[test]
    fn mapping_table() {
        assert_eq!(
            map_bools(Some(true), Some(false), vec![]).verdict,
            Verdict::Revert
        );
        assert_eq!(
            map_bools(Some(false), Some(true), vec![]).verdict,
            Verdict::Inhibit
        );
        assert_eq!(
            map_bools(Some(false), Some(false), vec![]).verdict,
            Verdict::Pass
        );
        assert_eq!(
            map_bools(None, Some(false), vec![]).verdict,
            Verdict::Revert
        );
    }

    #[test]
    fn fault_latches_without_another_call() {
        let mut evaluator = Evaluator { faulted: true };
        let mut monitor =
            crate::monitor::build_once(include_str!("../../../spec/monitor.lola")).unwrap();
        let record = InputRecord::try_new(1.0, true, 0.0, 1.0, 10.0, 0.0, 0.0).unwrap();
        let before = crate::monitor::build_count();
        let result = evaluator.tick(&mut monitor, &record, 10);
        assert_eq!(result.verdict, Verdict::Revert);
        assert_eq!(crate::monitor::build_count(), before);
    }

    #[test]
    fn live_revert_and_pass() {
        let mut evaluator = Evaluator::default();
        let mut monitor =
            crate::monitor::build_once(include_str!("../../../spec/monitor.lola")).unwrap();
        let bad = InputRecord::try_new(1.0, false, 0.0, 1.0, 10.0, 0.0, 0.0).unwrap();
        let good = InputRecord::try_new(1.0, true, 0.0, 1.0, 200.0, 0.0, 0.0).unwrap();
        assert_eq!(
            evaluator.tick(&mut monitor, &bad, 10).verdict,
            Verdict::Revert
        );
        assert_eq!(
            evaluator.tick(&mut monitor, &good, 20).verdict,
            Verdict::Pass
        );
    }
}
