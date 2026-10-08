//! Host binary. Forbidden: a direct MAVLink write that skips `rta-switch::decide`.
//! Interpreter and link crates are not dependencies until their pin task.

fn main() {
    println!("rta-host: toolchain skeleton, no link opened");
}
