//! Document and source-text checks for `rta-host/src/shadow.rs`.

#[test]
fn neither_network_constructs_a_command() {
    let source = include_str!("../../../rta-host/src/shadow.rs");
    let command = ["Switch", "Command"].concat();
    let decide = ["fn ", "decide"].concat();
    let outbound = ["Command", "::"].concat();
    assert!(!source.contains(&command));
    assert!(!source.contains(&decide));
    assert!(!source.contains(&outbound));
}
