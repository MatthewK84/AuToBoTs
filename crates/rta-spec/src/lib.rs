//! Input record and log schema. Forbidden: sockets, MAVLink, and the RTLola interpreter.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    Pass,
    Inhibit,
    Revert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reason {
    None,
    NoGrant,
    Coast,
    PilotOverride,
    Abort,
    Estimator,
    Energy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Search,
    Track,
    Commit,
    Abort,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TickInput {
    pub phase: Phase,
    pub grant_present: bool,
    pub coast_ok: bool,
    pub pilot_override: bool,
    pub abort_requested: bool,
    pub estimator_ok: bool,
    pub endurance_s: f64,
    pub time_to_rally_s: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TickLog {
    pub verdict: Verdict,
    pub reason: Reason,
    pub grant_hash: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HazardId {
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,
    H7,
    H8,
    H9,
    H10,
}

/// Recovery mode is chosen by config before the switch runs. The switch must not default it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryMode {
    Rtl,
    Loiter,
    Land,
}

pub fn hazard_verdict(id: HazardId) -> Option<Verdict> {
    match id {
        HazardId::H1
        | HazardId::H2
        | HazardId::H3
        | HazardId::H4
        | HazardId::H6
        | HazardId::H7
        | HazardId::H8
        | HazardId::H10 => Some(Verdict::Revert),
        HazardId::H5 => Some(Verdict::Inhibit),
        HazardId::H9 => None,
    }
}

const FORBIDDEN_LOG_FIELDS: &[&str] = &["grant_body", "signature", "frame", "image"];

pub fn log_schema_rejects(field: &str) -> bool {
    FORBIDDEN_LOG_FIELDS.contains(&field)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_grant_body_signature_and_frame() {
        assert!(log_schema_rejects("grant_body"));
        assert!(log_schema_rejects("signature"));
        assert!(log_schema_rejects("frame"));
        assert!(!log_schema_rejects("grant_hash"));
    }

    #[test]
    fn hazard_table_matches_the_list() {
        let rows = [
            (HazardId::H1, Some(Verdict::Revert)),
            (HazardId::H2, Some(Verdict::Revert)),
            (HazardId::H3, Some(Verdict::Revert)),
            (HazardId::H4, Some(Verdict::Revert)),
            (HazardId::H5, Some(Verdict::Inhibit)),
            (HazardId::H6, Some(Verdict::Revert)),
            (HazardId::H7, Some(Verdict::Revert)),
            (HazardId::H8, Some(Verdict::Revert)),
            (HazardId::H9, None),
            (HazardId::H10, Some(Verdict::Revert)),
        ];
        for (id, verdict) in rows {
            assert_eq!(hazard_verdict(id), verdict);
        }
    }
}
