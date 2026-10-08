//! Host binary. Forbidden: a direct MAVLink write that skips `rta-switch::decide`.
//! Config load failure exits before a socket type is constructed.

mod config;
mod event;
mod fence;
mod mode;
mod monitor;
mod spec;
mod stale;
mod tick;

use std::env;
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
    let spec_text = std::fs::read_to_string(&config.spec.path).unwrap_or_default();
    if let Err(err) = spec::load_spec(&spec_text) {
        eprintln!("spec: {err}");
        return ExitCode::from(1);
    }
    let built = match monitor::build_once(&spec_text) {
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
    let _verdict = ticks.tick(
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
    let _mode = modes.send(rta_spec::Verdict::Pass, None, recovery, &mut sink);
    println!("rta-host: config accepted, no link opened");
    ExitCode::SUCCESS
}
