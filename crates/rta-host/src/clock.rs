//! Monotonic tick clock. A wall step back is ignored.

/// Last monotonic reading. A smaller reading does not move it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonoClock {
    last_ms: u64,
}

impl MonoClock {
    pub fn new() -> Self {
        Self { last_ms: 0 }
    }

    /// Accept a monotonic reading. A backward step returns the last reading.
    pub fn observe(&mut self, reading_ms: u64) -> u64 {
        if reading_ms < self.last_ms {
            return self.last_ms;
        }
        self.last_ms = reading_ms;
        reading_ms
    }
}

impl Default for MonoClock {
    fn default() -> Self {
        Self::new()
    }
}

/// Age from a monotonic now. A missing sample is not zero.
pub fn age_ms(received: Option<u64>, now_ms: u64) -> f64 {
    match received {
        Some(last) => now_ms.saturating_sub(last) as f64,
        None => f64::MAX,
    }
}

/// Grant expiry on the same clock the tick log uses.
pub fn grant_expired(now_ms: u64, not_after_ms: u64) -> bool {
    now_ms > not_after_ms
}

#[cfg(test)]
mod tests {
    use super::{age_ms, grant_expired, MonoClock};

    #[test]
    fn wall_clock_jump_back_does_not_shrink_age() {
        let mut clock = MonoClock::new();
        let received = 1_000;
        let now = clock.observe(2_000);
        let before = age_ms(Some(received), now);
        let jumped = clock.observe(500);
        let after = age_ms(Some(received), jumped);
        assert!(after >= before);
        assert_eq!(after, before);
    }

    #[test]
    fn grant_expiry_uses_the_same_clock_as_the_tick_log() {
        let mut clock = MonoClock::new();
        let now = clock.observe(3_000);
        assert!(grant_expired(now, 2_500));
        let jumped = clock.observe(1_000);
        assert_eq!(jumped, now);
        assert!(grant_expired(jumped, 2_500));
        let doc = include_str!("../../../docs/time.md");
        assert!(doc.contains("same"));
        assert!(doc.contains("monotonic"));
    }
}
