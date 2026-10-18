//! Document and source-text checks for `rta-host/src/mode.rs`.

#[test]
fn no_public_clear() {
    let source = include_str!("../../../rta-host/src/mode.rs");
    let name = ["clear", "_recovery"].concat();
    assert!(!source.contains(&name));
}
