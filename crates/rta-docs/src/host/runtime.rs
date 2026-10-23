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

/// Every runtime test runs under the host pause guard, the guard keeps its limits, and the
/// stress note, the script and the PX4 harness say how host timing is told from a failure.
#[test]
fn every_runtime_test_is_pause_guarded_and_the_stress_note_classifies_the_failures() {
    let runtime = include_str!("../../../rta-host/src/runtime.rs");
    let tests = runtime.split("\nmod tests {").nth(1).unwrap();
    let count = tests.matches("    #[test]\n").count();
    assert!(count >= 25, "{count}");
    assert_eq!(
        tests.matches("        host_pause::guard(|| {\n").count(),
        count
    );
    let guard = include_str!("../../../rta-host/src/host_pause.rs");
    for item in [
        "pub const SAMPLE: Duration = Duration::from_millis(5);",
        "pub const PAUSE: Duration = Duration::from_secs(1);",
        "pub const ATTEMPTS: u32 = 3;",
        "panic::resume_unwind(failure);",
        "inconclusive, not a pass",
        "RTA_HOST_PAUSE_LOG",
    ] {
        assert!(guard.contains(item), "{item}");
    }
    let lib = include_str!("../../../rta-host/src/lib.rs");
    assert!(lib.contains("#[cfg(test)]\nmod host_pause;"));
    let note = include_str!("../../../../docs/stress.md");
    for item in [
        "Product bug",
        "Test assumption too tight for a loaded host",
        "Whole-host pause",
        "virtual machine fork",
        "The 5 ms production tick deadline is not a test knob",
        "`fix_age_ms=320`",
        "No product bug was found",
        "`scripts/stress.sh`",
    ] {
        assert!(note.contains(item), "{item}");
    }
    let script = include_str!("../../../../scripts/stress.sh");
    for item in [
        "RUNS:-60",
        "HOGS:-16",
        "RTA_HOST_PAUSE_LOG",
        "trap stop_hogs EXIT",
    ] {
        assert!(script.contains(item), "{item}");
    }
    let px4 = include_str!("../../../../scripts/sitl_px4.py");
    for item in [
        "host deadline miss",
        "PX4 simulator starved",
        "attempt1.json",
        "\"No valid data from\", \"Failsafe activated\"",
    ] {
        assert!(px4.contains(item), "{item}");
    }
    let sitl = include_str!("../../../../docs/sitl.md");
    assert!(sitl.contains("### Host deadline misses"));
    assert!(sitl.contains("The deadline is not raised for SITL."));
    let contributing = include_str!("../../../../CONTRIBUTING.md");
    assert!(contributing.contains("A failure with no pause is never retried"));
    let config = include_str!("../../../../config/example.toml");
    assert!(config.contains("deadline_ms = 5"));
    assert!(px4.contains("deadline_ms = 5"));
}
