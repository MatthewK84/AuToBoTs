//! Test harness only: tell a whole-box pause from a failure.
//!
//! The box that runs the stress loops is a virtual machine that is sometimes snapshotted and
//! resumed. Every thread stops at once, the kernel logs `random: crng reseeded due to virtual
//! machine fork`, and on resume `CLOCK_MONOTONIC` (Rust's `Instant`) has jumped by seconds.
//! A test that measures real time across that jump fails for a reason that is not in the code
//! (`docs/stress.md`).
//!
//! [`guard`] runs a test body while a sampler thread wakes every [`SAMPLE`] and records every
//! gap of at least [`PAUSE`] between two wakes. A body that passes passes. A body that fails
//! with no such gap inside it fails as usual. A body that fails across a gap is reported as
//! `host paused` and run again, and the rerun stands; [`ATTEMPTS`] failures, each across a
//! gap, fail as inconclusive. The guard never turns a failure into a pass by itself: the
//! test has to pass on a run the host did not pause.
//!
//! [`PAUSE`] is 1 s: three times the longest stall seen from CPU starvation alone (320 ms
//! under 16 busy loops on 8 cores) and two hundred times the 5 ms tick deadline, which this
//! does not touch.

use std::io::Write;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

/// How often the sampler wakes.
pub const SAMPLE: Duration = Duration::from_millis(5);
/// A gap between wakes at least this long is a pause of the whole host.
pub const PAUSE: Duration = Duration::from_secs(1);
/// Runs of one test body before a failure across every one is called inconclusive.
pub const ATTEMPTS: u32 = 3;

/// Gaps of at least [`PAUSE`] between two wakes, as (last wake before, first wake after).
pub struct Gaps(Mutex<Vec<(Instant, Instant)>>);

impl Gaps {
    pub const fn new() -> Self {
        Self(Mutex::new(Vec::new()))
    }

    pub fn push(&self, before: Instant, after: Instant) {
        lock(&self.0).push((before, after));
    }

    /// The longest gap that overlapped `from..to`, if any.
    pub fn between(&self, from: Instant, to: Instant) -> Option<Duration> {
        lock(&self.0)
            .iter()
            .filter(|(before, after)| *after >= from && *before <= to)
            .map(|(before, after)| after.duration_since(*before))
            .max()
    }
}

/// The gaps of this process, fed by one sampler thread started on first use.
fn host() -> &'static Gaps {
    static GAPS: Gaps = Gaps::new();
    static SAMPLER: OnceLock<()> = OnceLock::new();
    SAMPLER.get_or_init(|| {
        thread::Builder::new()
            .name("host-pause-sampler".into())
            .spawn(|| {
                let mut last = Instant::now();
                loop {
                    thread::sleep(SAMPLE);
                    let now = Instant::now();
                    if now.duration_since(last) >= PAUSE {
                        GAPS.push(last, now);
                    }
                    last = now;
                }
            })
            .unwrap();
        // Let the sampler take its first reading before anything is timed against it.
        thread::sleep(SAMPLE * 2);
    });
    &GAPS
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Record a pause where the stress script can count it: stderr, and a line in the file named
/// by `RTA_HOST_PAUSE_LOG` when that is set.
fn report(name: &str, attempt: u32, paused: Duration) {
    let line = format!("host paused: {name} attempt {attempt}: monotonic gap {paused:?}");
    eprintln!("{line}");
    if let Some(path) = std::env::var_os("RTA_HOST_PAUSE_LOG") {
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(file, "{line}");
        }
    }
}

/// Run a real-time test body under the pause guard. See the module note.
pub fn guard(body: impl Fn()) {
    guard_with(host(), body);
}

/// [`guard`] against a given record of gaps. The tests below use their own, so a pause they
/// stage cannot excuse a real test running beside them.
pub fn guard_with(gaps: &Gaps, body: impl Fn()) {
    let name = thread::current().name().unwrap_or("test").to_owned();
    for attempt in 1..=ATTEMPTS {
        let started = Instant::now();
        let outcome = panic::catch_unwind(AssertUnwindSafe(&body));
        let Err(failure) = outcome else {
            return;
        };
        // The sampler notices a gap on its first wake after it; give it that wake.
        thread::sleep(SAMPLE * 4);
        let Some(paused) = gaps.between(started, Instant::now()) else {
            panic::resume_unwind(failure);
        };
        report(&name, attempt, paused);
        if attempt == ATTEMPTS {
            panic!(
                "inconclusive, not a pass: {name} failed across a host pause in all \
                 {ATTEMPTS} attempts (last gap {paused:?})"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Stand in for a resume: a gap that ends now, inside the attempt, as the sampler would
    /// record it on its first wake after the host came back.
    fn stage_pause(gaps: &Gaps) {
        let now = Instant::now();
        gaps.push(now.checked_sub(PAUSE).unwrap(), now);
    }

    #[test]
    fn a_failure_with_no_pause_fails() {
        let gaps = Gaps::new();
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            guard_with(&gaps, || panic!("a real failure"));
        }));
        let message = result.unwrap_err();
        assert_eq!(message.downcast_ref::<&str>(), Some(&"a real failure"));
    }

    #[test]
    fn a_failure_across_a_pause_runs_again_and_the_rerun_stands() {
        let gaps = Gaps::new();
        let runs = AtomicU32::new(0);
        guard_with(&gaps, || {
            if runs.fetch_add(1, Ordering::SeqCst) == 0 {
                stage_pause(&gaps);
                panic!("timing broke across the pause");
            }
        });
        assert_eq!(runs.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_rerun_that_fails_with_no_pause_fails() {
        let gaps = Gaps::new();
        let runs = AtomicU32::new(0);
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            guard_with(&gaps, || {
                if runs.fetch_add(1, Ordering::SeqCst) == 0 {
                    stage_pause(&gaps);
                }
                panic!("broken either way");
            });
        }));
        assert!(result.is_err());
        assert_eq!(runs.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn failing_across_a_pause_every_time_is_inconclusive_not_a_pass() {
        let gaps = Gaps::new();
        let runs = AtomicU32::new(0);
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            guard_with(&gaps, || {
                runs.fetch_add(1, Ordering::SeqCst);
                stage_pause(&gaps);
                panic!("timing broke");
            });
        }));
        let message = result.unwrap_err();
        let text = message.downcast_ref::<String>().unwrap();
        assert!(text.starts_with("inconclusive, not a pass"), "{text}");
        assert_eq!(runs.load(Ordering::SeqCst), ATTEMPTS);
    }

    #[test]
    fn a_pause_before_the_attempt_does_not_excuse_it() {
        let gaps = Gaps::new();
        let long_ago = Instant::now();
        gaps.push(long_ago, long_ago + Duration::from_millis(1));
        thread::sleep(SAMPLE * 2);
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            guard_with(&gaps, || panic!("real"));
        }));
        assert!(result.is_err());
    }

    #[test]
    fn a_test_thread_that_sleeps_past_the_limit_is_not_a_pause() {
        // Only the sampler's own missed wakes count; the sampler runs while this sleeps.
        let from = Instant::now();
        let gaps = host();
        thread::sleep(PAUSE + SAMPLE * 4);
        assert_eq!(gaps.between(from, Instant::now()), None);
    }
}
