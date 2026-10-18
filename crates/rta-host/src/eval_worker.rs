//! Preemptive deadline. The spec is evaluated on its own thread; the tick thread waits at
//! most `tick.deadline_ms` for the answer and never longer.
//!
//! The handoff is two channels of capacity one. One request is in flight at a time. A tick
//! that cannot hand off, or that does not get its own answer inside the deadline, is a
//! fault. A fault latches like an interpreter error: the worker is not asked again until
//! restart, and a late answer is discarded. A hung evaluator holds only its own thread.

use crate::eval::{Evaluator, TickResult};
use crate::monitor::BuiltMonitor;
use rta_spec::InputRecord;
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::thread;
use std::time::{Duration, Instant};

/// One evaluation. The real one owns the RTLola monitor, which is not `Send`, so it is built
/// on the worker thread.
pub trait Evaluate {
    fn evaluate(&mut self, record: &InputRecord, at_ms: u64) -> TickResult;
}

/// The spec monitor and the latching evaluator from `eval.rs`.
pub struct SpecEvaluator {
    monitor: BuiltMonitor,
    evaluator: Evaluator,
}

impl SpecEvaluator {
    pub fn new(monitor: BuiltMonitor) -> Self {
        Self {
            monitor,
            evaluator: Evaluator::default(),
        }
    }
}

impl Evaluate for SpecEvaluator {
    fn evaluate(&mut self, record: &InputRecord, at_ms: u64) -> TickResult {
        self.evaluator.tick(&mut self.monitor, record, at_ms)
    }
}

struct Request {
    sequence: u64,
    record: InputRecord,
    at_ms: u64,
}

struct Response {
    sequence: u64,
    result: TickResult,
}

/// What one tick got from the worker.
// `Done` carries a fixed `Reasons` (about 600 bytes). Boxing it would allocate every tick,
// which the per-tick allocation test forbids; the value lives on the stack.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The answer to this tick, inside the deadline.
    Done { result: TickResult, took: Duration },
    /// No answer inside the deadline. Latches.
    Late { took: Duration },
    /// The previous request is still running. Latches.
    Busy,
    /// The worker thread is gone. Latches.
    Gone,
    /// An earlier tick latched the fault. The worker is not asked.
    Faulted,
}

pub struct EvalWorker {
    requests: Option<SyncSender<Request>>,
    responses: Receiver<Response>,
    sequence: u64,
    faulted: bool,
    handle: Option<thread::JoinHandle<()>>,
}

impl EvalWorker {
    /// Start the worker and build the evaluator on it. A build error is returned here, before
    /// the caller binds a socket.
    pub fn start<E, F>(build: F) -> Result<Self, String>
    where
        E: Evaluate + 'static,
        F: FnOnce() -> Result<E, String> + Send + 'static,
    {
        let (request_tx, request_rx) = sync_channel::<Request>(1);
        let (response_tx, response_rx) = sync_channel::<Response>(1);
        let (ready_tx, ready_rx) = sync_channel::<Result<(), String>>(1);
        let handle = thread::Builder::new()
            .name("rta-eval".into())
            .spawn(move || {
                let mut evaluator = match build() {
                    Ok(evaluator) => {
                        let _ = ready_tx.send(Ok(()));
                        evaluator
                    }
                    Err(err) => {
                        let _ = ready_tx.send(Err(err));
                        return;
                    }
                };
                while let Ok(request) = request_rx.recv() {
                    let result = evaluator.evaluate(&request.record, request.at_ms);
                    let response = Response {
                        sequence: request.sequence,
                        result,
                    };
                    if response_tx.send(response).is_err() {
                        break;
                    }
                }
            })
            .map_err(|err| err.to_string())?;
        match ready_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                requests: Some(request_tx),
                responses: response_rx,
                sequence: 0,
                faulted: false,
                handle: Some(handle),
            }),
            Ok(Err(err)) => {
                let _ = handle.join();
                Err(err)
            }
            Err(_) => {
                let _ = handle.join();
                Err("evaluator thread exited before it was ready".into())
            }
        }
    }

    pub fn faulted(&self) -> bool {
        self.faulted
    }

    /// Hand one record to the worker and wait at most `deadline` for this tick's answer.
    pub fn evaluate(&mut self, record: InputRecord, at_ms: u64, deadline: Duration) -> Outcome {
        if self.faulted {
            return Outcome::Faulted;
        }
        let Some(requests) = &self.requests else {
            self.faulted = true;
            return Outcome::Gone;
        };
        // An answer to an earlier tick is not this tick's answer.
        while self.responses.try_recv().is_ok() {}
        self.sequence = self.sequence.wrapping_add(1);
        let started = Instant::now();
        let request = Request {
            sequence: self.sequence,
            record,
            at_ms,
        };
        match requests.try_send(request) {
            Ok(()) => {}
            Err(TrySendError::Full(_)) => {
                self.faulted = true;
                return Outcome::Busy;
            }
            Err(TrySendError::Disconnected(_)) => {
                self.faulted = true;
                return Outcome::Gone;
            }
        }
        loop {
            let left = deadline.saturating_sub(started.elapsed());
            match self.responses.recv_timeout(left) {
                Ok(response) if response.sequence == self.sequence => {
                    let took = started.elapsed();
                    if took > deadline {
                        self.faulted = true;
                        return Outcome::Late { took };
                    }
                    return Outcome::Done {
                        result: response.result,
                        took,
                    };
                }
                Ok(_) => continue,
                Err(RecvTimeoutError::Timeout) => {
                    self.faulted = true;
                    return Outcome::Late {
                        took: started.elapsed(),
                    };
                }
                Err(RecvTimeoutError::Disconnected) => {
                    self.faulted = true;
                    return Outcome::Gone;
                }
            }
        }
    }

    /// Stop the worker. An idle worker sees the closed channel and is joined. A faulted worker
    /// may be hung; it is detached so shutdown cannot wait on it.
    pub fn shutdown(mut self) {
        self.requests = None;
        if let Some(handle) = self.handle.take() {
            if !self.faulted {
                let _ = handle.join();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor::build_once;
    use rta_spec::Verdict;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    const DEADLINE: Duration = Duration::from_millis(50);

    fn record(fence_ok: bool) -> InputRecord {
        InputRecord::try_new(30.0, fence_ok, 10.0, 0.9, 400.0, 10.0, 10.0).unwrap()
    }

    /// Returns `Pass` after `delay`, or never while `hang` is set.
    struct Scripted {
        delay: Duration,
        hang: Arc<AtomicBool>,
        calls: u32,
    }

    impl Evaluate for Scripted {
        fn evaluate(&mut self, _record: &InputRecord, _at_ms: u64) -> TickResult {
            self.calls += 1;
            while self.hang.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(5));
            }
            thread::sleep(self.delay);
            TickResult {
                verdict: Verdict::Pass,
                reasons: crate::reason::Reasons::new(),
            }
        }
    }

    fn scripted(delay: Duration, hang: &Arc<AtomicBool>) -> EvalWorker {
        let hang = Arc::clone(hang);
        EvalWorker::start(move || {
            Ok(Scripted {
                delay,
                hang,
                calls: 0,
            })
        })
        .unwrap()
    }

    #[test]
    fn the_spec_evaluates_on_the_worker_and_matches_the_inline_verdict() {
        let mut worker = EvalWorker::start(|| {
            build_once(include_str!("../../../spec/monitor.lola"))
                .map(SpecEvaluator::new)
                .map_err(|err| format!("{err:?}"))
        })
        .unwrap();
        let wait = Duration::from_secs(2);
        match worker.evaluate(record(true), 10, wait) {
            Outcome::Done { result, .. } => assert_eq!(result.verdict, Verdict::Pass),
            other => panic!("{other:?}"),
        }
        match worker.evaluate(record(false), 20, wait) {
            Outcome::Done { result, .. } => assert_eq!(result.verdict, Verdict::Revert),
            other => panic!("{other:?}"),
        }
        assert!(!worker.faulted());
        worker.shutdown();
    }

    #[test]
    fn a_spec_build_error_returns_before_any_tick() {
        let err = EvalWorker::start(|| {
            build_once("input x: Bool\n")
                .map(SpecEvaluator::new)
                .map_err(|err| format!("{err:?}"))
        })
        .err()
        .unwrap();
        assert!(!err.is_empty());
    }

    #[test]
    // trace: H7
    fn a_hung_evaluator_is_late_within_one_deadline_and_latches() {
        let hang = Arc::new(AtomicBool::new(true));
        let mut worker = scripted(Duration::ZERO, &hang);
        let started = Instant::now();
        let outcome = worker.evaluate(record(true), 0, DEADLINE);
        let waited = started.elapsed();
        assert!(matches!(outcome, Outcome::Late { .. }), "{outcome:?}");
        assert!(waited >= DEADLINE, "{waited:?}");
        assert!(waited < DEADLINE * 3, "{waited:?}");
        assert!(worker.faulted());
        // The next tick does not wait on the hung thread at all.
        let started = Instant::now();
        assert_eq!(
            worker.evaluate(record(true), 50, DEADLINE),
            Outcome::Faulted
        );
        assert!(started.elapsed() < Duration::from_millis(5));
        // Shutdown detaches the hung worker instead of joining it.
        let started = Instant::now();
        worker.shutdown();
        assert!(started.elapsed() < Duration::from_millis(50));
        hang.store(false, Ordering::SeqCst);
    }

    #[test]
    fn a_slow_evaluator_that_finishes_past_the_deadline_is_late() {
        let hang = Arc::new(AtomicBool::new(false));
        let mut worker = scripted(DEADLINE * 2, &hang);
        let outcome = worker.evaluate(record(true), 0, DEADLINE);
        assert!(matches!(outcome, Outcome::Late { .. }), "{outcome:?}");
        // Its answer arrives later and is never read as a later tick's verdict.
        thread::sleep(DEADLINE * 3);
        assert_eq!(
            worker.evaluate(record(true), 50, DEADLINE),
            Outcome::Faulted
        );
        worker.shutdown();
    }

    #[test]
    fn a_fast_evaluator_is_unaffected() {
        let hang = Arc::new(AtomicBool::new(false));
        let mut worker = scripted(Duration::ZERO, &hang);
        for tick in 0..20 {
            match worker.evaluate(record(true), tick * 50, DEADLINE) {
                Outcome::Done { result, took } => {
                    assert_eq!(result.verdict, Verdict::Pass);
                    assert!(took <= DEADLINE);
                }
                other => panic!("{other:?}"),
            }
        }
        assert!(!worker.faulted());
        worker.shutdown();
    }

    #[test]
    fn a_dead_worker_is_gone_and_latches() {
        struct Dies;
        impl Evaluate for Dies {
            fn evaluate(&mut self, _record: &InputRecord, _at_ms: u64) -> TickResult {
                panic!("evaluator died");
            }
        }
        let mut worker = EvalWorker::start(|| Ok(Dies)).unwrap();
        let outcome = worker.evaluate(record(true), 0, Duration::from_secs(1));
        assert_eq!(outcome, Outcome::Gone);
        assert!(worker.faulted());
        worker.shutdown();
    }
}

#[cfg(test)]
mod doc_tests {
    #[test]
    fn the_deadline_note_names_the_handoff_the_latch_and_the_cost() {
        let note = include_str!("../../../docs/deadline.md");
        for item in [
            "capacity one",
            "latches like an interpreter error",
            "discarded by sequence number",
            "shutdown detaches it",
            "No queue grows and no thread is spawned per tick.",
            "instead of one tick",
        ] {
            assert!(note.contains(item), "{item}");
        }
        let requirements = include_str!("../../../docs/requirements.md");
        assert!(requirements.contains("whether or not eval returns, and latch revert"));
        let source = include_str!("eval_worker.rs");
        assert!(source.contains("sync_channel::<Request>(1)"));
        assert!(source.contains("sync_channel::<Response>(1)"));
    }
}
