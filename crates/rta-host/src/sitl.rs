//! PX4 SITL hover sample. The port is the example config port.

use crate::config::{Config, Fence};
use crate::eval::Evaluator;
use crate::fence::{fence_ok, FenceInput};
use crate::monitor::BuiltMonitor;
use rta_spec::{InputRecord, Verdict};

pub const EXAMPLE_ENDPOINT: &str = "udp:127.0.0.1:14540";

pub const TIMING_NOTE: &str = "SITL timing is not target timing";

pub fn timing_header(spec_hash: u64, samples_ms: &[u64]) -> String {
    let dist = distribution(samples_ms);
    format!(
        "# rta-log 1 spec_hash={spec_hash:016x} timing=sitl-not-target\n# {TIMING_NOTE}\n# eval_ms {dist}\n"
    )
}

pub fn distribution(samples_ms: &[u64]) -> String {
    if samples_ms.is_empty() {
        return "count=0".into();
    }
    let mut ordered = samples_ms.to_vec();
    ordered.sort_unstable();
    let at = |q: f64| ordered[((ordered.len() - 1) as f64 * q) as usize];
    format!(
        "count={} min={} p50={} p95={} max={}",
        ordered.len(),
        ordered[0],
        at(0.50),
        at(0.95),
        ordered[ordered.len() - 1]
    )
}

pub fn hover_record(config: &Config) -> Result<InputRecord, &'static str> {
    let Fence::Host {
        polygon,
        vertical_cap_m,
        v_max,
        a_max,
    } = &config.fence
    else {
        return Err("sitl hover needs the host fence");
    };
    let point = centroid(polygon);
    let inside = fence_ok(
        point,
        30.0,
        FenceInput {
            polygon,
            vertical_cap_m: *vertical_cap_m,
            v_max: *v_max,
            a_max: *a_max,
            mode_change_latency_s: config.tick.mode_change_latency_ms as f64 / 1_000.0,
        },
    );
    if !inside {
        return Err("hover is outside the fence margin");
    }
    InputRecord::try_new(30.0, true, 10.0, 0.9, 400.0, 10.0, 10.0)
}

pub fn hover_verdict(
    eval: &mut Evaluator,
    monitor: &mut BuiltMonitor,
    record: &InputRecord,
) -> Verdict {
    eval.tick(monitor, record, 1_000).verdict
}

pub fn paused_watchdog_commands_recovery(period_ms: u64, misses: u32) -> bool {
    use crate::mode::ModeSender;
    use crate::watchdog::Watchdog;
    use rta_spec::RecoveryMode;
    let mut dog = Watchdog::new(period_ms, misses);
    dog.publish(0);
    let mut sender = ModeSender::default();
    let mut sink = Vec::new();
    dog.check(
        period_ms * u64::from(misses),
        RecoveryMode::Loiter,
        RecoveryMode::Rtl,
        &mut sender,
        &mut sink,
    );
    sink == ["mode Rtl"]
}

fn centroid(polygon: &[[f64; 2]]) -> [f64; 2] {
    let n = polygon.len() as f64;
    let (x, y) = polygon.iter().fold((0.0, 0.0), |acc, point| {
        (acc.0 + point[0], acc.1 + point[1])
    });
    [x / n, y / n]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::parse_str;
    use crate::monitor::build_once;

    #[test]
    fn script_port_matches_example_config() {
        let example = include_str!("../../../config/example.toml");
        let sitl = include_str!("../../../config/sitl.toml");
        let script = include_str!("../../../scripts/sitl.sh");
        assert!(example.contains(EXAMPLE_ENDPOINT));
        assert!(sitl.contains(EXAMPLE_ENDPOINT));
        assert!(script.contains("14540"));
        assert!(script.contains("127.0.0.1"));
        let loaded = parse_str(example).expect("example");
        assert_eq!(loaded.link.endpoint, EXAMPLE_ENDPOINT);
    }

    #[test]
    fn nominal_hover_passes_inside_limits() {
        let config = parse_str(include_str!("../../../config/sitl.toml")).expect("sitl");
        let record = hover_record(&config).expect("inside");
        let spec = include_str!("../../../spec/monitor.lola");
        let mut built = build_once(spec).expect("spec");
        let mut eval = Evaluator::default();
        assert_eq!(hover_verdict(&mut eval, &mut built, &record), Verdict::Pass);
    }

    #[test]
    fn sitl_timing_is_not_target_timing() {
        let header = timing_header(0, &[1, 2, 3, 4, 5]);
        assert!(header.contains(TIMING_NOTE));
        assert!(header.contains("timing=sitl-not-target"));
        assert!(header.contains("count=5"));
        let doc = include_str!("../../../docs/sitl.md");
        assert!(doc.contains(TIMING_NOTE));
        let artifact = include_str!("../../../crates/rta-host/fixtures/sitl-timing.txt");
        assert!(artifact.contains(TIMING_NOTE));
        assert!(artifact.contains("eval_ms"));
    }

    #[test]
    fn paused_eval_commands_recovery() {
        assert!(paused_watchdog_commands_recovery(50, 3));
    }

    #[test]
    fn deadline_stays_five_ms() {
        let config = parse_str(include_str!("../../../config/sitl.toml")).expect("sitl");
        assert_eq!(config.tick.deadline_ms, 5);
        let note = include_str!("../../../docs/deadline.md");
        assert!(note.contains("deadline stays 5 ms"));
        assert!(note.contains("not raised"));
    }

    #[test]
    #[test]
    fn assurance_gap_covers_m11() {
        let note = include_str!("../../../docs/assurance-gap.md");
        for id in ["H1", "H2", "H3", "H4", "H5", "H6"] {
            assert!(note.contains(id), "{id}");
        }
        assert!(note.contains("The monitor and the recovery are not pedigreed components."));
        assert!(note.contains(
            "This build bounds flight and commit. It does not assure the tactical decision."
        ));
        assert!(note.contains("Following it is not a compliance claim."));
    }

    fn runbook_has_no_clear_fault() {
        let book = include_str!("../../../docs/runbook.md");
        let name = ["clear", "_fault"].concat();
        assert!(book.contains("no `clear_fault`"));
        assert!(!book.contains(&format!("pub fn {name}")));
        assert!(book.contains("Inhibit"));
        assert!(book.contains("Revert"));
        assert!(book.contains("rta.log"));
        assert!(book.contains("golden-trace review"));
        assert!(book.contains("restart"));
    }
}
