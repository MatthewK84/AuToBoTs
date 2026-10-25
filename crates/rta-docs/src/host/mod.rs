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
