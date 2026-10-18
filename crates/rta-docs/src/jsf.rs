//! JSF Air Vehicle baseline. Mapped rules, not a C++ port.

#[cfg(test)]
mod tests {
    #[test]
    fn the_baseline_names_the_document_and_the_mapped_shall_rules() {
        let baseline = include_str!("../../../docs/jsf-av.md");
        let review = include_str!("../../../docs/review.md");
        let sdp = include_str!("../../../docs/do178/sdp.md");
        assert!(baseline.contains("2RDU00001 Rev C"));
        assert!(baseline.contains("December 2005"));
        assert!(baseline.contains("Shall rules are mandatory"));
        for rule in [
            "AV Rule 1",
            "AV Rule 3",
            "AV Rule 119",
            "AV Rule 206",
            "AV Rule 208",
        ] {
            assert!(baseline.contains(rule), "{rule}");
        }
        assert!(baseline.contains("not a certification claim"));
        assert!(review.contains("jsf-av.md"));
        assert!(sdp.contains("jsf-av.md"));
    }
}
