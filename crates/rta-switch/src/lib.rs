//! Pure switch. Forbidden: rtlola-interpreter, mavlink, and a release actuator.

use rta_spec::{Phase, Reason, TickInput, TickLog, Verdict};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Idle,
    Navigation,
    Commit,
    Abort,
    Recovery,
}

pub fn decide(input: &TickInput) -> (Command, TickLog) {
    if input.pilot_override {
        return inhibit(Reason::PilotOverride);
    }
    if input.abort_requested || input.phase == Phase::Abort {
        return (
            Command::Abort,
            TickLog {
                verdict: Verdict::Revert,
                reason: Reason::Abort,
                grant_hash: None,
            },
        );
    }
    if !input.estimator_ok {
        return (
            Command::Recovery,
            TickLog {
                verdict: Verdict::Revert,
                reason: Reason::Estimator,
                grant_hash: None,
            },
        );
    }
    if input.endurance_s <= input.time_to_rally_s {
        return (
            Command::Recovery,
            TickLog {
                verdict: Verdict::Revert,
                reason: Reason::Energy,
                grant_hash: None,
            },
        );
    }
    if !input.coast_ok {
        return inhibit(Reason::Coast);
    }
    if !input.grant_present || input.phase != Phase::Commit {
        return inhibit(Reason::NoGrant);
    }
    (
        Command::Commit,
        TickLog {
            verdict: Verdict::Pass,
            reason: Reason::None,
            grant_hash: Some("hash-only".into()),
        },
    )
}

fn inhibit(reason: Reason) -> (Command, TickLog) {
    (
        Command::Navigation,
        TickLog {
            verdict: Verdict::Inhibit,
            reason,
            grant_hash: None,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok() -> TickInput {
        TickInput {
            phase: Phase::Commit,
            grant_present: true,
            coast_ok: true,
            pilot_override: false,
            abort_requested: false,
            estimator_ok: true,
            endurance_s: 100.0,
            time_to_rally_s: 10.0,
        }
    }

    #[test]
    fn missing_grant_does_not_commit() {
        let mut input = ok();
        input.grant_present = false;
        let (command, log) = decide(&input);
        assert_ne!(command, Command::Commit);
        assert_eq!(log.verdict, Verdict::Inhibit);
        assert_eq!(log.reason, Reason::NoGrant);
    }

    #[test]
    fn coast_failure_does_not_commit() {
        let mut input = ok();
        input.coast_ok = false;
        let (command, log) = decide(&input);
        assert_ne!(command, Command::Commit);
        assert_eq!(log.reason, Reason::Coast);
    }

    #[test]
    fn pilot_override_does_not_commit() {
        let mut input = ok();
        input.pilot_override = true;
        let (command, _) = decide(&input);
        assert_ne!(command, Command::Commit);
    }

    #[test]
    fn inhibit_does_not_keep_commit() {
        let mut input = ok();
        input.grant_present = false;
        let (command, log) = decide(&input);
        assert_eq!(log.verdict, Verdict::Inhibit);
        assert_ne!(command, Command::Commit);
    }

    #[test]
    fn abort_does_not_commit() {
        let mut input = ok();
        input.abort_requested = true;
        let (command, log) = decide(&input);
        assert_eq!(command, Command::Abort);
        assert_ne!(command, Command::Commit);
        assert_eq!(log.reason, Reason::Abort);
    }
}
