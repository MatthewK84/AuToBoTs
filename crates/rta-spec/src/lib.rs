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
}
