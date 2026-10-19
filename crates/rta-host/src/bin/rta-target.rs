//! Eval timing measurement with an explicit, truthful evidence label. Does not change the
//! deadline.
//!
//! The line says where it ran (`measured_on`) and what kind of evidence it is (`evidence`):
//! `simulated` (qemu user mode, or a host run scaled by a slowdown model), `host-standin` (a
//! native host run, unscaled), or `real-target` (only when the operator says so on the
//! board, unscaled and built for aarch64). Nothing in this binary names a board it did not
//! run on. How to run it on a real Jetson is in `docs/target-timing.md`.

#![cfg_attr(
    not(test),
    deny(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::panic,
        clippy::unreachable,
        clippy::todo,
        clippy::unimplemented
    )
)]

use rta_host::config::parse_str;
use rta_host::eval::Evaluator;
use rta_host::mode::ModeSender;
use rta_host::monitor::build_once;
use rta_host::sitl::hover_record;
use rta_host::watchdog::Watchdog;
use rta_spec::RecoveryMode;
use std::fs;
use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant};

/// One-microsecond bins up to this bound; a longer sample is counted past the end and kept
/// exactly as the worst case.
const HIST_US: usize = 200_000;
/// Coarse bins printed in the artifact, in microseconds.
const PRINTED_BINS_US: [u64; 12] = [
    50, 100, 200, 500, 1_000, 2_000, 5_000, 10_000, 20_000, 50_000, 100_000, 200_000,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Evidence {
    Simulated,
    HostStandin,
    RealTarget,
}

impl Evidence {
    fn label(self) -> &'static str {
        match self {
            Self::Simulated => "simulated",
            Self::HostStandin => "host-standin",
            Self::RealTarget => "real-target",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Args {
    ticks: u64,
    minutes: u64,
    paced: bool,
    measured_on: String,
    evidence: Evidence,
    models: String,
    slowdown: f64,
    date: String,
    out: String,
}

fn usage() -> &'static str {
    "usage: rta-target [--ticks N] [--minutes M] [--mode paced|tight] [--measured-on LABEL] \
     [--evidence simulated|host-standin|real-target] [--models NAME] [--slowdown F] \
     [--date YYYY-MM-DD] [--out PATH]"
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut parsed = Args {
        ticks: 10_000,
        minutes: 0,
        paced: true,
        measured_on: "host".into(),
        evidence: Evidence::HostStandin,
        models: "none".into(),
        slowdown: 1.0,
        date: "unknown".into(),
        out: "target-timing.txt".into(),
    };
    let mut evidence: Option<Evidence> = None;
    let mut iter = args.iter();
    while let Some(flag) = iter.next() {
        let mut value = || {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{flag} needs a value"))
        };
        match flag.as_str() {
            "--ticks" => parsed.ticks = value()?.parse().map_err(|_| "bad --ticks")?,
            "--minutes" => parsed.minutes = value()?.parse().map_err(|_| "bad --minutes")?,
            "--mode" => {
                parsed.paced = match value()?.as_str() {
                    "paced" => true,
                    "tight" => false,
                    _ => return Err("--mode is paced or tight".into()),
                }
            }
            "--measured-on" => parsed.measured_on = value()?,
            "--evidence" => {
                evidence = Some(match value()?.as_str() {
                    "simulated" => Evidence::Simulated,
                    "host-standin" => Evidence::HostStandin,
                    "real-target" => Evidence::RealTarget,
                    _ => return Err("--evidence is simulated, host-standin or real-target".into()),
                });
            }
            "--models" => parsed.models = value()?,
            "--slowdown" => {
                parsed.slowdown = value()?.parse().map_err(|_| "bad --slowdown")?;
                if !(parsed.slowdown.is_finite() && parsed.slowdown >= 1.0) {
                    return Err("--slowdown is a finite factor of at least 1".into());
                }
            }
            "--date" => parsed.date = value()?,
            "--out" => parsed.out = value()?,
            _ => return Err(format!("unknown argument {flag}")),
        }
    }
    if parsed.ticks == 0 && parsed.minutes == 0 {
        return Err("--ticks or --minutes must be above zero".into());
    }
    if parsed.measured_on.chars().any(char::is_whitespace) || parsed.models.contains(' ') {
        return Err("--measured-on and --models take one word".into());
    }
    let label = parsed.measured_on.to_ascii_lowercase();
    let emulated = label.starts_with("qemu") || label.contains("sim");
    let scaled = parsed.slowdown > 1.0;
    parsed.evidence = match evidence {
        // A real-target line must be unscaled, unemulated, and from an aarch64 build.
        Some(Evidence::RealTarget) if emulated || scaled || !cfg!(target_arch = "aarch64") => {
            return Err(
                "real-target needs an unscaled aarch64 run on the board, not an emulator or a model"
                    .into(),
            );
        }
        Some(Evidence::HostStandin) if emulated || scaled => {
            return Err("an emulated or scaled run is simulated evidence".into());
        }
        Some(chosen) => chosen,
        None if emulated || scaled => Evidence::Simulated,
        None => Evidence::HostStandin,
    };
    Ok(parsed)
}

fn proc_stat() -> String {
    fs::read_to_string("/proc/self/stat").unwrap_or_default()
}

/// Field `index` (zero-based) after the `(comm)` field of `/proc/self/stat`. A short or
/// unreadable line reads as `None`.
fn stat_field(text: &str, index: usize) -> Option<String> {
    let rest = text.rsplit_once(')').map(|(_, rest)| rest)?;
    // Field 3 (state) is the first after the comm, so field n is at n - 3.
    rest.split_whitespace()
        .nth(index.checked_sub(3)?)
        .map(str::to_string)
}

fn cpu_seconds() -> f64 {
    let text = proc_stat();
    // utime and stime are fields 14 and 15 of /proc/self/stat, counted from one.
    let field = |index: usize| {
        stat_field(&text, index - 1)
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    (field(14) + field(15)) / 100.0
}

/// The scheduling policy (`/proc/self/stat` field 41) and the allowed CPUs, as the kernel
/// reports them. Under qemu user mode these are the host kernel's view.
fn sched_note() -> String {
    let policy = match stat_field(&proc_stat(), 40).as_deref() {
        Some("0") => "other",
        Some("1") => "fifo",
        Some("2") => "rr",
        Some("3") => "batch",
        Some("5") => "idle",
        Some("6") => "deadline",
        _ => "unknown",
    };
    let cpus = fs::read_to_string("/proc/self/status")
        .unwrap_or_default()
        .lines()
        .find_map(|line| line.strip_prefix("Cpus_allowed_list:"))
        .map_or_else(|| "unknown".to_string(), |list| list.trim().to_string());
    format!("sched={policy} cpus={cpus}")
}

/// Exact percentiles over one-microsecond bins.
struct Histogram {
    bins: Vec<u64>,
    past_end: u64,
    count: u64,
    worst_us: u64,
}

impl Histogram {
    fn new() -> Self {
        Self {
            bins: vec![0; HIST_US],
            past_end: 0,
            count: 0,
            worst_us: 0,
        }
    }

    fn add(&mut self, us: u64) {
        self.count += 1;
        self.worst_us = self.worst_us.max(us);
        match usize::try_from(us).ok().and_then(|i| self.bins.get_mut(i)) {
            Some(bin) => *bin += 1,
            None => self.past_end += 1,
        }
    }

    /// The smallest value at or under which `per_mille` thousandths of samples fall.
    fn percentile(&self, per_mille: u64) -> u64 {
        let rank = (self.count.saturating_mul(per_mille))
            .div_ceil(1_000)
            .max(1);
        let mut seen = 0;
        for (us, count) in self.bins.iter().enumerate() {
            seen += count;
            if seen >= rank {
                return u64::try_from(us).unwrap_or(u64::MAX);
            }
        }
        self.worst_us
    }

    fn at_or_under(&self, bound_us: u64) -> u64 {
        let end = usize::try_from(bound_us)
            .unwrap_or(HIST_US)
            .min(HIST_US - 1);
        self.bins.iter().take(end + 1).sum()
    }
}

fn scaled(us: u64, slowdown: f64) -> u64 {
    // The model multiplies measured time; it never shrinks it.
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let out = ((us as f64) * slowdown).ceil() as u64;
    out.max(us)
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(&raw) {
        Ok(args) => args,
        Err(err) => {
            eprintln!("target: {err}\n{}", usage());
            return ExitCode::from(1);
        }
    };
    let config = match parse_str(include_str!("../../../../config/sitl.toml")) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("target: config: {err}");
            return ExitCode::from(1);
        }
    };
    let spec = include_str!("../../../../spec/monitor.lola");
    let mut built = match build_once(spec) {
        Ok(built) => built,
        Err(err) => {
            eprintln!("target: spec: {err:?}");
            return ExitCode::from(1);
        }
    };
    let record = match hover_record(&config) {
        Ok(record) => record,
        Err(err) => {
            eprintln!("target: hover record: {err}");
            return ExitCode::from(1);
        }
    };
    let deadline_ms = config.tick.deadline_ms;
    let deadline_us = deadline_ms * 1_000;
    let period = Duration::from_millis(config.tick.period_ms);
    let start_cpu = cpu_seconds();
    let started = Instant::now();
    let mut eval = Evaluator::default();
    // Measured eval time, the slowdown-model eval time, and how late each paced tick woke.
    let mut raw_hist = Histogram::new();
    let mut model_hist = Histogram::new();
    let mut wake_late = Histogram::new();
    let mut over = 0u64;
    let mut samples = 0u64;
    // The first evaluation runs cold (first touch of the monitor's memory, and translation
    // under qemu). It is reported on its own as well as in the percentiles.
    let mut first_us = 0u64;
    let mut worst_after_first_us = 0u64;
    let mut next = Instant::now();
    loop {
        let by_ticks = args.ticks > 0 && samples >= args.ticks;
        let by_time = args.minutes > 0 && started.elapsed().as_secs() >= args.minutes * 60;
        if by_ticks || by_time {
            break;
        }
        let tick = Instant::now();
        if args.paced && samples > 0 {
            let late = tick.saturating_duration_since(next).as_micros();
            wake_late.add(u64::try_from(late).unwrap_or(u64::MAX));
        }
        let _ = eval.tick(
            &mut built,
            &record,
            samples.saturating_mul(config.tick.period_ms),
        );
        let raw_us = u64::try_from(tick.elapsed().as_micros()).unwrap_or(u64::MAX);
        let model_us = scaled(raw_us, args.slowdown);
        raw_hist.add(raw_us);
        model_hist.add(model_us);
        if samples == 0 {
            first_us = model_us;
        } else {
            worst_after_first_us = worst_after_first_us.max(model_us);
        }
        if model_us > deadline_us {
            over += 1;
        }
        samples += 1;
        if args.paced {
            next += period;
            let now = Instant::now();
            if next > now {
                thread::sleep(next - now);
            } else {
                next = now;
            }
        }
    }
    let elapsed = started.elapsed().as_secs_f64().max(0.001);
    let cpu = ((cpu_seconds() - start_cpu) / elapsed * 100.0).max(0.0);
    let mut dog = Watchdog::new(config.tick.period_ms, config.tick.watchdog_misses);
    dog.publish(0);
    let mut sender = ModeSender::default();
    let mut sink = Vec::new();
    let paused_ms = config.tick.period_ms * u64::from(config.tick.watchdog_misses);
    dog.check(
        paused_ms,
        RecoveryMode::Loiter,
        RecoveryMode::Rtl,
        &mut sender,
        &mut sink,
    );
    let mode = if args.paced { "paced" } else { "tight" };
    let header = match args.evidence {
        Evidence::Simulated => {
            "# SIMULATED timing, not a measurement on the modelled board. See docs/target-timing.md."
        }
        Evidence::HostStandin => {
            "# HOST STAND-IN timing, not a measurement on any target board. See docs/target-timing.md."
        }
        Evidence::RealTarget => {
            "# REAL-TARGET timing, as declared by the operator on the board. See docs/target-timing.md."
        }
    };
    let pct = |hist: &Histogram, prefix: &str| {
        format!(
            "{prefix}p50_us={} {prefix}p90_us={} {prefix}p99_us={} {prefix}p999_us={} {prefix}worst_us={}",
            hist.percentile(500),
            hist.percentile(900),
            hist.percentile(990),
            hist.percentile(999),
            hist.worst_us
        )
    };
    let wake = if args.paced {
        format!(
            "wake_late_p99_us={} wake_late_worst_us={}",
            wake_late.percentile(990),
            wake_late.worst_us
        )
    } else {
        "wake_late_p99_us=none wake_late_worst_us=none".to_string()
    };
    let mut report = format!(
        "{header}\nevidence={} measured_on={} models={} arch={} slowdown={} date={} mode={mode} samples={samples} period_ms={} deadline_ms={deadline_ms} {} {} model_first_us={first_us} model_worst_after_first_us={worst_after_first_us} over_deadline={over} {wake} cpu_pct={cpu:.1} {} watchdog={} decision=deadline-unchanged\n",
        args.evidence.label(),
        args.measured_on,
        args.models,
        std::env::consts::ARCH,
        args.slowdown,
        args.date,
        config.tick.period_ms,
        pct(&raw_hist, "raw_"),
        pct(&model_hist, "model_"),
        sched_note(),
        sink.first().map(String::as_str).unwrap_or("none")
    );
    for bound in PRINTED_BINS_US {
        report.push_str(&format!(
            "model_hist le_us={bound} count={}\n",
            model_hist.at_or_under(bound)
        ));
    }
    report.push_str(&format!(
        "model_hist le_us=inf count={}\n",
        model_hist.count
    ));
    if fs::write(&args.out, &report).is_err() {
        eprintln!("target: artifact failed");
        return ExitCode::from(1);
    }
    print!("{report}");
    if over > 0 {
        eprintln!("target: {over} evaluations over the deadline; deadline unchanged");
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Result<Args, String> {
        parse_args(&list.iter().map(|s| (*s).to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn an_emulated_or_scaled_run_is_simulated_and_cannot_claim_the_board() {
        assert_eq!(
            args(&["--measured-on", "qemu-aarch64"]).unwrap().evidence,
            Evidence::Simulated
        );
        assert_eq!(
            args(&["--measured-on", "host-sim", "--slowdown", "2.5"])
                .unwrap()
                .evidence,
            Evidence::Simulated
        );
        assert_eq!(args(&[]).unwrap().evidence, Evidence::HostStandin);
        assert!(args(&["--measured-on", "qemu-aarch64", "--evidence", "real-target"]).is_err());
        assert!(args(&["--slowdown", "2", "--evidence", "real-target"]).is_err());
        assert!(args(&["--slowdown", "2", "--evidence", "host-standin"]).is_err());
        assert!(args(&["--slowdown", "0.5"]).is_err());
        if !cfg!(target_arch = "aarch64") {
            assert!(args(&["--measured-on", "orin", "--evidence", "real-target"]).is_err());
        }
    }

    #[test]
    fn percentiles_come_from_the_samples() {
        let mut hist = Histogram::new();
        for us in 1..=1_000 {
            hist.add(us);
        }
        hist.add(300_000);
        assert_eq!(hist.percentile(500), 501);
        assert_eq!(hist.percentile(990), 991);
        assert_eq!(hist.worst_us, 300_000);
        assert_eq!(hist.at_or_under(100), 100);
        assert_eq!(scaled(1_000, 2.5), 2_500);
        assert_eq!(scaled(3, 1.0), 3);
    }
}
