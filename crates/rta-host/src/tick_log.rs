//! Append-only tick log. Open failure is fatal. A write failure latches revert.
//!
//! A line is formatted into a fixed buffer owned by the log and written with one
//! `write_all`. Appending allocates nothing. A line longer than [`LINE_CAPACITY`] is a write
//! error, which latches Revert like any other.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use crate::reason::Reasons;
use rta_spec::Verdict;
use rta_switch::SwitchCommand;

pub const VERSION: u32 = 1;

/// Bytes in the fixed line buffer. The worst-case line, every float the longest `f64`
/// rendering and every reason slot full, is about 2.6 KB (`the_longest_line_fits`).
pub const LINE_CAPACITY: usize = 4_096;

#[derive(Debug, Clone, PartialEq)]
pub struct TickRecord {
    pub tick: u64,
    pub fence_ok: bool,
    pub fix_age_ms: f64,
    pub link_age_ms: f64,
    pub fc_heartbeat_age_ms: f64,
    pub track_conf: f64,
    pub range_m: f64,
    pub verdict: Verdict,
    pub filter_intervened: bool,
    pub reasons: Reasons,
    pub command: &'static str,
    pub eval_ms: u64,
    pub spec_hash: u64,
    pub alt_m: f64,
    pub indication: String,
}

pub struct TickLog<W: Write> {
    writer: W,
    every: u32,
    since_flush: u32,
    line: [u8; LINE_CAPACITY],
}

impl TickLog<File> {
    pub fn open(path: &Path, every: u32, spec_hash: u64) -> io::Result<Self> {
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "# rta-log {VERSION} spec_hash={spec_hash:016x}")?;
        file.flush()?;
        Ok(Self {
            writer: file,
            every,
            since_flush: 0,
            line: [0; LINE_CAPACITY],
        })
    }
}

impl<W: Write> TickLog<W> {
    #[cfg(test)]
    pub fn from_writer(writer: W, every: u32) -> Self {
        Self {
            writer,
            every,
            since_flush: 0,
            line: [0; LINE_CAPACITY],
        }
    }

    pub fn append(&mut self, record: &TickRecord) -> io::Result<()> {
        let mut cursor = io::Cursor::new(&mut self.line[..]);
        writeln!(
            cursor,
            "tick={} fence_ok={} fix_age_ms={} link_age_ms={} fc_heartbeat_age_ms={} track_conf={} range_m={} verdict={} filter_intervened={} reasons={} command={} eval_ms={} spec_hash={:016x} indication={}",
            record.tick,
            record.fence_ok as u8,
            record.fix_age_ms,
            record.link_age_ms,
            record.fc_heartbeat_age_ms,
            record.track_conf,
            record.range_m,
            verdict_name(record.verdict),
            record.filter_intervened as u8,
            record.reasons,
            record.command,
            record.eval_ms,
            record.spec_hash,
            record.indication,
        )?;
        let written = usize::try_from(cursor.position()).unwrap_or(LINE_CAPACITY);
        let line = self
            .line
            .get(..written)
            .ok_or_else(|| io::Error::other("tick line longer than buffer"))?;
        self.writer.write_all(line)?;
        self.since_flush += 1;
        if self.since_flush >= self.every {
            self.writer.flush()?;
            self.since_flush = 0;
        }
        Ok(())
    }
}

impl<W: Write> TickLog<W> {
    /// Flush whatever `flush_every_n` has not. Shutdown calls this after the recovery mode.
    pub fn flush(&mut self) -> io::Result<()> {
        self.since_flush = 0;
        self.writer.flush()
    }
}

pub fn command_name(command: &SwitchCommand) -> &'static str {
    match command {
        SwitchCommand::Mode { .. } => "mode",
        SwitchCommand::Setpoints { .. } => "setpoints",
        SwitchCommand::Idle => "idle",
    }
}

pub fn on_write_error() -> Verdict {
    Verdict::Revert
}

pub fn spec_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn verdict_name(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Pass => "pass",
        Verdict::Inhibit => "inhibit",
        Verdict::Revert => "revert",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reason::{Reason, REASON_CAPACITY};
    struct FailWrite;

    impl Write for FailWrite {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("injected"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn sample() -> TickRecord {
        TickRecord {
            tick: 1,
            fence_ok: true,
            fix_age_ms: 10.0,
            link_age_ms: 20.0,
            fc_heartbeat_age_ms: 30.0,
            track_conf: 0.2,
            range_m: 40.0,
            verdict: Verdict::Inhibit,
            filter_intervened: true,
            reasons: {
                let mut reasons = Reasons::new();
                reasons.push(Reason::Spec("weak_track"));
                reasons
            },
            command: "idle",
            eval_ms: 1,
            spec_hash: 7,
            alt_m: 0.0,
            indication: "indication=commit_inhibited reason=weak_track".into(),
        }
    }

    #[test]
    fn three_sink_cases_are_asserted_from_the_log() {
        use rta_switch::authority::{
            indication, indication_for_tick, indication_line, AuthorityReason, CommitDecision,
        };
        let cases = [
            indication(CommitDecision::Inhibit(AuthorityReason::NoGrant)),
            indication_for_tick(true, CommitDecision::Commit),
            indication(CommitDecision::Inhibit(AuthorityReason::Revoked)),
        ];
        let mut buf = Vec::new();
        let mut log = TickLog::from_writer(&mut buf, 1);
        for (tick, shown) in cases.iter().enumerate() {
            let mut record = sample();
            record.tick = tick as u64;
            record.indication = indication_line(*shown);
            log.append(&record).unwrap();
        }
        let text = String::from_utf8(buf).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].contains("indication=commit_inhibited reason=no_grant"));
        assert!(!lines[0].contains("indication=reverted"));
        assert!(lines[1].contains("indication=reverted"));
        assert!(!lines[1].contains("commit_armed"));
        assert!(lines[2].contains("indication=commit_inhibited reason=grant_revoked"));
        assert!(!lines[2].contains("indication=commit_armed"));
    }

    #[test]
    fn write_error_latches_revert() {
        let mut log = TickLog::from_writer(FailWrite, 1);
        assert!(log.append(&sample()).is_err());
        assert_eq!(on_write_error(), Verdict::Revert);
    }

    #[test]
    fn unwritable_path_fails_open() {
        assert!(TickLog::open(Path::new("/no/such/dir/rta.log"), 1, 0).is_err());
    }

    #[test]
    fn header_is_versioned_and_record_keeps_the_fields() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("rta-log-test-{}.log", std::process::id()));
        let mut log = TickLog::open(&path, 1, 0xabc).unwrap();
        log.append(&sample()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(text.starts_with("# rta-log 1 spec_hash="));
        assert!(text.contains("verdict=inhibit"));
        assert!(text.contains("reasons=weak_track"));
        assert!(text.contains("command=idle"));
    }

    #[test]
    fn intervention_sits_beside_the_verdict() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("rta-log-filter-{}.log", std::process::id()));
        let mut log = TickLog::open(&path, 1, 1).unwrap();
        log.append(&sample()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(text.contains("verdict=inhibit filter_intervened=1"));
    }

    #[test]
    fn the_line_is_byte_identical_to_the_old_format() {
        let mut record = sample();
        record.reasons.push(Reason::Latched);
        let mut buf = Vec::new();
        let mut log = TickLog::from_writer(&mut buf, 1);
        log.append(&record).unwrap();
        let old = format!(
            "tick={} fence_ok={} fix_age_ms={} link_age_ms={} fc_heartbeat_age_ms={} track_conf={} range_m={} verdict={} filter_intervened={} reasons={} command={} eval_ms={} spec_hash={:016x} indication={}\n",
            record.tick,
            record.fence_ok as u8,
            record.fix_age_ms,
            record.link_age_ms,
            record.fc_heartbeat_age_ms,
            record.track_conf,
            record.range_m,
            "inhibit",
            record.filter_intervened as u8,
            ["weak_track", "latched"].join(","),
            record.command,
            record.eval_ms,
            record.spec_hash,
            record.indication,
        );
        assert_eq!(String::from_utf8(buf).unwrap(), old);
    }

    #[test]
    fn the_longest_line_fits() {
        let mut record = sample();
        record.tick = u64::MAX;
        // The longest `Display` of an f64 is the smallest subnormal, written out in full.
        let longest = -f64::from_bits(1);
        assert!(f64::MAX.to_string().len() < longest.to_string().len());
        record.fix_age_ms = longest;
        record.link_age_ms = longest;
        record.fc_heartbeat_age_ms = longest;
        record.track_conf = longest;
        record.range_m = longest;
        record.eval_ms = u64::MAX;
        record.spec_hash = u64::MAX;
        record.indication = "indication=commit_inhibited reason=grant_revoked".into();
        record.reasons = Reasons::new();
        for _ in 0..REASON_CAPACITY {
            record.reasons.push(Reason::Fallback(
                crate::recovery::AckResult::TemporarilyRejected,
            ));
        }
        let mut buf = Vec::new();
        let mut log = TickLog::from_writer(&mut buf, 1);
        log.append(&record).unwrap();
        // About 2.6 KB today; keep at least 1 KB of headroom.
        assert!(buf.len() + 1_024 < LINE_CAPACITY, "{}", buf.len());
        assert!(buf.ends_with(b"\n"));
    }

    #[test]
    fn a_line_longer_than_the_buffer_is_a_write_error() {
        let mut record = sample();
        record.indication = "x".repeat(LINE_CAPACITY);
        let mut buf = Vec::new();
        let mut log = TickLog::from_writer(&mut buf, 1);
        assert!(log.append(&record).is_err());
        assert!(buf.is_empty(), "a partial line was written");
    }
}
