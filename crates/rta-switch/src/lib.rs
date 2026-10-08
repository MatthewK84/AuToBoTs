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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Request {
    pub north: f64,
    pub east: f64,
    pub down: f64,
    pub yaw: f64,
    pub commit: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SwitchCommand {
    Setpoints {
        north: f64,
        east: f64,
        down: f64,
        yaw: f64,
        commit: bool,
    },
    Mode {
        recovery: rta_spec::RecoveryMode,
    },
    Idle,
}

pub trait Complex {
    fn request(&mut self) -> Option<Request>;
}

#[derive(Debug, Default)]
pub struct StubComplex;

impl Complex for StubComplex {
    fn request(&mut self) -> Option<Request> {
        None
    }
}

pub fn decide(
    verdict: Verdict,
    request: Option<Request>,
    reported_mode: rta_spec::RecoveryMode,
    recovery: rta_spec::RecoveryMode,
) -> SwitchCommand {
    match verdict {
        Verdict::Revert if reported_mode == recovery => SwitchCommand::Idle,
        Verdict::Revert => SwitchCommand::Mode { recovery },
        Verdict::Inhibit => setpoints(request, false),
        Verdict::Pass => setpoints(request, request.is_some_and(|item| item.commit)),
    }
}

fn setpoints(request: Option<Request>, commit: bool) -> SwitchCommand {
    let request = request.unwrap_or(Request {
        north: 0.0,
        east: 0.0,
        down: 0.0,
        yaw: 0.0,
        commit: false,
    });
    SwitchCommand::Setpoints {
        north: request.north,
        east: request.east,
        down: request.down,
        yaw: request.yaw,
        commit,
    }
}

pub fn decide_input(input: &TickInput) -> (Command, TickLog) {
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
    fn pass_does_not_invent_commit() {
        let request = Request {
            north: 1.0,
            east: 0.0,
            down: 0.0,
            yaw: 0.0,
            commit: false,
        };
        match decide(
            Verdict::Pass,
            Some(request),
            rta_spec::RecoveryMode::Loiter,
            rta_spec::RecoveryMode::Rtl,
        ) {
            SwitchCommand::Setpoints { commit: false, .. } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn inhibit_clears_commit() {
        let request = Request {
            north: 1.0,
            east: 0.0,
            down: 0.0,
            yaw: 0.0,
            commit: true,
        };
        match decide(
            Verdict::Inhibit,
            Some(request),
            rta_spec::RecoveryMode::Rtl,
            rta_spec::RecoveryMode::Loiter,
        ) {
            SwitchCommand::Setpoints { commit: false, .. } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn revert_has_no_commit_field() {
        match decide(
            Verdict::Revert,
            Some(Request {
                north: 1.0,
                east: 0.0,
                down: 0.0,
                yaw: 0.0,
                commit: true,
            }),
            rta_spec::RecoveryMode::Rtl,
            rta_spec::RecoveryMode::Land,
        ) {
            SwitchCommand::Mode {
                recovery: rta_spec::RecoveryMode::Land,
            } => {}
            SwitchCommand::Idle => panic!("revert was already in the recovery mode"),
            SwitchCommand::Mode { .. } => panic!("revert changed the configured mode"),
            SwitchCommand::Setpoints { .. } => panic!("revert returned setpoints"),
        }
    }

    #[test]
    fn reported_mode_table() {
        let request = Request {
            north: 1.0,
            east: 0.0,
            down: 0.0,
            yaw: 0.0,
            commit: true,
        };
        let same = rta_spec::RecoveryMode::Rtl;
        let other = rta_spec::RecoveryMode::Land;
        assert!(matches!(
            decide(Verdict::Pass, Some(request), other, same),
            SwitchCommand::Setpoints { commit: true, .. }
        ));
        assert!(matches!(
            decide(Verdict::Pass, None, other, same),
            SwitchCommand::Setpoints { commit: false, .. }
        ));
        assert!(matches!(
            decide(Verdict::Inhibit, Some(request), other, same),
            SwitchCommand::Setpoints { commit: false, .. }
        ));
        assert!(matches!(
            decide(Verdict::Inhibit, None, other, same),
            SwitchCommand::Setpoints { commit: false, .. }
        ));
        assert!(matches!(
            decide(Verdict::Revert, Some(request), other, same),
            SwitchCommand::Mode { .. }
        ));
        assert!(matches!(
            decide(Verdict::Revert, None, same, same),
            SwitchCommand::Idle
        ));
    }

    #[test]
    fn stub_request_never_appears_in_revert() {
        let mut stub = StubComplex;
        let request = stub.request();
        assert!(request.is_none());
        match decide(
            Verdict::Revert,
            request,
            rta_spec::RecoveryMode::Loiter,
            rta_spec::RecoveryMode::Rtl,
        ) {
            SwitchCommand::Mode { .. } => {}
            SwitchCommand::Idle => panic!("stub request was treated as already recovered"),
            SwitchCommand::Setpoints { .. } => panic!("stub request appeared in revert"),
        }
    }

    #[test]
    fn six_verdict_rows() {
        let recovery = rta_spec::RecoveryMode::Rtl;
        let pending = Request {
            north: 1.0,
            east: 2.0,
            down: 3.0,
            yaw: 4.0,
            commit: true,
        };
        let rows = [
            (Verdict::Revert, Some(pending), true),
            (Verdict::Revert, None, true),
            (Verdict::Inhibit, Some(pending), false),
            (Verdict::Inhibit, None, false),
            (Verdict::Pass, Some(pending), false),
            (Verdict::Pass, None, false),
        ];
        for (verdict, request, mode) in rows {
            let command = decide(verdict, request, rta_spec::RecoveryMode::Loiter, recovery);
            assert_eq!(matches!(command, SwitchCommand::Mode { .. }), mode);
            if let SwitchCommand::Setpoints { commit, .. } = command {
                assert!(!commit || verdict == Verdict::Pass);
            }
        }
        match decide(
            Verdict::Pass,
            Some(pending),
            rta_spec::RecoveryMode::Loiter,
            recovery,
        ) {
            SwitchCommand::Setpoints {
                commit: true,
                north: 1.0,
                ..
            } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn switch_has_no_socket_or_interpreter() {
        let source = include_str!("lib.rs");
        let manifest = include_str!("../Cargo.toml");
        let socket = ["use std", "::net"].concat();
        assert!(!source.contains(&socket));
        assert!(!manifest.contains("rtlola"));
        assert!(!manifest.contains("mavlink"));
    }

    #[test]
    fn missing_grant_does_not_commit() {
        let mut input = ok();
        input.grant_present = false;
        let (command, log) = decide_input(&input);
        assert_ne!(command, Command::Commit);
        assert_eq!(log.verdict, Verdict::Inhibit);
        assert_eq!(log.reason, Reason::NoGrant);
    }

    #[test]
    fn coast_failure_does_not_commit() {
        let mut input = ok();
        input.coast_ok = false;
        let (command, log) = decide_input(&input);
        assert_ne!(command, Command::Commit);
        assert_eq!(log.reason, Reason::Coast);
    }

    #[test]
    fn pilot_override_does_not_commit() {
        let mut input = ok();
        input.pilot_override = true;
        let (command, _) = decide_input(&input);
        assert_ne!(command, Command::Commit);
    }

    #[test]
    fn inhibit_does_not_keep_commit() {
        let mut input = ok();
        input.grant_present = false;
        let (command, log) = decide_input(&input);
        assert_eq!(log.verdict, Verdict::Inhibit);
        assert_ne!(command, Command::Commit);
    }

    #[test]
    fn abort_does_not_commit() {
        let mut input = ok();
        input.abort_requested = true;
        let (command, log) = decide_input(&input);
        assert_eq!(command, Command::Abort);
        assert_ne!(command, Command::Commit);
        assert_eq!(log.reason, Reason::Abort);
    }
}
