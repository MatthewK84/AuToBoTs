//! Host aging. The spec still decides the verdict.

#[derive(Debug, Clone, PartialEq)]
pub struct PresentedTrack {
    pub track_conf: f64,
    pub spec_range_m: Option<f64>,
    pub logged_range_m: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PresentedFix {
    pub fix_age_ms: f64,
    pub fresh: bool,
}

pub fn present_track(
    ever_seen: bool,
    age_ms: f64,
    conf: f64,
    range_m: f64,
    hold_ms: u64,
) -> PresentedTrack {
    if !ever_seen || age_ms > hold_ms as f64 {
        return PresentedTrack {
            track_conf: 0.0,
            spec_range_m: None,
            logged_range_m: if ever_seen { Some(range_m) } else { None },
        };
    }
    PresentedTrack {
        track_conf: conf,
        spec_range_m: Some(range_m),
        logged_range_m: Some(range_m),
    }
}

pub fn present_fix(last_accept_age_ms: Option<f64>, now_age_ms: f64) -> PresentedFix {
    match last_accept_age_ms {
        Some(accepted) => PresentedFix {
            fix_age_ms: now_age_ms.max(accepted),
            fresh: false,
        },
        None => PresentedFix {
            fix_age_ms: f64::MAX,
            fresh: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_tracker_sample_ever_presents_zero_confidence() {
        let presented = present_track(false, 0.0, 0.9, 40.0, 200);
        assert_eq!(presented.track_conf, 0.0);
        assert_eq!(presented.spec_range_m, None);
    }

    #[test]
    fn position_gap_grows_age_and_is_not_fresh() {
        let first = present_fix(None, 0.0);
        assert!(first.fix_age_ms > 1_000_000.0);
        assert!(!first.fresh);
        let later = present_fix(Some(100.0), 350.0);
        assert_eq!(later.fix_age_ms, 350.0);
        assert!(!later.fresh);
    }
}
