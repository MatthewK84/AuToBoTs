//! Project safety scale. Not a finding.

#[cfg(test)]
mod tests {
    #[test]
    fn every_hazard_has_both_columns_and_the_control_exists() {
        let method = include_str!("../../../docs/safety-method.md");
        let sources = [
            include_str!("../../rta-host/src/fence.rs"),
            include_str!("../../rta-host/src/mavlink_link.rs"),
            include_str!("../../rta-host/src/link_age.rs"),
            include_str!("../../rta-host/src/fail_closed.rs"),
            include_str!("../../rta-host/src/tracker.rs"),
            include_str!("../../rta-spec/src/lib.rs"),
            include_str!("../../rta-switch/src/verify.rs"),
            include_str!("../../rta-host/src/bundle.rs"),
            include_str!("../../rta-host/src/quarantine.rs"),
            include_str!("../../../spec/monitor.lola"),
        ];
        let joined = sources.join("\n");
        let hazards = [
            "H1", "H2", "H3", "H4", "H5", "H6", "H7", "H8", "H9", "H10", "H11", "H12", "H13",
            "H14", "H15",
        ];
        let mut rows = 0;
        for line in method.lines() {
            if !hazards.iter().any(|h| line.contains(&format!("| {h} |"))) {
                continue;
            }
            rows += 1;
            assert!(
                line.contains("Before")
                    || line.contains("(A)")
                    || line.contains("(B)")
                    || line.contains("(C)")
                    || line.contains("(D)")
                    || line.contains("(E)"),
                "{line}"
            );
            let test = line
                .split("test `")
                .nth(1)
                .and_then(|s| s.split('`').next());
            let test = test.expect(line);
            assert!(joined.contains(test), "{test}");
        }
        assert_eq!(rows, 15);
        assert!(method.contains("project scale"));
        assert!(method.contains("not a finding"));
        assert!(method.contains("Eliminated (F) is not used"));
        assert!(!method.contains("| Eliminated"));
    }
}
