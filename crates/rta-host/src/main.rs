//! Host binary. Forbidden: a direct MAVLink write that skips `rta-switch::decide`.
//! Config load failure exits before a socket type is constructed.

use rta_host::tracker::Tracker;
use rta_host::*;

use std::env;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(path) = env::args().nth(1) else {
        eprintln!("usage: rta-host <config.toml>");
        return ExitCode::from(2);
    };
    let config = match config::load(std::path::Path::new(&path)) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("config: {err}");
            return ExitCode::from(1);
        }
    };
    let _dialect = std::any::type_name::<mavlink::dialects::common::MavMessage>();
    let _spec = std::any::type_name::<rtlola_frontend::RtLolaError>();
    let _value = std::any::type_name::<rtlola_interpreter::Value>();
    let _track = stale::present_track(false, 0.0, 0.0, 0.0, config.tick.tracker_hold_ms);
    let _fix = stale::present_fix(None, 0.0);
    let _fence = match &config.fence {
        config::Fence::Host {
            polygon,
            vertical_cap_m,
            v_max,
            a_max,
        } => {
            fence::fence_ok(
                [0.0, 0.0],
                0.0,
                fence::FenceInput {
                    polygon: &[],
                    vertical_cap_m: *vertical_cap_m,
                    v_max: *v_max,
                    a_max: *a_max,
                    mode_change_latency_s: config.tick.mode_change_latency_ms as f64 / 1_000.0,
                },
            ) || polygon.is_empty()
        }
        config::Fence::Fc => false,
    };
    let spec_text = match fault::load_spec_file(Path::new(&config.spec.path)) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("spec: {err}");
            return ExitCode::from(1);
        }
    };
    let mut built = match monitor::build_once(&spec_text) {
        Ok(built) => built,
        Err(err) => {
            eprintln!("spec: {err:?}");
            return ExitCode::from(1);
        }
    };
    let _monitor = built.borrow_monitor();
    let _builds = monitor::build_count();
    let record =
        rta_spec::InputRecord::try_new(0.0, false, 0.0, 0.0, 0.0, 0.0, 0.0).expect("record");
    let _event = event::event(&record);
    let _order = event::spec_input_order(&spec_text);
    let mut ticks = tick::TickState::default();
    let _timed = ticks.tick(
        &mut tick::Idle,
        std::time::Duration::from_millis(config.tick.deadline_ms),
    );
    let _faulted = ticks.faulted();
    let mut modes = mode::ModeSender::default();
    let mut sink = Vec::new();
    let recovery = match config.recovery.mode {
        config::RecoveryMode::Rtl => rta_spec::RecoveryMode::Rtl,
        config::RecoveryMode::Loiter => rta_spec::RecoveryMode::Loiter,
        config::RecoveryMode::Land => rta_spec::RecoveryMode::Land,
    };
    let _mode = modes.send(
        rta_spec::Verdict::Pass,
        None,
        rta_spec::RecoveryMode::Loiter,
        recovery,
        &mut sink,
    );
    let spec_hash = tick_log::spec_hash(spec_text.as_bytes());
    let mut tick_log = match tick_log::TickLog::open(
        std::path::Path::new(&config.log.path),
        config.log.flush_every_n,
        spec_hash,
    ) {
        Ok(log) => log,
        Err(err) => {
            eprintln!("log: {err}");
            return ExitCode::from(1);
        }
    };
    let record_line = tick_log::TickRecord {
        tick: 0,
        fence_ok: false,
        fix_age_ms: 0.0,
        link_age_ms: 0.0,
        fc_heartbeat_age_ms: 0.0,
        track_conf: 0.0,
        range_m: 0.0,
        verdict: rta_spec::Verdict::Pass,
        filter_intervened: false,
        reasons: Vec::new(),
        command: tick_log::command_name(&rta_switch::SwitchCommand::Idle),
        eval_ms: 0,
        spec_hash,
        alt_m: 0.0,
    };
    if tick_log.append(&record_line).is_err() {
        let _latched = tick_log::on_write_error();
    }
    let endpoint = match link::parse_endpoint(&config.link.endpoint) {
        Ok(endpoint) => endpoint,
        Err(err) => {
            eprintln!("link: {err:?}");
            return ExitCode::from(1);
        }
    };
    let mut bound = None;
    if let Err(err) = link::open(
        &endpoint,
        config.link.system_id,
        config.link.component_id,
        &mut |address, _system, _component| {
            bound = Some(address.to_string());
        },
    ) {
        eprintln!("link: {err:?}");
        return ExitCode::from(1);
    }
    let mut partial = read::PartialRecord::default();
    let _read = read::apply(&[], 0, &mut partial);
    let mut link_age = link_age::LinkAge::new();
    link_age.observe(link_age::LinkMessage::StatusText, 0);
    let _age = link_age.age_ms(0);
    let mut evaluator = eval::Evaluator::default();
    let _tick = evaluator.tick(&mut built, &record, 0);
    let _converted = eval::monotonic_to_interpreter(config.tick.period_ms);
    let _faulted = evaluator.faulted();
    let mut complex: Box<dyn rta_switch::Complex> = Box::new(rta_switch::StubComplex);
    let _request = complex.request();
    let mut closed = fail_closed::FailClosed::default();
    let _closed = closed.command(fail_closed::TickCase {
        verdict: None,
        late: false,
        heartbeat_age_ms: None,
        request: None,
        reported: rta_spec::RecoveryMode::Loiter,
        recovery,
        write_ok: true,
    });
    let mut dog = watchdog::Watchdog::new(config.tick.period_ms, config.tick.watchdog_misses);
    dog.publish(0);
    let mut dog_sink = Vec::new();
    dog.check(
        0,
        rta_spec::RecoveryMode::Loiter,
        recovery,
        &mut modes,
        &mut dog_sink,
    );
    if let Ok(mut mav) =
        mavlink_link::MavLink::open(&endpoint, config.link.system_id, config.link.component_id)
    {
        let _beat = mav.send_heartbeat();
        mav.note(&mavlink_link::classify(&mavlink_link::heartbeat()), 0);
        let mut vehicle = mavlink_link::VehicleState::default();
        mavlink_link::apply(
            &mut vehicle,
            &mavlink_link::Inbound::Position { alt_mm: 0 },
            0,
        );
        let _fix_age = mavlink_link::age_ms(vehicle.fix_received_ms, 0);
        let _age = mav.fc_heartbeat_age_ms(0);
    }
    let mut tracker = tracker::FakeTracker::script(vec![tracker::weak_inside_commit(0)]);
    let _track = tracker.sample();
    let _outbound =
        command_out::to_message(&rta_switch::SwitchCommand::Idle, config.link.system_id);
    let _reported = command_out::reported_mode(0);
    println!("rta-host: config accepted, stub bind {bound:?}");
    ExitCode::SUCCESS
}
