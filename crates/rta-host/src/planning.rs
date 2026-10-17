//! Planning-data check. Documents, not a certification claim.

/// The five DO-178C planning documents.
pub const PLANS: &[&str] = &[
    include_str!("../../../docs/do178/psac.md"),
    include_str!("../../../docs/do178/sdp.md"),
    include_str!("../../../docs/do178/svp.md"),
    include_str!("../../../docs/do178/scmp.md"),
    include_str!("../../../docs/do178/sqa.md"),
];

/// Hazard ids the plans must point at.
pub const HAZARD_IDS: &[&str] = &[
    "H1", "H2", "H3", "H4", "H5", "H6", "H7", "H8", "H9", "H10", "H11", "H12", "H13", "H14", "H15",
];

#[cfg(test)]
mod tests {
    use super::{HAZARD_IDS, PLANS};

    #[test]
    fn five_plans_point_at_hazard_ids_and_the_choice_is_a_project_choice() {
        assert_eq!(PLANS.len(), 5);
        let joined = PLANS.join("\n");
        for id in HAZARD_IDS {
            assert!(joined.contains(id), "{id}");
        }
        assert!(joined.contains("project choice, not an authority finding"));
        assert!(joined.contains("Level C"));
        assert!(joined.contains("Level E"));
        assert!(joined.contains("ASTM F3269"));
        assert!(joined.contains("DO-178C"));
        assert!(joined.contains("DO-333"));
        assert!(joined.contains("not a certification claim"));
    }
}
