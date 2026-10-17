//! Unqualified-tool criteria. A note, not a claim.

/// Tools that produce verification evidence and have no qualification kit.
pub const UNQUALIFIED: &[&str] = &["rtlola-interpreter", "Kani", "untraced"];

#[cfg(test)]
mod tests {
    use super::UNQUALIFIED;

    #[test]
    fn one_row_per_tool_and_none_is_called_qualified() {
        let note = include_str!("../../../docs/do178/tool-qualification.md");
        for tool in UNQUALIFIED {
            assert!(note.contains(tool), "{tool}");
        }
        assert!(note.contains("Golden replay fixtures"));
        assert!(note.contains("Switch table tests"));
        assert!(note.contains("an_untraced_hazard_fails_ci"));
        assert!(note.contains("Kani is not a qualified tool"));
        assert!(note.contains("`rtlola-interpreter` is not a qualified tool"));
        assert!(!note.contains("Kani is a qualified tool"));
        assert!(!note.contains("interpreter is a qualified tool"));
    }
}
