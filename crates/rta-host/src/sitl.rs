//! PX4 SITL hover sample. The port is the example config port.

use crate::config::{Config, Fence};
use crate::eval::Evaluator;
use crate::fence::{fence_ok, FenceInput};
use crate::monitor::BuiltMonitor;
use rta_spec::{InputRecord, Verdict};

pub const EXAMPLE_ENDPOINT: &str = "udp:127.0.0.1:14540";

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
}
