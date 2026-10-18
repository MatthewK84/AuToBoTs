//! Document and source-text checks for `rta-host/src/startup.rs`.

#[test]
fn the_startup_note_says_a_failed_step_does_not_skip_ahead() {
    let note = include_str!("../../../../docs/startup.md");
    assert!(note.contains("does not skip ahead"));
}
