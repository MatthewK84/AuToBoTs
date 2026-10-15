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
}
