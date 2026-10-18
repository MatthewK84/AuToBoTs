//! Bidirectional trace. Generated from tags in the tests.

/// One hazard, one requirement, one stream or invariant, one test, one result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceRow {
    pub hazard: &'static str,
    pub requirement: &'static str,
    pub evidence: &'static str,
    pub test_name: &'static str,
    pub result: &'static str,
}

/// Hazard to requirement to spec stream or switch invariant to test to result.
pub const TRACE: &[TraceRow] = &[
    TraceRow {
        hazard: "H1",
        requirement: "fence exit, including the stopping-distance margin, is Revert",
        evidence: "host fence_ok",
        test_name: "margin_band_is_not_ok",
        result: "ci-log",
    },
    TraceRow {
        hazard: "H2",
        requirement: "stale position fix is Revert",
        evidence: "spec stream fix_age_ms",
        test_name: "position_resets_fix_age",
        result: "ci-log",
    },
    TraceRow {
        hazard: "H3",
        requirement: "stale command link is Revert",
        evidence: "spec stream link_age_ms",
        test_name: "silent_gap_over_the_spec_limit_is_stale",
        result: "ci-log",
    },
    TraceRow {
        hazard: "H4",
        requirement: "stale flight-controller heartbeat is Revert",
        evidence: "spec stream fc_heartbeat_age_ms",
        test_name: "missing_heartbeat_is_stale",
        result: "ci-log",
    },
    TraceRow {
        hazard: "H5",
        requirement: "weak track inside commit range is Inhibit",
        evidence: "spec stream range_m and track_conf",
        test_name: "fake_emits_a_weak_track_inside_commit_range",
        result: "ci-log",
    },
    TraceRow {
        hazard: "H6",
        requirement: "non-finite numeric input is rejected",
        evidence: "host record constructor",
        test_name: "rejects_non_finite_confidence_and_negative_age",
        result: "ci-log",
    },
    TraceRow {
        hazard: "H7",
        requirement: "eval longer than deadline_ms is Revert",
        evidence: "host timer",
        test_name: "late_tick_discards_pass",
        result: "ci-log",
    },
    TraceRow {
        hazard: "H8",
        requirement: "interpreter load or accept_event error is Revert",
        evidence: "host latch on Err",
        test_name: "fault_stays_revert",
        result: "ci-log",
    },
    TraceRow {
        hazard: "H9",
        requirement: "setpoints and a mode change are not both a command",
        evidence: "switch invariant: Command is an enum",
        test_name: "decide_variants_are_exclusive",
        result: "kani-log",
    },
    TraceRow {
        hazard: "H10",
        requirement: "log write failure in flight is Revert",
        evidence: "host latch on a failed flush",
        test_name: "write_failure_latches_and_retries",
        result: "ci-log",
    },
    TraceRow {
        hazard: "H11",
        requirement: "unsigned reload is refused",
        evidence: "bundle gate",
        test_name: "unsigned_is_rejected_and_the_monitor_stays",
        result: "ci-log",
    },
    TraceRow {
        hazard: "H12",
        requirement: "a spec that flips must-hold is refused",
        evidence: "quarantine fixture",
        test_name: "a_spec_that_flips_must_hold_is_refused",
        result: "ci-log",
    },
    TraceRow {
        hazard: "H13",
        requirement: "an over-deadline eval is refused",
        evidence: "quarantine deadline",
        test_name: "an_over_deadline_eval_is_refused",
        result: "ci-log",
    },
    TraceRow {
        hazard: "H14",
        requirement: "a candidate weight file does not change inference",
        evidence: "active path",
        test_name: "a_candidate_weight_file_does_not_change_inference",
        result: "ci-log",
    },
    TraceRow {
        hazard: "H15",
        requirement: "a fault mid-swap stays on the old spec",
        evidence: "host latch",
        test_name: "a_fault_mid_swap_cannot_leave_the_command_path_on_the_new_spec",
        result: "ci-log",
    },
];

const SOURCES: &[&str] = &[
    include_str!("../../rta-host/src/fence.rs"),
    include_str!("../../rta-host/src/mavlink_link.rs"),
    include_str!("../../rta-host/src/link_age.rs"),
    include_str!("../../rta-host/src/fail_closed.rs"),
    include_str!("../../rta-host/src/tracker.rs"),
    include_str!("../../rta-spec/src/lib.rs"),
    include_str!("../../rta-switch/src/verify.rs"),
    include_str!("../../rta-host/src/bundle.rs"),
    include_str!("../../rta-host/src/quarantine.rs"),
];

/// Hazards with no `trace: Hn` tag next to the named test. A non-empty result fails CI.
pub fn untraced() -> Vec<&'static str> {
    TRACE
        .iter()
        .filter(|row| {
            let tag = format!("trace: {}", row.hazard);
            !SOURCES
                .iter()
                .any(|source| source.contains(&tag) && source.contains(row.test_name))
        })
        .map(|row| row.hazard)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{untraced, TRACE};

    #[test]
    fn an_untraced_hazard_fails_ci() {
        let missing = untraced();
        assert!(missing.is_empty(), "{missing:?}");
        assert_eq!(TRACE.len(), 15);
        let doc = include_str!("../../../docs/do178/trace.md");
        for row in TRACE {
            assert!(doc.contains(row.hazard), "{}", row.hazard);
            assert!(doc.contains(row.test_name), "{}", row.test_name);
        }
        assert!(doc.contains("ci-log"));
        assert!(doc.contains("kani-log"));
    }
}
