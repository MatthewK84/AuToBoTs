//! Kani harness over `decide` only. The host tests cover a missing verdict.
//! Run with `cargo kani -p rta-switch`. Default `cargo test` does not build this.

use super::{decide, Request, SwitchCommand};
use rta_spec::{RecoveryMode, Verdict};

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
