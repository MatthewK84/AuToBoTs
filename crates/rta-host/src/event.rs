//! Record to interpreter event. Order is the spec input order, written once.

use ordered_float::NotNan;
use rta_spec::{InputRecord, SPEC_INPUT_ORDER};
use rtlola_interpreter::Value;

pub fn event(record: &InputRecord) -> [Value; 6] {
    debug_assert_eq!(SPEC_INPUT_ORDER.len(), 6);
    [
        Value::Bool(record.fence_ok()),
        float(record.fix_age_ms()),
        float(record.link_age_ms()),
        float(record.fc_heartbeat_age_ms()),
        float(record.track_conf()),
        float(record.range_m()),
    ]
}

fn float(value: f64) -> Value {
    Value::Float(NotNan::new(value).expect("record already rejected non-finite"))
}

pub fn spec_input_order(spec: &str) -> Vec<String> {
    spec.lines()
        .filter_map(|line| {
            let line = line.trim();
            line.starts_with("input ").then(|| {
                line.split_whitespace()
                    .nth(1)
                    .unwrap_or_default()
                    .trim_end_matches(':')
                    .to_string()
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_order_matches_the_const() {
        let order = spec_input_order(include_str!("../../../spec/monitor.lola"));
        assert_eq!(order, SPEC_INPUT_ORDER);
    }

    #[test]
    fn values_survive_the_mapping() {
        let record = InputRecord::try_new(10.0, true, 12.0, 0.4, 30.0, 8.0, 40.0).unwrap();
        let mapped = event(&record);
        assert_eq!(mapped[0], Value::Bool(true));
        assert_eq!(mapped[1], float(12.0));
        assert_eq!(mapped[2], float(8.0));
        assert_eq!(mapped[3], float(40.0));
        assert_eq!(mapped[4], float(0.4));
        assert_eq!(mapped[5], float(30.0));
    }
}
