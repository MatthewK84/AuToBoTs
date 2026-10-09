//! Scripted trips. Assertions read the tick log, not a ground station.

use rta_host::command_out::{custom_mode, reported_mode};
use rta_host::config::parse_str;
use rta_host::eval::Evaluator;
use rta_host::monitor::build_once;
use rta_host::tick_log::command_name;
use rta_spec::{InputRecord, RecoveryMode, Verdict};
use rta_switch::{decide, Request, SwitchCommand};
use std::fs;
use std::path::Path;
use std::process::ExitCode;

struct Trip {
    name: &'static str,
    record: InputRecord,
    expect: Verdict,
    commit: bool,
    report: Option<RecoveryMode>,
}

fn line(trip: &Trip, verdict: Verdict, command: &SwitchCommand) -> String {
    let report = trip
        .report
        .map(|mode| format!("fc_report={:?}\n", mode).to_ascii_lowercase())
        .unwrap_or_default();
    format!(
        "{report}tick=1 fence_ok={} fix_age_ms={} link_age_ms={} fc_heartbeat_age_ms={} track_conf={} range_m={} verdict={} command={} commit={}\n",
        trip.record.fence_ok() as u8,
        trip.record.fix_age_ms(),
        trip.record.link_age_ms(),
        trip.record.fc_heartbeat_age_ms(),
        trip.record.track_conf(),
        trip.record.range_m(),
        match verdict {
            Verdict::Pass => "pass",
            Verdict::Inhibit => "inhibit",
            Verdict::Revert => "revert",
        },
        command_name(command),
        if matches!(command, SwitchCommand::Setpoints { commit: true, .. }) { 1 } else { 0 },
    )
}

fn main() -> ExitCode {
    let config = match parse_str(include_str!("../../../../config/sitl.toml")) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("trips: {err}");
            return ExitCode::from(1);
        }
    };
    let spec = include_str!("../../../../spec/monitor.lola");
    let mut built = match build_once(spec) {
        Ok(built) => built,
        Err(err) => {
            eprintln!("trips: {err:?}");
            return ExitCode::from(1);
        }
    };
    let recovery = RecoveryMode::Rtl;
    let trips = [
        Trip {
            name: "fence",
            record: InputRecord::try_new(30.0, false, 10.0, 0.9, 400.0, 10.0, 10.0).expect("fence"),
            expect: Verdict::Revert,
            commit: false,
            report: Some(recovery),
        },
        Trip {
            name: "heartbeat",
            record: InputRecord::try_new(30.0, true, 10.0, 0.9, 400.0, 10.0, 5_000.0)
                .expect("heartbeat"),
            expect: Verdict::Revert,
            commit: false,
            report: None,
        },
        Trip {
            name: "weak",
            record: InputRecord::try_new(30.0, true, 10.0, 0.2, 40.0, 10.0, 10.0).expect("weak"),
            expect: Verdict::Inhibit,
            commit: false,
            report: None,
        },
    ];
    let dir = Path::new("trips");
    if fs::create_dir_all(dir).is_err() {
        eprintln!("trips: cannot create log dir");
        return ExitCode::from(1);
    }
    for trip in &trips {
        let mut eval = Evaluator::default();
        let verdict = eval.tick(&mut built, &trip.record, 1_000).verdict;
        if verdict != trip.expect {
            eprintln!("trips: {} was {verdict:?}", trip.name);
            return ExitCode::from(1);
        }
        let request = Request {
            north: 1.0,
            east: 0.0,
            down: 0.0,
            yaw: 0.0,
            commit: true,
        };
        let command = decide(verdict, Some(request), RecoveryMode::Loiter, recovery);
        let committed = matches!(command, SwitchCommand::Setpoints { commit: true, .. });
        if committed != trip.commit {
            eprintln!("trips: {} commit flag", trip.name);
            return ExitCode::from(1);
        }
        if let Some(mode) = trip.report {
            let reported = reported_mode(custom_mode(mode));
            if reported != Some(mode) {
                eprintln!("trips: flight controller did not report {mode:?}");
                return ExitCode::from(1);
            }
        }
        let text = line(trip, verdict, &command);
        if fs::write(dir.join(format!("{}.log", trip.name)), &text).is_err() {
            eprintln!("trips: log write failed");
            return ExitCode::from(1);
        }
        let _ = config.link.endpoint;
    }
    println!("trips: three logs written");
    ExitCode::SUCCESS
}
