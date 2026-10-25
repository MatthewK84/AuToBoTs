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
    fn a_dirty_index_is_refused() {
        assert_eq!(tag_allowed(""), Ok(()));
        assert_eq!(tag_allowed(" M docs/release.md"), Err(TagError::Dirty));
    }
}
