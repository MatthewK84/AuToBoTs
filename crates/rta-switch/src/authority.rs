//! Software authority for commit. It does not close a circuit, arm a device, or release anything.

use crate::{decide, Request, SwitchCommand};
use rta_spec::{RecoveryMode, Verdict};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    pub issuer: String,
    pub not_before_ms: u64,
    pub not_after_ms: u64,
    pub fence_hash: String,
    pub max_range_m: u64,
    pub nonce: String,
    pub signature: String,
}

#[derive(Debug, Clone)]
pub struct AuthorityTick<'a> {
    pub verdict: Verdict,
    pub filter_rejected: bool,
    pub grant: Option<&'a Grant>,
    pub now_ms: u64,
    pub fence_hash: &'a str,
    pub range_m: u64,
    pub inside_fence: bool,
    pub revoked: &'a [String],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorityReason {
    NoGrant,
    Expired,
    FenceHash,
    Revoked,
    BadSignature,
    OutsideWindow,
    FilterRejected,
    NotPass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitDecision {
    Commit,
    Inhibit(AuthorityReason),
    Revert,
}

pub fn grant_bytes(grant: &Grant) -> String {
    format!(
        "issuer={}\nnot_before={}\nnot_after={}\nfence_hash={}\nmax_range={}\nnonce={}\n",
        grant.issuer,
        grant.not_before_ms,
        grant.not_after_ms,
        grant.fence_hash,
        grant.max_range_m,
        grant.nonce
    )
}

pub fn sign_grant(grant: &Grant) -> String {
    format!("{:016x}", pin(grant_bytes(grant).as_bytes()))
}

pub fn authorize(tick: &AuthorityTick<'_>) -> CommitDecision {
    if tick.verdict == Verdict::Revert {
        return CommitDecision::Revert;
    }
    if tick.verdict != Verdict::Pass {
        return CommitDecision::Inhibit(AuthorityReason::NotPass);
    }
    if tick.filter_rejected {
        return CommitDecision::Inhibit(AuthorityReason::FilterRejected);
    }
    let Some(grant) = tick.grant else {
        return CommitDecision::Inhibit(AuthorityReason::NoGrant);
    };
    if grant.signature.is_empty() || grant.signature != sign_grant(grant) {
        return CommitDecision::Inhibit(AuthorityReason::BadSignature);
    }
    if tick.now_ms < grant.not_before_ms || tick.now_ms > grant.not_after_ms {
        return CommitDecision::Inhibit(AuthorityReason::Expired);
    }
    if grant.fence_hash != tick.fence_hash {
        return CommitDecision::Inhibit(AuthorityReason::FenceHash);
    }
    if !tick.inside_fence || tick.range_m > grant.max_range_m {
        return CommitDecision::Inhibit(AuthorityReason::OutsideWindow);
    }
    if tick.revoked.iter().any(|nonce| nonce == &grant.nonce) {
        return CommitDecision::Inhibit(AuthorityReason::Revoked);
    }
    CommitDecision::Commit
}

pub fn decide_authorized(
    verdict: Verdict,
    request: Option<Request>,
    reported_mode: RecoveryMode,
    recovery: RecoveryMode,
    tick: &AuthorityTick<'_>,
) -> SwitchCommand {
    let command = decide(verdict, request, reported_mode, recovery);
    if authorize(tick) == CommitDecision::Commit {
        return command;
    }
    match command {
        SwitchCommand::Setpoints {
            north,
            east,
            down,
            yaw,
            ..
        } => SwitchCommand::Setpoints {
            north,
            east,
            down,
            yaw,
            commit: false,
        },
        other => other,
    }
}

fn pin(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grant() -> Grant {
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

    fn tick<'a>(grant: Option<&'a Grant>, verdict: Verdict) -> AuthorityTick<'a> {
        AuthorityTick {
            verdict,
            filter_rejected: false,
            grant,
            now_ms: 2_000,
            fence_hash: "fence-1",
            range_m: 40,
            inside_fence: true,
            revoked: &[],
        }
    }

    #[test]
    fn no_grant_is_inhibit() {
        assert_eq!(
            authorize(&tick(None, Verdict::Pass)),
            CommitDecision::Inhibit(AuthorityReason::NoGrant)
        );
    }

    #[test]
    fn expired_grant_is_inhibit() {
        let grant = grant();
        let mut tick = tick(Some(&grant), Verdict::Pass);
        tick.now_ms = 9_000;
        assert_eq!(
            authorize(&tick),
            CommitDecision::Inhibit(AuthorityReason::Expired)
        );
    }

    #[test]
    fn wrong_fence_hash_is_inhibit() {
        let grant = grant();
        let mut tick = tick(Some(&grant), Verdict::Pass);
        tick.fence_hash = "other";
        assert_eq!(
            authorize(&tick),
            CommitDecision::Inhibit(AuthorityReason::FenceHash)
        );
    }

    #[test]
    fn revoked_nonce_is_inhibit() {
        let grant = grant();
        let revoked = ["n-1".to_string()];
        let mut tick = tick(Some(&grant), Verdict::Pass);
        tick.revoked = &revoked;
        assert_eq!(
            authorize(&tick),
            CommitDecision::Inhibit(AuthorityReason::Revoked)
        );
    }

    #[test]
    fn a_bad_signature_is_inhibit_not_a_crash() {
        let mut grant = grant();
        grant.signature = "nope".into();
        assert_eq!(
            authorize(&tick(Some(&grant), Verdict::Pass)),
            CommitDecision::Inhibit(AuthorityReason::BadSignature)
        );
    }

    #[test]
    fn revert_beats_a_valid_grant() {
        let grant = grant();
        assert_eq!(
            authorize(&tick(Some(&grant), Verdict::Revert)),
            CommitDecision::Revert
        );
        let command = decide_authorized(
            Verdict::Revert,
            Some(Request {
                north: 1.0,
                east: 0.0,
                down: 0.0,
                yaw: 0.0,
                commit: true,
            }),
            RecoveryMode::Rtl,
            RecoveryMode::Loiter,
            &tick(Some(&grant), Verdict::Revert),
        );
        assert!(matches!(command, SwitchCommand::Mode { .. }));
    }

    #[test]
    fn a_valid_grant_in_window_may_commit() {
        let grant = grant();
        assert_eq!(
            authorize(&tick(Some(&grant), Verdict::Pass)),
            CommitDecision::Commit
        );
    }

    #[test]
    fn a_rejected_filter_inhibits_commit() {
        let grant = grant();
        let mut tick = tick(Some(&grant), Verdict::Pass);
        tick.filter_rejected = true;
        assert_eq!(
            authorize(&tick),
            CommitDecision::Inhibit(AuthorityReason::FilterRejected)
        );
    }

    #[test]
    fn commit_cannot_be_set_from_the_tracker_or_the_filter() {
        let filter = include_str!("../../rta-filter/src/lib.rs");
        let filter_cargo = include_str!("../../rta-filter/Cargo.toml");
        let tracker = include_str!("../../rta-host/src/tracker.rs");
        assert!(!filter.contains("commit"));
        assert!(!filter_cargo.contains("rta-switch"));
        assert!(!tracker.contains("Command::Commit"));
        assert!(!tracker.contains("commit: true"));
        let contract = include_str!("authority.rs")
            .lines()
            .next()
            .expect("contract");
        assert!(contract.contains("does not close a circuit"));
        assert!(!contract.contains("Command::Commit"));
    }
}
