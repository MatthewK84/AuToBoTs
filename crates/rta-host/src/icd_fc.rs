//! Flight-controller interface. One dialect, one link.

/// Messages the adapter sends. Common dialect ids.
pub const SENT: &[(&str, u32)] = &[
    ("HEARTBEAT", 0),
    ("COMMAND_LONG", 76),
    ("SET_POSITION_TARGET_LOCAL_NED", 84),
    ("STATUSTEXT", 253),
];

/// Messages the adapter reads. Common dialect ids.
pub const READ: &[(&str, u32)] = &[
    ("HEARTBEAT", 0),
    ("GLOBAL_POSITION_INT", 33),
    ("STATUSTEXT", 253),
    ("SYS_STATUS", 1),
];

#[cfg(test)]
mod tests {
    use super::{READ, SENT};

    #[test]
    fn the_icd_names_sent_and_read_and_death_is_the_flight_controller() {
        let icd = include_str!("../../../docs/icd-fc.md");
        let deps = include_str!("../../../docs/dependencies.md");
        for (name, id) in SENT.iter().chain(READ.iter()) {
            assert!(icd.contains(name), "{name}");
            assert!(icd.contains(&id.to_string()), "{id}");
        }
        assert!(icd.contains("common"));
        assert!(deps.contains("common"));
        assert!(icd.contains("flight controller's own failsafe"));
        assert!(icd.contains("fromOffboardLossActParam"));
        assert!(!icd.contains("the companion flies the recovery"));
    }
}
