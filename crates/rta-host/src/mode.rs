//! Recovery is sent once. The flag clears only on process restart.

use rta_spec::{RecoveryMode, Verdict};
use rta_switch::{decide, Request, SwitchCommand};

#[derive(Debug, Default)]
pub struct ModeSender {
    recovery_sent: bool,
}

impl ModeSender {
    pub fn send(
        &mut self,
        verdict: Verdict,
        request: Option<Request>,
        recovery: RecoveryMode,
        sink: &mut Vec<String>,
    ) -> SwitchCommand {
        let command = decide(verdict, request, recovery);
        if let SwitchCommand::Mode { recovery } = command {
            if !self.recovery_sent {
                sink.push(format!("mode {recovery:?}"));
                self.recovery_sent = true;
            }
        }
        command
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_revert_ticks_send_one_mode() {
        let mut sender = ModeSender::default();
        let mut sink = Vec::new();
        sender.send(Verdict::Revert, None, RecoveryMode::Rtl, &mut sink);
        sender.send(Verdict::Revert, None, RecoveryMode::Rtl, &mut sink);
        assert_eq!(sink.len(), 1);
    }

    #[test]
    fn no_public_clear() {
        let source = include_str!("mode.rs");
        let name = ["clear", "_recovery"].concat();
        assert!(!source.contains(&name));
    }
}
