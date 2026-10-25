//! Spec change gate. A spec hunk without a notes hunk fails.

const SPEC: &str = "spec/monitor.lola";
const NOTES: &str = "docs/spec-notes.md";
const GOLDEN: &str = "fixtures/golden";

/// A spec change requires the notes file and the golden fixtures in the same change.
pub fn gate(changed: &[&str]) -> Result<(), &'static str> {
    let spec_changed = changed.iter().any(|path| path.ends_with(SPEC));
    if !spec_changed {
        return Ok(());
    }
    let notes_changed = changed.iter().any(|path| path.ends_with(NOTES));
    if !notes_changed {
        return Err("spec change without notes");
    }
    let golden_changed = changed.iter().any(|path| path.contains(GOLDEN));
    if !golden_changed {
        return Err("spec change without golden fixtures");
    }
    Ok(())
}

/// A hazard-row edit requires the notes file in the same change.
pub fn hazard_gate(changed: &[&str]) -> Result<(), &'static str> {
    let hazard_changed = changed
        .iter()
        .any(|path| path.ends_with("docs/safety-method.md") || path.ends_with("docs/hazards.md"));
    if !hazard_changed {
        return Ok(());
    }
    let notes_changed = changed
        .iter()
        .any(|path| path.ends_with("docs/spec-notes.md"));
    if !notes_changed {
        return Err("hazard change without notes");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{gate, hazard_gate};

    #[test]
    fn a_hazard_hunk_without_a_notes_hunk_fails() {
        let changed = ["docs/safety-method.md"];
        assert_eq!(hazard_gate(&changed), Err("hazard change without notes"));
        let with_notes = ["docs/safety-method.md", "docs/spec-notes.md"];
        assert_eq!(hazard_gate(&with_notes), Ok(()));
        let hazards = ["docs/hazards.md"];
        assert_eq!(hazard_gate(&hazards), Err("hazard change without notes"));
        let method = include_str!("../../../docs/safety-method.md");
        assert!(method.contains("spec-notes.md"));
    }

    #[test]
    fn a_spec_hunk_without_a_notes_hunk_fails() {
        let changed = ["spec/monitor.lola"];
        assert_eq!(gate(&changed), Err("spec change without notes"));
        let with_notes = ["spec/monitor.lola", "docs/spec-notes.md"];
        assert_eq!(
            gate(&with_notes),
            Err("spec change without golden fixtures")
        );
        let complete = [
            "spec/monitor.lola",
            "docs/spec-notes.md",
            "crates/rta-host/fixtures/golden/pass.log",
        ];
        assert_eq!(gate(&complete), Ok(()));
        let no_spec = ["docs/spec-notes.md"];
        assert_eq!(gate(&no_spec), Ok(()));
    }
}
