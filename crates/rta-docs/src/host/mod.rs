//! Checks that name a flight module of `rta-host`. One file per module.

mod clock;
mod eval_worker;
mod fail_closed;
mod gate;
mod mode;
mod monitor;
mod operator;
mod quarantine;
mod recovery;
mod replay;
mod runtime;
mod shadow;
mod sitl;
mod spec;
mod startup;
mod target;
mod tracker;
mod watchdog;
mod weights;

use std::fs;
use std::path::{Path, PathBuf};

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// The split holds: `rta-host` reads only input data at compile time (the spec, a config,
/// a fixture), never a note, a script, a workflow, a manifest, or its own source.
#[test]
fn rta_host_reads_no_document_or_source_text() {
    let host = Path::new(env!("CARGO_MANIFEST_DIR")).join("../rta-host");
    let mut files = Vec::new();
    rust_files(&host.join("src"), &mut files);
    rust_files(&host.join("tests"), &mut files);
    assert!(files.len() > 40, "{}", files.len());
    let allowed = [
        "spec/monitor.lola",
        "config/example.toml",
        "config/sitl.toml",
        "fixtures/",
    ];
    let mut found = 0;
    for file in files {
        let text = fs::read_to_string(&file).unwrap();
        for piece in text.split("include_str!(\"").skip(1) {
            let target = piece.split('"').next().unwrap();
            found += 1;
            assert!(
                allowed.iter().any(|ok| target.contains(ok)),
                "{}: include_str!({target})",
                file.display()
            );
        }
    }
    assert!(found > 10, "{found}");
}

#[test]
fn the_note_names_the_split_the_properties_and_the_fuzz_targets() {
    let note = include_str!("../../../../docs/doc-checks.md");
    let ci = include_str!("../../../../.github/workflows/ci.yml");
    for item in [
        "No binary depends on `rta-docs`",
        "`cargo test --locked --workspace`",
        "crates/rta-host/tests/properties.rs",
        "fixed seed",
        "fuzz-smoke",
    ] {
        assert!(note.contains(item), "{item}");
    }
    assert!(ci.contains("fuzz-smoke:"));
    let fuzz = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz");
    for target in ["read_apply", "mavlink_frames", "config_parse"] {
        assert!(note.contains(&format!("`{target}`")), "{target}");
        assert!(ci.contains(target), "{target}");
        assert!(
            fuzz.join(format!("fuzz_targets/{target}.rs")).is_file(),
            "{target}"
        );
        assert!(
            fs::read_dir(fuzz.join(format!("corpus/{target}")))
                .unwrap()
                .count()
                > 0,
            "{target} has no seed"
        );
    }
    // No binary links the check crate.
    for manifest in [
        include_str!("../../../rta-host/Cargo.toml"),
        include_str!("../../../rta-switch/Cargo.toml"),
        include_str!("../../../rta-replay/Cargo.toml"),
    ] {
        assert!(!manifest.contains("rta-docs"));
    }
}

#[test]
fn the_release_checklist_maps_to_files() {
    let doc = include_str!("../../../../docs/release.md");
    for item in [
        "compile",
        "coverage",
        "docs/do178/sci.md",
        "docs/target.md",
        "docs/do178/ao-packet.md",
        "docs/spec-notes.md",
    ] {
        assert!(doc.contains(item), "{item}");
    }
    assert!(doc.contains("dirty"));
    assert!(doc.contains("No in-flight learning bundle"));
}

#[test]
fn the_notice_lists_each_direct_dependency_and_deny_matches() {
    let notice = include_str!("../../../../docs/third-party.md");
    let deny = include_str!("../../../../deny.toml");
    for crate_name in [
        "rtlola-interpreter",
        "rtlola-frontend",
        "mavlink",
        "serde",
        "toml",
        "ordered-float",
        "ctrlc",
        "sha2",
    ] {
        assert!(notice.contains(crate_name), "{crate_name}");
    }
    assert!(notice.contains("does not link libgstreamer"));
    assert!(deny.contains("Apache-2.0"));
    assert!(deny.contains("priority-queue"));
    assert!(deny.contains("serialport"));
    assert!(notice.contains("A new copyleft dependency fails CI"));
}

#[test]
fn every_common_mode_item_appears_and_no_row_is_closed() {
    let residual = include_str!("../../../../docs/residual-risk.md");
    let common = include_str!("../../../../docs/common-mode.md");
    for item in [
        "Wrong-but-fresh",
        "Confident wrong track",
        "Grant issued on bad information",
        "Model error in the barrier",
    ] {
        assert!(residual.contains(item), "{item}");
    }
    assert!(common.contains("wrong-but-fresh"));
    assert!(common.contains("confident wrong track"));
    assert!(common.contains("grant issued on bad information"));
    assert!(!residual.to_lowercase().contains("risk is closed"));
    assert!(residual.contains("does not accept"));
}

/// A requirements change requires a test hunk.
pub fn requirements_gate(changed: &[&str]) -> Result<(), &'static str> {
    let req = changed.iter().any(|p| p.ends_with("docs/requirements.md"));
    if !req {
        return Ok(());
    }
    let test = changed
        .iter()
        .any(|p| p.contains("tests/") || p.ends_with("_test.rs") || p.contains("/tests.rs"));
    if !test {
        return Err("requirements change without a test hunk");
    }
    Ok(())
}

#[test]
fn a_requirements_only_diff_fails() {
    let doc = include_str!("../../../../docs/requirements.md");
    assert!(doc.contains("old shall"));
    assert!(doc.contains("new shall"));
    assert!(doc.contains("tests that move"));
    assert_eq!(
        requirements_gate(&["docs/requirements.md"]),
        Err("requirements change without a test hunk")
    );
    assert_eq!(
        requirements_gate(&[
            "docs/requirements.md",
            "crates/rta-host/tests/properties.rs"
        ]),
        Ok(())
    );
}

#[test]
fn each_derived_shall_names_a_type_and_a_test_id() {
    let doc = include_str!("../../../../docs/requirements.md");
    let derived = doc.split("## Derived").nth(1).expect("derived section");
    let mut rows = 0;
    for line in derived.lines() {
        if !line.starts_with("| D-") {
            continue;
        }
        rows += 1;
        let cells: Vec<&str> = line
            .split('|')
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .collect();
        assert!(cells.len() >= 4, "{line}");
        assert!(!cells[2].is_empty(), "enforcing type missing: {line}");
        assert!(!cells[3].is_empty(), "test id missing: {line}");
    }
    assert_eq!(rows, 3);
    assert!(derived.contains("SwitchCommand"));
    assert!(derived.contains("decide"));
    assert!(derived.contains("decide_input"));
}

#[test]
fn port_shalls_are_in_the_requirement_file_and_the_switch_does_not_import_the_tracker() {
    let doc = include_str!("../../../../docs/requirements.md");
    let port = doc.split("## Port").nth(1).expect("port section");
    for id in ["P-1", "P-2", "P-3"] {
        assert!(port.contains(id), "{id}");
    }
    assert!(port.contains("0 to 1"));
    assert!(port.contains("non-negative"));
    assert!(port.contains("monotonic"));
    let switch = include_str!("../../../rta-switch/Cargo.toml");
    assert!(!switch.contains("rta-tracker"));
    assert!(!switch.contains("gstreamer"));
}
