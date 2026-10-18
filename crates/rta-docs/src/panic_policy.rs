//! Panic policy. The library and binaries deny the panicking constructs outside tests, and a
//! release build aborts on panic (`docs/panics.md`). This module holds only the checks.

#[cfg(test)]
mod tests {
    const LINTS: [&str; 7] = [
        "clippy::unwrap_used",
        "clippy::expect_used",
        "clippy::indexing_slicing",
        "clippy::panic,",
        "clippy::unreachable",
        "clippy::todo",
        "clippy::unimplemented",
    ];

    #[test]
    fn the_library_and_every_binary_deny_panics_outside_tests() {
        let sources = [
            ("lib.rs", include_str!("../../rta-host/src/lib.rs")),
            ("main.rs", include_str!("../../rta-host/src/main.rs")),
            (
                "rta-replay.rs",
                include_str!("../../rta-host/src/bin/rta-replay.rs"),
            ),
            (
                "rta-sitl.rs",
                include_str!("../../rta-host/src/bin/rta-sitl.rs"),
            ),
            (
                "rta-target.rs",
                include_str!("../../rta-host/src/bin/rta-target.rs"),
            ),
            (
                "rta-trips.rs",
                include_str!("../../rta-host/src/bin/rta-trips.rs"),
            ),
        ];
        for (name, source) in sources {
            assert!(
                source.contains("#![cfg_attr(\n    not(test),\n    deny("),
                "{name}"
            );
            for lint in LINTS {
                assert!(source.contains(lint), "{name}: {lint}");
            }
        }
        let bins = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../rta-host/src/bin"))
            .unwrap()
            .count();
        assert_eq!(bins, sources.len() - 2, "a new binary needs the deny block");
    }

    #[test]
    fn release_builds_abort_and_the_note_says_what_the_vehicle_does() {
        let manifest = include_str!("../../../Cargo.toml");
        assert!(manifest.contains("[profile.release]\npanic = \"abort\""));
        let note = include_str!("../../../docs/panics.md");
        for item in [
            "`panic = \"abort\"`",
            "offboard-loss failsafe",
            "not values this repository sets",
            "The host does not restart itself.",
            "`not_ready_at_arm`",
            "Lints are not a proof.",
            "Debug and test builds keep unwinding",
        ] {
            assert!(note.contains(item), "{item}");
        }
        let degraded = include_str!("../../../docs/degraded.md");
        assert!(degraded.contains("docs/panics.md"));
    }
}
