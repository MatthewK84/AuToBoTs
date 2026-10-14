//! Pinned weight file. A hash mismatch refuses the file and does not activate the shadow.

use crate::tick_log::spec_hash;

/// The named board has not closed power and deadline. The shadow stays off.
pub const SHADOW_DEFAULT_ON: bool = false;

pub fn shadow_may_enable() -> bool {
    SHADOW_DEFAULT_ON
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightError {
    HashMismatch,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InferenceGate {
    Finished,
    Silent,
}

pub fn pin_hash(bytes: &[u8]) -> String {
    format!("{:016x}", spec_hash(bytes))
}

pub fn load_pinned(bytes: &[u8], expected: &str) -> Result<(), WeightError> {
    if expected.is_empty() || pin_hash(bytes) != expected {
        return Err(WeightError::HashMismatch);
    }
    Ok(())
}

/// A load failure keeps the primary. The shadow is not activated.
pub fn shadow_after_load(loaded: Result<(), WeightError>) -> bool {
    loaded.is_ok() && shadow_may_enable()
}

/// Over-budget inference is a silent primary. Silence is not `Pass`.
pub fn gate_inference(elapsed_us: u64, budget_ms: u64) -> InferenceGate {
    if budget_ms == 0 || elapsed_us > budget_ms.saturating_mul(1_000) {
        InferenceGate::Silent
    } else {
        InferenceGate::Finished
    }
}

pub fn inference_yields_pass(gate: InferenceGate) -> bool {
    matches!(gate, InferenceGate::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_mismatch_refuses_the_file() {
        let bytes = b"weights-v1";
        let wrong = "0000000000000000";
        assert_eq!(load_pinned(bytes, wrong), Err(WeightError::HashMismatch));
        assert!(!shadow_after_load(load_pinned(bytes, wrong)));
        let right = pin_hash(bytes);
        assert!(load_pinned(bytes, &right).is_ok());
        assert!(!shadow_after_load(load_pinned(bytes, &right)));
    }

    #[test]
    fn over_budget_inference_is_not_pass() {
        let gate = gate_inference(3_000, 2);
        assert_eq!(gate, InferenceGate::Silent);
        assert!(!inference_yields_pass(gate));
        assert!(inference_yields_pass(gate_inference(1_000, 2)));
    }

    #[test]
    fn shadow_is_off_until_power_and_deadline_close() {
        assert!(!shadow_may_enable());
        let note = include_str!("../../../docs/shadow-net.md");
        assert!(note.contains("SHADOW_DEFAULT_ON"));
        assert!(note.contains("compiled out"));
        assert!(note.contains("not raised"));
        assert!(note.contains("Group 1"));
        assert!(note.contains("unmeasured"));
    }
}
