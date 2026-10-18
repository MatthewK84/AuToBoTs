//! Coverage gate. An uncovered Revert arm fails the job.

#[cfg(test)]
mod tests {
    #[test]
    fn the_psac_names_the_target_and_an_uncovered_revert_fails() {
        let psac = include_str!("../../../docs/do178/psac.md");
        let note = include_str!("../../../docs/coverage.md");
        let script = include_str!("../../../scripts/coverage.sh");
        let ci = include_str!("../../../.github/workflows/ci.yml");
        assert!(psac.contains("100 percent of `decide`"));
        assert!(note.contains("not a proof"));
        assert!(script.contains("Verdict::Revert"));
        assert!(script.contains("exit 1"));
        assert!(ci.contains("switch-coverage"));
    }
}
