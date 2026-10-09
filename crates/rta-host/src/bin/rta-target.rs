//! Ten-minute eval measurement. Does not change the deadline.

use rta_host::config::parse_str;
use rta_host::eval::Evaluator;
use rta_host::mode::ModeSender;
use rta_host::monitor::build_once;
use rta_host::sitl::hover_record;
use rta_host::watchdog::Watchdog;
use rta_spec::RecoveryMode;
use std::fs;
use std::process::ExitCode;
use std::time::Instant;

fn cpu_seconds() -> f64 {
    let text = fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let fields: Vec<&str> = text.split_whitespace().collect();
    if fields.len() < 15 {
        return 0.0;
    }
    let ticks = fields[13].parse::<f64>().unwrap_or(0.0) + fields[14].parse::<f64>().unwrap_or(0.0);
    ticks / 100.0
}

fn main() -> ExitCode {
    let minutes: u64 = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "10".into())
        .parse()
        .unwrap_or(10);
    let config = parse_str(include_str!("../../../../config/sitl.toml")).expect("config");
    let spec = include_str!("../../../../spec/monitor.lola");
    let mut built = build_once(spec).expect("spec");
    let record = hover_record(&config).expect("hover");
    let deadline_ms = config.tick.deadline_ms;
    let start_cpu = cpu_seconds();
    let started = Instant::now();
    let mut eval = Evaluator::default();
    let mut samples = 0u64;
    let mut worst_us = 0u128;
    while started.elapsed().as_secs() < minutes * 60 {
        let tick = Instant::now();
        let _ = eval.tick(&mut built, &record, samples.saturating_mul(50));
        worst_us = worst_us.max(tick.elapsed().as_micros());
        samples += 1;
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
    let report = format!(
        "board=Jetson Orin Nano 8GB date=2026-10-09 samples={samples} worst_us={worst_us} deadline_ms={deadline_ms} cpu_pct={cpu:.1} power_note=10W watchdog={}\n",
        sink.first().map(String::as_str).unwrap_or("none")
    );
    if fs::write("target-timing.txt", &report).is_err() {
        eprintln!("target: artifact failed");
        return ExitCode::from(1);
    }
    println!("{report}");
    if worst_us > u128::from(deadline_ms) * 1_000 {
        eprintln!("target: worst case exceeds the deadline");
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}
