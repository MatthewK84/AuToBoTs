//! Tick-log reason codes. A tick builds a fixed-size [`Reasons`] value and the log renders
//! it, comma-separated, exactly as the old `Vec<String>` was joined. Nothing here allocates.
//!
//! Spec reasons are the names of the spec's Boolean outputs. [`intern`] turns each name into
//! a `&'static str` once, when a monitor is built; a name already seen is not stored again,
//! so the table is bounded by the distinct output names of the specs loaded.

use crate::recovery::{AckResult, State, Step};
use std::fmt;
use std::sync::Mutex;

/// One reason code. `Display` is the text in the tick log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// A spec output that was true this tick, by name (`fix_stale`, `weak_track`, ...).
    Spec(&'static str),
    /// The evaluator latched an interpreter fault.
    Fault,
    /// The evaluation worker was busy, gone, or had faulted.
    EvalFault,
    /// The input record constructor rejected the sample.
    Record,
    /// The answer missed the deadline.
    Late,
    ReadyTimeout,
    NotReadyAtArm,
    Disarmed,
    NotReady,
    Latched,
    NoFc,
    NoRequest,
    NotSent,
    /// A setpoint whose position or yaw is not finite as an `f32`. Not forwarded.
    BadSetpoint,
    /// A recovery step went on the wire this tick.
    Sent(Step),
    /// The recovery latch state, when engaged.
    Recovery(State),
    Attempts(u32),
    Step(Step),
    Ack(AckResult),
    Fallback(AckResult),
    /// More reasons than [`REASON_CAPACITY`]. Takes the last slot.
    Truncated,
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Reason::Spec(name) => f.write_str(name),
            Reason::Fault => f.write_str("fault"),
            Reason::EvalFault => f.write_str("eval_fault"),
            Reason::Record => f.write_str("record"),
            Reason::Late => f.write_str("late"),
            Reason::ReadyTimeout => f.write_str("ready_timeout"),
            Reason::NotReadyAtArm => f.write_str("not_ready_at_arm"),
            Reason::Disarmed => f.write_str("disarmed"),
            Reason::NotReady => f.write_str("not_ready"),
            Reason::Latched => f.write_str("latched"),
            Reason::NoFc => f.write_str("no_fc"),
            Reason::NoRequest => f.write_str("no_request"),
            Reason::NotSent => f.write_str("not_sent"),
            Reason::BadSetpoint => f.write_str("bad_setpoint"),
            Reason::Sent(step) => write!(f, "sent:{}", step.name()),
            Reason::Recovery(state) => f.write_str(match state {
                State::Idle => "recovery_idle",
                State::Pending => "recovery_pending",
                State::Confirmed => "recovery_confirmed",
                State::Unconfirmed => "recovery_unconfirmed",
            }),
            Reason::Attempts(count) => write!(f, "attempts:{count}"),
            Reason::Step(step) => write!(f, "step:{}", step.name()),
            Reason::Ack(result) => write!(f, "ack:{result}"),
            Reason::Fallback(result) => write!(f, "fallback:{result}"),
            Reason::Truncated => f.write_str("truncated"),
        }
    }
}

/// Reason slots per tick. A tick can name at most about 17 today: five spec outputs, the
/// evaluation and gate codes, `latched`, one send, and five recovery codes.
pub const REASON_CAPACITY: usize = 24;

/// The reasons of one tick, in the order they were pushed. Fixed size and `Copy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reasons {
    items: [Reason; REASON_CAPACITY],
    len: usize,
}

impl Default for Reasons {
    fn default() -> Self {
        Self::new()
    }
}

impl Reasons {
    pub const fn new() -> Self {
        Self {
            items: [Reason::Truncated; REASON_CAPACITY],
            len: 0,
        }
    }

    /// Append one reason. The last slot is kept for [`Reason::Truncated`]: once the others are
    /// full, the next push writes it and later pushes are dropped.
    pub fn push(&mut self, reason: Reason) {
        let item = if self.len + 1 < REASON_CAPACITY {
            reason
        } else if self.len + 1 == REASON_CAPACITY {
            Reason::Truncated
        } else {
            return;
        };
        if let Some(slot) = self.items.get_mut(self.len) {
            *slot = item;
            self.len += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn as_slice(&self) -> &[Reason] {
        self.items.get(..self.len).unwrap_or_default()
    }

    pub fn contains(&self, reason: Reason) -> bool {
        self.as_slice().contains(&reason)
    }

    /// The rendered codes, for tests and tools. Allocates; not for the tick path.
    pub fn codes(&self) -> Vec<String> {
        self.as_slice().iter().map(ToString::to_string).collect()
    }
}

impl fmt::Display for Reasons {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, reason) in self.as_slice().iter().enumerate() {
            if index > 0 {
                f.write_str(",")?;
            }
            write!(f, "{reason}")?;
        }
        Ok(())
    }
}

static NAMES: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

/// A `&'static str` for a spec output name. Called when a monitor is built, never per tick.
/// Each distinct name is leaked once for the life of the process.
pub fn intern(name: &str) -> &'static str {
    let mut names = match NAMES.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(found) = names.iter().find(|known| **known == name) {
        return found;
    }
    let leaked: &'static str = Box::leak(name.to_owned().into_boxed_str());
    names.push(leaked);
    leaked
}

#[cfg(test)]
mod tests {
    use super::*;
    use rta_spec::RecoveryMode;

    #[test]
    fn rendering_matches_the_old_joined_strings() {
        let mut reasons = Reasons::new();
        assert_eq!(reasons.to_string(), "");
        for reason in [
            Reason::Spec(intern("fix_stale")),
            Reason::EvalFault,
            Reason::Record,
            Reason::Late,
            Reason::ReadyTimeout,
            Reason::NotReadyAtArm,
            Reason::Disarmed,
            Reason::NotReady,
            Reason::Latched,
            Reason::NoFc,
            Reason::NoRequest,
            Reason::NotSent,
            Reason::BadSetpoint,
            Reason::Sent(Step::SetMode(RecoveryMode::Rtl)),
            Reason::Recovery(State::Pending),
            Reason::Attempts(3),
            Reason::Step(Step::ReturnToLaunch),
            Reason::Ack(AckResult::Other(9)),
            Reason::Fallback(AckResult::Denied),
            Reason::Fault,
        ] {
            reasons.push(reason);
        }
        assert_eq!(
            reasons.to_string(),
            "fix_stale,eval_fault,record,late,ready_timeout,not_ready_at_arm,disarmed,not_ready,\
             latched,no_fc,no_request,not_sent,bad_setpoint,sent:set_mode,recovery_pending,attempts:3,\
             step:nav_rtl,ack:result_9,fallback:denied,fault"
        );
    }

    #[test]
    fn overflow_ends_in_one_truncated_marker() {
        let mut reasons = Reasons::new();
        for _ in 0..REASON_CAPACITY * 2 {
            reasons.push(Reason::Late);
        }
        assert_eq!(reasons.len(), REASON_CAPACITY);
        assert_eq!(reasons.as_slice().last(), Some(&Reason::Truncated));
        assert_eq!(
            reasons
                .as_slice()
                .iter()
                .filter(|r| **r == Reason::Truncated)
                .count(),
            1
        );
    }

    #[test]
    fn a_name_is_interned_once() {
        let first = intern("weak_track");
        let again = intern(&String::from("weak_track"));
        assert!(std::ptr::eq(first, again));
    }
}
