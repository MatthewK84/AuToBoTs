//! Document and source-text checks for `rta-host/src/operator.rs`.

#[test]
fn the_timing_note_names_the_withdraw_window_as_an_assumption() {
    let timing = include_str!("../../../../docs/timing.md");
    assert!(timing.contains("100 ms"));
    assert!(timing.contains("assumption"));
}
