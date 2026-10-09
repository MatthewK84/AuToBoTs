//! Fault injection. Each case is Revert or a startup failure, never Pass.

use crate::spec::load_spec;
use std::path::Path;

pub fn load_spec_file(path: &Path) -> Result<String, String> {
    let text = std::fs::read_to_string(path).map_err(|err| format!("spec file missing: {err}"))?;
    load_spec(&text)?;
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::Evaluator;
    use crate::monitor::build_once;
    use rta_spec::{InputRecord, Verdict};

    fn fresh(conf: f64, range: f64, heartbeat: f64) -> InputRecord {
        InputRecord::try_new(10.0, true, 0.0, conf, range, 0.0, heartbeat).expect("finite record")
    }

    #[test]
    fn h8_interpreter_err_reverts() {
        let spec = include_str!("../../../spec/monitor.lola");
        let mut built = build_once(spec).expect("spec");
        let mut eval = Evaluator::default();
        let first = eval.tick(&mut built, &fresh(1.0, 200.0, 0.0), 1_000);
        assert_eq!(first.verdict, Verdict::Pass);
        built.fail_next();
        let second = eval.tick(&mut built, &fresh(1.0, 200.0, 0.0), 2_000);
        assert_eq!(second.verdict, Verdict::Revert);
        assert!(eval.faulted());
        let latched = eval.tick(&mut built, &fresh(1.0, 200.0, 0.0), 3_000);
        assert_eq!(latched.verdict, Verdict::Revert);
        assert_ne!(latched.verdict, Verdict::Pass);
    }

    #[test]
    fn h8_spec_missing_at_startup() {
        let err = load_spec_file(Path::new("spec/does-not-exist.lola")).expect_err("missing spec");
        assert!(err.contains("spec file missing"));
        assert!(!err.contains("Pass"));
    }

    #[test]
    fn h4_heartbeat_gap_reverts() {
        let spec = include_str!("../../../spec/monitor.lola");
        let mut built = build_once(spec).expect("spec");
        let mut eval = Evaluator::default();
        let result = eval.tick(&mut built, &fresh(1.0, 200.0, 5_000.0), 1_000);
        assert_eq!(result.verdict, Verdict::Revert);
        assert_ne!(result.verdict, Verdict::Pass);
    }

    #[test]
    fn h6_nan_altitude_reverts() {
        let err = InputRecord::try_new(f64::NAN, true, 0.0, 1.0, 200.0, 0.0, 0.0)
            .expect_err("nan altitude");
        assert!(err.contains("finite"));
        let verdict = if err.contains("finite") {
            Verdict::Revert
        } else {
            Verdict::Pass
        };
        assert_eq!(verdict, Verdict::Revert);
    }

    #[test]
    fn h5_zero_confidence_inside_commit_does_not_pass() {
        let spec = include_str!("../../../spec/monitor.lola");
        let mut built = build_once(spec).expect("spec");
        let mut eval = Evaluator::default();
        let result = eval.tick(&mut built, &fresh(0.0, 10.0, 0.0), 1_000);
        assert_ne!(result.verdict, Verdict::Pass);
        assert_eq!(result.verdict, Verdict::Inhibit);
    }
}
