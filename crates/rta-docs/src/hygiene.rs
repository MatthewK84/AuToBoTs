//! Repository hygiene: license, contribution norms, changelog, ignore rules, README state.

#[cfg(test)]
mod tests {
    const LICENSE: &str = include_str!("../../../LICENSE");
    const CONTRIBUTING: &str = include_str!("../../../CONTRIBUTING.md");
    const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");
    const README: &str = include_str!("../../../README.md");
    const IGNORE: &str = include_str!("../../../.gitignore");
    const CI: &str = include_str!("../../../.github/workflows/ci.yml");

    #[test]
    fn the_license_is_the_apache_text_every_crate_declares() {
        for item in [
            "Apache License",
            "Version 2.0, January 2004",
            "http://www.apache.org/licenses/",
            "TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION",
            "END OF TERMS AND CONDITIONS",
        ] {
            assert!(LICENSE.contains(item), "{item}");
        }
        for manifest in [
            include_str!("../../rta-spec/Cargo.toml"),
            include_str!("../../rta-switch/Cargo.toml"),
            include_str!("../../rta-host/Cargo.toml"),
            include_str!("../../rta-filter/Cargo.toml"),
            include_str!("../../rta-tracker/Cargo.toml"),
            include_str!("../../rta-replay/Cargo.toml"),
            include_str!("../Cargo.toml"),
            include_str!("../../../fuzz/Cargo.toml"),
        ] {
            assert!(manifest.contains("license = \"Apache-2.0\""), "{manifest}");
        }
        assert!(README.contains("Apache License, Version 2.0. See `LICENSE`."));
    }

    #[test]
    fn contributing_lists_the_checks_ci_runs() {
        for command in [
            "cargo fmt --check",
            "cargo clippy --locked --workspace --all-targets -- -D warnings",
            "cargo test --locked --workspace",
            "cargo run --locked -q -p rta-host --bin rta-replay -- crates/rta-host/fixtures/golden spec/monitor.lola",
            "bash scripts/coverage.sh",
            "bash scripts/sitl-trips.sh",
            "bash scripts/sitl.sh",
            "cargo build --locked --release --workspace",
        ] {
            assert!(CONTRIBUTING.contains(command), "{command}");
            assert!(CI.contains(command), "CI does not run {command}");
        }
        for norm in [
            "Never amend, rebase, or force-push a commit that has been pushed.",
            "`Issue: #N`",
            "Tests are deterministic.",
            "crates/rta-docs",
            "docs/do178/sci.md",
            "Owner decisions",
            "section 3.6",
        ] {
            assert!(CONTRIBUTING.contains(norm), "{norm}");
        }
    }

    #[test]
    fn the_changelog_covers_every_merged_change_and_tags_nothing() {
        assert!(CHANGELOG.contains("## [Unreleased]"));
        for number in 195..=206 {
            assert!(CHANGELOG.contains(&format!("(#{number})")), "#{number}");
        }
        // No release is claimed until the owner tags one.
        assert!(!CHANGELOG.contains("## [0."));
        assert!(!CHANGELOG.contains("## [1."));
    }

    #[test]
    fn scratch_is_ignored_and_the_readme_lists_no_closed_gap() {
        for rule in [
            "/target/",
            "*.profraw",
            "/trips/",
            "/sitl.log",
            "/sitl-timing.txt",
        ] {
            assert!(IGNORE.lines().any(|line| line == rule), "{rule}");
        }
        // The committed timing artifact is not caught by the root-only rule.
        assert!(!IGNORE.lines().any(|line| line == "sitl-timing.txt"));
        let missing = README
            .split("## Missing features")
            .nth(1)
            .and_then(|rest| rest.split("\n## ").next())
            .unwrap();
        for closed in 17..=23 {
            assert!(!missing.contains(&format!("issues/{closed})")), "#{closed}");
        }
        for stale in [
            "One recovery-mode command, then silence.",
            "`rta-host/src/tick.rs`",
        ] {
            assert!(!README.contains(stale), "{stale}");
        }
    }
}
