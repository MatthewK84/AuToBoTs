//! Operator path. A grant is a parsed link message. A mode change is not a grant.

use crate::mavlink_link::Inbound;
use mavlink::dialects::common::MavMessage;
use rta_switch::authority::{
    authorize, indication, indication_line, AuthorityTick, CommitDecision, Grant,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorMessage {
    Grant(Grant),
    Withdraw { nonce: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkEvent {
    Operator(OperatorMessage),
    ModeChange { custom_mode: u32 },
    Other,
}

#[derive(Debug, Clone, Default)]
pub struct OperatorPath {
    pub grant: Option<Grant>,
    pub revoked: Vec<String>,
    pub log: Vec<String>,
    pub in_flight: Option<String>,
}

pub fn parse_operator(text: &str) -> Option<OperatorMessage> {
    let text = text.trim();
    if let Some(rest) = text.strip_prefix("WITHDRAW ") {
        let nonce = field(rest, "nonce")?;
        return Some(OperatorMessage::Withdraw { nonce });
    }
    if let Some(rest) = text.strip_prefix("GRANT ") {
        let grant = Grant {
            issuer: field(rest, "issuer")?,
            not_before_ms: field(rest, "not_before")?.parse().ok()?,
            not_after_ms: field(rest, "not_after")?.parse().ok()?,
            fence_hash: field(rest, "fence_hash")?,
            max_range_m: field(rest, "max_range")?.parse().ok()?,
            nonce: field(rest, "nonce")?,
            signature: field(rest, "sig")?,
        };
        return Some(OperatorMessage::Grant(grant));
    }
    None
}

pub fn event_from_text(text: &str) -> LinkEvent {
    match parse_operator(text) {
        Some(message) => LinkEvent::Operator(message),
        None => LinkEvent::Other,
    }
}

#[allow(deprecated)]
pub fn event_from_mav(message: &MavMessage) -> LinkEvent {
    match message {
        MavMessage::SET_MODE(mode) => LinkEvent::ModeChange {
            custom_mode: mode.custom_mode,
        },
        MavMessage::HEARTBEAT(heartbeat) => LinkEvent::ModeChange {
            custom_mode: heartbeat.custom_mode,
        },
        MavMessage::STATUSTEXT(_) => LinkEvent::Other,
        _ => LinkEvent::Other,
    }
}

pub fn apply(path: &mut OperatorPath, event: LinkEvent) {
    match event {
        LinkEvent::Operator(OperatorMessage::Grant(grant)) => path.grant = Some(grant),
        LinkEvent::Operator(OperatorMessage::Withdraw { nonce }) => {
            if !path.revoked.contains(&nonce) {
                path.revoked.push(nonce);
            }
        }
        LinkEvent::ModeChange { .. } | LinkEvent::Other => {}
    }
}

pub fn hand_to_socket(path: &mut OperatorPath, command: &str) {
    path.in_flight = Some(command.to_string());
    path.log.push(format!("in flight {command}"));
}

pub fn tick<'a>(path: &'a OperatorPath, sample: AuthorityTick<'a>) -> CommitDecision {
    let mut sample = sample;
    sample.grant = path.grant.as_ref();
    sample.revoked = &path.revoked;
    let decision = authorize(&sample);
    path_log(path, decision);
    decision
}

fn path_log(path: &OperatorPath, decision: CommitDecision) {
    let _ = (path, indication_line(indication(decision)));
}

pub fn log_indication(path: &mut OperatorPath, decision: CommitDecision) {
    path.log.push(indication_line(indication(decision)));
}

pub fn inbound_is_grant(inbound: &Inbound) -> bool {
    matches!(inbound, Inbound::StatusText(text) if parse_operator(text).is_some())
}

fn field(text: &str, name: &str) -> Option<String> {
    let prefix = format!("{name}=");
    text.split_whitespace()
        .find_map(|part| part.strip_prefix(&prefix).map(str::to_string))
}

#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;
    use mavlink::dialects::common::{MavMode, SET_MODE_DATA};
    use rta_spec::Verdict;
    use rta_switch::authority::{sign_grant, AuthorityReason};

    fn signed() -> Grant {
        let mut grant = Grant {
            issuer: "operator".into(),
            not_before_ms: 1_000,
            not_after_ms: 5_000,
            fence_hash: "fence-1".into(),
            max_range_m: 150,
            nonce: "n-1".into(),
            signature: String::new(),
        };
        grant.signature = sign_grant(&grant);
        grant
    }

    fn line(grant: &Grant) -> String {
        format!(
            "GRANT issuer={} not_before={} not_after={} fence_hash={} max_range={} nonce={} sig={}",
            grant.issuer,
            grant.not_before_ms,
            grant.not_after_ms,
            grant.fence_hash,
            grant.max_range_m,
            grant.nonce,
            grant.signature
        )
    }

    fn sample() -> AuthorityTick<'static> {
        AuthorityTick {
            verdict: Verdict::Pass,
            filter_rejected: false,
            grant: None,
            now_ms: 2_000,
            fence_hash: "fence-1",
            range_m: 40,
            inside_fence: true,
            revoked: &[],
        }
    }

    #[test]
    fn withdraw_mid_window_inhibits_the_following_tick() {
        let grant = signed();
        let mut path = OperatorPath::default();
        apply(&mut path, event_from_text(&line(&grant)));
        let first = {
            let decision = authorize(&AuthorityTick {
                grant: path.grant.as_ref(),
                revoked: &path.revoked,
                ..sample()
            });
            log_indication(&mut path, decision);
            decision
        };
        assert_eq!(first, CommitDecision::Commit);
        apply(&mut path, event_from_text("WITHDRAW nonce=n-1"));
        let second = authorize(&AuthorityTick {
            grant: path.grant.as_ref(),
            revoked: &path.revoked,
            ..sample()
        });
        log_indication(&mut path, second);
        assert_eq!(second, CommitDecision::Inhibit(AuthorityReason::Revoked));
        assert!(path.log[1].contains("commit_inhibited"));
        assert!(path.log[1].contains("grant_revoked"));
    }

    #[test]
    #[allow(deprecated)]
    fn withdraw_during_eval_inhibits_the_following_tick_and_leaves_the_sent_command() {
        let grant = signed();
        let mut path = OperatorPath::default();
        apply(&mut path, event_from_text(&line(&grant)));
        let first = authorize(&AuthorityTick {
            grant: path.grant.as_ref(),
            revoked: &path.revoked,
            ..sample()
        });
        assert_eq!(first, CommitDecision::Commit);
        hand_to_socket(&mut path, "commit");
        apply(&mut path, event_from_text("WITHDRAW nonce=n-1"));
        assert_eq!(path.in_flight.as_deref(), Some("commit"));
        let second = authorize(&AuthorityTick {
            grant: path.grant.as_ref(),
            revoked: &path.revoked,
            ..sample()
        });
        assert_eq!(second, CommitDecision::Inhibit(AuthorityReason::Revoked));
        assert!(path
            .log
            .iter()
            .any(|line| line.contains("in flight commit")));
        let timing = include_str!("../../../docs/timing.md");
        assert!(timing.contains("100 ms"));
        assert!(timing.contains("assumption"));
    }

    #[test]
    #[allow(deprecated)]
    fn a_mode_change_is_not_a_grant() {
        let heartbeat = MavMessage::HEARTBEAT(mavlink::dialects::common::HEARTBEAT_DATA {
            custom_mode: 6,
            ..Default::default()
        });
        let mode = MavMessage::SET_MODE(SET_MODE_DATA {
            custom_mode: 4,
            target_system: 1,
            base_mode: MavMode::MAV_MODE_PREFLIGHT,
        });
        let mut path = OperatorPath::default();
        apply(&mut path, event_from_mav(&heartbeat));
        apply(&mut path, event_from_mav(&mode));
        assert!(path.grant.is_none());
        assert!(path.revoked.is_empty());
        assert!(matches!(
            event_from_mav(&mode),
            LinkEvent::ModeChange { custom_mode: 4 }
        ));
    }

    #[test]
    fn a_grant_line_parses_into_the_token() {
        let grant = signed();
        let parsed = parse_operator(&line(&grant)).expect("grant");
        assert_eq!(parsed, OperatorMessage::Grant(grant));
    }
}
