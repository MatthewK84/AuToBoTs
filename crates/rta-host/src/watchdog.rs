//! Watchdog. It reads the published tick only. It does not call the interpreter.

use crate::mode::ModeSender;
use rta_spec::{RecoveryMode, Verdict};

#[derive(Debug)]
pub struct Watchdog {
    last_published_ms: u64,
    period_ms: u64,
    misses: u32,
}

impl Watchdog {
    pub fn new(period_ms: u64, misses: u32) -> Self {
        Self {
            last_published_ms: 0,
            period_ms,
            misses,
        }
    }

    pub fn publish(&mut self, now_ms: u64) {
        self.last_published_ms = now_ms;
    }

    pub fn check(
        &self,
        now_ms: u64,
        reported: RecoveryMode,
        recovery: RecoveryMode,
        sender: &mut ModeSender,
        sink: &mut Vec<String>,
    ) {
        let window = self.period_ms.saturating_mul(self.misses as u64);
        if now_ms.saturating_sub(self.last_published_ms) >= window {
            sender.send(Verdict::Revert, None, reported, recovery, sink);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paused_eval_sends_mode_inside_the_miss_window() {
        let mut dog = Watchdog::new(50, 3);
        dog.publish(0);
        let mut sender = ModeSender::default();
        let mut sink = Vec::new();
        dog.check(
            149,
            RecoveryMode::Loiter,
            RecoveryMode::Rtl,
            &mut sender,
            &mut sink,
        );
        assert!(sink.is_empty());
        dog.check(
            150,
            RecoveryMode::Loiter,
            RecoveryMode::Rtl,
            &mut sender,
            &mut sink,
        );
        assert_eq!(sink.len(), 1);
    }

    #[test]
    fn watchdog_does_not_name_the_interpreter() {
        let source = include_str!("watchdog.rs");
        let interpreter = ["rt", "lola"].concat();
        let monitor = ["Mon", "itor"].concat();
        assert!(!source.contains(&interpreter));
        assert!(!source.contains(&monitor));
    }
}
