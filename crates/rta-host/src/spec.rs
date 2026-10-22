//! Spec load. A missing required stream fails before a socket is opened. No hot reload.

use rta_spec::REQUIRED_OUTPUTS;
use rtlola_frontend::{parse, ParserConfig};

pub fn load_spec(text: &str) -> Result<(), String> {
    let cfg = ParserConfig::for_string(text.to_string());
    let mir = parse(&cfg).map_err(|err| format!("spec parse failed: {err:?}"))?;
    let names: Vec<&str> = mir
        .outputs
        .iter()
        .map(|output| output.name.as_str())
        .collect();
    for required in REQUIRED_OUTPUTS {
        if !names.contains(required) {
            return Err(format!("spec missing required output {required}"));
        }
    }
    Ok(())
}

/// Thresholds the host needs as well as the spec. Read from the spec text, never copied.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpecLimits {
    /// `heartbeat_stale`: the flight-controller heartbeat age above which the spec reverts.
    pub heartbeat_stale_ms: f64,
}

/// Read the host's limits from the spec. A spec whose stream is missing or is not a plain
/// `input > number` comparison is an error, so startup stops before the link.
pub fn limits(text: &str) -> Result<SpecLimits, String> {
    Ok(SpecLimits {
        heartbeat_stale_ms: threshold(text, "heartbeat_stale", "fc_heartbeat_age_ms", '>')?,
    })
}

/// The number in `output <name>: Bool := <input> <op> <number>`. Exactly one such line.
pub fn threshold(text: &str, name: &str, input: &str, op: char) -> Result<f64, String> {
    let head = format!("output {name}:");
    let mut lines = text
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with(&head));
    let line = lines
        .next()
        .ok_or_else(|| format!("spec has no output {name}"))?;
    if lines.next().is_some() {
        return Err(format!("spec defines {name} twice"));
    }
    let expression = line
        .split_once(":=")
        .map(|(_, expression)| expression.trim())
        .ok_or_else(|| format!("{name} has no expression"))?;
    let rest = expression
        .strip_prefix(input)
        .map(str::trim_start)
        .and_then(|rest| rest.strip_prefix(op))
        .map(str::trim)
        .ok_or_else(|| format!("{name} is not `{input} {op} number`"))?;
    let value: f64 = rest
        .parse()
        .map_err(|_| format!("{name} threshold `{rest}` is not a number"))?;
    if !value.is_finite() || value < 0.0 {
        return Err(format!(
            "{name} threshold {value} is not a finite non-negative number"
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_in_spec_loads() {
        let text = include_str!("../../../spec/monitor.lola");
        load_spec(text).expect("monitor.lola");
    }

    #[test]
    fn missing_revert_fails_load() {
        let text = "input fence_ok: Bool\noutput inhibit: Bool := fence_ok\n";
        let err = load_spec(text).expect_err("missing revert");
        assert!(err.contains("revert"));
    }

    #[test]
    fn broken_spec_fails_parse() {
        assert!(load_spec("this is not a spec").is_err());
    }

    #[test]
    fn first_monitor_has_named_streams() {
        let text = include_str!("../../../spec/monitor.lola");
        load_spec(text).expect("monitor");
        for name in [
            "fix_stale",
            "link_stale",
            "heartbeat_stale",
            "weak_track",
            "inside_commit",
            "revert",
            "inhibit",
        ] {
            assert!(text.contains(name), "{name}");
        }
        for code in [
            "\"fence\"",
            "\"fix\"",
            "\"link\"",
            "\"heartbeat\"",
            "\"weak_track\"",
        ] {
            assert!(text.contains(code), "{code}");
        }
    }

    #[test]
    fn the_host_heartbeat_limit_is_the_spec_threshold() {
        let text = include_str!("../../../spec/monitor.lola");
        let limits = limits(text).expect("limits");
        // The parsed number is the one the monitor uses: just under is fresh, just over is
        // stale, evaluated by the real spec.
        use crate::eval::Evaluator;
        use crate::monitor::build_once;
        use rta_spec::{InputRecord, Verdict};
        let verdict = |age: f64| {
            let mut built = build_once(text).expect("spec");
            let record =
                InputRecord::try_new(30.0, true, 10.0, 0.9, 400.0, 10.0, age).expect("record");
            Evaluator::default().tick(&mut built, &record, 1).verdict
        };
        assert_eq!(verdict(limits.heartbeat_stale_ms), Verdict::Pass);
        assert_eq!(verdict(limits.heartbeat_stale_ms + 1.0), Verdict::Revert);
    }

    #[test]
    fn a_1_hz_flight_controller_heartbeat_with_jitter_is_fresh_and_one_missed_beat_is_stale() {
        // PX4 sends HEARTBEAT at 1 Hz on the onboard link. The age the tick sees runs up to
        // one period plus jitter between beats; PX4 SITL v1.16.2 reached 1014 ms. A limit of
        // one period reverted a healthy vehicle within seconds of arming. One missed beat
        // takes the age to two periods.
        use crate::eval::Evaluator;
        use crate::monitor::build_once;
        use rta_spec::{InputRecord, Verdict};
        let text = include_str!("../../../spec/monitor.lola");
        let verdict = |age: f64| {
            let mut built = build_once(text).expect("spec");
            let record =
                InputRecord::try_new(30.0, true, 10.0, 0.9, 400.0, 10.0, age).expect("record");
            Evaluator::default().tick(&mut built, &record, 1).verdict
        };
        for healthy in [1_000.0, 1_014.0, 1_100.0, 1_250.0] {
            assert_eq!(verdict(healthy), Verdict::Pass, "{healthy}");
        }
        let limit = limits(text).expect("limits").heartbeat_stale_ms;
        assert!(limit < 2_000.0, "{limit}: one missed beat must trip");
        assert_eq!(verdict(2_000.0), Verdict::Revert);
    }

    #[test]
    fn a_changed_spec_threshold_moves_the_host_limit() {
        let text = include_str!("../../../spec/monitor.lola");
        let line = text
            .lines()
            .find(|line| line.starts_with("output heartbeat_stale:"))
            .unwrap();
        let changed = text.replace(
            line,
            "output heartbeat_stale: Bool := fc_heartbeat_age_ms > 750.0",
        );
        assert_eq!(limits(&changed).unwrap().heartbeat_stale_ms, 750.0);
    }

    #[test]
    fn an_unreadable_threshold_is_a_startup_error() {
        let text = include_str!("../../../spec/monitor.lola");
        let line = text
            .lines()
            .find(|line| line.starts_with("output heartbeat_stale:"))
            .unwrap();
        for bad in [
            "output heartbeat_stale: Bool := fc_heartbeat_age_ms >= 1.0",
            "output heartbeat_stale: Bool := 1.0 < fc_heartbeat_age_ms",
            "output heartbeat_stale: Bool := fc_heartbeat_age_ms > soon",
            "output heartbeat_stale: Bool := fc_heartbeat_age_ms > -1.0",
            "output stale_heartbeat: Bool := fc_heartbeat_age_ms > 1.0",
        ] {
            assert!(limits(&text.replace(line, bad)).is_err(), "{bad}");
        }
        let twice = format!("{text}\n{line}\n");
        assert!(limits(&twice).is_err());
    }
}
