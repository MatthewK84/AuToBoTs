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

#[cfg(test)]
mod tests {
    use super::gate;

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
