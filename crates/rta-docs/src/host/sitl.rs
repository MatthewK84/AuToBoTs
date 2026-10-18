//! Document and source-text checks for `rta-host/src/sitl.rs`.

use rta_host::config::parse_str;
use rta_host::sitl::*;

#[test]
fn script_port_matches_example_config() {
    let example = include_str!("../../../../config/example.toml");
    let sitl = include_str!("../../../../config/sitl.toml");
    let script = include_str!("../../../../scripts/sitl.sh");
    assert!(example.contains(EXAMPLE_ENDPOINT));
    assert!(sitl.contains(EXAMPLE_ENDPOINT));
    assert!(script.contains("14540"));
    assert!(script.contains("127.0.0.1"));
    let loaded = parse_str(example).expect("example");
    assert_eq!(loaded.link.endpoint, EXAMPLE_ENDPOINT);
}

#[test]
fn assurance_gap_covers_m11() {
    let note = include_str!("../../../../docs/assurance-gap.md");
    for id in ["H1", "H2", "H3", "H4", "H5", "H6"] {
        assert!(note.contains(id), "{id}");
    }
    assert!(note.contains("The monitor and the recovery are not pedigreed components."));
    assert!(note.contains(
        "This build bounds flight and commit. It does not assure the tactical decision."
    ));
    assert!(note.contains("Following it is not a compliance claim."));
}

#[test]
fn runbook_has_no_clear_fault() {
    let book = include_str!("../../../../docs/runbook.md");
    let name = ["clear", "_fault"].concat();
    assert!(book.contains("no `clear_fault`"));
    assert!(!book.contains(&format!("pub fn {name}")));
    assert!(book.contains("Inhibit"));
    assert!(book.contains("Revert"));
    assert!(book.contains("rta.log"));
    assert!(book.contains("golden-trace review"));
    assert!(book.contains("restart"));
}

#[test]
fn barrier_evidence_names_the_architecture_and_the_residual() {
    let note = include_str!("../../../../docs/cbf-evidence.md");
    assert!(note.contains("ASTM F3269-21"));
    assert!(note.contains("enclosing architecture"));
    assert!(note.contains("Forward invariance of the backup set under the point-mass model"));
    assert!(note.contains("not a replacement for the recovery"));
    assert!(note.contains("Invariance under the real aircraft is not shown."));
    assert!(note.contains("Invariance under wind is not shown."));
    assert!(note.contains("Invariance under a wrong fence is not shown."));
    assert!(note.contains("Residual model error is explicitly unmeasured."));
    assert!(note.contains("not a hidden margin"));
    assert!(note.contains("not a compliance claim"));
}

#[test]
fn the_sitl_note_and_artifact_say_sitl_timing_is_not_target_timing() {
    let doc = include_str!("../../../../docs/sitl.md");
    assert!(doc.contains(TIMING_NOTE));
    let artifact = include_str!("../../../../crates/rta-host/fixtures/sitl-timing.txt");
    assert!(artifact.contains(TIMING_NOTE));
    assert!(artifact.contains("eval_ms"));
}

#[test]
fn the_deadline_note_says_the_sitl_deadline_stays_five_ms() {
    let note = include_str!("../../../../docs/deadline.md");
    assert!(note.contains("deadline stays 5 ms"));
    assert!(note.contains("not raised"));
}

#[test]
fn the_filter_crate_has_no_link_or_interpreter_dependency() {
    let manifest = include_str!("../../../../crates/rta-filter/Cargo.toml");
    assert!(!manifest.to_ascii_lowercase().contains("mavlink"));
    assert!(!manifest.to_ascii_lowercase().contains("rtlola"));
}

#[test]
fn the_cbf_contract_names_the_margin_and_the_log_field() {
    let note = include_str!("../../../../docs/cbf-contract.md");
    assert!(note.contains("not a hidden margin"));
    assert!(note.contains("unmeasured"));
    assert!(note.contains("filter_intervened"));
}
