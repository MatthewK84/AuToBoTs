//! Shared inputs. A wrong-but-fresh estimate is named, not closed.

#[cfg(test)]
mod tests {
    #[test]
    fn each_shared_input_lists_its_consumers_and_diversity_is_not_claimed() {
        let note = include_str!("../../../docs/common-mode.md");
        for input in ["Position estimate", "Fix age", "Track confidence", "Grant"] {
            assert!(note.contains(input), "{input}");
        }
        assert!(note.contains("Fence check, barrier, shadow net"));
        assert!(note.contains("not an independent measurement"));
        assert!(note.contains("does not have a second GNSS"));
        assert!(!note.contains("diverse position estimate"));
        assert!(!note.contains("independent position sensor"));
    }
}
