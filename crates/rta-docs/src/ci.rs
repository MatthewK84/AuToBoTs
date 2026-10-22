//! CI hardening. Actions pinned to commits, read-only tokens, the declared minimum Rust.

#[cfg(test)]
mod tests {
    const CI: &str = include_str!("../../../.github/workflows/ci.yml");
    const KANI: &str = include_str!("../../../.github/workflows/kani.yml");

    #[test]
    fn every_action_is_pinned_to_a_commit_with_its_release_named() {
        let mut uses = 0;
        for (name, workflow) in [("ci.yml", CI), ("kani.yml", KANI)] {
            for line in workflow.lines().filter(|line| line.contains("uses:")) {
                uses += 1;
                let reference = line
                    .split('@')
                    .nth(1)
                    .and_then(|rest| rest.split_whitespace().next())
                    .unwrap_or_default();
                assert!(
                    reference.len() == 40 && reference.chars().all(|c| c.is_ascii_hexdigit()),
                    "{name}: {line}"
                );
                assert!(line.contains(" # "), "{name}: no release comment: {line}");
            }
            let checkouts = workflow.matches("actions/checkout@").count();
            assert_eq!(
                workflow.matches("persist-credentials: false").count(),
                checkouts,
                "{name}"
            );
        }
        assert!(uses >= 15, "{uses}");
    }

    #[test]
    fn tokens_are_read_only_except_closing_an_issue() {
        for workflow in [CI, KANI] {
            assert!(workflow.contains("\npermissions:\n  contents: read\n"));
        }
        assert_eq!(CI.matches("permissions:").count(), 2);
        assert!(
            CI.contains("    permissions:\n      contents: read\n      issues: write\n    steps:")
        );
        assert!(!CI.contains("write-all"));
        assert!(!CI.contains("pull-requests: write"));
        assert!(!CI.contains("contents: write"));
    }

    #[test]
    fn the_jobs_in_the_note_are_the_jobs_in_the_workflow() {
        let note = include_str!("../../../docs/ci.md");
        for job in [
            "compile",
            "release",
            "aarch64",
            "target-sim",
            "px4-sitl",
            "sitl",
            "deny",
            "coverage",
            "fuzz-smoke",
            "evidence",
            "close-named-issue",
        ] {
            assert!(CI.contains(&format!("\n  {job}:\n")), "{job}");
            assert!(note.contains(&format!("| `{job}` |")), "{job}");
        }
        assert!(KANI.contains("\n  prove:\n"));
        assert!(CI.contains("--target aarch64-unknown-linux-gnu"));
        assert!(CI.contains("cargo build --locked --release --workspace"));
        assert!(CI.contains("bash scripts/sitl.sh"));
        assert!(CI.contains("bash scripts/sitl-trips.sh"));
        assert!(CI.contains("bash scripts/target-sim.sh qemu host-sim"));
        assert!(CI.contains("bash scripts/sitl-px4.sh"));
        for item in [
            "There is no separate MSRV job",
            "persist-credentials: false",
            "Whether to keep the job is an owner decision.",
        ] {
            assert!(note.contains(item), "{item}");
        }
    }

    #[test]
    fn the_px4_sitl_job_installs_the_pinned_toolchain_once_and_the_script_names_it() {
        let toolchain = include_str!("../../../rust-toolchain.toml");
        let channel = toolchain
            .lines()
            .find_map(|line| line.strip_prefix("channel = "))
            .unwrap()
            .trim_matches('"');
        let job = CI.split("\n  px4-sitl:\n").nth(1).unwrap();
        let job = job.split("\n\n  ").next().unwrap();
        assert!(job.contains(&format!(
            "rustup toolchain install {channel} --profile minimal --no-self-update"
        )));
        assert!(!job.contains("dtolnay/rust-toolchain"), "{job}");
        let script = include_str!("../../../scripts/sitl-px4.sh");
        assert!(script.contains("RUSTUP_TOOLCHAIN=\"$toolchain\" cargo build"));
        assert!(script.contains("rev-parse --verify -q \"$PX4_TAG^{commit}\""));
    }

    #[test]
    fn every_crate_declares_the_pinned_toolchain_as_its_minimum() {
        let toolchain = include_str!("../../../rust-toolchain.toml");
        let channel = toolchain
            .lines()
            .find_map(|line| line.strip_prefix("channel = "))
            .unwrap()
            .trim_matches('"');
        let line = format!("rust-version = \"{channel}\"");
        for manifest in [
            include_str!("../../rta-spec/Cargo.toml"),
            include_str!("../../rta-switch/Cargo.toml"),
            include_str!("../../rta-host/Cargo.toml"),
            include_str!("../../rta-filter/Cargo.toml"),
            include_str!("../../rta-tracker/Cargo.toml"),
            include_str!("../../rta-replay/Cargo.toml"),
            include_str!("../Cargo.toml"),
        ] {
            assert!(manifest.contains(&line), "{manifest}");
        }
        assert_eq!(
            std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/.."))
                .unwrap()
                .count(),
            7,
            "a new crate needs rust-version"
        );
    }
}
