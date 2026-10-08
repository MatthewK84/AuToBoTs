//! Host binary. Forbidden: a direct MAVLink write that skips `rta-switch::decide`.
//! Dialect pin is common only. Interpreter and MAVLink are linked here, not in the switch.

fn main() {
    let _dialect = std::any::type_name::<mavlink::dialects::common::MavMessage>();
    let _spec = std::any::type_name::<rtlola_frontend::RtLolaError>();
    let _value = std::any::type_name::<rtlola_interpreter::Value>();
    println!("rta-host: pins loaded, no link opened");
}
