//! Unqualified-tool criteria. A note, not a claim.

/// Tools that produce verification evidence and have no qualification kit.
pub const UNQUALIFIED: &[&str] = &["rtlola-interpreter", "Kani", "untraced"];

/// A cited result with no independent check. A non-empty result fails CI.
pub fn cited_without_check() -> Vec<&'static str> {
    let note = include_str!("../../../docs/do178/tool-qualification.md");
    let switch = include_str!("../../rta-switch/src/lib.rs");
    let verify = include_str!("../../rta-switch/src/verify.rs");
    let mut missing = Vec::new();
    if !note.contains("Golden replay fixtures") {
        missing.push("rtlola-interpreter");
    }
    if !note.contains("Switch table tests") || !switch.contains("fn verdict_and_fail_closed_table")
    {
        missing.push("Kani");
    }
    if !note.contains("an_untraced_hazard_fails_ci")
        || !verify.contains("fn decide_variants_are_exclusive")
    {
        missing.push("untraced");
    }
    missing
}

#[cfg(test)]
mod tests {
    use super::{cited_without_check, UNQUALIFIED};

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
        let missing = cited_without_check();
        assert!(missing.is_empty(), "{missing:?}");
        let packet = include_str!("../../../docs/do178/ao-packet.md");
        assert!(packet.contains("tool-qualification.md"));
        assert!(packet.contains("M17.2"));
    }
}
