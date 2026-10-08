//! Append-only tick log. Open failure is fatal. A write failure latches revert.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use rta_spec::Verdict;
use rta_switch::SwitchCommand;

pub const VERSION: u32 = 1;

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
    pub reasons: Vec<String>,
    pub command: String,
    pub eval_ms: u64,
    pub spec_hash: u64,
}

pub struct TickLog<W: Write> {
    writer: W,
    every: u32,
    since_flush: u32,
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
        }
    }

    pub fn append(&mut self, record: &TickRecord) -> io::Result<()> {
        let reasons = record.reasons.join(",");
        writeln!(
            self.writer,
            "tick={} fence_ok={} fix_age_ms={} link_age_ms={} fc_heartbeat_age_ms={} track_conf={} range_m={} verdict={} reasons={} command={} eval_ms={} spec_hash={:016x}",
            record.tick,
            record.fence_ok as u8,
            record.fix_age_ms,
            record.link_age_ms,
            record.fc_heartbeat_age_ms,
            record.track_conf,
            record.range_m,
            verdict_name(record.verdict),
            reasons,
            record.command,
            record.eval_ms,
            record.spec_hash,
        )?;
        self.since_flush += 1;
        if self.since_flush >= self.every {
            self.writer.flush()?;
            self.since_flush = 0;
        }
        Ok(())
    }
}

pub fn command_name(command: &SwitchCommand) -> String {
    match command {
        SwitchCommand::Mode { .. } => "mode".into(),
        SwitchCommand::Setpoints { .. } => "setpoints".into(),
        SwitchCommand::Idle => "idle".into(),
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
            reasons: vec!["weak_track".into()],
            command: "idle".into(),
            eval_ms: 1,
            spec_hash: 7,
        }
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
}
