//! Shadow proposal selection. Neither network constructs a command.

use rta_spec::Verdict;
use rta_switch::{Complex, Request};

pub const SHADOW_TAKEOVER: &str = "shadow_takeover";

#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    pub active: Option<Request>,
    pub shadow: Option<Request>,
    pub reason: Option<&'static str>,
    pub revert: bool,
}

#[derive(Debug, Clone)]
pub struct ShadowSelector {
    silent_ticks: u32,
    threshold: u32,
}

impl ShadowSelector {
    pub fn new(threshold: u32) -> Self {
        Self {
            silent_ticks: 0,
            threshold: threshold.max(1),
        }
    }

    /// Primary is filtered and switched. Shadow is returned every tick so the host can log it.
    /// Shadow becomes the active proposal only after `threshold` ticks without a finite primary,
    /// and only while the verdict is not `Revert`. Both silent is `Revert`.
    pub fn select(
        &mut self,
        verdict: Verdict,
        primary: &mut dyn Complex,
        shadow: &mut dyn Complex,
    ) -> Selection {
        let primary_req = finite(primary.request());
        let shadow_req = finite(shadow.request());
        if verdict == Verdict::Revert {
            self.silent_ticks = 0;
            return Selection {
                active: None,
                shadow: None,
                reason: None,
                revert: true,
            };
        }
        if primary_req.is_some() {
            self.silent_ticks = 0;
            return Selection {
                active: primary_req,
                shadow: shadow_req,
                reason: None,
                revert: false,
            };
        }
        self.silent_ticks = self.silent_ticks.saturating_add(1);
        if shadow_req.is_none() {
            return Selection {
                active: None,
                shadow: None,
                reason: None,
                revert: true,
            };
        }
        if self.silent_ticks >= self.threshold {
            return Selection {
                active: shadow_req,
                shadow: shadow_req,
                reason: Some(SHADOW_TAKEOVER),
                revert: false,
            };
        }
        Selection {
            active: None,
            shadow: shadow_req,
            reason: None,
            revert: false,
        }
    }
}

fn finite(request: Option<Request>) -> Option<Request> {
    request.filter(|item| {
        item.north.is_finite()
            && item.east.is_finite()
            && item.down.is_finite()
            && item.yaw.is_finite()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(Option<Request>);

    impl Complex for Fixed {
        fn request(&mut self) -> Option<Request> {
            self.0
        }
    }

    fn point(north: f64) -> Request {
        Request {
            north,
            east: 0.0,
            down: 10.0,
            yaw: 0.0,
            commit: false,
        }
    }

    #[test]
    fn neither_network_constructs_a_command() {
        let source = include_str!("shadow.rs");
        let command = ["Switch", "Command"].concat();
        let decide = ["fn ", "decide"].concat();
        let outbound = ["Command", "::"].concat();
        assert!(!source.contains(&command));
        assert!(!source.contains(&decide));
        assert!(!source.contains(&outbound));
        let primary = Fixed(Some(point(1.0)));
        let shadow = Fixed(Some(point(2.0)));
        let _ = (primary.0, shadow.0);
    }

    #[test]
    fn revert_drops_both_proposals() {
        let mut selector = ShadowSelector::new(1);
        let mut primary = Fixed(Some(point(1.0)));
        let mut shadow = Fixed(Some(point(2.0)));
        let got = selector.select(Verdict::Revert, &mut primary, &mut shadow);
        assert!(got.active.is_none());
        assert!(got.shadow.is_none());
        assert!(got.revert);
        assert!(got.reason.is_none());
    }

    #[test]
    fn shadow_takeover_is_the_logged_reason() {
        let mut selector = ShadowSelector::new(2);
        let mut primary = Fixed(None);
        let mut shadow = Fixed(Some(point(4.0)));
        let first = selector.select(Verdict::Pass, &mut primary, &mut shadow);
        assert!(first.reason.is_none());
        assert!(first.active.is_none());
        assert!(!first.revert);
        let second = selector.select(Verdict::Inhibit, &mut primary, &mut shadow);
        assert_eq!(second.reason, Some(SHADOW_TAKEOVER));
        assert_eq!(second.active.map(|item| item.north), Some(4.0));
        assert!(!second.revert);
    }

    #[test]
    fn both_silent_is_revert() {
        let mut selector = ShadowSelector::new(3);
        let mut primary = Fixed(None);
        let mut shadow = Fixed(None);
        let got = selector.select(Verdict::Pass, &mut primary, &mut shadow);
        assert!(got.revert);
        assert!(got.active.is_none());
    }

    #[test]
    fn finite_primary_keeps_the_shadow_logged_and_inactive() {
        let mut selector = ShadowSelector::new(1);
        let mut primary = Fixed(Some(point(1.0)));
        let mut shadow = Fixed(Some(point(9.0)));
        let got = selector.select(Verdict::Pass, &mut primary, &mut shadow);
        assert_eq!(got.active.map(|item| item.north), Some(1.0));
        assert_eq!(got.shadow.map(|item| item.north), Some(9.0));
        assert!(got.reason.is_none());
        assert!(!got.revert);
    }
}
