//! Recovery retry. One latch, shared by the tick thread and the watchdog.
//!
//! Once engaged the latch never clears before process restart. It resends the recovery
//! command on a bounded backoff until the flight controller reports the mode in `HEARTBEAT`,
//! reads `COMMAND_ACK` for the command it sent, and falls back down a fixed order on
//! `DENIED` or `UNSUPPORTED`. When the order is used up without a confirmed mode it stops
//! sending and stays latched. Detail in `docs/recovery.md`.
//!
//! The latch is plain state. It allocates nothing and owns no socket. The caller holds the
//! one lock around `poll`, the send, and `sent`, so the tick thread and the watchdog cannot
//! both send the same attempt.

use mavlink::dialects::common::{MavCmd, MavMessage, MavModeFlag, COMMAND_LONG_DATA};
use rta_spec::RecoveryMode;

use crate::command_out::{px4_mode, Target};
use crate::reason::{Reason, Reasons};

/// Resend schedule. The wait after attempt `n` is `initial_ms * 2^(n-1)`, capped at `cap_ms`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    pub initial_ms: u64,
    pub cap_ms: u64,
    pub attempts_per_step: u32,
    /// After an `ACCEPTED` or `IN_PROGRESS` ack, the next resend waits at least this long.
    /// The flight controller reports its mode only in `HEARTBEAT`, which PX4 sends at 1 Hz,
    /// so a wait shorter than one heartbeat period resends a command that already took. The
    /// runtime sets it to the spec's `heartbeat_stale` limit: no heartbeat in that window is
    /// a stale link, which reverts on its own.
    pub ack_hold_ms: u64,
}

impl Policy {
    /// Wait after the `attempt`-th send, counting from one.
    pub fn wait_after(&self, attempt: u32) -> u64 {
        let doublings = attempt.saturating_sub(1).min(32);
        self.initial_ms
            .saturating_mul(1_u64 << doublings)
            .min(self.cap_ms)
    }
}

/// One way of asking for recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// `MAV_CMD_DO_SET_MODE` with the PX4 AUTO sub-mode.
    SetMode(RecoveryMode),
    /// `MAV_CMD_NAV_RETURN_TO_LAUNCH`. PX4 answers it with AUTO_RTL.
    ReturnToLaunch,
    /// `MAV_CMD_NAV_LAND` at the current position. PX4 answers it with AUTO_LAND.
    Land,
}

impl Step {
    /// The reported mode that confirms this step.
    pub fn expects(self) -> RecoveryMode {
        match self {
            Step::SetMode(mode) => mode,
            Step::ReturnToLaunch => RecoveryMode::Rtl,
            Step::Land => RecoveryMode::Land,
        }
    }

    pub fn command(self) -> MavCmd {
        match self {
            Step::SetMode(_) => MavCmd::MAV_CMD_DO_SET_MODE,
            Step::ReturnToLaunch => MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH,
            Step::Land => MavCmd::MAV_CMD_NAV_LAND,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Step::SetMode(_) => "set_mode",
            Step::ReturnToLaunch => "nav_rtl",
            Step::Land => "nav_land",
        }
    }

    /// The `COMMAND_LONG` for this step. `confirmation` counts resends, as MAVLink asks.
    /// Addressed to the flight controller's heard system and component.
    pub fn message(self, target: Target, confirmation: u8) -> MavMessage {
        let (param1, param2, param3, rest) = match self {
            Step::SetMode(mode) => {
                let px4 = px4_mode(mode);
                (
                    f32::from(MavModeFlag::MAV_MODE_FLAG_CUSTOM_MODE_ENABLED.bits()),
                    f32::from(px4.main),
                    f32::from(px4.sub),
                    0.0,
                )
            }
            Step::ReturnToLaunch => (0.0, 0.0, 0.0, 0.0),
            // NaN yaw, latitude, longitude, and altitude: land here, heading unchanged.
            Step::Land => (0.0, 0.0, 0.0, f32::NAN),
        };
        MavMessage::COMMAND_LONG(COMMAND_LONG_DATA {
            param1,
            param2,
            param3,
            param4: rest,
            param5: rest,
            param6: rest,
            param7: rest,
            command: self.command(),
            target_system: target.system,
            target_component: target.component,
            confirmation,
        })
    }
}

const ORDER_RTL: [Step; 3] = [
    Step::SetMode(RecoveryMode::Rtl),
    Step::ReturnToLaunch,
    Step::Land,
];
const ORDER_LOITER: [Step; 3] = [
    Step::SetMode(RecoveryMode::Loiter),
    Step::ReturnToLaunch,
    Step::Land,
];
const ORDER_LAND: [Step; 2] = [Step::SetMode(RecoveryMode::Land), Step::Land];

/// The documented fallback order for each configured recovery mode.
pub fn fallback_order(recovery: RecoveryMode) -> &'static [Step] {
    match recovery {
        RecoveryMode::Rtl => &ORDER_RTL,
        RecoveryMode::Loiter => &ORDER_LOITER,
        RecoveryMode::Land => &ORDER_LAND,
    }
}

/// `COMMAND_ACK.result`, read from the raw byte so a value the dialect crate lacks is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AckResult {
    Accepted,
    TemporarilyRejected,
    Denied,
    Unsupported,
    Failed,
    InProgress,
    Cancelled,
    Other(u8),
}

impl AckResult {
    pub fn from_raw(value: u8) -> Self {
        match value {
            0 => Self::Accepted,
            1 => Self::TemporarilyRejected,
            2 => Self::Denied,
            3 => Self::Unsupported,
            4 => Self::Failed,
            5 => Self::InProgress,
            6 => Self::Cancelled,
            other => Self::Other(other),
        }
    }

    /// Reason code in the tick log. Allocates; the tick path uses `Display`.
    pub fn code(self) -> String {
        self.to_string()
    }
}

impl std::fmt::Display for AckResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Accepted => f.write_str("accepted"),
            Self::TemporarilyRejected => f.write_str("temporarily_rejected"),
            Self::Denied => f.write_str("denied"),
            Self::Unsupported => f.write_str("unsupported"),
            Self::Failed => f.write_str("failed"),
            Self::InProgress => f.write_str("in_progress"),
            Self::Cancelled => f.write_str("cancelled"),
            Self::Other(value) => write!(f, "result_{value}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Not engaged. Nothing has been asked for.
    Idle,
    /// Engaged and not yet seen in `HEARTBEAT`.
    Pending,
    /// The flight controller reported a mode the latch asked for.
    Confirmed,
    /// The fallback order is used up without a confirmed mode. Nothing more is sent.
    Unconfirmed,
}

/// The recovery-in-progress latch.
#[derive(Debug, Clone)]
pub struct Latch {
    policy: Policy,
    order: &'static [Step],
    step: usize,
    engaged: bool,
    confirmed: bool,
    exhausted: bool,
    attempts: u32,
    step_attempts: u32,
    next_due_ms: u64,
    last_ack: Option<AckResult>,
    fallback: Option<AckResult>,
}

impl Latch {
    pub fn new(recovery: RecoveryMode, policy: Policy) -> Self {
        Self {
            policy,
            order: fallback_order(recovery),
            step: 0,
            engaged: false,
            confirmed: false,
            exhausted: false,
            attempts: 0,
            step_attempts: 0,
            next_due_ms: 0,
            last_ack: None,
            fallback: None,
        }
    }

    /// Engage. The first attempt is due at once. A second call changes nothing.
    pub fn engage(&mut self, now_ms: u64) {
        if !self.engaged {
            self.engaged = true;
            self.next_due_ms = now_ms;
        }
    }

    pub fn engaged(&self) -> bool {
        self.engaged
    }

    pub fn attempts(&self) -> u32 {
        self.attempts
    }

    pub fn state(&self) -> State {
        if !self.engaged {
            State::Idle
        } else if self.confirmed {
            State::Confirmed
        } else if self.exhausted {
            State::Unconfirmed
        } else {
            State::Pending
        }
    }

    fn current(&self) -> Option<Step> {
        self.order.get(self.step).copied()
    }

    /// The step due now, if any. The caller sends it and then calls `sent`.
    pub fn poll(&mut self, now_ms: u64, reported: Option<RecoveryMode>) -> Option<Step> {
        if !self.engaged || self.confirmed {
            return None;
        }
        // Any mode asked for so far confirms, including one that lands after a fallback.
        let asked = self.order.get(..=self.step).unwrap_or(self.order);
        if reported.is_some_and(|mode| asked.iter().any(|step| step.expects() == mode)) {
            self.confirmed = true;
            return None;
        }
        if self.exhausted || now_ms < self.next_due_ms {
            return None;
        }
        if self.step_attempts >= self.policy.attempts_per_step {
            self.advance(None);
            if self.exhausted {
                return None;
            }
        }
        self.current()
    }

    /// Record the outcome of the send `poll` asked for. A local send error does not count as
    /// an attempt; the step stays due.
    pub fn sent(&mut self, now_ms: u64, ok: bool) {
        if !ok {
            return;
        }
        self.attempts = self.attempts.saturating_add(1);
        self.step_attempts = self.step_attempts.saturating_add(1);
        self.next_due_ms = now_ms.saturating_add(self.policy.wait_after(self.attempts));
    }

    /// `confirmation` for the next `COMMAND_LONG` of this step.
    pub fn confirmation(&self) -> u8 {
        u8::try_from(self.step_attempts).unwrap_or(u8::MAX)
    }

    /// Apply one `COMMAND_ACK`. An ack for a command other than the one in flight is stale
    /// and changes nothing.
    pub fn on_ack(&mut self, now_ms: u64, command: u16, result: AckResult) {
        if !self.engaged || self.confirmed || self.exhausted || self.step_attempts == 0 {
            return;
        }
        let Some(step) = self.current() else {
            return;
        };
        if step.command() as u16 != command {
            return;
        }
        self.last_ack = Some(result);
        match result {
            // Accepted or still running: give the vehicle a full wait to report the mode.
            AckResult::Accepted | AckResult::InProgress => {
                let wait = self
                    .policy
                    .wait_after(self.attempts)
                    .max(self.policy.ack_hold_ms);
                self.next_due_ms = self.next_due_ms.max(now_ms.saturating_add(wait));
            }
            // Refused for good: the next step in the order is due at once.
            AckResult::Denied | AckResult::Unsupported => {
                self.advance(Some(result));
                self.next_due_ms = now_ms;
            }
            // Refused for now, failed, or cancelled: resend on the schedule.
            AckResult::TemporarilyRejected
            | AckResult::Failed
            | AckResult::Cancelled
            | AckResult::Other(_) => {}
        }
    }

    fn advance(&mut self, reason: Option<AckResult>) {
        if self.step + 1 < self.order.len() {
            self.step += 1;
            self.step_attempts = 0;
            if reason.is_some() {
                self.fallback = reason;
            }
        } else {
            self.exhausted = true;
        }
    }

    /// Reason codes for the tick log. Empty while idle. Allocates nothing.
    pub fn reasons(&self, out: &mut Reasons) {
        let state = self.state();
        if state == State::Idle {
            return;
        }
        out.push(Reason::Recovery(state));
        out.push(Reason::Attempts(self.attempts));
        if let Some(step) = self.current() {
            out.push(Reason::Step(step));
        }
        if let Some(ack) = self.last_ack {
            out.push(Reason::Ack(ack));
        }
        if let Some(reason) = self.fallback {
            out.push(Reason::Fallback(reason));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const POLICY: Policy = Policy {
        initial_ms: 300,
        cap_ms: 1_000,
        attempts_per_step: 3,
        ack_hold_ms: 0,
    };

    /// Poll every millisecond from `from` to `to` and send whatever is due.
    fn drive(
        latch: &mut Latch,
        from: u64,
        to: u64,
        reported: Option<RecoveryMode>,
    ) -> Vec<(u64, Step)> {
        let mut sends = Vec::new();
        for now in from..=to {
            if let Some(step) = latch.poll(now, reported) {
                latch.sent(now, true);
                sends.push((now, step));
            }
        }
        sends
    }

    #[test]
    fn the_wait_doubles_then_caps() {
        let waits: Vec<u64> = (1..=6).map(|n| POLICY.wait_after(n)).collect();
        assert_eq!(waits, [300, 600, 1_000, 1_000, 1_000, 1_000]);
        assert_eq!(POLICY.wait_after(u32::MAX), 1_000);
    }

    #[test]
    fn idle_sends_nothing_and_logs_nothing() {
        let mut latch = Latch::new(RecoveryMode::Rtl, POLICY);
        assert!(drive(&mut latch, 0, 5_000, None).is_empty());
        let mut reasons = Reasons::new();
        latch.reasons(&mut reasons);
        let reasons = reasons.codes();
        assert!(reasons.is_empty());
    }

    #[test]
    fn a_vehicle_that_never_switches_gets_the_whole_order_on_the_schedule_then_silence() {
        let mut latch = Latch::new(RecoveryMode::Rtl, POLICY);
        latch.engage(0);
        let sends = drive(&mut latch, 0, 60_000, None);
        let times: Vec<u64> = sends.iter().map(|(at, _)| *at).collect();
        let steps: Vec<Step> = sends.iter().map(|(_, step)| *step).collect();
        assert_eq!(
            times,
            [0, 300, 900, 1_900, 2_900, 3_900, 4_900, 5_900, 6_900]
        );
        assert_eq!(
            steps,
            [
                Step::SetMode(RecoveryMode::Rtl),
                Step::SetMode(RecoveryMode::Rtl),
                Step::SetMode(RecoveryMode::Rtl),
                Step::ReturnToLaunch,
                Step::ReturnToLaunch,
                Step::ReturnToLaunch,
                Step::Land,
                Step::Land,
                Step::Land,
            ]
        );
        assert_eq!(latch.state(), State::Unconfirmed);
        let mut reasons = Reasons::new();
        latch.reasons(&mut reasons);
        let reasons = reasons.codes();
        assert!(reasons.contains(&"recovery_unconfirmed".to_string()));
        assert!(reasons.contains(&"attempts:9".to_string()));
    }

    #[test]
    fn a_reported_mode_confirms_and_stops_the_resends() {
        let mut latch = Latch::new(RecoveryMode::Rtl, POLICY);
        latch.engage(0);
        assert_eq!(drive(&mut latch, 0, 400, None).len(), 2);
        assert!(drive(&mut latch, 401, 60_000, Some(RecoveryMode::Rtl)).is_empty());
        assert_eq!(latch.state(), State::Confirmed);
        // A later report of another mode does not restart the resends.
        assert!(drive(&mut latch, 60_001, 70_000, None).is_empty());
    }

    #[test]
    fn a_mode_not_asked_for_does_not_confirm() {
        let mut latch = Latch::new(RecoveryMode::Rtl, POLICY);
        latch.engage(0);
        assert_eq!(
            drive(&mut latch, 0, 1_000, Some(RecoveryMode::Land)).len(),
            3
        );
        assert_eq!(latch.state(), State::Pending);
    }

    #[test]
    fn already_in_the_recovery_mode_sends_nothing() {
        let mut latch = Latch::new(RecoveryMode::Land, POLICY);
        latch.engage(0);
        assert!(drive(&mut latch, 0, 10_000, Some(RecoveryMode::Land)).is_empty());
        assert_eq!(latch.state(), State::Confirmed);
    }

    #[test]
    fn engage_twice_does_not_reset_the_schedule() {
        let mut latch = Latch::new(RecoveryMode::Rtl, POLICY);
        latch.engage(0);
        assert_eq!(drive(&mut latch, 0, 10, None).len(), 1);
        latch.engage(11);
        assert!(drive(&mut latch, 11, 299, None).is_empty());
    }

    #[test]
    fn a_local_send_error_is_not_an_attempt_and_stays_due() {
        let mut latch = Latch::new(RecoveryMode::Rtl, POLICY);
        latch.engage(0);
        assert!(latch.poll(0, None).is_some());
        latch.sent(0, false);
        assert_eq!(latch.attempts(), 0);
        assert!(latch.poll(1, None).is_some());
        latch.sent(1, true);
        assert_eq!(latch.attempts(), 1);
        assert!(latch.poll(2, None).is_none());
    }

    fn sent_once(recovery: RecoveryMode) -> Latch {
        let mut latch = Latch::new(recovery, POLICY);
        latch.engage(0);
        assert!(latch.poll(0, None).is_some());
        latch.sent(0, true);
        latch
    }

    const SET_MODE: u16 = MavCmd::MAV_CMD_DO_SET_MODE as u16;

    #[test]
    fn denied_and_unsupported_fall_back_at_once() {
        for result in [AckResult::Denied, AckResult::Unsupported] {
            let mut latch = sent_once(RecoveryMode::Rtl);
            latch.on_ack(20, SET_MODE, result);
            assert_eq!(
                latch.poll(20, None),
                Some(Step::ReturnToLaunch),
                "{result:?}"
            );
            latch.sent(20, true);
            let mut reasons = Reasons::new();
            latch.reasons(&mut reasons);
            let reasons = reasons.codes();
            assert!(reasons.contains(&format!("fallback:{}", result.code())));
            assert!(reasons.contains(&"step:nav_rtl".to_string()));
            // The fallback is confirmed by the mode it asks for.
            assert!(latch.poll(5_000, Some(RecoveryMode::Rtl)).is_none());
            assert_eq!(latch.state(), State::Confirmed);
        }
    }

    #[test]
    fn denied_on_the_last_step_stops_and_stays_latched() {
        let mut latch = sent_once(RecoveryMode::Land);
        latch.on_ack(10, SET_MODE, AckResult::Denied);
        assert_eq!(latch.poll(10, None), Some(Step::Land));
        latch.sent(10, true);
        latch.on_ack(20, MavCmd::MAV_CMD_NAV_LAND as u16, AckResult::Denied);
        assert_eq!(latch.state(), State::Unconfirmed);
        assert!(latch.poll(60_000, None).is_none());
        assert!(latch.engaged());
    }

    #[test]
    fn accepted_and_in_progress_hold_the_resend_for_a_full_wait() {
        for result in [AckResult::Accepted, AckResult::InProgress] {
            let mut latch = sent_once(RecoveryMode::Rtl);
            latch.on_ack(250, SET_MODE, result);
            assert!(latch.poll(300, None).is_none(), "{result:?}");
            assert!(latch.poll(549, None).is_none(), "{result:?}");
            assert_eq!(
                latch.poll(550, None),
                Some(Step::SetMode(RecoveryMode::Rtl))
            );
        }
    }

    #[test]
    fn an_accepting_ack_holds_the_resend_for_the_heartbeat_window_when_that_is_longer() {
        // PX4 reports a mode change only in its 1 Hz HEARTBEAT. The hold covers one beat.
        let policy = Policy {
            ack_hold_ms: 1_500,
            ..POLICY
        };
        for result in [AckResult::Accepted, AckResult::InProgress] {
            let mut latch = Latch::new(RecoveryMode::Rtl, policy);
            latch.engage(0);
            assert!(latch.poll(0, None).is_some());
            latch.sent(0, true);
            latch.on_ack(250, SET_MODE, result);
            assert!(latch.poll(1_749, None).is_none(), "{result:?}");
            assert_eq!(
                latch.poll(1_750, None),
                Some(Step::SetMode(RecoveryMode::Rtl))
            );
            // A heartbeat that shows the mode inside the hold confirms with no resend.
            let mut latch = Latch::new(RecoveryMode::Rtl, policy);
            latch.engage(0);
            assert!(latch.poll(0, None).is_some());
            latch.sent(0, true);
            latch.on_ack(5, SET_MODE, result);
            assert!(latch.poll(1_000, Some(RecoveryMode::Rtl)).is_none());
            assert_eq!(latch.state(), State::Confirmed);
        }
        // A refusal is not held: it still resends on the schedule.
        let mut latch = Latch::new(RecoveryMode::Rtl, policy);
        latch.engage(0);
        assert!(latch.poll(0, None).is_some());
        latch.sent(0, true);
        latch.on_ack(5, SET_MODE, AckResult::TemporarilyRejected);
        assert!(latch.poll(300, None).is_some());
    }

    #[test]
    fn temporarily_rejected_failed_and_cancelled_resend_on_the_schedule() {
        for result in [
            AckResult::TemporarilyRejected,
            AckResult::Failed,
            AckResult::Cancelled,
            AckResult::Other(9),
        ] {
            let mut latch = sent_once(RecoveryMode::Rtl);
            latch.on_ack(10, SET_MODE, result);
            assert!(latch.poll(299, None).is_none(), "{result:?}");
            assert_eq!(
                latch.poll(300, None),
                Some(Step::SetMode(RecoveryMode::Rtl))
            );
            let mut reasons = Reasons::new();
            latch.reasons(&mut reasons);
            let reasons = reasons.codes();
            assert!(reasons.contains(&format!("ack:{}", result.code())));
            assert!(!reasons.iter().any(|r| r.starts_with("fallback:")));
        }
    }

    #[test]
    fn a_stale_ack_for_another_command_changes_nothing() {
        let mut latch = sent_once(RecoveryMode::Rtl);
        latch.on_ack(10, MavCmd::MAV_CMD_NAV_LAND as u16, AckResult::Denied);
        assert_eq!(
            latch.poll(300, None),
            Some(Step::SetMode(RecoveryMode::Rtl))
        );
        let mut idle = Latch::new(RecoveryMode::Rtl, POLICY);
        idle.on_ack(10, SET_MODE, AckResult::Denied);
        idle.engage(20);
        assert_eq!(idle.poll(20, None), Some(Step::SetMode(RecoveryMode::Rtl)));
    }

    #[test]
    fn the_raw_result_codes_are_distinct() {
        let codes: Vec<String> = (0..=7).map(|v| AckResult::from_raw(v).code()).collect();
        assert_eq!(
            codes,
            [
                "accepted",
                "temporarily_rejected",
                "denied",
                "unsupported",
                "failed",
                "in_progress",
                "cancelled",
                "result_7"
            ]
        );
    }

    #[test]
    fn the_fallback_order_ends_on_a_land_and_never_names_offboard() {
        for recovery in [RecoveryMode::Rtl, RecoveryMode::Loiter, RecoveryMode::Land] {
            let order = fallback_order(recovery);
            assert_eq!(order[0], Step::SetMode(recovery));
            assert_eq!(order.last(), Some(&Step::Land));
            for step in order {
                if let MavMessage::COMMAND_LONG(data) = step.message(
                    Target {
                        system: 1,
                        component: 1,
                    },
                    0,
                ) {
                    assert_eq!(data.command, step.command());
                    if let Step::SetMode(_) = step {
                        assert_eq!(data.param2, 4.0);
                    }
                }
            }
        }
    }
}
