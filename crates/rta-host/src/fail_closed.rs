//! Host fail-closed paths. A missing verdict is Revert. There is no clear method.
//!
//! Every latch the host holds reaches the switch here: an evaluation fault and a log write
//! failure each latch Revert until restart, whatever command the tick would have sent. The
//! heartbeat limit is the spec's `heartbeat_stale` threshold (`spec::limits`), not a copy.

use crate::ring::Ring;
use crate::spec::SpecLimits;
use rta_spec::{RecoveryMode, Verdict};
use rta_switch::{decide, Request, SwitchCommand};

pub struct TickCase {
    pub verdict: Option<Verdict>,
    pub late: bool,
    pub heartbeat_age_ms: Option<f64>,
    pub request: Option<Request>,
    pub reported: RecoveryMode,
    pub recovery: RecoveryMode,
    /// The last tick-log write succeeded.
    pub write_ok: bool,
    /// The evaluation worker has faulted: a missed deadline, a busy or dead worker, or an
    /// interpreter error.
    pub eval_fault: bool,
}

#[derive(Debug)]
pub struct FailClosed {
    heartbeat_limit_ms: f64,
    faulted: bool,
    write_fault: bool,
    log: Ring<&'static str, NOTE_CAPACITY>,
}

impl FailClosed {
    pub fn new(limits: SpecLimits) -> Self {
        Self {
            heartbeat_limit_ms: limits.heartbeat_stale_ms,
            faulted: false,
            write_fault: false,
            log: Ring::new(""),
        }
    }

    /// The evaluation-fault latch.
    pub fn faulted(&self) -> bool {
        self.faulted
    }

    /// The write-failure latch.
    pub fn write_faulted(&self) -> bool {
        self.write_fault
    }

    pub fn command(&mut self, tick: TickCase) -> SwitchCommand {
        if tick.eval_fault && !self.faulted {
            self.faulted = true;
            self.note("evaluation fault latched");
        }
        // A failed write latches on any command, not only on a mode change.
        if !tick.write_ok && !self.write_fault {
            self.write_fault = true;
            self.note("log write failed");
        }
        // A missing heartbeat age, or a NaN one, is stale.
        let heartbeat_fresh = tick
            .heartbeat_age_ms
            .is_some_and(|age| age <= self.heartbeat_limit_ms);
        let forced = self.faulted
            || self.write_fault
            || tick.late
            || tick.verdict.is_none()
            || !heartbeat_fresh;
        if tick.late {
            self.note("late verdict discarded");
        }
        if forced {
            decide(Verdict::Revert, None, tick.reported, tick.recovery)
        } else {
            decide(
                tick.verdict.unwrap_or(Verdict::Revert),
                tick.request,
                tick.reported,
                tick.recovery,
            )
        }
    }

    /// The note log is a ring of the last `NOTE_CAPACITY` static lines. A late verdict every
    /// tick overwrites the oldest note; it neither grows nor allocates.
    fn note(&mut self, line: &'static str) {
        self.log.push(line);
    }

    /// The notes, oldest first.
    pub fn notes(&self) -> impl Iterator<Item = &&'static str> + '_ {
        self.log.iter()
    }
}

/// Lines kept by the fail-closed note log.
pub const NOTE_CAPACITY: usize = 64;

#[cfg(test)]
mod tests {
    use super::*;

    const LIMITS: SpecLimits = SpecLimits {
        heartbeat_stale_ms: 1_234.0,
    };

    fn host() -> FailClosed {
        FailClosed::new(LIMITS)
    }

    fn recovery() -> (RecoveryMode, RecoveryMode) {
        (RecoveryMode::Loiter, RecoveryMode::Rtl)
    }

    fn case(
        verdict: Option<Verdict>,
        late: bool,
        heartbeat_age_ms: Option<f64>,
        write_ok: bool,
    ) -> TickCase {
        let (reported, recovery) = recovery();
        TickCase {
            verdict,
            late,
            heartbeat_age_ms,
            request: None,
            reported,
            recovery,
            write_ok,
            eval_fault: false,
        }
    }

    #[test]
    fn no_verdict_is_revert() {
        let mut host = host();
        assert!(matches!(
            host.command(case(None, false, Some(0.0), true)),
            SwitchCommand::Mode { .. }
        ));
    }

    #[test]
    // trace: H7
    fn late_tick_discards_pass() {
        let mut host = host();
        let command = host.command(case(Some(Verdict::Pass), true, Some(0.0), true));
        assert!(matches!(command, SwitchCommand::Mode { .. }));
        assert!(host.log.iter().any(|line| line.contains("discarded")));
    }

    #[test]
    fn a_late_verdict_every_tick_keeps_the_note_ring_at_capacity() {
        let mut host = host();
        for _ in 0..10 * NOTE_CAPACITY {
            host.command(case(Some(Verdict::Pass), true, Some(0.0), true));
        }
        assert_eq!(host.notes().count(), NOTE_CAPACITY);
        assert!(host.notes().all(|line| line.contains("discarded")));
    }

    #[test]
    // trace: H8
    fn fault_stays_revert() {
        let mut host = host();
        let mut first = case(Some(Verdict::Pass), false, Some(0.0), true);
        first.eval_fault = true;
        assert!(matches!(host.command(first), SwitchCommand::Mode { .. }));
        // The worker's fault reaches the switch and stays, with no fault on later ticks.
        let command = host.command(case(Some(Verdict::Pass), false, Some(0.0), true));
        assert!(matches!(command, SwitchCommand::Mode { .. }));
        assert!(host.faulted());
    }

    #[test]
    // trace: H4
    fn missing_heartbeat_is_stale() {
        let mut host = host();
        assert!(matches!(
            host.command(case(Some(Verdict::Pass), false, None, true)),
            SwitchCommand::Mode { .. }
        ));
    }

    #[test]
    // trace: H10
    fn write_failure_latches_and_retries() {
        let mut host = host();
        let first = host.command(case(Some(Verdict::Revert), false, Some(0.0), false));
        let second = host.command(case(Some(Verdict::Pass), false, Some(0.0), true));
        assert!(matches!(first, SwitchCommand::Mode { .. }));
        assert!(matches!(second, SwitchCommand::Mode { .. }));
        assert!(host.log.iter().any(|line| line.contains("write failed")));
    }

    #[test]
    fn a_write_failure_on_a_setpoint_tick_latches_too() {
        let mut host = host();
        // A Pass tick whose write failed is not let through as setpoints.
        let first = host.command(case(Some(Verdict::Pass), false, Some(0.0), false));
        assert!(matches!(first, SwitchCommand::Mode { .. }), "{first:?}");
        let later = host.command(case(Some(Verdict::Pass), false, Some(0.0), true));
        assert!(matches!(later, SwitchCommand::Mode { .. }));
        assert!(host.write_faulted());
    }

    #[test]
    fn the_heartbeat_limit_comes_from_the_spec_limits() {
        let mut host = host();
        let at = |host: &mut FailClosed, age: f64| {
            host.command(case(Some(Verdict::Pass), false, Some(age), true))
        };
        assert!(matches!(
            at(&mut host, 1_234.0),
            SwitchCommand::Setpoints { .. }
        ));
        assert!(matches!(at(&mut host, 1_234.5), SwitchCommand::Mode { .. }));
        assert!(matches!(
            at(&mut host, f64::NAN),
            SwitchCommand::Mode { .. }
        ));
        let spec = crate::spec::limits(include_str!("../../../spec/monitor.lola")).unwrap();
        let mut real = FailClosed::new(spec);
        let fresh = real.command(case(
            Some(Verdict::Pass),
            false,
            Some(spec.heartbeat_stale_ms),
            true,
        ));
        assert!(matches!(fresh, SwitchCommand::Setpoints { .. }));
    }

    #[test]
    fn no_clear_fault() {
        let source = include_str!("fail_closed.rs");
        let name = ["pub fn ", "clear_fault"].concat();
        assert!(!source.contains(&name));
    }
}
