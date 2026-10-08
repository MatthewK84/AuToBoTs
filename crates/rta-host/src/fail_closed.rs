//! Host fail-closed paths. A missing verdict is Revert. There is no clear method.

use rta_spec::{RecoveryMode, Verdict};
use rta_switch::{decide, Request, SwitchCommand};

pub struct TickCase {
    pub verdict: Option<Verdict>,
    pub late: bool,
    pub heartbeat_age_ms: Option<f64>,
    pub request: Option<Request>,
    pub reported: RecoveryMode,
    pub recovery: RecoveryMode,
    pub write_ok: bool,
}

#[derive(Debug, Default)]
pub struct FailClosed {
    faulted: bool,
    write_fault: bool,
    log: Vec<String>,
}

impl FailClosed {
    pub fn command(&mut self, tick: TickCase) -> SwitchCommand {
        if self.faulted || self.write_fault {
            return self.mode_or_retry(tick.reported, tick.recovery, tick.write_ok);
        }
        if tick.late
            || tick.verdict.is_none()
            || tick.heartbeat_age_ms.unwrap_or(f64::MAX) > 1_000.0
        {
            if tick.late {
                self.log.push("late verdict discarded".into());
            }
            return self.mode_or_retry(tick.reported, tick.recovery, tick.write_ok);
        }
        decide(
            tick.verdict.unwrap_or(Verdict::Revert),
            tick.request,
            tick.reported,
            tick.recovery,
        )
    }

    fn mode_or_retry(
        &mut self,
        reported: RecoveryMode,
        recovery: RecoveryMode,
        write_ok: bool,
    ) -> SwitchCommand {
        let command = decide(Verdict::Revert, None, reported, recovery);
        if matches!(command, SwitchCommand::Mode { .. }) && !write_ok {
            self.write_fault = true;
            self.log.push("mode write failed".into());
        }
        command
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        }
    }

    #[test]
    fn no_verdict_is_revert() {
        let mut host = FailClosed::default();
        assert!(matches!(
            host.command(case(None, false, Some(0.0), true)),
            SwitchCommand::Mode { .. }
        ));
    }

    #[test]
    fn late_tick_discards_pass() {
        let mut host = FailClosed::default();
        let command = host.command(case(Some(Verdict::Pass), true, Some(0.0), true));
        assert!(matches!(command, SwitchCommand::Mode { .. }));
        assert!(host.log.iter().any(|line| line.contains("discarded")));
    }

    #[test]
    fn fault_stays_revert() {
        let mut host = FailClosed {
            faulted: true,
            write_fault: false,
            log: Vec::new(),
        };
        let command = host.command(case(Some(Verdict::Pass), false, Some(0.0), true));
        assert!(matches!(command, SwitchCommand::Mode { .. }));
    }

    #[test]
    fn missing_heartbeat_is_stale() {
        let mut host = FailClosed::default();
        assert!(matches!(
            host.command(case(Some(Verdict::Pass), false, None, true)),
            SwitchCommand::Mode { .. }
        ));
    }

    #[test]
    fn write_failure_latches_and_retries() {
        let mut host = FailClosed::default();
        let first = host.command(case(Some(Verdict::Revert), false, Some(0.0), false));
        let second = host.command(case(Some(Verdict::Pass), false, Some(0.0), true));
        assert!(matches!(first, SwitchCommand::Mode { .. }));
        assert!(matches!(second, SwitchCommand::Mode { .. }));
        assert!(host.log.iter().any(|line| line.contains("write failed")));
    }

    #[test]
    fn no_clear_fault() {
        let source = include_str!("fail_closed.rs");
        let name = ["clear", "_fault"].concat();
        assert!(!source.contains(&name));
    }
}
