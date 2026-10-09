//! Connect to the SITL port, send one heartbeat, and log a nominal hover.

use rta_host::config::load;
use rta_host::eval::Evaluator;
use rta_host::fault::load_spec_file;
use rta_host::link::{open, parse_endpoint};
use rta_host::mavlink_link::MavLink;
use rta_host::monitor::build_once;
use rta_host::sitl::{hover_record, hover_verdict, timing_header, EXAMPLE_ENDPOINT};
use rta_host::tick_log::{command_name, TickLog, TickRecord};
use rta_spec::Verdict;
use rta_switch::SwitchCommand;
use std::env;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "config/sitl.toml".into());
    let config = match load(Path::new(&path)) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("sitl: {err}");
            return ExitCode::from(1);
        }
    };
    if config.link.endpoint != EXAMPLE_ENDPOINT {
        eprintln!("sitl: endpoint must match example.toml");
        return ExitCode::from(1);
    }
    let endpoint = match parse_endpoint(&config.link.endpoint) {
        Ok(endpoint) => endpoint,
        Err(err) => {
            eprintln!("sitl: {err:?}");
            return ExitCode::from(1);
        }
    };
    if let Err(err) = open(
        &endpoint,
        config.link.system_id,
        config.link.component_id,
        &mut |_address, _system, _component| {},
    ) {
        eprintln!("sitl: {err:?}");
        return ExitCode::from(1);
    }
    match MavLink::open(&endpoint, config.link.system_id, config.link.component_id) {
        Ok(mut link) => {
            if link.send_heartbeat().is_err() {
                eprintln!("sitl: heartbeat failed");
                return ExitCode::from(1);
            }
        }
        Err(err) => {
            eprintln!("sitl: {err:?}");
            return ExitCode::from(1);
        }
    }
    let spec = match load_spec_file(Path::new(&config.spec.path)) {
        Ok(spec) => spec,
        Err(err) => {
            eprintln!("sitl: {err}");
            return ExitCode::from(1);
        }
    };
    let record = match hover_record(&config) {
        Ok(record) => record,
        Err(err) => {
            eprintln!("sitl: {err}");
            return ExitCode::from(1);
        }
    };
    let mut built = match build_once(&spec) {
        Ok(built) => built,
        Err(err) => {
            eprintln!("sitl: {err:?}");
            return ExitCode::from(1);
        }
    };
    let mut eval = Evaluator::default();
    let started = std::time::Instant::now();
    let verdict = hover_verdict(&mut eval, &mut built, &record);
    let eval_ms = started.elapsed().as_millis() as u64;
    if verdict != Verdict::Pass {
        eprintln!("sitl: hover was {verdict:?}");
        return ExitCode::from(1);
    }
    let header = timing_header(0, &[eval_ms]);
    if std::fs::write(Path::new("sitl-timing.txt"), &header).is_err() {
        eprintln!("sitl: timing artifact failed");
        return ExitCode::from(1);
    }
    if std::fs::write(Path::new(&config.log.path), &header).is_err() {
        eprintln!("sitl: log header failed");
        return ExitCode::from(1);
    }
    let mut log = match TickLog::open(Path::new(&config.log.path), config.log.flush_every_n, 0) {
        Ok(log) => log,
        Err(err) => {
            eprintln!("sitl: {err}");
            return ExitCode::from(1);
        }
    };
    let line = TickRecord {
        tick: 1,
        fence_ok: true,
        fix_age_ms: record.fix_age_ms(),
        link_age_ms: record.link_age_ms(),
        fc_heartbeat_age_ms: record.fc_heartbeat_age_ms(),
        track_conf: record.track_conf(),
        range_m: record.range_m(),
        verdict,
        filter_intervened: false,
        reasons: Vec::new(),
        command: command_name(&SwitchCommand::Idle),
        eval_ms: 1,
        spec_hash: 0,
        alt_m: 30.0,
    };
    if log.append(&line).is_err() {
        eprintln!("sitl: log write failed");
        return ExitCode::from(1);
    }
    println!("sitl: heartbeat sent, hover logged pass");
    ExitCode::SUCCESS
}
