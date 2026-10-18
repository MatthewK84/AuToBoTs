//! Document and source-text checks: the assertions that a note, a plan, an index, a script,
//! or a source file says what the code does. Not flight code. No binary depends on this
//! crate, so nothing here is linked into `rta-host`; `cargo test --workspace` (CI `compile`)
//! still runs every check (`docs/doc-checks.md`).
//!
//! Top-level modules are the evidence tables and their checks. `host` holds the checks that
//! name a flight module of `rta-host`, one file per module.

pub mod ci;
pub mod claims;
pub mod common_mode;
pub mod coverage_gate;
pub mod degraded;
#[cfg(test)]
mod hygiene;
pub mod icd_fc;
pub mod jsf;
pub mod learning_hazards;
pub mod lockfile;
pub mod packet;
pub mod panic_policy;
pub mod planning;
pub mod requirements;
pub mod safety;
pub mod sci;
pub mod tools;
pub mod trace;

#[cfg(test)]
mod host;
