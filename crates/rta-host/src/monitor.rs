//! One monitor, built at startup. A load or type error returns before any socket type exists.

use rtlola_frontend::mir::{FloatTy, Type};
use rtlola_frontend::{parse, ParserConfig, RtLolaMir};
use rtlola_interpreter::config::OfflineMode;
use rtlola_interpreter::input::ArrayFactory;
use rtlola_interpreter::monitor::{Incremental, Monitor};
use rtlola_interpreter::time::RelativeFloat;
use rtlola_interpreter::{ConfigBuilder, Value};
use std::convert::Infallible;
use std::sync::atomic::{AtomicU64, Ordering};

static BUILDS: AtomicU64 = AtomicU64::new(0);

type HostMonitor = Monitor<
    ArrayFactory<6, Infallible, [Value; 6]>,
    OfflineMode<RelativeFloat>,
    Incremental,
    RelativeFloat,
>;

pub struct BuiltMonitor {
    monitor: HostMonitor,
}

#[derive(Debug, PartialEq)]
pub enum BuildError {
    Parse(String),
    TypeMismatch(String),
}

pub fn build_once(spec: &str) -> Result<BuiltMonitor, BuildError> {
    let cfg = ParserConfig::for_string(spec.to_string());
    let ir = parse(&cfg).map_err(|err| BuildError::Parse(format!("{err:?}")))?;
    check_types(&ir)?;
    let monitor = ConfigBuilder::new()
        .with_ir(ir)
        .offline::<RelativeFloat>()
        .with_array_events::<6, Infallible, [Value; 6]>()
        .with_verdict::<Incremental>()
        .monitor()
        .map_err(|err| BuildError::Parse(err.to_string()))?;
    BUILDS.fetch_add(1, Ordering::SeqCst);
    Ok(BuiltMonitor { monitor })
}

fn check_types(ir: &RtLolaMir) -> Result<(), BuildError> {
    let expected = [
        ("fence_ok", Type::Bool),
        ("fix_age_ms", Type::Float(FloatTy::Float64)),
        ("link_age_ms", Type::Float(FloatTy::Float64)),
        ("fc_heartbeat_age_ms", Type::Float(FloatTy::Float64)),
        ("track_conf", Type::Float(FloatTy::Float64)),
        ("range_m", Type::Float(FloatTy::Float64)),
    ];
    for (name, ty) in expected {
        match ir.inputs.iter().find(|input| input.name == name) {
            Some(input) if input.ty == ty => {}
            Some(input) => {
                return Err(BuildError::TypeMismatch(format!(
                    "{name} is {:?}, expected {ty:?}",
                    input.ty
                )));
            }
            None => return Err(BuildError::TypeMismatch(format!("{name} missing"))),
        }
    }
    Ok(())
}

pub fn build_count() -> u64 {
    BUILDS.load(Ordering::SeqCst)
}

impl BuiltMonitor {
    pub fn borrow_monitor(&self) -> &HostMonitor {
        &self.monitor
    }

    pub fn accept(
        &mut self,
        event: [Value; 6],
        at: std::time::Duration,
    ) -> Result<rtlola_interpreter::monitor::Verdicts<Incremental, RelativeFloat>, String> {
        self.monitor
            .accept_event(event, at)
            .map_err(|err| err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_mismatch_is_err_before_any_socket() {
        let spec = "input fence_ok: Bool\ninput fix_age_ms: Bool\ninput link_age_ms: Float\ninput fc_heartbeat_age_ms: Float\ninput track_conf: Float\ninput range_m: Float\noutput revert: Bool := !fence_ok\noutput inhibit: Bool := revert\n";
        match build_once(spec) {
            Err(BuildError::TypeMismatch(_)) => {}
            Err(other) => panic!("expected type mismatch, got {other:?}"),
            Ok(_) => panic!("expected type mismatch"),
        }
        assert!(!include_str!("main.rs").contains("UdpSocket"));
    }

    #[test]
    fn good_spec_builds_once() {
        let before = build_count();
        let built = build_once(include_str!("../../../spec/monitor.lola")).expect("build");
        let _borrowed = built.borrow_monitor();
        assert!(build_count() > before);
    }
}
