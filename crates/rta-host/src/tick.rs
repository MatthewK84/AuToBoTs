//! One tick. A late result is discarded. An interpreter fault latches.

use rta_spec::Verdict;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outputs {
    pub revert: bool,
    pub inhibit: bool,
}

#[derive(Debug)]
pub struct TickFault;

pub trait Eval {
    fn accept(&mut self) -> Result<Outputs, TickFault>;
}

#[derive(Debug, Default)]
pub struct TickState {
    interpreter_fault: bool,
}

pub struct Idle;

impl Eval for Idle {
    fn accept(&mut self) -> Result<Outputs, TickFault> {
        Ok(Outputs {
            revert: false,
            inhibit: false,
        })
    }
}

impl TickState {
    pub fn faulted(&self) -> bool {
        self.interpreter_fault
    }

    pub fn tick(&mut self, monitor: &mut impl Eval, deadline: Duration) -> TimedTick {
        if self.interpreter_fault {
            return TimedTick {
                verdict: Verdict::Revert,
                duration: Duration::ZERO,
            };
        }
        let started = Instant::now();
        let result = monitor.accept();
        let duration = started.elapsed();
        if duration > deadline {
            return TimedTick {
                verdict: Verdict::Revert,
                duration,
            };
        }
        let verdict = match result {
            Err(TickFault) => {
                self.interpreter_fault = true;
                Verdict::Revert
            }
            Ok(outputs) if outputs.revert => Verdict::Revert,
            Ok(outputs) if outputs.inhibit => Verdict::Inhibit,
            Ok(_) => Verdict::Pass,
        };
        TimedTick { verdict, duration }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimedTick {
    pub verdict: Verdict,
    pub duration: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    struct Slow {
        calls: u32,
    }

    impl Eval for Slow {
        fn accept(&mut self) -> Result<Outputs, TickFault> {
            self.calls += 1;
            thread::sleep(Duration::from_millis(20));
            Ok(Outputs {
                revert: false,
                inhibit: false,
            })
        }
    }

    struct Fault {
        calls: u32,
    }

    impl Eval for Fault {
        fn accept(&mut self) -> Result<Outputs, TickFault> {
            self.calls += 1;
            Err(TickFault)
        }
    }

    #[test]
    fn fast_pass_stays_pass() {
        let mut state = TickState::default();
        let mut monitor = Idle;
        let timed = state.tick(&mut monitor, Duration::from_millis(50));
        assert_eq!(timed.verdict, Verdict::Pass);
        assert!(timed.duration < Duration::from_millis(50));
    }

    #[test]
    fn late_pass_is_revert() {
        let mut state = TickState::default();
        let mut monitor = Slow { calls: 0 };
        let timed = state.tick(&mut monitor, Duration::from_millis(5));
        assert_eq!(timed.verdict, Verdict::Revert);
        assert!(timed.duration > Duration::from_millis(5));
        assert_eq!(monitor.calls, 1);
    }

    #[test]
    fn fault_latches_and_skips_the_next_call() {
        let mut state = TickState::default();
        let mut monitor = Fault { calls: 0 };
        assert_eq!(
            state.tick(&mut monitor, Duration::from_millis(50)).verdict,
            Verdict::Revert
        );
        assert_eq!(
            state.tick(&mut monitor, Duration::from_millis(50)).verdict,
            Verdict::Revert
        );
        assert_eq!(monitor.calls, 1);
        assert!(state.faulted());
    }
}
