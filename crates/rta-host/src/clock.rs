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
    }
}

/// Frozen offset between wall time and the monotonic clock, recorded once at startup.
/// A wall-clock step does not extend a grant. A missing wall clock refuses the grant path.
#[derive(Debug, Clone, Copy)]
pub struct GrantClock {
    /// wall_ms - mono_ms at startup. None if wall clock was unavailable.
    offset_ms: Option<i64>,
    /// Latched when a wall step exceeds the config bound.
    latched: bool,
    bound_ms: i64,
}

impl GrantClock {
    /// Record the offset. A negative offset or a missing wall clock refuses the grant path.
    pub fn new(wall_ms: Option<u64>, mono_ms: u64, bound_ms: i64) -> Self {
        let offset_ms = wall_ms.and_then(|wall| {
            let delta = wall as i64 - mono_ms as i64;
            if delta < 0 {
                None
            } else {
                Some(delta)
            }
        });
        Self {
            offset_ms,
            latched: false,
            bound_ms,
        }
    }

    /// Remaining grant time in ms, computed from the frozen offset.
    /// None if the grant path was refused at startup or latched.
    pub fn remaining_ms(
        &mut self,
        wall_ms: Option<u64>,
        mono_ms: u64,
        not_after_ms: u64,
    ) -> Option<i64> {
        if self.latched || self.offset_ms.is_none() {
            return None;
        }
        let wall = wall_ms?;
        let expected_wall = mono_ms as i64 + self.offset_ms.unwrap_or(0);
        let step = (wall as i64 - expected_wall).abs();
        if step > self.bound_ms {
            self.latched = true;
            return None;
        }
        Some(not_after_ms as i64 - (mono_ms as i64 + self.offset_ms.unwrap_or(0)))
    }

    pub fn grant_path_open(&self) -> bool {
        self.offset_ms.is_some() && !self.latched
    }
}

#[cfg(test)]
mod grant_clock_tests {
    use super::*;

    #[test]
    fn wall_jump_forward_does_not_lengthen_remaining() {
        let mut clock = GrantClock::new(Some(1_000), 0, 500);
        let first = clock.remaining_ms(Some(1_000), 0, 2_000);
        // Wall jumps forward 10s; mono advances 100ms. Remaining must not grow.
        let second = clock.remaining_ms(Some(11_000), 100, 2_000);
        assert!(first.unwrap() > second.unwrap_or(i64::MIN) || second.is_none());
    }

    #[test]
    fn wall_jump_back_does_not_revive_expired() {
        let mut clock = GrantClock::new(Some(5_000), 0, 500);
        // Expire the grant on mono time.
        let expired = clock.remaining_ms(Some(5_000), 3_000, 2_000);
        assert!(expired.unwrap_or(0) <= 0 || expired.is_none());
        // Wall jumps back; grant stays expired.
        let revived = clock.remaining_ms(Some(1_000), 3_100, 2_000);
        assert!(revived.unwrap_or(0) <= 0 || revived.is_none());
    }

    #[test]
    fn missing_wall_clock_refuses_grant_path() {
        let clock = GrantClock::new(None, 0, 500);
        assert!(!clock.grant_path_open());
    }

    #[test]
    fn negative_offset_refuses_grant_path() {
        let clock = GrantClock::new(Some(100), 1_000, 500);
        assert!(!clock.grant_path_open());
    }
}
