//! Frozen shall statements. A row with no method fails the check.

#[cfg(test)]
mod tests {
    #[test]
    fn every_hazard_has_a_shall_and_every_shall_has_a_method() {
        let doc = include_str!("../../../docs/requirements.md");
        let hazards = [
            "H1", "H2", "H3", "H4", "H5", "H6", "H7", "H8", "H9", "H10", "H11", "H12", "H13",
            "H14", "H15",
        ];
        for hazard in hazards {
            assert!(doc.contains(&format!("| {hazard} |")), "{hazard}");
        }
        let mut rows = 0;
        for line in doc.lines() {
            if !line.starts_with("| R-") {
                continue;
            }
            rows += 1;
            let cells: Vec<&str> = line
                .split('|')
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .collect();
            assert!(cells.len() >= 4, "{line}");
            let method = cells[3];
            assert!(
                method == "test" || method == "analysis" || method == "review",
                "{line}"
            );
        }
        assert!(rows >= 15);
        assert!(doc.contains("docs/spec-notes.md"));
        assert!(!doc.contains("| 500 |"));
        assert!(!doc.contains("| 0.40 |"));
    }
}
