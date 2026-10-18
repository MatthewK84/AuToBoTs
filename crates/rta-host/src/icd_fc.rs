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
    ("LOCAL_POSITION_NED", 32),
    ("STATUSTEXT", 253),
    ("SYS_STATUS", 1),
    ("COMMAND_ACK", 77),
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
        for item in [
            "`MAV_CMD_NAV_RETURN_TO_LAUNCH` (20)",
            "`MAV_CMD_NAV_LAND` (21)",
            "`IN_PROGRESS` (5)",
            "Never confirms a mode; only `HEARTBEAT` does.",
        ] {
            assert!(icd.contains(item), "{item}");
        }
    }

    #[test]
    fn the_icd_names_the_setpoint_encoding_and_the_heard_target() {
        let icd = include_str!("../../../docs/icd-fc.md");
        let mask = crate::command_out::POSITION_AND_YAW.bits();
        assert!(icd.contains(&format!("`0x{mask:04X}`")), "{mask:#06X}");
        for item in [
            "yaw in radians",
            "last heard flight-controller `HEARTBEAT`",
            "monotonic run clock",
            "wrapping at 2^32",
            "`bad_setpoint`",
            "system and component only",
        ] {
            assert!(icd.contains(item), "{item}");
        }
    }

    #[test]
    fn the_icd_mode_table_matches_the_px4_encoding() {
        use crate::command_out::{custom_mode, px4_mode};
        use rta_spec::RecoveryMode;
        let icd = include_str!("../../../docs/icd-fc.md");
        for (mode, name) in [
            (RecoveryMode::Rtl, "Rtl"),
            (RecoveryMode::Loiter, "Loiter"),
            (RecoveryMode::Land, "Land"),
        ] {
            let px4 = px4_mode(mode);
            let row = format!(
                "| `{name}` | AUTO ({}) | `PX4_CUSTOM_SUB_MODE_AUTO_{}` ({}) | `{:#010x}` |",
                px4.main,
                name.to_uppercase(),
                px4.sub,
                custom_mode(mode)
            );
            assert!(icd.contains(&row), "{row}");
        }
        assert!(icd.contains("px4_custom_mode.h"));
        assert!(icd.contains("(custom_mode >> 16) & 0xFF"));
        assert!(icd.contains("(custom_mode >> 24) & 0xFF"));
        for doc in [
            include_str!("../../../docs/mavlink.md"),
            include_str!("../../../docs/runbook.md"),
        ] {
            assert!(!doc.contains("6 for rtl, 5 for loiter, and 4 for land"));
        }
    }
}
