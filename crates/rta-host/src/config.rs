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
    #[serde(default)]
    pub shadow: Shadow,
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
    /// Wait after the first recovery send before the first resend. Doubles per resend.
    #[serde(default = "default_retry_initial_ms")]
    pub retry_initial_ms: u64,
    /// Longest wait between resends.
    #[serde(default = "default_retry_cap_ms")]
    pub retry_cap_ms: u64,
    /// Sends of one step of the fallback order before the next step.
    #[serde(default = "default_attempts_per_step")]
    pub attempts_per_step: u32,
}

fn default_retry_initial_ms() -> u64 {
    500
}

fn default_retry_cap_ms() -> u64 {
    2_000
}

fn default_attempts_per_step() -> u32 {
    3
}

/// Upper bound on `recovery.attempts_per_step`. Keeps the whole order short and finite.
pub const MAX_ATTEMPTS_PER_STEP: u32 = 10;

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

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Shadow {
    pub enabled: bool,
    pub inference_budget_ms: u64,
    pub weights_path: String,
    pub weights_hash: String,
    pub silence_ticks: u32,
}

impl Default for Shadow {
    fn default() -> Self {
        Self {
            enabled: false,
            inference_budget_ms: 2,
            weights_path: String::new(),
            weights_hash: String::new(),
            silence_ticks: 3,
        }
    }
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
    let recovery = &config.recovery;
    if recovery.retry_initial_ms < config.tick.mode_change_latency_ms {
        return Err(ConfigError(
            "recovery.retry_initial_ms must be at least tick.mode_change_latency_ms".into(),
        ));
    }
    if recovery.retry_cap_ms < recovery.retry_initial_ms {
        return Err(ConfigError(
            "recovery.retry_cap_ms must be at least recovery.retry_initial_ms".into(),
        ));
    }
    if recovery.attempts_per_step < 1 || recovery.attempts_per_step > MAX_ATTEMPTS_PER_STEP {
        return Err(ConfigError(
            "recovery.attempts_per_step must be from 1 to 10".into(),
        ));
    }
    if config.log.flush_every_n < 1 {
        return Err(ConfigError("log.flush_every_n must be at least 1".into()));
    }
    if config.shadow.inference_budget_ms == 0
        || config.shadow.inference_budget_ms >= config.tick.deadline_ms
    {
        return Err(ConfigError(
            "shadow.inference_budget_ms must be at least 1 and less than tick.deadline_ms".into(),
        ));
    }
    let endpoint = &config.link.endpoint;
    let ok = endpoint.starts_with("udp:")
        || endpoint.starts_with("udpin:")
        || endpoint.starts_with("serial:");
    if !ok {
        return Err(ConfigError(
            "link.endpoint must start with udp:, udpin:, or serial:".into(),
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
        assert_eq!(config.recovery.retry_initial_ms, 500);
        assert_eq!(config.recovery.retry_cap_ms, 2_000);
        assert_eq!(config.recovery.attempts_per_step, 3);
        assert!(matches!(config.fence, Fence::Host { .. }));
        assert!(!config.shadow.enabled);
        assert_eq!(config.shadow.inference_budget_ms, 2);
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
    fn the_retry_fields_default_and_are_checked() {
        // The reviewed example leaves them out, so the defaults are what it runs with.
        assert!(!example().contains("retry_"));
        let explicit = |initial: u64, cap: u64, per_step: u32| {
            example().replace(
                "mode = \"rtl\"",
                &format!(
                    "mode = \"rtl\"\nretry_initial_ms = {initial}\nretry_cap_ms = {cap}\nattempts_per_step = {per_step}"
                ),
            )
        };
        let defaults = parse_str(&example()).unwrap().recovery;
        assert_eq!(
            parse_str(&explicit(500, 2_000, 3)).unwrap().recovery,
            defaults
        );
        // Under the mode-change latency, cap under initial, zero or too many per step.
        for (initial, cap, per_step) in [
            (100, 2_000, 3),
            (500, 400, 3),
            (500, 2_000, 0),
            (500, 2_000, 11),
        ] {
            assert!(
                parse_str(&explicit(initial, cap, per_step)).is_err(),
                "{initial} {cap} {per_step}"
            );
        }
    }

    #[test]
    fn missing_file_is_err_before_any_socket() {
        let err = load(Path::new("config/does-not-exist.toml")).expect_err("missing");
        assert!(!err.0.is_empty());
    }
}
