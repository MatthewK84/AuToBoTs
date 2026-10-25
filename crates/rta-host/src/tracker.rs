//! Tracker port. Confidence, range, and sample time. No video decode.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackSample {
    pub confidence: f64,
    pub range_m: f64,
    pub sample_time_ms: u64,
}

pub trait Tracker {
    fn sample(&mut self) -> Option<TrackSample>;
}

#[derive(Debug, Default)]
pub struct FakeTracker {
    script: Vec<TrackSample>,
}

impl FakeTracker {
    pub fn script(script: Vec<TrackSample>) -> Self {
        Self { script }
    }
}

impl Tracker for FakeTracker {
    fn sample(&mut self) -> Option<TrackSample> {
        if self.script.is_empty() {
            None
        } else {
            Some(self.script.remove(0))
        }
    }
}

#[derive(Debug, Default)]
pub struct PortGate {
    last_ms: Option<u64>,
}

impl PortGate {
    /// A sample is accepted only if it meets the port shalls. A step back is rejected.
    pub fn accept(&mut self, sample: TrackSample) -> Option<TrackSample> {
        let accepted = accept(sample)?;
        if self
            .last_ms
            .is_some_and(|last| accepted.sample_time_ms < last)
        {
            return None;
        }
        self.last_ms = Some(accepted.sample_time_ms);
        Some(accepted)
    }
}

/// The only crossing from a detector. No frame bytes cross this function.
pub fn accept(sample: TrackSample) -> Option<TrackSample> {
    if !(0.0..=1.0).contains(&sample.confidence) || sample.range_m < 0.0 {
        return None;
    }
    Some(sample)
}

pub fn weak_inside_commit(sample_time_ms: u64) -> TrackSample {
    TrackSample {
        confidence: 0.2,
        range_m: 40.0,
        sample_time_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_port_does_not_carry_a_frame() {
        let accepted = accept(TrackSample {
            confidence: 0.4,
            range_m: 12.0,
            sample_time_ms: 10,
        });
        assert!(accepted.is_some());
    }

    #[test]
    // trace: H5
    fn confidence_outside_range_is_rejected() {
        assert!(accept(TrackSample {
            confidence: 1.1,
            range_m: 1.0,
            sample_time_ms: 1
        })
        .is_none());
        assert!(accept(TrackSample {
            confidence: -0.1,
            range_m: 1.0,
            sample_time_ms: 1
        })
        .is_none());
    }

    #[test]
    fn negative_range_is_rejected() {
        assert!(accept(TrackSample {
            confidence: 0.5,
            range_m: -1.0,
            sample_time_ms: 1
        })
        .is_none());
    }

    #[test]
    fn a_step_back_in_sample_time_is_rejected() {
        let mut gate = PortGate::default();
        assert!(gate
            .accept(TrackSample {
                confidence: 0.5,
                range_m: 1.0,
                sample_time_ms: 10
            })
            .is_some());
        assert!(gate
            .accept(TrackSample {
                confidence: 0.5,
                range_m: 1.0,
                sample_time_ms: 9
            })
            .is_none());
        assert!(gate
            .accept(TrackSample {
                confidence: 0.5,
                range_m: 1.0,
                sample_time_ms: 11
            })
            .is_some());
    }

    #[test]
    fn fake_emits_a_weak_track_inside_commit_range() {
        let mut tracker = FakeTracker::script(vec![weak_inside_commit(10)]);
        let sample = tracker.sample().unwrap();
        assert!(sample.confidence < 0.40);
        assert!(sample.range_m < 150.0);
        assert_eq!(sample.sample_time_ms, 10);
        assert!(tracker.sample().is_none());
    }
}
