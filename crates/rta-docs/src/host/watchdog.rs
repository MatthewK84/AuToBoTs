//! Document and source-text checks for `rta-host/src/watchdog.rs`.

#[test]
fn watchdog_does_not_name_the_interpreter() {
    let source = include_str!("../../../rta-host/src/watchdog.rs");
    let interpreter = ["rt", "lola"].concat();
    let monitor = ["Mon", "itor"].concat();
    assert!(!source.contains(&interpreter));
    assert!(!source.contains(&monitor));
}

#[test]
fn the_watchdog_names_no_interpreter_filter_or_tracker_and_the_note_names_its_inputs() {
    let source = include_str!("../../../rta-host/src/watchdog.rs");
    let interpreter = ["rt", "lola"].concat();
    let filter = ["rta_", "filter"].concat();
    let tracker = ["rta_", "tracker"].concat();
    assert!(!source.contains(&interpreter));
    assert!(!source.contains(&filter));
    assert!(!source.contains(&tracker));
    let note = include_str!("../../../../docs/watchdog.md");
    assert!(note.contains("Socket"));
    assert!(note.contains("Clock"));
}
