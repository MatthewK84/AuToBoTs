//! Host binary. Forbidden: a direct MAVLink write that skips `rta-switch::decide`.
//! Config load failure exits before a socket type is constructed.

mod config;
mod fence;
mod spec;
mod stale;

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
    if let Err(err) =
        spec::load_spec(&std::fs::read_to_string(&config.spec.path).unwrap_or_default())
    {
        eprintln!("spec: {err}");
        return ExitCode::from(1);
    }
    println!("rta-host: config accepted, no link opened");
    ExitCode::SUCCESS
}
