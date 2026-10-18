//! Document and source-text checks for `rta-host/src/gate.rs`.

use rta_host::config::{self, Startup, StartupMode};

#[test]
fn the_startup_note_matches_the_gate_and_the_defaults() {
    let note = include_str!("../../../../docs/startup.md");
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
    let example = config::parse_str(include_str!("../../../../config/example.toml")).unwrap();
    assert_eq!(example.startup, defaults);
    let requirements = include_str!("../../../../docs/requirements.md");
    assert!(requirements.contains("| R-S1 | startup |"));
    assert!(requirements.contains("| R-S2 | startup |"));
    let icd = include_str!("../../../../docs/icd-fc.md");
    assert!(icd.contains("`MAV_MODE_FLAG_SAFETY_ARMED` (128)"));
}
