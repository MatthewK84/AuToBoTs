//! Replay a tick log against the spec. A verdict mismatch is a failure.

use crate::eval::Evaluator;
use crate::monitor::build_once;
use crate::tick_log::TickRecord;
use rta_spec::{InputRecord, Verdict};
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct Mismatch {
    pub tick: u64,
    pub expected: String,
    pub actual: String,
    pub record: String,
}

pub fn check(log: &str, spec: &str) -> Result<(), Mismatch> {
    let mut monitor = build_once(spec).map_err(|_| mismatch(0, "build", "fault", log))?;
    let mut evaluator = Evaluator::default();
    for line in log.lines().filter(|line| line.starts_with("tick=")) {
        let record = parse_record(line).map_err(|_| mismatch(0, "parse", "fault", line))?;
        let input = InputRecord::try_new(
            0.0,
            record.fence_ok,
            record.fix_age_ms,
            record.track_conf,
            record.range_m,
            record.link_age_ms,
            record.fc_heartbeat_age_ms,
        )
        .map_err(|_| mismatch(record.tick, "input", "fault", line))?;
        let actual = evaluator.tick(&mut monitor, &input, record.tick);
        let expected = verdict_name(record.verdict);
        let got = verdict_name(actual.verdict);
        if expected != got {
            return Err(mismatch(record.tick, expected, got, line));
        }
    }
    Ok(())
}

pub fn check_files(log_path: &Path, spec_path: &Path) -> Result<(), Mismatch> {
    let log = std::fs::read_to_string(log_path).map_err(|_| mismatch(0, "log", "missing", ""))?;
    let spec =
        std::fs::read_to_string(spec_path).map_err(|_| mismatch(0, "spec", "missing", ""))?;
    check(&log, &spec)
}

fn mismatch(tick: u64, expected: &str, actual: &str, record: &str) -> Mismatch {
    Mismatch {
        tick,
        expected: expected.into(),
        actual: actual.into(),
        record: record.into(),
    }
}

fn parse_record(line: &str) -> Result<TickRecord, &'static str> {
    let mut tick = None;
    let mut fence_ok = None;
    let mut fix_age_ms = None;
    let mut link_age_ms = None;
    let mut fc_heartbeat_age_ms = None;
    let mut track_conf = None;
    let mut range_m = None;
    let mut verdict = None;
    let mut reasons = Vec::new();
    let mut command = String::new();
    let mut eval_ms = 0;
    let mut spec_hash = 0;
    for field in line.split_whitespace() {
        let Some((key, value)) = field.split_once('=') else {
            return Err("field");
        };
        match key {
            "tick" => tick = value.parse().ok(),
            "fence_ok" => fence_ok = Some(value == "1"),
            "fix_age_ms" => fix_age_ms = value.parse().ok(),
            "link_age_ms" => link_age_ms = value.parse().ok(),
            "fc_heartbeat_age_ms" => fc_heartbeat_age_ms = value.parse().ok(),
            "track_conf" => track_conf = value.parse().ok(),
            "range_m" => range_m = value.parse().ok(),
            "verdict" => verdict = parse_verdict(value),
            "reasons" => {
                if !value.is_empty() {
                    reasons = value.split(',').map(str::to_string).collect();
                }
            }
            "command" => command = value.to_string(),
            "eval_ms" => eval_ms = value.parse().unwrap_or(0),
            "spec_hash" => spec_hash = u64::from_str_radix(value, 16).unwrap_or(0),
            _ => {}
        }
    }
    Ok(TickRecord {
        tick: tick.ok_or("field")?,
        fence_ok: fence_ok.ok_or("field")?,
        fix_age_ms: fix_age_ms.ok_or("field")?,
        link_age_ms: link_age_ms.ok_or("field")?,
        fc_heartbeat_age_ms: fc_heartbeat_age_ms.ok_or("field")?,
        track_conf: track_conf.ok_or("field")?,
        range_m: range_m.ok_or("field")?,
        verdict: verdict.ok_or("field")?,
        reasons,
        command,
        eval_ms,
        spec_hash,
    })
}

fn parse_verdict(value: &str) -> Option<Verdict> {
    match value {
        "pass" => Some(Verdict::Pass),
        "inhibit" => Some(Verdict::Inhibit),
        "revert" => Some(Verdict::Revert),
        _ => None,
    }
}

fn verdict_name(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Pass => "pass",
        Verdict::Inhibit => "inhibit",
        Verdict::Revert => "revert",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> String {
        std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/monitor.lola"),
        )
        .unwrap()
    }

    #[test]
    fn golden_log_matches() {
        let log = include_str!("../fixtures/golden.log");
        assert!(check(log, &spec()).is_ok());
    }

    #[test]
    fn edited_verdict_fails() {
        let log = include_str!("../fixtures/edited.log");
        let err = check(log, &spec()).unwrap_err();
        assert_eq!(err.tick, 1);
        assert_eq!(err.expected, "revert");
        assert_eq!(err.actual, "pass");
        assert!(err.record.contains("fence_ok=1"));
    }
}
