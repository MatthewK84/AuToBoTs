//! Degraded-mode matrix. Commit is never allowed on a degraded row.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    pub name: &'static str,
    pub navigation: bool,
    pub commit: bool,
    pub recovery: bool,
}

pub const MATRIX: &[Row] = &[
    Row {
        name: "Lost tracker",
        navigation: true,
        commit: false,
        recovery: false,
    },
    Row {
        name: "Lost grant",
        navigation: true,
        commit: false,
        recovery: false,
    },
    Row {
        name: "Shadow only",
        navigation: true,
        commit: false,
        recovery: false,
    },
    Row {
        name: "Filter timeout",
        navigation: false,
        commit: false,
        recovery: true,
    },
    Row {
        name: "Stale link",
        navigation: false,
        commit: false,
        recovery: true,
    },
    Row {
        name: "Recovery unconfirmed",
        navigation: false,
        commit: false,
        recovery: true,
    },
];

#[cfg(test)]
mod tests {
    use super::MATRIX;

    #[test]
    fn the_matrix_matches_the_verdict_tests_and_no_row_allows_commit() {
        let doc = include_str!("../../../docs/degraded.md");
        let verdicts = include_str!("../../rta-switch/src/lib.rs");
        for row in MATRIX {
            assert!(!row.commit, "{}", row.name);
            assert!(doc.contains(row.name), "{}", row.name);
        }
        assert!(verdicts.contains("inhibit_clears_commit"));
        assert!(verdicts.contains("revert_has_no_commit_field"));
        assert!(doc.contains("never allowed"));
    }

    #[test]
    fn the_matrix_defines_a_vehicle_that_never_confirms_recovery() {
        let doc = include_str!("../../../docs/degraded.md");
        for item in [
            "| Recovery unconfirmed | not allowed | not allowed | commanded until the order is used up |",
            "never confirms the recovery mode",
            "stops sending and Revert stays latched until restart",
            "offboard-loss failsafe",
            "`recovery_unconfirmed`",
            "docs/recovery.md",
        ] {
            assert!(doc.contains(item), "{item}");
        }
    }
}
