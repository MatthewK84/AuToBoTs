//! Link age is local time since the last command-path message. Status text does not count.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkMessage {
    Command,
    StatusText,
}

#[derive(Debug, Clone, Copy)]
pub struct LinkAge {
    last_command_ms: Option<u64>,
}

impl LinkAge {
    pub fn new() -> Self {
        Self {
            last_command_ms: None,
        }
    }

    pub fn observe(&mut self, message: LinkMessage, now_ms: u64) {
        if message == LinkMessage::Command {
            self.last_command_ms = Some(now_ms);
        }
    }

    pub fn age_ms(&self, now_ms: u64) -> f64 {
        match self.last_command_ms {
            Some(last) => now_ms.saturating_sub(last) as f64,
            None => f64::MAX,
        }
    }
}

impl Default for LinkAge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silent_gap_over_the_spec_limit_is_stale() {
        let age = LinkAge::new();
        assert!(age.age_ms(400) > 300.0);
    }

    #[test]
    fn fresh_message_clears_the_gap() {
        let mut age = LinkAge::new();
        age.observe(LinkMessage::StatusText, 100);
        assert!(age.age_ms(400) > 300.0);
        age.observe(LinkMessage::Command, 350);
        assert!(age.age_ms(400) < 300.0);
    }
}
