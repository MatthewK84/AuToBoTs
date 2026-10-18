//! Document and source-text checks for `rta-host/src/monitor.rs`.

#[test]
fn main_does_not_open_a_socket_itself() {
    assert!(!include_str!("../../../rta-host/src/main.rs").contains("UdpSocket"));
}
