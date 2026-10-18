//! Connect to the SITL port, send one heartbeat, and log a nominal hover.

use rta_host::config::load;
use rta_host::eval::Evaluator;
use rta_host::fail_closed::{FailClosed, TickCase};
use rta_host::fault::load_spec_file;
use rta_host::link::{open, parse_endpoint};
use rta_host::mavlink_link::MavLink;
use rta_host::monitor::build_once;
use rta_host::reason::{Reason, Reasons};
use rta_host::runtime::recovery_of;
use rta_host::sitl::{hover_record, hover_verdict, timing_header, EXAMPLE_ENDPOINT};
use rta_host::spec::limits;
use rta_host::tick_log::{command_name, spec_hash, TickLog, TickRecord};
use rta_spec::Verdict;
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
    if let Err(err) = open(&endpoint, std::time::Duration::from_millis(20)) {
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
    let spec_hash = spec_hash(spec.as_bytes());
    let limits = match limits(&spec) {
        Ok(limits) => limits,
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
    let eval_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    if verdict != Verdict::Pass {
        eprintln!("sitl: hover was {verdict:?}");
        return ExitCode::from(1);
    }
    let header = timing_header(spec_hash, &[eval_ms]);
    if std::fs::write(Path::new("sitl-timing.txt"), &header).is_err() {
        eprintln!("sitl: timing artifact failed");
        return ExitCode::from(1);
    }
    if std::fs::write(Path::new(&config.log.path), &header).is_err() {
        eprintln!("sitl: log header failed");
        return ExitCode::from(1);
    }
    let mut log = match TickLog::open(
        Path::new(&config.log.path),
        config.log.flush_every_n,
        spec_hash,
    ) {
        Ok(log) => log,
        Err(err) => {
            eprintln!("sitl: {err}");
            return ExitCode::from(1);
        }
    };
    // The hover goes through the same fail-closed path as the runtime, so the deadline, the
    // spec's heartbeat limit, and the write latch all reach the logged command. Nothing is
    // sent: the hover only heartbeats.
    let recovery = recovery_of(&config);
    let mut closed = FailClosed::new(limits);
    let command = closed.command(TickCase {
        verdict: Some(verdict),
        late: eval_ms > config.tick.deadline_ms,
        heartbeat_age_ms: Some(record.fc_heartbeat_age_ms()),
        request: None,
        reported: recovery,
        recovery,
        write_ok: true,
        eval_fault: false,
    });
    let line = TickRecord {
        tick: 1,
        fence_ok: record.fence_ok(),
        fix_age_ms: record.fix_age_ms(),
        link_age_ms: record.link_age_ms(),
        fc_heartbeat_age_ms: record.fc_heartbeat_age_ms(),
        track_conf: record.track_conf(),
        range_m: record.range_m(),
        verdict,
        filter_intervened: false,
        reasons: {
            let mut reasons = Reasons::new();
            reasons.push(Reason::NotSent);
            reasons
        },
        command: command_name(&command),
        eval_ms,
        spec_hash,
        alt_m: 30.0,
        indication: String::new(),
    };
    // A failed write is the write latch: the hover fails rather than logging a pass.
    if log.append(&line).and_then(|()| log.flush()).is_err() {
        eprintln!("sitl: log write failed");
        return ExitCode::from(1);
    }
    println!("sitl: heartbeat sent, hover logged pass");
    ExitCode::SUCCESS
}
