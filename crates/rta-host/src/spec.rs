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
}
