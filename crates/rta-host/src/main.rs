//! Host binary. Forbidden: a direct MAVLink write that skips `rta-switch::decide`.
//! Config load failure exits before a socket type is constructed.
//!
//! Startup is config, spec load, log open, then socket. The tick loop runs until SIGINT or
//! `--ticks N`, then shuts down in order: stop setpoints, recovery mode, flush log, exit.

use rta_host::{config, eval_worker, fault, link, monitor, runtime, spec, tick_log};

use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const USAGE: &str = "usage: rta-host <config.toml> [--ticks N]";

/// Parsed command line. `--ticks` bounds a run for tests and bench checks.
#[derive(Debug, PartialEq, Eq)]
struct Args {
    config: String,
    max_ticks: Option<u64>,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let config = args.next().ok_or(USAGE)?;
    let mut max_ticks = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--ticks" => {
                let value = args.next().ok_or(USAGE)?;
                let ticks: u64 = value.parse().map_err(|_| format!("--ticks {value}"))?;
                if ticks == 0 {
                    return Err("--ticks must be at least 1".into());
                }
                max_ticks = Some(ticks);
            }
            other => return Err(format!("unknown argument {other}; {USAGE}")),
        }
    }
    Ok(Args { config, max_ticks })
}

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::from(2);
        }
    };
    let config = match config::load(Path::new(&args.config)) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("config: {err}");
            return ExitCode::from(1);
        }
    };
    let endpoint = match link::parse_endpoint(&config.link.endpoint) {
        Ok(endpoint) => endpoint,
        Err(err) => {
            eprintln!("link: {err:?}");
            return ExitCode::from(1);
        }
    };
    let spec_text = match fault::load_spec_file(Path::new(&config.spec.path)) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("spec: {err}");
            return ExitCode::from(1);
        }
    };
    let spec_hash = tick_log::spec_hash(spec_text.as_bytes());
    // Thresholds the host checks as well as the spec are read from the spec, not copied.
    let limits = match spec::limits(&spec_text) {
        Ok(limits) => limits,
        Err(err) => {
            eprintln!("spec: {err}");
            return ExitCode::from(1);
        }
    };
    // The monitor is built once, on the evaluation worker. A parse or type error exits here,
    // before the log and the socket.
    let worker = match eval_worker::EvalWorker::start(move || {
        monitor::build_once(&spec_text)
            .map(eval_worker::SpecEvaluator::new)
            .map_err(|err| format!("{err:?}"))
    }) {
        Ok(worker) => worker,
        Err(err) => {
            eprintln!("spec: {err}");
            return ExitCode::from(1);
        }
    };
    let mut tick_log = match tick_log::TickLog::open(
        Path::new(&config.log.path),
        config.log.flush_every_n,
        spec_hash,
    ) {
        Ok(log) => log,
        Err(err) => {
            eprintln!("log: {err}");
            return ExitCode::from(1);
        }
    };
    let link = match link::open(&endpoint, runtime::RECV_TIMEOUT) {
        Ok(link) => link,
        Err(err) => {
            eprintln!("link: {err:?}");
            return ExitCode::from(1);
        }
    };
    let stop = Arc::new(AtomicBool::new(false));
    let on_signal = Arc::clone(&stop);
    if let Err(err) = ctrlc::set_handler(move || on_signal.store(true, Ordering::SeqCst)) {
        eprintln!("signal: {err}");
        return ExitCode::from(1);
    }
    let bound = link
        .local_addr()
        .map(|address| address.to_string())
        .unwrap_or_default();
    println!(
        "rta-host: running, bound {bound}, endpoint {}",
        config.link.endpoint
    );
    let mut complex = rta_switch::StubComplex;
    match runtime::run(
        &config,
        worker,
        &mut tick_log,
        spec_hash,
        link,
        &mut complex,
        runtime::RunOptions {
            max_ticks: args.max_ticks,
            stop,
            limits,
        },
    ) {
        Ok(report) => {
            println!(
                "rta-host: stopped after {} ticks, overruns {}, recovery_sent {}, recovery_attempts {}, recovery_confirmed {}, watchdog_fired {}",
                report.ticks,
                report.overruns,
                report.recovery_sent,
                report.recovery_attempts,
                report.recovery_confirmed,
                report.watchdog_fired
            );
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("runtime: {err:?}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Result<Args, String> {
        parse_args(list.iter().map(|item| item.to_string()))
    }

    #[test]
    fn ticks_bound_parses_and_bad_arguments_are_rejected() {
        assert_eq!(
            args(&["c.toml"]),
            Ok(Args {
                config: "c.toml".into(),
                max_ticks: None
            })
        );
        assert_eq!(
            args(&["c.toml", "--ticks", "5"]).map(|a| a.max_ticks),
            Ok(Some(5))
        );
        assert!(args(&[]).is_err());
        assert!(args(&["c.toml", "--ticks"]).is_err());
        assert!(args(&["c.toml", "--ticks", "0"]).is_err());
        assert!(args(&["c.toml", "--ticks", "x"]).is_err());
        assert!(args(&["c.toml", "--fast"]).is_err());
    }
}
