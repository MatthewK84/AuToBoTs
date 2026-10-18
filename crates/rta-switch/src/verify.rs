//! Kani harnesses over `decide` and `decide_input`. The host tests cover a missing verdict.
//! Run with `cargo kani -p rta-switch`. Default `cargo test` does not build this.

use super::{decide, decide_input, Command, Request, SwitchCommand};
use rta_spec::{Phase, Reason, RecoveryMode, TickInput, Verdict};

fn any_verdict() -> Verdict {
    match kani::any::<u8>() % 3 {
        0 => Verdict::Pass,
        1 => Verdict::Inhibit,
        _ => Verdict::Revert,
    }
}

fn any_mode() -> RecoveryMode {
    match kani::any::<u8>() % 3 {
        0 => RecoveryMode::Rtl,
        1 => RecoveryMode::Loiter,
        _ => RecoveryMode::Land,
    }
}

fn any_request() -> Option<Request> {
    if !kani::any::<bool>() {
        return None;
    }
    Some(Request {
        north: f64::from(kani::any::<u8>()),
        east: f64::from(kani::any::<u8>()),
        down: f64::from(kani::any::<u8>()),
        yaw: f64::from(kani::any::<u8>()),
        commit: kani::any::<bool>(),
    })
}

#[kani::proof]
// trace: H9
fn decide_variants_are_exclusive() {
    let verdict = any_verdict();
    let request = any_request();
    let reported = any_mode();
    let recovery = any_mode();
    let command = decide(verdict, request, reported, recovery);
    match command {
        SwitchCommand::Mode { recovery: mode } => {
            let _mode = mode;
            assert!(!matches!(command, SwitchCommand::Setpoints { .. }));
        }
        SwitchCommand::Setpoints { commit, .. } => {
            assert!(!matches!(command, SwitchCommand::Mode { .. }));
            assert!(verdict != Verdict::Revert);
            if verdict == Verdict::Inhibit {
                assert!(!commit);
            }
        }
        SwitchCommand::Idle => {
            assert!(!matches!(command, SwitchCommand::Setpoints { .. }));
        }
    }
}

#[kani::proof]
fn revert_never_yields_setpoints() {
    let request = any_request();
    let command = decide(Verdict::Revert, request, any_mode(), any_mode());
    assert!(!matches!(command, SwitchCommand::Setpoints { .. }));
}

#[kani::proof]
fn inhibit_never_yields_commit() {
    let request = any_request();
    let command = decide(Verdict::Inhibit, request, any_mode(), any_mode());
    if let SwitchCommand::Setpoints { commit, .. } = command {
        assert!(!commit);
    }
}

fn any_phase() -> Phase {
    match kani::any::<u8>() % 4 {
        0 => Phase::Search,
        1 => Phase::Track,
        2 => Phase::Commit,
        _ => Phase::Abort,
    }
}

/// Every flag and every `f64`, including NaN, infinities, and negatives. Built
/// as a struct literal, so it is not filtered by `TickInput::try_new`.
fn any_input() -> TickInput {
    TickInput {
        phase: any_phase(),
        grant_present: kani::any(),
        coast_ok: kani::any(),
        pilot_override: kani::any(),
        abort_requested: kani::any(),
        estimator_ok: kani::any(),
        endurance_s: kani::any::<f64>(),
        time_to_rally_s: kani::any::<f64>(),
    }
}

#[kani::proof]
fn decide_input_commit_needs_every_gate() {
    let input = any_input();
    let (command, log) = decide_input(&input);
    if command == Command::Commit {
        assert!(input.grant_present);
        assert!(input.phase == Phase::Commit);
        assert!(!input.pilot_override);
        assert!(!input.abort_requested);
        assert!(input.estimator_ok);
        assert!(input.coast_ok);
        assert!(input.endurance_s.is_finite());
        assert!(input.time_to_rally_s.is_finite());
        assert!(input.time_to_rally_s >= 0.0);
        assert!(input.endurance_s > input.time_to_rally_s);
        assert!(log.verdict == Verdict::Pass);
    }
}

#[kani::proof]
fn decide_input_non_finite_energy_reverts() {
    let input = any_input();
    kani::assume(!input.endurance_s.is_finite() || !input.time_to_rally_s.is_finite());
    let (command, log) = decide_input(&input);
    assert!(command != Command::Commit);
    assert!(log.verdict != Verdict::Pass);
    if !input.pilot_override && !input.abort_requested && input.phase != Phase::Abort {
        assert!(command == Command::Recovery);
        assert!(log.verdict == Verdict::Revert);
        assert!(log.reason == Reason::Estimator || log.reason == Reason::Energy);
    }
}

#[kani::proof]
fn decide_input_verdict_matches_command() {
    let input = any_input();
    let (command, log) = decide_input(&input);
    match log.verdict {
        Verdict::Revert => {
            assert!(command == Command::Abort || command == Command::Recovery);
            assert!(log.grant_hash.is_none());
        }
        Verdict::Inhibit => {
            assert!(command == Command::Navigation);
            assert!(log.grant_hash.is_none());
        }
        Verdict::Pass => {
            assert!(command == Command::Commit);
            assert!(log.grant_hash.is_some());
        }
    }
}

#[kani::proof]
fn tick_input_try_new_admits_only_finite_energy() {
    let endurance = kani::any::<f64>();
    let rally = kani::any::<f64>();
    let built = TickInput::try_new(
        any_phase(),
        kani::any(),
        kani::any(),
        kani::any(),
        kani::any(),
        kani::any(),
        endurance,
        rally,
    );
    if let Ok(input) = built {
        assert!(input.endurance_s.is_finite() && input.endurance_s >= 0.0);
        assert!(input.time_to_rally_s.is_finite() && input.time_to_rally_s >= 0.0);
    } else {
        assert!(!endurance.is_finite() || !rally.is_finite() || endurance < 0.0 || rally < 0.0);
    }
}
