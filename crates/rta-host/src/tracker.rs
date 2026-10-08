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
    fn fake_emits_a_weak_track_inside_commit_range() {
        let mut tracker = FakeTracker::script(vec![weak_inside_commit(10)]);
        let sample = tracker.sample().unwrap();
        assert!(sample.confidence < 0.40);
        assert!(sample.range_m < 150.0);
        assert_eq!(sample.sample_time_ms, 10);
        assert!(tracker.sample().is_none());
    }

    #[test]
    fn crate_has_no_video_library() {
        let manifest = include_str!("../Cargo.toml");
        let video = ["g", "streamer"].concat();
        assert!(!manifest.contains(&video));
        assert!(!manifest.contains("opencv"));
        assert!(!manifest.contains("ffmpeg"));
    }
}
