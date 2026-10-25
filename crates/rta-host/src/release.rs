//! Ground release checklist. A dirty index is refused.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagError {
    Dirty,
}

/// A tag is refused when the index is dirty.
pub fn tag_allowed(porcelain: &str) -> Result<(), TagError> {
    if porcelain.trim().is_empty() {
        Ok(())
    } else {
        Err(TagError::Dirty)
    }
}

#[cfg(test)]
mod tests {
    use super::{tag_allowed, TagError};

    #[test]
    fn the_checklist_maps_to_files_and_a_dirty_index_is_refused() {
        let doc = include_str!("../../../docs/release.md");
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
        assert_eq!(tag_allowed(""), Ok(()));
        assert_eq!(tag_allowed(" M docs/release.md"), Err(TagError::Dirty));
    }
}
