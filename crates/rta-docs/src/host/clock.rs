//! Document and source-text checks for `rta-host/src/clock.rs`.

#[test]
fn the_time_note_names_one_monotonic_clock() {
    let doc = include_str!("../../../../docs/time.md");
    assert!(doc.contains("same"));
    assert!(doc.contains("monotonic"));
}
