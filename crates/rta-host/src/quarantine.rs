//! Quarantine a candidate spec beside the running one. Swap is not immediate.

use crate::bundle::{accept, Bundle, BundleError, RunningSpec};
use crate::monitor::build_once;
use crate::replay::check;
use rta_spec::Verdict;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldFixture<'a> {
    pub name: &'a str,
    pub log: &'a str,
    pub must_hold: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivePath {
    pub spec: RunningSpec,
    pub weights: Vec<u8>,
    pub candidate_weights: Option<Vec<u8>>,
    pub quarantined: bool,
    pub latched: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapRecord {
    pub old_hash: String,
    pub new_hash: String,
    pub tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwapError {
    Bundle(BundleError),
    Fixture(&'static str),
    Deadline,
    Fault,
}

impl ActivePath {
    pub fn new(spec: RunningSpec) -> Self {
        Self {
            spec,
            weights: Vec::new(),
            candidate_weights: None,
            quarantined: true,
            latched: false,
        }
    }

    /// Learning may write a candidate file. It may not point the active path at it.
    pub fn write_candidate(&mut self, bytes: Vec<u8>) {
        self.candidate_weights = Some(bytes);
    }

    pub fn inference_weights(&self) -> &[u8] {
        &self.weights
    }

    pub fn command_spec(&self) -> &RunningSpec {
        &self.spec
    }

    pub fn verdict(&self, spec_verdict: Verdict) -> Verdict {
        if self.latched || !self.quarantined {
            Verdict::Revert
        } else {
            spec_verdict
        }
    }
}

pub struct SwapRequest<'a> {
    pub bundle: &'a Bundle,
    pub holds: &'a [HoldFixture<'a>],
    pub recent: &'a str,
    pub eval_us: u64,
    pub deadline_us: u64,
    pub tick: u64,
    pub fault: bool,
}

pub fn quarantine_swap(
    active: &mut ActivePath,
    request: SwapRequest<'_>,
) -> Result<SwapRecord, SwapError> {
    let SwapRequest {
        bundle,
        holds,
        recent,
        eval_us,
        deadline_us,
        tick,
        fault,
    } = request;
    let command_before = active.spec.clone();
    let accepted = match accept(&active.spec, bundle) {
        Ok(spec) => spec,
        Err(err) => return Err(SwapError::Bundle(err)),
    };
    if build_once(&accepted.text).is_err() || fault {
        active.latched = true;
        active.spec = command_before;
        return Err(SwapError::Fault);
    }
    if eval_us > deadline_us {
        active.spec = command_before;
        return Err(SwapError::Deadline);
    }
    for hold in holds.iter().filter(|hold| hold.must_hold) {
        if check(hold.log, &accepted.text).is_err() {
            active.spec = command_before;
            return Err(SwapError::Fixture("must_hold"));
        }
    }
    if !recent.is_empty() && check(recent, &accepted.text).is_err() {
        active.spec = command_before;
        return Err(SwapError::Fixture("recent"));
    }
    let record = SwapRecord {
        old_hash: command_before.hash,
        new_hash: accepted.hash.clone(),
        tick,
    };
    active.spec = accepted;
    active.quarantined = true;
    active.latched = false;
    if let Some(weights) = &bundle.weights {
        active.weights = weights.clone();
        active.candidate_weights = None;
    }
    let _ = command_before;
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::{sign_manifest, spec_pin};

    fn running() -> RunningSpec {
        let text = include_str!("../../../spec/monitor.lola").to_string();
        let hash = spec_pin(&text);
        RunningSpec { text, hash }
    }

    fn signed(running: &RunningSpec, spec: &str, weights: Option<Vec<u8>>) -> Bundle {
        let new_hash = spec_pin(spec);
        Bundle {
            spec: spec.to_string(),
            weights,
            previous_hash: running.hash.clone(),
            new_hash: new_hash.clone(),
            signature: Some(sign_manifest(&running.hash, &new_hash)),
        }
    }

    fn pass_log() -> &'static str {
        include_str!("../../../crates/rta-host/fixtures/golden/pass.log")
    }

    fn flipped(text: &str) -> String {
        text.replace(
            "output fix_stale: Bool := fix_age_ms > 500.0",
            "output fix_stale: Bool := fix_age_ms > 0.0",
        )
    }

    #[test]
    fn a_spec_that_flips_must_hold_is_refused() {
        let spec = running();
        let mut active = ActivePath::new(spec.clone());
        let before = active.command_spec().clone();
        let candidate = flipped(&spec.text);
        let bundle = signed(&spec, &candidate, None);
        let holds = [HoldFixture {
            name: "pass",
            log: pass_log(),
            must_hold: true,
        }];
        let err = quarantine_swap(
            &mut active,
            SwapRequest {
                bundle: &bundle,
                holds: &holds,
                recent: "",
                eval_us: 100,
                deadline_us: 5_000,
                tick: 4,
                fault: false,
            },
        )
        .expect_err("flipped");
        assert_eq!(err, SwapError::Fixture("must_hold"));
        assert_eq!(active.command_spec(), &before);
        assert!(!active.latched);
    }

    #[test]
    fn a_candidate_weight_file_does_not_change_inference() {
        let spec = running();
        let mut active = ActivePath::new(spec.clone());
        active.weights = b"active".to_vec();
        active.write_candidate(b"learned".to_vec());
        assert_eq!(active.inference_weights(), b"active");
        let bundle = signed(&spec, &spec.text, Some(b"from-bundle".to_vec()));
        let holds = [HoldFixture {
            name: "pass",
            log: pass_log(),
            must_hold: true,
        }];
        let record = quarantine_swap(
            &mut active,
            SwapRequest {
                bundle: &bundle,
                holds: &holds,
                recent: "",
                eval_us: 100,
                deadline_us: 5_000,
                tick: 9,
                fault: false,
            },
        )
        .expect("swap");
        assert_eq!(record.tick, 9);
        assert_eq!(record.old_hash, spec.hash);
        assert_eq!(active.inference_weights(), b"from-bundle");
        assert!(active.candidate_weights.is_none());
    }

    #[test]
    fn a_fault_mid_swap_cannot_leave_the_command_path_on_the_new_spec() {
        let spec = running();
        let mut active = ActivePath::new(spec.clone());
        let before = active.command_spec().clone();
        let bundle = signed(&spec, &spec.text, None);
        let err = quarantine_swap(
            &mut active,
            SwapRequest {
                bundle: &bundle,
                holds: &[],
                recent: "",
                eval_us: 100,
                deadline_us: 5_000,
                tick: 3,
                fault: true,
            },
        )
        .expect_err("fault");
        assert_eq!(err, SwapError::Fault);
        assert_eq!(active.command_spec(), &before);
        assert!(active.latched);
        assert_eq!(active.verdict(Verdict::Pass), Verdict::Revert);
        let source = include_str!("quarantine.rs");
        let clear = ["fn ", "clear_fault"].concat();
        assert!(!source.contains(&clear));
    }

    #[test]
    fn an_over_deadline_eval_is_refused() {
        let spec = running();
        let mut active = ActivePath::new(spec.clone());
        let before = active.command_spec().clone();
        let bundle = signed(&spec, &spec.text, None);
        let err = quarantine_swap(
            &mut active,
            SwapRequest {
                bundle: &bundle,
                holds: &[],
                recent: "",
                eval_us: 6_000,
                deadline_us: 5_000,
                tick: 1,
                fault: false,
            },
        )
        .expect_err("deadline");
        assert_eq!(err, SwapError::Deadline);
        assert_eq!(active.command_spec(), &before);
    }
}
