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
    fn every_stream_has_a_note() {
        let spec = include_str!("../../../spec/monitor.lola");
        let notes = include_str!("../../../docs/spec-notes.md");
        for line in spec.lines() {
            let line = line.trim();
            if line.starts_with("input ") || line.starts_with("output ") {
                let name = line
                    .split_whitespace()
                    .nth(1)
                    .unwrap()
                    .trim_end_matches(':');
                assert!(notes.contains(name), "{name}");
            }
        }
        assert!(notes.contains("## Blind spots"));
        for spot in [
            "Target identity",
            "Collateral",
            "Rules of engagement",
            "Tracker truth",
            "Estimator truth",
        ] {
            assert!(notes.contains(spot), "{spot}");
        }
    }

    #[test]
    fn host_has_no_threshold_literals() {
        let host = [
            include_str!("main.rs"),
            include_str!("config.rs"),
            include_str!("fence.rs"),
            include_str!("stale.rs"),
        ]
        .concat();
        for literal in ["500.0", "300.0", "1000.0", "0.40", "150.0"] {
            assert!(!host.contains(literal), "{literal}");
        }
    }
}
