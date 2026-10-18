//! Document and source-text checks for `rta-host/src/quarantine.rs`.

/// The H15 latch has no way out: the quarantine names no `clear_fault`.
#[test]
fn the_quarantine_has_no_clear_fault() {
    let source = include_str!("../../../rta-host/src/quarantine.rs");
    let clear = ["fn ", "clear_fault"].concat();
    assert!(!source.contains(&clear));
}
