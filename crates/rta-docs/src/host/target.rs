//! Document and source-text checks for `rta-host/src/bin/rta-target.rs` and the timing
//! fixtures: simulated or stand-in timing can never be read as a measurement on the board.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const SOURCE: &str = include_str!("../../../rta-host/src/bin/rta-target.rs");
const NOTE: &str = include_str!("../../../../docs/target-timing.md");
const SCRIPT: &str = include_str!("../../../../scripts/target-sim.sh");

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../rta-host/fixtures")
}

/// Every timing artifact: a fixture file with a `measured_on=` or a `board=` field.
fn timing_files() -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let mut dirs = vec![fixtures()];
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().is_some_and(|ext| ext == "txt") {
                let text = fs::read_to_string(&path).unwrap_or_default();
                if text.contains("measured_on=") || text.contains("board=") {
                    out.push((path, text));
                }
            }
        }
    }
    out
}

/// The key=value fields of the first line that starts with `evidence=`.
fn fields(text: &str) -> BTreeMap<&str, &str> {
    let line = text
        .lines()
        .find(|line| line.starts_with("evidence="))
        .unwrap_or_default();
    line.split(' ')
        .filter_map(|pair| pair.split_once('='))
        .collect()
}

#[test]
fn the_target_binary_names_no_board_it_did_not_run_on() {
    assert!(!SOURCE.contains("named_board"));
    assert!(!SOURCE.contains("Orin Nano 8GB"));
    for label in ["simulated", "host-standin", "real-target", "measured_on="] {
        assert!(SOURCE.contains(label), "{label}");
    }
    // The refusals that keep an emulated or scaled run from claiming the board.
    assert!(SOURCE.contains("real-target needs an unscaled aarch64 run on the board"));
    assert!(SOURCE.contains("an emulated or scaled run is simulated evidence"));
}

#[test]
fn every_timing_fixture_carries_a_truthful_evidence_label() {
    let files = timing_files();
    assert!(files.len() >= 4, "{files:?}");
    for (path, text) in &files {
        let name = path.display();
        assert!(!text.contains("named_board="), "{name}");
        assert!(
            !text.contains("\nboard=") && !text.starts_with("board="),
            "{name}"
        );
        let f = fields(text);
        let evidence = f.get("evidence").copied().unwrap_or("missing");
        let measured_on = f.get("measured_on").copied().unwrap_or("missing");
        assert!(
            ["simulated", "host-standin", "real-target"].contains(&evidence),
            "{name}: evidence={evidence}"
        );
        assert_ne!(measured_on, "missing", "{name}");
        let emulated = measured_on.starts_with("qemu") || measured_on.contains("sim");
        let scaled = f.get("slowdown").is_some_and(|s| *s != "1");
        if emulated || scaled {
            assert_eq!(evidence, "simulated", "{name}");
        }
        if evidence == "simulated" {
            assert!(text.starts_with("# SIMULATED"), "{name}");
        }
        // No board run exists yet. Adding one changes this line in the same reviewed change
        // as the artifact and docs/target.md (docs/target-timing.md).
        assert_ne!(
            evidence, "real-target",
            "{name}: no board run has been recorded"
        );
    }
}

#[test]
fn the_note_states_simulated_and_matches_the_fixtures() {
    for item in [
        "**Everything measured here is SIMULATED. None of it is a measurement on a Jetson Orin Nano 8GB.**",
        "do not close the Orin target measurement",
        "slowdown = host_mhz / target_mhz * ipc_penalty",
        "PREEMPT_RT",
        "isolcpus",
        "chrt -f 80",
        "mlockall",
        "nvpmodel",
        "jetson_clocks",
        "REAL_TARGET=1",
        "scripts/target-sim.sh",
    ] {
        assert!(NOTE.contains(item), "{item}");
    }
    for run in ["qemu-aarch64", "host-sim"] {
        let text = fs::read_to_string(fixtures().join(format!("target-sim/{run}.txt"))).unwrap();
        let f = fields(&text);
        assert_eq!(f.get("evidence"), Some(&"simulated"), "{run}");
        assert_eq!(f.get("measured_on"), Some(&run), "{run}");
        let samples: u64 = f["samples"].parse().unwrap();
        assert!(samples >= 10_000, "{run}: {samples}");
        let row = format!(
            "| {run} | {} | {} | {} | {} | {} | {} of {samples} | {} |",
            f["slowdown"],
            f["model_p50_us"],
            f["model_p99_us"],
            f["model_p999_us"],
            f["model_worst_us"],
            f["over_deadline"],
            f["wake_late_worst_us"],
        );
        assert!(NOTE.contains(&row), "the note does not hold {row}");
    }
    let others = [
        include_str!("../../../../docs/target.md"),
        include_str!("../../../../docs/deadline.md"),
        include_str!("../../../../docs/production-hooks.md"),
    ];
    for note in others {
        assert!(note.contains("docs/target-timing.md"));
        assert!(note.contains("not") && note.contains("imulated"));
    }
    assert!(include_str!("../../../../docs/production-hooks.md")
        .contains("Simulated timing does not close this hook."));
}

#[test]
fn the_script_labels_each_run_and_gates_the_board_run() {
    assert!(SCRIPT.contains("--measured-on qemu-aarch64 --evidence simulated"));
    assert!(SCRIPT.contains("--measured-on host-sim --evidence simulated"));
    assert!(SCRIPT.contains("REAL_TARGET:-}\" != 1 || \"$(uname -m)\" != aarch64"));
    assert!(SCRIPT.contains("qemu-aarch64 -L /usr/aarch64-linux-gnu"));
}
