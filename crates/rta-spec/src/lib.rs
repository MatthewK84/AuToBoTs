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

pub const INPUT_RECORD_VERSION: u32 = 1;

pub const REQUIRED_OUTPUTS: &[&str] = &["revert", "inhibit"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputRecord {
    version: u32,
    alt_m: f64,
    fence_ok: bool,
    fix_age_ms: f64,
    track_conf: f64,
    range_m: f64,
    link_age_ms: f64,
    fc_heartbeat_age_ms: f64,
}

impl InputRecord {
    pub fn try_new(
        alt_m: f64,
        fence_ok: bool,
        fix_age_ms: f64,
        track_conf: f64,
        range_m: f64,
        link_age_ms: f64,
        fc_heartbeat_age_ms: f64,
    ) -> Result<Self, &'static str> {
        for value in [
            alt_m,
            fix_age_ms,
            track_conf,
            range_m,
            link_age_ms,
            fc_heartbeat_age_ms,
        ] {
            if !value.is_finite() {
                return Err("numeric input must be finite");
            }
        }
        if !(0.0..=1.0).contains(&track_conf) {
            return Err("track_conf must be in 0.0..=1.0");
        }
        if fix_age_ms < 0.0 || link_age_ms < 0.0 || fc_heartbeat_age_ms < 0.0 || range_m < 0.0 {
            return Err("ages and range must be non-negative");
        }
        Ok(Self {
            version: INPUT_RECORD_VERSION,
            alt_m,
            fence_ok,
            fix_age_ms,
            track_conf,
            range_m,
            link_age_ms,
            fc_heartbeat_age_ms,
        })
    }

    pub fn version(&self) -> u32 {
        self.version
    }
}

impl std::fmt::Display for InputRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "v={} alt_m={} fence_ok={} fix_age_ms={} track_conf={} range_m={} link_age_ms={} fc_heartbeat_age_ms={}",
            self.version,
            self.alt_m,
            self.fence_ok,
            self.fix_age_ms,
            self.track_conf,
            self.range_m,
            self.link_age_ms,
            self.fc_heartbeat_age_ms
        )
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
    fn rejects_non_finite_confidence_and_negative_age() {
        assert!(InputRecord::try_new(f64::NAN, true, 0.0, 0.5, 1.0, 0.0, 0.0).is_err());
        assert!(InputRecord::try_new(1.0, true, 0.0, 0.5, f64::INFINITY, 0.0, 0.0).is_err());
        assert!(InputRecord::try_new(1.0, true, 0.0, 1.1, 1.0, 0.0, 0.0).is_err());
        assert!(InputRecord::try_new(1.0, true, -1.0, 0.5, 1.0, 0.0, 0.0).is_err());
    }

    #[test]
    fn input_record_round_trips() {
        let record = InputRecord::try_new(10.0, true, 12.0, 0.4, 30.0, 8.0, 40.0).unwrap();
        let encoded = serde_json::to_string(&record).unwrap();
        let decoded: InputRecord = serde_json::from_str(&encoded).unwrap();
        assert_eq!(record, decoded);
        assert_eq!(decoded.version(), INPUT_RECORD_VERSION);
        let shown = record.to_string();
        assert!(shown.contains("alt_m=10"));
        assert!(shown.find("alt_m").unwrap() < shown.find("fence_ok").unwrap());
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
