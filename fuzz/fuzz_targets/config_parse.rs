//! Config text and the link endpoint string. Either parses or is refused; a panic is a
//! finding. An accepted config must meet the cross-field rules in `config::validate`.
#![no_main]

use libfuzzer_sys::fuzz_target;
use rta_host::{config, link};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let _ = link::parse_endpoint(text);
    if let Ok(config) = config::parse_str(text) {
        assert!(config.tick.period_ms > config.tick.deadline_ms);
        assert!(config.tick.watchdog_misses >= 1);
        assert!(config.recovery.retry_cap_ms >= config.recovery.retry_initial_ms);
        assert!((1..=10).contains(&config.recovery.attempts_per_step));
        assert!(config.startup.ready_fix_age_ms >= config.tick.period_ms);
        assert!(config.log.flush_every_n >= 1);
        let _ = link::parse_endpoint(&config.link.endpoint);
    }
});
