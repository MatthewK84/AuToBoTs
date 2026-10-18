//! Document and source-text checks for `rta-host/src/replay.rs`.

#[test]
fn the_log_format_names_version_one_and_replay_exits_nonzero() {
    let format = include_str!("../../../../docs/log-format.md");
    assert!(format.contains("Current version is `1`"));
    let main = include_str!("../../../rta-host/src/bin/rta-replay.rs");
    assert!(main.contains("ExitCode::from(1)"));
}
