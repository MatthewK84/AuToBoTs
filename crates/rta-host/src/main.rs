//! Host binary. Forbidden: a direct MAVLink write that skips `rta-switch::decide`.
//! Config load failure exits before a socket type is constructed.

mod config;
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
    println!("rta-host: config accepted, no link opened");
    ExitCode::SUCCESS
}
