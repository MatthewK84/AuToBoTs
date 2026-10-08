//! Re-evaluate a tick log, or every log in a directory, against the spec.

use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(log) = args.next() else {
        eprintln!("usage: rta-replay <log-or-dir> <spec>");
        return ExitCode::from(2);
    };
    let Some(spec) = args.next() else {
        eprintln!("usage: rta-replay <log-or-dir> <spec>");
        return ExitCode::from(2);
    };
    match rta_host::replay::check_path(std::path::Path::new(&log), std::path::Path::new(&spec)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!(
                "tick {} expected {} actual {} {}",
                err.tick, err.expected, err.actual, err.record
            );
            ExitCode::from(1)
        }
    }
}
