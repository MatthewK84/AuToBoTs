//! Document and source-text checks for `rta-host/src/spec.rs`.

#[test]
fn every_stream_has_a_note() {
    let spec = include_str!("../../../../spec/monitor.lola");
    let notes = include_str!("../../../../docs/spec-notes.md");
    for line in spec.lines() {
        let line = line.trim();
        if line.starts_with("input ") || line.starts_with("output ") {
            let name = line
                .split_whitespace()
                .nth(1)
                .unwrap()
                .trim_end_matches(':');
            assert!(notes.contains(name), "{name}");
        }
    }
    assert!(notes.contains("## Blind spots"));
    for spot in [
        "Target identity",
        "Collateral",
        "Rules of engagement",
        "Tracker truth",
        "Estimator truth",
    ] {
        assert!(notes.contains(spot), "{spot}");
    }
}

#[test]
fn host_has_no_threshold_literals() {
    let host = [
        include_str!("../../../rta-host/src/main.rs"),
        include_str!("../../../rta-host/src/config.rs"),
        include_str!("../../../rta-host/src/fence.rs"),
        include_str!("../../../rta-host/src/stale.rs"),
        include_str!("../../../rta-host/src/runtime.rs"),
        include_str!("../../../rta-host/src/fail_closed.rs"),
        include_str!("../../../rta-host/src/sitl.rs"),
        include_str!("../../../rta-host/src/bin/rta-sitl.rs"),
    ]
    .concat();
    for literal in ["500.0", "300.0", "1500.0", "0.40", "150.0"] {
        assert!(!host.contains(literal), "{literal}");
    }
    // A digit separator does not hide a copied comparison.
    let plain: String = host.chars().filter(|c| *c != '_').collect();
    for comparison in ["> 500.", "> 300.", "> 1000.", "< 0.4", "< 150."] {
        assert!(!plain.contains(comparison), "{comparison}");
    }
}

#[test]
fn the_notes_say_the_host_reads_the_spec() {
    let notes = include_str!("../../../../docs/spec-notes.md");
    assert!(notes.contains("reads the number from the loaded spec at startup (`spec::limits`)"));
    let runtime = include_str!("../../../../docs/runtime.md");
    assert!(runtime.contains("uses the spec's `heartbeat_stale` threshold"));
    assert!(runtime.contains("No path substitutes an empty polygon."));
    let sitl = include_str!("../../../../docs/sitl.md");
    assert!(sitl.contains("carry the hash of the spec `rta-sitl` loaded, not zero"));
}
