//! Runtime configuration. A parse error is returned before any socket type is constructed.

use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub link: Link,
    pub tick: Tick,
    pub recovery: Recovery,
    pub spec: Spec,
    pub log: Log,
    pub fence: Fence,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub endpoint: String,
    pub system_id: u8,
    pub component_id: u8,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tick {
    pub period_ms: u64,
    pub deadline_ms: u64,
    pub watchdog_misses: u32,
    pub mode_change_latency_ms: u64,
    pub tracker_hold_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recovery {
    pub mode: RecoveryMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecoveryMode {
    Rtl,
    Loiter,
    Land,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Log {
    pub path: String,
    pub flush_every_n: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, tag = "source", rename_all = "lowercase")]
pub enum Fence {
    Fc,
    Host {
        polygon: Vec<[f64; 2]>,
        vertical_cap_m: f64,
        v_max: f64,
        a_max: f64,
    },
}

#[derive(Debug)]
pub struct ConfigError(pub String);

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub fn parse_str(text: &str) -> Result<Config, ConfigError> {
    let config: Config = toml::from_str(text).map_err(|err| ConfigError(err.to_string()))?;
    validate(&config)?;
    Ok(config)
}

pub fn load(path: &Path) -> Result<Config, ConfigError> {
    let text = fs::read_to_string(path).map_err(|err| ConfigError(err.to_string()))?;
    parse_str(&text)
}

fn validate(config: &Config) -> Result<(), ConfigError> {
    if config.tick.period_ms <= config.tick.deadline_ms {
        return Err(ConfigError(
            "tick.period_ms must be greater than tick.deadline_ms".into(),
        ));
    }
    if config.tick.watchdog_misses < 1 {
        return Err(ConfigError(
            "tick.watchdog_misses must be at least 1".into(),
        ));
    }
    if config.tick.mode_change_latency_ms < 1 {
        return Err(ConfigError(
            "tick.mode_change_latency_ms must be at least 1".into(),
        ));
    }
    if config.log.flush_every_n < 1 {
        return Err(ConfigError("log.flush_every_n must be at least 1".into()));
    }
    let endpoint = &config.link.endpoint;
    let ok = endpoint.starts_with("udp:") || endpoint.starts_with("serial:");
    if !ok {
        return Err(ConfigError(
            "link.endpoint must start with udp: or serial:".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example() -> String {
        include_str!("../../../config/example.toml").to_string()
    }

    #[test]
    fn example_round_trips() {
        let config = parse_str(&example()).expect("example");
        assert_eq!(config.tick.period_ms, 50);
        assert_eq!(config.tick.deadline_ms, 5);
        assert_eq!(config.recovery.mode, RecoveryMode::Rtl);
        assert!(matches!(config.fence, Fence::Host { .. }));
    }

    #[test]
    fn deadline_greater_than_period_is_err() {
        let text = example().replace("period_ms = 50", "period_ms = 5");
        assert!(parse_str(&text).is_err());
    }

    #[test]
    fn bad_enum_is_err() {
        let text = example().replace("mode = \"rtl\"", "mode = \"hover\"");
        assert!(parse_str(&text).is_err());
    }

    #[test]
    fn missing_file_is_err_before_any_socket() {
        let err = load(Path::new("config/does-not-exist.toml")).expect_err("missing");
        assert!(!err.0.is_empty());
    }
}
