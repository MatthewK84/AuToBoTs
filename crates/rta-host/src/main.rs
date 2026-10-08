//! Host binary. Forbidden: a direct MAVLink write that skips `rta-switch::decide`.
//! Config load failure exits before a socket type is constructed.

mod config;

use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(path) = env::args().nth(1) else {
        eprintln!("usage: rta-host <config.toml>");
        return ExitCode::from(2);
    };
    if let Err(err) = config::load(std::path::Path::new(&path)) {
        eprintln!("config: {err}");
        return ExitCode::from(1);
    }
    let _dialect = std::any::type_name::<mavlink::dialects::common::MavMessage>();
    let _spec = std::any::type_name::<rtlola_frontend::RtLolaError>();
    let _value = std::any::type_name::<rtlola_interpreter::Value>();
    println!("rta-host: config accepted, no link opened");
    ExitCode::SUCCESS
}
