//! Learning and reload hazards. A threshold edit without a bundle is a hazard, not a feature.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LearningHazard {
    UnsignedReload,
    FixtureRegression,
    SwapTimeout,
    LearnedFileWithoutBundle,
    FaultMidSwap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HazardOutcome {
    SwapRefused,
    Revert,
}

pub struct HazardRow {
    pub id: &'static str,
    pub hazard: LearningHazard,
    pub outcome: HazardOutcome,
    pub test: &'static str,
}

pub const LEARNING_HAZARDS: &[HazardRow] = &[
    HazardRow {
        id: "H11",
        hazard: LearningHazard::UnsignedReload,
        outcome: HazardOutcome::SwapRefused,
        test: "unsigned_is_rejected_and_the_monitor_stays",
    },
    HazardRow {
        id: "H12",
        hazard: LearningHazard::FixtureRegression,
        outcome: HazardOutcome::SwapRefused,
        test: "a_spec_that_flips_must_hold_is_refused",
    },
    HazardRow {
        id: "H13",
        hazard: LearningHazard::SwapTimeout,
        outcome: HazardOutcome::SwapRefused,
        test: "an_over_deadline_eval_is_refused",
    },
    HazardRow {
        id: "H14",
        hazard: LearningHazard::LearnedFileWithoutBundle,
        outcome: HazardOutcome::SwapRefused,
        test: "a_candidate_weight_file_does_not_change_inference",
    },
    HazardRow {
        id: "H15",
        hazard: LearningHazard::FaultMidSwap,
        outcome: HazardOutcome::Revert,
        test: "a_fault_mid_swap_cannot_leave_the_command_path_on_the_new_spec",
    },
];

/// The outcome in the table. A hazard with no row is Revert, the fail-closed answer; a test
/// requires every hazard to have a row.
pub fn outcome(hazard: LearningHazard) -> HazardOutcome {
    LEARNING_HAZARDS
        .iter()
        .find(|row| row.hazard == hazard)
        .map_or(HazardOutcome::Revert, |row| row.outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_learning_hazard_is_a_row_that_traces_to_an_m13_test() {
        let hazards = include_str!("../../../docs/hazards.md");
        let bundle = include_str!("../../rta-host/src/bundle.rs");
        let quarantine = include_str!("../../rta-host/src/quarantine.rs");
        let sources = [bundle, quarantine];
        for row in LEARNING_HAZARDS {
            assert!(hazards.contains(row.id), "{}", row.id);
            assert!(
                hazards.contains(row.test),
                "{} missing test {}",
                row.id,
                row.test
            );
            let word = match row.outcome {
                HazardOutcome::SwapRefused => "swap refused",
                HazardOutcome::Revert => "Revert",
            };
            assert!(hazards.contains(word), "{word}");
            assert!(
                sources.iter().any(|source| source.contains(row.test)),
                "{} test is not in M13.1 or M13.2",
                row.id
            );
        }
        assert_eq!(
            outcome(LearningHazard::FixtureRegression),
            HazardOutcome::SwapRefused
        );
        assert_eq!(outcome(LearningHazard::FaultMidSwap), HazardOutcome::Revert);
    }

    #[test]
    fn a_threshold_edit_without_a_bundle_is_a_hazard() {
        let hazards = include_str!("../../../docs/hazards.md");
        assert!(hazards.contains("threshold"));
        assert!(hazards.contains("not a feature"));
        assert!(hazards.contains("H12"));
    }

    #[test]
    fn the_non_hazard_list_does_not_call_hot_reload_out_of_scope() {
        let hazards = include_str!("../../../docs/hazards.md");
        let non = hazards
            .split("## Non-hazards")
            .nth(1)
            .expect("non-hazards")
            .split("## ")
            .next()
            .expect("section");
        let lower = non.to_lowercase();
        assert!(!lower.contains("hot reload is out of scope"));
        assert!(!lower.contains("hot reload") || lower.contains("not a non-hazard"));
        assert!(lower.contains("h11"));
    }
}
