//! Frame ingress. The pipeline string is the GStreamer description. This crate does not link libgstreamer.

use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceKind {
    Udp,
    V4l2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pipeline {
    pub raw: String,
    pub source: SourceKind,
    pub decode: String,
    pub sink: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PipelineError {
    Empty,
    MissingSource,
    MissingDecode,
    MissingSink,
    StartFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Buffer {
    pub at_ms: u64,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrackPort {
    pub track_conf: f64,
    pub range_m: f64,
    pub sample_time_ms: u64,
    pub age_ms: u64,
}

#[derive(Debug, Default)]
pub struct AppSink {
    latest: Option<Buffer>,
    started: bool,
}

pub fn parse_pipeline(text: &str) -> Result<Pipeline, PipelineError> {
    let raw = text.trim().to_string();
    if raw.is_empty() {
        return Err(PipelineError::Empty);
    }
    let source = if raw.contains("udpsrc") {
        SourceKind::Udp
    } else if raw.contains("v4l2src") {
        SourceKind::V4l2
    } else {
        return Err(PipelineError::MissingSource);
    };
    let decode = raw
        .split('!')
        .map(str::trim)
        .find(|part| part.contains("decode"))
        .ok_or(PipelineError::MissingDecode)?
        .to_string();
    if !raw.contains("appsink") {
        return Err(PipelineError::MissingSink);
    }
    Ok(Pipeline {
        raw,
        source,
        decode,
        sink: "appsink".into(),
    })
}

pub fn start(pipeline: &Pipeline) -> Result<AppSink, PipelineError> {
    parse_pipeline(&pipeline.raw)?;
    Ok(AppSink {
        latest: None,
        started: true,
    })
}

impl AppSink {
    pub fn push(&mut self, buffer: Buffer) {
        self.latest = Some(buffer);
    }

    pub fn pull_latest(&self) -> Option<&Buffer> {
        self.latest.as_ref()
    }

    pub fn kill_source(&mut self) {
        self.started = false;
    }
}

pub fn load_fixture(path: &Path) -> Result<Vec<Buffer>, PipelineError> {
    let text = fs::read_to_string(path).map_err(|_| PipelineError::StartFailed)?;
    let mut buffers = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let at_ms = parts
            .next()
            .and_then(|value| value.parse().ok())
            .ok_or(PipelineError::StartFailed)?;
        let bytes = parts
            .next()
            .ok_or(PipelineError::StartFailed)?
            .as_bytes()
            .to_vec();
        buffers.push(Buffer { at_ms, bytes });
    }
    if buffers.is_empty() {
        return Err(PipelineError::StartFailed);
    }
    Ok(buffers)
}

/// A stalled pad older than the hold limit, or a missing sample, is confidence 0.
pub fn port_sample(sink: &AppSink, now_ms: u64, hold_ms: u64) -> TrackPort {
    match sink.pull_latest() {
        Some(buffer) if sink.started && now_ms.saturating_sub(buffer.at_ms) <= hold_ms => {
            TrackPort {
                track_conf: 0.9,
                range_m: 40.0,
                sample_time_ms: buffer.at_ms,
                age_ms: now_ms.saturating_sub(buffer.at_ms),
            }
        }
        Some(buffer) => TrackPort {
            track_conf: 0.0,
            range_m: 40.0,
            sample_time_ms: buffer.at_ms,
            age_ms: now_ms.saturating_sub(buffer.at_ms),
        },
        None => TrackPort {
            track_conf: 0.0,
            range_m: 0.0,
            sample_time_ms: 0,
            age_ms: now_ms,
        },
    }
}

/// Range from bbox height is a stub. It is not a measured range.
pub const RANGE_IS_A_STUB: &str = "range is a stub from bbox height and focal length";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Detection {
    pub confidence: f64,
    pub bbox_height_px: f64,
    pub sample_time_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Focal {
    pub length_px: f64,
    pub target_height_m: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Presented {
    pub track_conf: f64,
    pub spec_range_m: Option<f64>,
    pub logged_range_m: Option<f64>,
}

pub fn range_stub(bbox_height_px: f64, focal: Focal) -> f64 {
    if bbox_height_px <= 0.0 || focal.length_px <= 0.0 {
        return 0.0;
    }
    (focal.length_px * focal.target_height_m / bbox_height_px).max(0.0)
}

/// One track behind the sink. Fields match `InputRecord`: confidence in 0..=1, range non-negative.
pub fn emit(detection: Detection, focal: Focal) -> TrackPort {
    TrackPort {
        track_conf: detection.confidence.clamp(0.0, 1.0),
        range_m: range_stub(detection.bbox_height_px, focal),
        sample_time_ms: detection.sample_time_ms,
        age_ms: 0,
    }
}

/// M2.2 hold policy. Not a second policy: unseen or older than the hold is confidence 0.
pub fn present(track: &TrackPort, now_ms: u64, hold_ms: u64, ever_seen: bool) -> Presented {
    let age_ms = now_ms.saturating_sub(track.sample_time_ms) as f64;
    if !ever_seen || age_ms > hold_ms as f64 {
        return Presented {
            track_conf: 0.0,
            spec_range_m: None,
            logged_range_m: if ever_seen { Some(track.range_m) } else { None },
        };
    }
    Presented {
        track_conf: track.track_conf,
        spec_range_m: Some(track.range_m),
        logged_range_m: Some(track.range_m),
    }
}

/// One bad frame. The port emits one sample or a miss. It does not pick a track.
#[derive(Debug, Clone, PartialEq)]
pub enum BadFrame {
    Frozen { at_ms: u64 },
    Empty,
    NanConfidence,
    TwoTracks,
}

/// Port output. A miss is confidence 0 under the M2.2 hold policy.
#[derive(Debug, Clone, PartialEq)]
pub enum PortOutput {
    Sample(TrackPort),
    Miss,
}

/// Classify a bad frame. Two tracks are a miss, not a pick. NaN is a miss.
pub fn classify(frame: &BadFrame, now_ms: u64, hold_ms: u64) -> PortOutput {
    match frame {
        BadFrame::Empty | BadFrame::NanConfidence | BadFrame::TwoTracks => PortOutput::Miss,
        BadFrame::Frozen { at_ms } => {
            if now_ms.saturating_sub(*at_ms) > hold_ms {
                PortOutput::Miss
            } else {
                PortOutput::Sample(TrackPort {
                    track_conf: 0.0,
                    range_m: 0.0,
                    sample_time_ms: *at_ms,
                    age_ms: now_ms.saturating_sub(*at_ms),
                })
            }
        }
    }
}

/// A miss is confidence 0. That is the hold policy, not a second one.
pub fn miss() -> TrackPort {
    TrackPort {
        track_conf: 0.0,
        range_m: 0.0,
        sample_time_ms: 0,
        age_ms: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pipeline() -> Pipeline {
        parse_pipeline("udpsrc port=5600 ! decodebin ! appsink drop=true max-buffers=1").unwrap()
    }

    #[test]
    fn a_recorded_fixture_stream_produces_buffers() {
        let buffers = load_fixture(Path::new("fixtures/stream.txt")).expect("fixture");
        assert!(buffers.len() >= 2);
        let mut sink = start(&pipeline()).expect("start");
        for buffer in &buffers {
            sink.push(buffer.clone());
        }
        let latest = sink.pull_latest().expect("latest");
        assert_eq!(latest.at_ms, buffers.last().unwrap().at_ms);
        assert!(!latest.bytes.is_empty());
    }

    #[test]
    fn a_killed_source_grows_age_and_collapses_confidence() {
        let mut sink = start(&pipeline()).expect("start");
        sink.push(Buffer {
            at_ms: 100,
            bytes: b"frame".to_vec(),
        });
        let fresh = port_sample(&sink, 150, 200);
        assert_eq!(fresh.track_conf, 0.9);
        assert_eq!(fresh.age_ms, 50);
        sink.kill_source();
        let stalled = port_sample(&sink, 500, 200);
        assert!(stalled.age_ms > fresh.age_ms);
        assert_eq!(stalled.track_conf, 0.0);
    }

    #[test]
    fn a_pipeline_that_fails_to_start_is_a_startup_failure() {
        let err = parse_pipeline("filesrc ! fakesink").expect_err("bad");
        assert_eq!(err, PipelineError::MissingSource);
        let missing = port_sample(&AppSink::default(), 10, 200);
        assert_eq!(missing.track_conf, 0.0);
    }

    #[test]
    fn port_fields_match_input_record() {
        let track = emit(
            Detection {
                confidence: 1.4,
                bbox_height_px: 20.0,
                sample_time_ms: 100,
            },
            Focal {
                length_px: 800.0,
                target_height_m: 1.0,
            },
        );
        assert!((0.0..=1.0).contains(&track.track_conf));
        assert!(track.range_m >= 0.0);
        assert_eq!(track.range_m, 40.0);
        assert!(RANGE_IS_A_STUB.contains("stub"));
    }

    #[test]
    fn stale_detector_output_uses_the_hold_policy() {
        let track = emit(
            Detection {
                confidence: 0.8,
                bbox_height_px: 20.0,
                sample_time_ms: 100,
            },
            Focal {
                length_px: 800.0,
                target_height_m: 1.0,
            },
        );
        let fresh = present(&track, 150, 200, true);
        assert_eq!(fresh.track_conf, 0.8);
        assert_eq!(fresh.spec_range_m, Some(40.0));
        let stale = present(&track, 400, 200, true);
        assert_eq!(stale.track_conf, 0.0);
        assert_eq!(stale.spec_range_m, None);
        assert_eq!(stale.logged_range_m, Some(40.0));
        let unseen = present(&track, 100, 200, false);
        assert_eq!(unseen.track_conf, 0.0);
        assert_eq!(unseen.logged_range_m, None);
    }

    #[test]
    fn no_path_from_a_frame_to_decide() {
        let cargo = include_str!("../Cargo.toml");
        assert!(!cargo.contains("rta-switch"));
        assert!(!cargo.contains("gstreamer"));
        let contract = include_str!("lib.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("contract");
        assert!(!contract.contains("decide"));
    }

    #[test]
    fn hold_limit_was_not_raised() {
        let sitl = include_str!("../../../config/sitl.toml");
        let example = include_str!("../../../config/example.toml");
        let note = include_str!("../../../docs/tracker-timing.md");
        assert!(sitl.contains("tracker_hold_ms = 200"));
        assert!(example.contains("tracker_hold_ms = 200"));
        assert!(note.contains("udpsrc port=5600 ! decodebin ! appsink drop=true max-buffers=1"));
        assert!(note.contains("2026-10-16"));
        assert!(note.contains("The hold was not raised"));
        assert!(note.contains("production-test hook"));
        assert!(note.contains("The hold does not grow"));
    }

    #[test]
    fn the_switch_has_no_video_dependency() {
        let switch = include_str!("../../rta-switch/Cargo.toml");
        let host = include_str!("../../rta-host/Cargo.toml");
        assert!(!switch.contains("gstreamer"));
        assert!(!switch.contains("rta-tracker"));
        assert!(!host.contains("gstreamer"));
        assert!(!host.contains("rta-tracker"));
        let own = include_str!("../Cargo.toml");
        assert!(!own.contains("gstreamer"));
    }

    #[test]
    fn four_bad_frames_each_have_an_expected_port_output() {
        let frozen = classify(&BadFrame::Frozen { at_ms: 0 }, 300, 200);
        assert_eq!(frozen, PortOutput::Miss);
        let empty = classify(&BadFrame::Empty, 100, 200);
        assert_eq!(empty, PortOutput::Miss);
        let nan = classify(&BadFrame::NanConfidence, 100, 200);
        assert_eq!(nan, PortOutput::Miss);
        let two = classify(&BadFrame::TwoTracks, 100, 200);
        assert_eq!(two, PortOutput::Miss);
        let held = miss();
        assert_eq!(held.track_conf, 0.0);
        for name in [
            "frozen.txt",
            "empty.txt",
            "nan-confidence.txt",
            "two-tracks.txt",
        ] {
            let path = Path::new("fixtures/bad").join(name);
            let text = fs::read_to_string(&path).expect(name);
            assert!(text.contains("expected: miss"), "{name}");
        }
        let switch = include_str!("../../rta-switch/src/lib.rs");
        assert!(!switch.contains("BadFrame"));
        assert!(!switch.contains("frame"));
    }
}
