//! Document and source-text checks for `rta-host/src/fail_closed.rs`.

#[test]
fn no_clear_fault() {
    let source = include_str!("../../../rta-host/src/fail_closed.rs");
    let name = ["pub fn ", "clear_fault"].concat();
    assert!(!source.contains(&name));
}
