//! Document and source-text checks for `rta-host/src/runtime.rs`.

#[test]
fn no_host_path_bypasses_the_configured_polygon() {
    for (name, source) in [
        ("main.rs", include_str!("../../../rta-host/src/main.rs")),
        (
            "runtime.rs",
            include_str!("../../../rta-host/src/runtime.rs"),
        ),
        ("sitl.rs", include_str!("../../../rta-host/src/sitl.rs")),
        (
            "bin/rta-sitl.rs",
            include_str!("../../../rta-host/src/bin/rta-sitl.rs"),
        ),
    ] {
        let body = source.split("#[cfg(test)]").next().unwrap_or_default();
        for bypass in [
            ["polygon: &", "[]"].concat(),
            ["polygon.", "is_empty()"].concat(),
        ] {
            assert!(!body.contains(&bypass), "{name}: {bypass}");
        }
    }
}

#[test]
fn the_runtime_note_names_the_threads_the_order_and_what_is_not_shown() {
    let note = include_str!("../../../../docs/runtime.md");
    for item in [
        "Tick",
        "Receive",
        "Watchdog",
        "stop setpoints",
        "flush the log",
        "no_fc",
        "no_request",
        "latched",
        "The deadline is preemptive.",
        "Recovery is resent until the flight controller reports it.",
        "never double-send",
        "the startup gate in `docs/startup.md`",
        "`not_ready_at_arm`",
        "udpin:host:port",
        "No PX4 SITL run of this loop is recorded.",
        "A steady-state tick allocates nothing on the tick thread.",
        "crates/rta-host/tests/tick_alloc.rs",
        "byte-identical",
    ] {
        assert!(note.contains(item), "{item}");
    }
    let deps = include_str!("../../../../docs/dependencies.md");
    let manifest = include_str!("../../../rta-host/Cargo.toml");
    assert!(deps.contains("| ctrlc | 3.5.2 |"));
    assert!(manifest.contains("ctrlc = \"=3.5.2\""));
}
