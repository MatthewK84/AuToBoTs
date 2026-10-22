//! Integrated runtime. One tick thread, one receive thread, one watchdog thread.
//!
//! The tick thread ages samples, builds the record, evaluates the spec, passes the verdict
//! through `FailClosed` and `decide`, sends the outbound message, and writes the tick log.
//! The receive thread applies inbound MAVLink to the shared vehicle state and hands each
//! `COMMAND_ACK` to the recovery latch. The watchdog thread reads the published tick only and,
//! while the tick thread is stalled, drives the same recovery latch on its own socket handle.

use crate::clock::age_ms;
use crate::command_out::{reported_mode, time_boot_ms, to_message, Target};
use crate::config::{self, Config};
use crate::eval_worker::{EvalWorker, Outcome};
use crate::fail_closed::{FailClosed, TickCase};
use crate::fence::{fence_ok, FenceInput};
use crate::gate::{self, Gate, Inputs};
use crate::link::{LinkError, UdpLink};
use crate::link_age::{LinkAge, LinkMessage};
use crate::mavlink_link::{
    decode_frames, encode, heartbeat, CommandAck, Frame, MAX_FRAME, MAX_FRAMES_PER_DATAGRAM,
};
use crate::reason::{Reason, Reasons};
use crate::recovery::{AckResult, Latch, Policy, State, Step};
use crate::spec::SpecLimits;
use crate::stale::present_track;
use crate::tick_log::{command_name, TickLog, TickRecord};
use crate::watchdog::Watchdog;
use mavlink::dialects::common::{MavAutopilot, MavMessage, MavModeFlag};
use mavlink::MavHeader;
use rta_spec::{InputRecord, RecoveryMode, Verdict};
use rta_switch::{Complex, SwitchCommand};
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

/// Companion heartbeat period. `docs/icd-fc.md` names 1 Hz.
pub const HEARTBEAT_PERIOD_MS: u64 = 1_000;
/// Receive timeout. Bounds how long the receive thread takes to see the stop flag.
pub const RECV_TIMEOUT: Duration = Duration::from_millis(20);
/// Receive buffer. One UDP datagram, several frames.
const RECV_BUFFER: usize = 2_048;

/// Monotonic milliseconds since the runtime started. One clock for every age.
#[derive(Debug, Clone, Copy)]
pub struct Clock {
    start: Instant,
}

impl Clock {
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub fn now_ms(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

/// Inbound state the tick reads. Fixed size. A status text is not stored.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Vehicle {
    pub fc_heartbeat_ms: Option<u64>,
    pub fc_custom_mode: Option<u32>,
    pub fc_system: Option<u8>,
    /// The component id in the flight controller's `HEARTBEAT` header.
    pub fc_component: Option<u8>,
    /// `MAV_MODE_FLAG_SAFETY_ARMED` in the last flight-controller `HEARTBEAT`.
    pub fc_armed: bool,
    pub fix_ms: Option<u64>,
    pub north_m: f64,
    pub east_m: f64,
    pub alt_m: f64,
    pub link: LinkAge,
}

impl Vehicle {
    /// Where commands go: the system and component of the last flight-controller
    /// `HEARTBEAT`. `None` until one is heard; nothing is commanded before that.
    pub fn fc_target(&self) -> Option<Target> {
        Some(Target {
            system: self.fc_system?,
            component: self.fc_component?,
        })
    }
}

/// Apply one inbound message. Ages use local receive time, never the sender timestamp.
pub fn apply_message(vehicle: &mut Vehicle, header: &MavHeader, message: &MavMessage, now_ms: u64) {
    match message {
        MavMessage::HEARTBEAT(beat) => {
            // A ground station or another companion reports no autopilot. It is not the
            // flight controller and does not reset the flight-controller heartbeat age.
            if beat.autopilot == MavAutopilot::MAV_AUTOPILOT_INVALID {
                return;
            }
            vehicle.fc_heartbeat_ms = Some(now_ms);
            vehicle.fc_custom_mode = Some(beat.custom_mode);
            vehicle.fc_system = Some(header.system_id);
            vehicle.fc_component = Some(header.component_id);
            vehicle.fc_armed = beat
                .base_mode
                .contains(MavModeFlag::MAV_MODE_FLAG_SAFETY_ARMED);
            vehicle.link.observe(LinkMessage::Command, now_ms);
        }
        MavMessage::LOCAL_POSITION_NED(position) => {
            vehicle.link.observe(LinkMessage::Command, now_ms);
            let (north, east, down) = (
                f64::from(position.x),
                f64::from(position.y),
                f64::from(position.z),
            );
            if north.is_finite() && east.is_finite() && down.is_finite() {
                vehicle.north_m = north;
                vehicle.east_m = east;
                vehicle.alt_m = -down;
                vehicle.fix_ms = Some(now_ms);
            }
        }
        MavMessage::STATUSTEXT(_) => vehicle.link.observe(LinkMessage::StatusText, now_ms),
        _ => vehicle.link.observe(LinkMessage::Command, now_ms),
    }
}

/// The record one tick presents to the spec, plus the host-side bits the log carries.
#[derive(Debug, Clone, PartialEq)]
pub struct Presented {
    pub record: InputRecord,
    pub fence_ok: bool,
}

/// Age the samples and build the record. A record the constructor rejects is an `Err`; the
/// tick treats it as no verdict.
pub fn present(config: &Config, vehicle: &Vehicle, now_ms: u64) -> Result<Presented, &'static str> {
    let fence_ok = match (&config.fence, vehicle.fix_ms) {
        (
            config::Fence::Host {
                polygon,
                vertical_cap_m,
                v_max,
                a_max,
            },
            Some(_),
        ) => fence_ok(
            [vehicle.north_m, vehicle.east_m],
            vehicle.alt_m,
            FenceInput {
                polygon,
                vertical_cap_m: *vertical_cap_m,
                v_max: *v_max,
                a_max: *a_max,
                mode_change_latency_s: config.tick.mode_change_latency_ms as f64 / 1_000.0,
            },
        ),
        // No fix is no fence. `source = "fc"` is not the early trip, so the host does not
        // claim the aircraft is inside.
        (config::Fence::Host { .. }, None) | (config::Fence::Fc, _) => false,
    };
    // No tracker is wired into the runtime. A lost tracker is inhibit (`docs/degraded.md`):
    // zero confidence at zero range is a weak track inside commit range.
    let track = present_track(false, 0.0, 0.0, 0.0, config.tick.tracker_hold_ms);
    let record = InputRecord::try_new(
        vehicle.alt_m,
        fence_ok,
        age_ms(vehicle.fix_ms, now_ms),
        track.track_conf,
        track.spec_range_m.unwrap_or(0.0),
        vehicle.link.age_ms(now_ms),
        age_ms(vehicle.fc_heartbeat_ms, now_ms),
    )?;
    Ok(Presented { record, fence_ok })
}

pub fn gate_mode_of(config: &Config) -> gate::Mode {
    match config.startup.mode {
        config::StartupMode::Armed => gate::Mode::Armed,
        config::StartupMode::Immediate => gate::Mode::Immediate,
    }
}

/// Ready: the flight controller has been heard and the fix is no older than
/// `startup.ready_fix_age_ms`.
pub fn ready(config: &Config, vehicle: &Vehicle, now_ms: u64) -> bool {
    vehicle.fc_heartbeat_ms.is_some()
        && vehicle
            .fix_ms
            .is_some_and(|at| now_ms.saturating_sub(at) <= config.startup.ready_fix_age_ms)
}

/// The resend schedule. `heartbeat_stale_ms` is the spec's limit (`spec::limits`): after an
/// accepting ack the next resend waits at least that long for a heartbeat to show the mode.
pub fn policy_of(config: &Config, heartbeat_stale_ms: f64) -> Policy {
    // A spec limit is a finite, non-negative number (`spec::limits`); anything else holds 0.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let ack_hold_ms = if heartbeat_stale_ms.is_finite() && heartbeat_stale_ms > 0.0 {
        heartbeat_stale_ms.ceil() as u64
    } else {
        0
    };
    Policy {
        initial_ms: config.recovery.retry_initial_ms,
        cap_ms: config.recovery.retry_cap_ms,
        attempts_per_step: config.recovery.attempts_per_step,
        ack_hold_ms,
    }
}

pub fn recovery_of(config: &Config) -> RecoveryMode {
    match config.recovery.mode {
        config::RecoveryMode::Rtl => RecoveryMode::Rtl,
        config::RecoveryMode::Loiter => RecoveryMode::Loiter,
        config::RecoveryMode::Land => RecoveryMode::Land,
    }
}

/// `decide` takes a reported mode, not an option. A flight controller that is in no recovery
/// mode is passed as a recovery mode that is not the configured one, so Revert still sends.
pub fn reported_or_other(reported: Option<RecoveryMode>, recovery: RecoveryMode) -> RecoveryMode {
    match reported {
        Some(mode) => mode,
        None => match recovery {
            RecoveryMode::Rtl => RecoveryMode::Loiter,
            RecoveryMode::Loiter | RecoveryMode::Land => RecoveryMode::Rtl,
        },
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// One outbound path: a socket handle, a sequence, and a fixed frame buffer.
struct Sender {
    link: UdpLink,
    system_id: u8,
    component_id: u8,
    sequence: u8,
    buffer: [u8; MAX_FRAME],
}

impl Sender {
    fn new(link: UdpLink, config: &Config) -> Self {
        Self {
            link,
            system_id: config.link.system_id,
            component_id: config.link.component_id,
            sequence: 0,
            buffer: [0; MAX_FRAME],
        }
    }

    fn send(&mut self, message: &MavMessage) -> Result<(), String> {
        let header = MavHeader {
            system_id: self.system_id,
            component_id: self.component_id,
            sequence: self.sequence,
        };
        self.sequence = self.sequence.wrapping_add(1);
        let count = encode(header, message, &mut self.buffer)?;
        let frame = self.buffer.get(..count).ok_or("frame longer than buffer")?;
        self.link.send(frame).map_err(|err| format!("{err:?}"))
    }
}

/// State the three threads share. Nothing here grows.
struct Shared {
    vehicle: Mutex<Vehicle>,
    stop: Arc<AtomicBool>,
    published_ms: AtomicU64,
    /// The one recovery-in-progress latch (`recovery.rs`). The tick thread and the watchdog
    /// both drive it under this lock, so one attempt is never sent twice.
    recovery: Mutex<Latch>,
    watchdog_fired: AtomicBool,
    /// The startup gate let the last tick act. The watchdog acts only when it did.
    gate_open: AtomicBool,
}

impl Shared {
    fn snapshot(&self) -> Vehicle {
        *lock(&self.vehicle)
    }

    fn recovery_engaged(&self) -> bool {
        lock(&self.recovery).engaged()
    }

    fn engage_recovery(&self, now_ms: u64) {
        lock(&self.recovery).engage(now_ms);
    }

    /// Send the recovery step that is due, if any, holding the latch lock across the send.
    /// Returns the step that went on the wire. A local send error leaves the step due.
    ///
    /// The clock is read here, under the latch lock, and not taken from the caller. A tick
    /// reads its clock at the top and can block for a long time before it gets here (a slow
    /// complex function, the evaluation deadline, a contended lock, a descheduled thread).
    /// Polled and recorded with that stale time, the send is stamped earlier than it went
    /// out and the next resend falls due early by the delay. The send is recorded at the
    /// time it completed, so the next one waits at least the full policy wait after it.
    fn drive_recovery(
        &self,
        sender: &mut Sender,
        target: Target,
        clock: &Clock,
        reported: Option<RecoveryMode>,
    ) -> Option<Step> {
        let mut latch = lock(&self.recovery);
        let step = latch.poll(clock.now_ms(), reported)?;
        let ok = sender
            .send(&step.message(target, latch.confirmation()))
            .is_ok();
        latch.sent(clock.now_ms(), ok);
        ok.then_some(step)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum RunError {
    Link(LinkError),
    Thread(String),
}

/// What one run did. The tick log is the record; this is for the caller and the tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RunReport {
    pub ticks: u64,
    pub overruns: u64,
    pub recovery_sent: bool,
    pub recovery_attempts: u32,
    pub recovery_confirmed: bool,
    pub watchdog_fired: bool,
}

pub struct RunOptions {
    /// Stop after this many ticks. `None` runs until `stop` is set.
    pub max_ticks: Option<u64>,
    /// Set by SIGINT, or by a caller, to begin the shutdown order.
    pub stop: Arc<AtomicBool>,
    /// Limits read from the loaded spec (`spec::limits`). The host keeps no copy.
    pub limits: SpecLimits,
}

/// Run until `stop` or `max_ticks`. Startup has already loaded config, built the monitor on
/// the evaluation worker,
/// opened the log, and bound the link, in that order (`docs/startup.md`).
pub fn run<W: Write>(
    config: &Config,
    mut worker: EvalWorker,
    log: &mut TickLog<W>,
    spec_hash: u64,
    link: UdpLink,
    complex: &mut dyn Complex,
    options: RunOptions,
) -> Result<RunReport, RunError> {
    let clock = Clock::start();
    let recovery = recovery_of(config);
    let period = Duration::from_millis(config.tick.period_ms);
    let deadline = Duration::from_millis(config.tick.deadline_ms);
    let shared = Arc::new(Shared {
        vehicle: Mutex::new(Vehicle::default()),
        stop: Arc::clone(&options.stop),
        published_ms: AtomicU64::new(0),
        recovery: Mutex::new(Latch::new(
            recovery,
            policy_of(config, options.limits.heartbeat_stale_ms),
        )),
        watchdog_fired: AtomicBool::new(false),
        gate_open: AtomicBool::new(false),
    });

    let receiver = spawn_receiver(link.try_clone().map_err(RunError::Link)?, &shared, clock)?;
    let watchdog = spawn_watchdog(
        Sender::new(link.try_clone().map_err(RunError::Link)?, config),
        &shared,
        clock,
        config,
    );
    let watchdog = match watchdog {
        Ok(handle) => handle,
        Err(err) => {
            shared.stop.store(true, Ordering::SeqCst);
            let _ = receiver.join();
            return Err(err);
        }
    };

    let mut sender = Sender::new(link, config);
    let mut gate = Gate::new(gate_mode_of(config), config.startup.ready_timeout_ms);
    let mut closed = FailClosed::new(options.limits);
    let mut report = RunReport::default();
    let mut log_failed = false;
    let mut last_heartbeat_ms: Option<u64> = None;
    let mut next = Instant::now();

    while !shared.stop.load(Ordering::SeqCst)
        && options.max_ticks.is_none_or(|limit| report.ticks < limit)
    {
        report.ticks += 1;
        let now_ms = clock.now_ms();
        if last_heartbeat_ms.is_none_or(|last| now_ms.saturating_sub(last) >= HEARTBEAT_PERIOD_MS) {
            let _ = sender.send(&heartbeat());
            last_heartbeat_ms = Some(now_ms);
        }

        let vehicle = shared.snapshot();
        let reported = vehicle.fc_custom_mode.and_then(reported_mode);
        let request = complex.request();
        let mut reasons = Reasons::new();
        let (verdict, late, eval_ms, presented) = match present(config, &vehicle, now_ms) {
            Ok(presented) => {
                // The tick thread waits at most `deadline` for the worker, whether or not the
                // evaluator returns. A miss latches like an interpreter fault.
                let millis = |took: Duration| u64::try_from(took.as_millis()).unwrap_or(u64::MAX);
                match worker.evaluate(presented.record.clone(), now_ms, deadline) {
                    Outcome::Done { result, took } => {
                        reasons = result.reasons;
                        (Some(result.verdict), false, millis(took), Some(presented))
                    }
                    Outcome::Late { took } => (None, true, millis(took), Some(presented)),
                    Outcome::Busy | Outcome::Gone | Outcome::Faulted => {
                        reasons.push(Reason::EvalFault);
                        (None, false, 0, Some(presented))
                    }
                }
            }
            Err(_) => {
                reasons.push(Reason::Record);
                (None, false, 0, None)
            }
        };
        if late {
            reasons.push(Reason::Late);
        }
        // The startup gate: monitor only while disarmed or not ready (`gate.rs`).
        let heard = vehicle.fc_heartbeat_ms.is_some();
        let gated = gate.step(
            now_ms,
            Inputs {
                heard,
                armed: vehicle.fc_armed,
                ready: ready(config, &vehicle, now_ms),
            },
            &mut reasons,
        );
        shared.gate_open.store(gated.act, Ordering::SeqCst);
        // Recovery in progress, a failed log write, an evaluation fault, or an arm before
        // readiness latches Revert until restart.
        let latched =
            shared.recovery_engaged() || log_failed || worker.faulted() || gated.force_revert;
        if latched {
            reasons.push(Reason::Latched);
        }
        let command = closed.command(TickCase {
            verdict: if latched {
                Some(Verdict::Revert)
            } else {
                verdict
            },
            late,
            heartbeat_age_ms: Some(age_ms(vehicle.fc_heartbeat_ms, now_ms)),
            request,
            reported: reported_or_other(reported, recovery),
            recovery,
            write_ok: !log_failed,
            eval_fault: worker.faulted(),
        });
        // The logged verdict is the one the switch acted on. Mode and Idle come only from
        // Revert; a forced Revert shows here even when the spec said Pass.
        let logged_verdict = match command {
            SwitchCommand::Mode { .. } | SwitchCommand::Idle => Verdict::Revert,
            SwitchCommand::Setpoints { .. } => verdict.unwrap_or(Verdict::Revert),
        };

        // Commands go to the system and component of the flight controller's heartbeat.
        let target = vehicle.fc_target();
        if let (Some(target), true) = (target, gated.act) {
            match command {
                SwitchCommand::Mode { .. } => shared.engage_recovery(now_ms),
                SwitchCommand::Setpoints { .. } if request.is_none() => {
                    // The switch fills a missing request with zeros. A zero setpoint is a
                    // real position, so nothing is forwarded when nothing was asked.
                    reasons.push(Reason::NoRequest);
                }
                SwitchCommand::Setpoints { .. } => {
                    match to_message(&command, target, time_boot_ms(clock.now_ms())) {
                        Some(message) => {
                            let _ = sender.send(&message);
                        }
                        None => reasons.push(Reason::BadSetpoint),
                    }
                }
                SwitchCommand::Idle => {}
            }
            // Resends run whatever this tick decided, until the latch is confirmed or used up.
            if let Some(step) = shared.drive_recovery(&mut sender, target, &clock, reported) {
                reasons.push(Reason::Sent(step));
            }
        } else if target.is_none() {
            // Nothing is commanded to a flight controller that has not been heard.
            reasons.push(Reason::NoFc);
        } else {
            // Monitor only. The gate has logged `disarmed` or `not_ready`.
        }
        lock(&shared.recovery).reasons(&mut reasons);

        let (fence, record) = match &presented {
            Some(presented) => (presented.fence_ok, Some(&presented.record)),
            None => (false, None),
        };
        let line = TickRecord {
            tick: report.ticks,
            fence_ok: fence,
            fix_age_ms: record.map_or(f64::MAX, InputRecord::fix_age_ms),
            link_age_ms: record.map_or(f64::MAX, InputRecord::link_age_ms),
            fc_heartbeat_age_ms: record.map_or(f64::MAX, InputRecord::fc_heartbeat_age_ms),
            track_conf: record.map_or(0.0, InputRecord::track_conf),
            range_m: record.map_or(0.0, InputRecord::range_m),
            verdict: logged_verdict,
            filter_intervened: false,
            reasons,
            command: command_name(&command),
            eval_ms,
            spec_hash,
            alt_m: vehicle.alt_m,
            // `String::new` does not allocate.
            indication: String::new(),
        };
        if log.append(&line).is_err() {
            log_failed = true;
        }
        shared.published_ms.store(clock.now_ms(), Ordering::SeqCst);

        next += period;
        let now = Instant::now();
        if next > now {
            thread::sleep(next - now);
        } else {
            report.overruns += 1;
            next = now;
        }
    }

    // Shutdown order: stop setpoints, recovery mode, flush log, exit (`docs/startup.md`).
    shared.stop.store(true, Ordering::SeqCst);
    let vehicle = shared.snapshot();
    let reported = vehicle.fc_custom_mode.and_then(reported_mode);
    let may_act = shared.gate_open.load(Ordering::SeqCst)
        && (gate_mode_of(config) == gate::Mode::Immediate || vehicle.fc_armed);
    if let (Some(target), true) = (vehicle.fc_target(), may_act && reported != Some(recovery)) {
        let now_ms = clock.now_ms();
        shared.engage_recovery(now_ms);
        shared.drive_recovery(&mut sender, target, &clock, reported);
    }
    let _ = log.flush();
    worker.shutdown();
    let _ = receiver.join();
    let _ = watchdog.join();
    {
        let latch = lock(&shared.recovery);
        report.recovery_attempts = latch.attempts();
        report.recovery_sent = latch.attempts() > 0;
        report.recovery_confirmed = latch.state() == State::Confirmed;
    }
    report.watchdog_fired = shared.watchdog_fired.load(Ordering::SeqCst);
    Ok(report)
}

fn spawn_receiver(
    link: UdpLink,
    shared: &Arc<Shared>,
    clock: Clock,
) -> Result<thread::JoinHandle<()>, RunError> {
    let shared = Arc::clone(shared);
    thread::Builder::new()
        .name("rta-recv".into())
        .spawn(move || {
            let mut buffer = [0_u8; RECV_BUFFER];
            while !shared.stop.load(Ordering::SeqCst) {
                match link.recv(&mut buffer) {
                    Ok(Some(count)) => {
                        let now_ms = clock.now_ms();
                        let datagram = buffer.get(..count).unwrap_or_default();
                        // Fixed room: a datagram yields at most MAX_FRAMES_PER_DATAGRAM frames.
                        let mut acks = [None::<CommandAck>; MAX_FRAMES_PER_DATAGRAM];
                        let mut count = 0;
                        {
                            let mut vehicle = lock(&shared.vehicle);
                            decode_frames(datagram, |header, frame| match frame {
                                Frame::Message(message) => {
                                    apply_message(&mut vehicle, header, message, now_ms);
                                }
                                Frame::CommandAck(ack) => {
                                    vehicle.link.observe(LinkMessage::Command, now_ms);
                                    // Only the flight controller's answer counts.
                                    let from = Target {
                                        system: header.system_id,
                                        component: header.component_id,
                                    };
                                    if vehicle.fc_target() == Some(from) {
                                        if let Some(slot) = acks.get_mut(count) {
                                            *slot = Some(ack);
                                            count += 1;
                                        }
                                    }
                                }
                            });
                        }
                        // The vehicle lock is released before the latch lock is taken. An ack
                        // is timed under the latch lock, like a send, so a wait for the lock
                        // does not shorten the wait the ack grants.
                        let mut latch = lock(&shared.recovery);
                        let acked_ms = clock.now_ms();
                        for ack in acks.iter().flatten() {
                            latch.on_ack(acked_ms, ack.command, AckResult::from_raw(ack.result));
                        }
                    }
                    Ok(None) => {}
                    Err(_) => thread::sleep(RECV_TIMEOUT),
                }
            }
        })
        .map_err(|err| RunError::Thread(err.to_string()))
}

fn spawn_watchdog(
    mut sender: Sender,
    shared: &Arc<Shared>,
    clock: Clock,
    config: &Config,
) -> Result<thread::JoinHandle<()>, RunError> {
    let shared = Arc::clone(shared);
    let mut dog = Watchdog::new(config.tick.period_ms, config.tick.watchdog_misses);
    let poll = Duration::from_millis((config.tick.period_ms / 2).max(1));
    let armed_gate = gate_mode_of(config) == gate::Mode::Armed;
    thread::Builder::new()
        .name("rta-watchdog".into())
        .spawn(move || {
            while !shared.stop.load(Ordering::SeqCst) {
                thread::sleep(poll);
                let now_ms = clock.now_ms();
                dog.publish(shared.published_ms.load(Ordering::SeqCst));
                let vehicle = shared.snapshot();
                if vehicle.fc_heartbeat_ms.is_none() || !dog.stalled(now_ms) {
                    continue;
                }
                // The same startup gate as the tick thread: the last tick's verdict, and the
                // arm state now, since the tick thread is not there to see a disarm.
                if !shared.gate_open.load(Ordering::SeqCst) || (armed_gate && !vehicle.fc_armed) {
                    continue;
                }
                // The tick thread is stalled. Engage the same latch and send what is due.
                shared.engage_recovery(now_ms);
                if shared.stop.load(Ordering::SeqCst) {
                    continue;
                }
                let reported = vehicle.fc_custom_mode.and_then(reported_mode);
                let Some(target) = vehicle.fc_target() else {
                    continue;
                };
                if shared
                    .drive_recovery(&mut sender, target, &clock, reported)
                    .is_some()
                {
                    shared.watchdog_fired.store(true, Ordering::SeqCst);
                }
            }
        })
        .map_err(|err| RunError::Thread(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_out::{custom_mode, PX4_MAIN_MODE_AUTO, PX4_SUB_MODE_AUTO_RTL};
    use crate::eval_worker::{Evaluate, SpecEvaluator};
    use crate::link::{open, Endpoint};
    use crate::mavlink_link::decode_each;
    use crate::monitor::build_once;
    use mavlink::dialects::common::{
        MavCmd, MavModeFlag, MavResult, MavState, MavType, COMMAND_ACK_DATA, HEARTBEAT_DATA,
        LOCAL_POSITION_NED_DATA, SET_POSITION_TARGET_LOCAL_NED_DATA,
    };
    use rta_switch::{Request, StubComplex};
    use std::net::{SocketAddr, UdpSocket};
    use std::path::PathBuf;

    const SPEC: &str = include_str!("../../../spec/monitor.lola");
    /// PX4 POSCTL: main mode 3, no sub-mode. Not a recovery mode.
    const PX4_POSCTL: u32 = 0x0003_0000;

    fn config(fc_port: u16, log: &str) -> Config {
        config::parse_str(&format!(
            r#"
[link]
endpoint = "udp:127.0.0.1:{fc_port}"
system_id = 1
component_id = 191

[tick]
period_ms = 100
# 80 of 100 ms: a cold first evaluation on a loaded runner can take over 50 ms, and a
# late tick latches Revert, which the nominal tests forbid.
deadline_ms = 80
watchdog_misses = 3
mode_change_latency_ms = 200
tracker_hold_ms = 200

[recovery]
mode = "rtl"
retry_initial_ms = 300
retry_cap_ms = 600
attempts_per_step = 2

[spec]
path = "spec/monitor.lola"

[log]
path = "{log}"
flush_every_n = 1

[fence]
source = "host"
polygon = [[0.0, 0.0], [200.0, 0.0], [200.0, 200.0], [0.0, 200.0]]
vertical_cap_m = 120.0
v_max = 15.0
a_max = 3.0

[startup]
mode = "armed"
ready_timeout_ms = 30000
ready_fix_age_ms = 200
"#
        ))
        .unwrap()
    }

    fn log_path(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("rta-runtime-{}-{name}.log", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    /// One `DO_SET_MODE` as the fake received it: param1, param2, param3, arrival.
    type SetMode = (f32, f32, f32, Instant);

    /// What the fake flight controller saw from the host.
    #[derive(Debug, Default, Clone)]
    struct Seen {
        set_mode: Vec<SetMode>,
        /// `MAV_CMD_NAV_RETURN_TO_LAUNCH` and `MAV_CMD_NAV_LAND`, with arrival.
        nav: Vec<(MavCmd, Instant)>,
        setpoints: usize,
        heartbeats: usize,
        /// Every `SET_POSITION_TARGET_LOCAL_NED` as decoded.
        setpoint_data: Vec<SET_POSITION_TARGET_LOCAL_NED_DATA>,
        /// `target_system` and `target_component` of every `COMMAND_LONG`.
        command_targets: Vec<(u8, u8)>,
    }

    impl Seen {
        /// Every recovery command in arrival order: the command and when it arrived.
        fn recovery(&self) -> Vec<(MavCmd, Instant)> {
            let mut all: Vec<(MavCmd, Instant)> = self
                .set_mode
                .iter()
                .map(|(_, _, _, at)| (MavCmd::MAV_CMD_DO_SET_MODE, *at))
                .chain(self.nav.iter().copied())
                .collect();
            all.sort_by_key(|(_, at)| *at);
            all
        }
    }

    /// How the fake answers a recovery command.
    #[derive(Debug, Clone, Copy)]
    enum Px4 {
        /// Switch, ack `ACCEPTED`, and heartbeat the new mode at once.
        Obey,
        /// Lose the first `n` recovery commands on the link, then obey.
        DropFirst(usize),
        /// Never switch. Ack with this result, or not at all.
        Ignore(Option<MavResult>),
        /// Ack `DO_SET_MODE` with `DENIED` and stay put; obey the NAV commands.
        DenySetMode,
    }

    /// A PX4-like peer on loopback. It heartbeats with its custom mode, streams
    /// `LOCAL_POSITION_NED` from `position`, and adopts a commanded AUTO sub-mode the way PX4
    /// reports it. It stops on `stop`.
    struct FakeFc {
        socket: UdpSocket,
        px4: Px4,
        position: Arc<Mutex<(f32, f32, f32)>>,
        /// Reported in `HEARTBEAT.base_mode`. A change is heartbeated at once.
        armed: Arc<AtomicBool>,
        /// Off: no `LOCAL_POSITION_NED` at all, so the host has no fix.
        streaming: Arc<AtomicBool>,
        seen: Arc<Mutex<Seen>>,
        stop: Arc<AtomicBool>,
        /// System and component ids in every frame the fake sends.
        ids: (u8, u8),
    }

    impl FakeFc {
        fn bind() -> Self {
            let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
            socket
                .set_read_timeout(Some(Duration::from_millis(10)))
                .unwrap();
            Self {
                socket,
                px4: Px4::Obey,
                position: Arc::new(Mutex::new((100.0, 100.0, -30.0))),
                armed: Arc::new(AtomicBool::new(true)),
                streaming: Arc::new(AtomicBool::new(true)),
                seen: Arc::new(Mutex::new(Seen::default())),
                stop: Arc::new(AtomicBool::new(false)),
                ids: (1, 1),
            }
        }

        fn port(&self) -> u16 {
            self.socket.local_addr().unwrap().port()
        }

        fn spawn(&self, host: SocketAddr) -> thread::JoinHandle<()> {
            let socket = self.socket.try_clone().unwrap();
            let position = Arc::clone(&self.position);
            let seen = Arc::clone(&self.seen);
            let stop = Arc::clone(&self.stop);
            let px4 = self.px4;
            let armed = Arc::clone(&self.armed);
            let streaming = Arc::clone(&self.streaming);
            let (system_id, component_id) = self.ids;
            thread::spawn(move || {
                let mut was_armed = armed.load(Ordering::SeqCst);
                let base_mode = |armed: bool| {
                    let mut flags = MavModeFlag::MAV_MODE_FLAG_CUSTOM_MODE_ENABLED;
                    if armed {
                        flags |= MavModeFlag::MAV_MODE_FLAG_SAFETY_ARMED;
                    }
                    flags
                };
                let mut mode = PX4_POSCTL;
                let mut lost = 0_usize;
                let mut sequence = 0_u8;
                let mut buffer = [0_u8; MAX_FRAME];
                let mut inbound = [0_u8; RECV_BUFFER];
                let mut last_beat: Option<Instant> = None;
                let mut send = |message: &MavMessage, sequence: &mut u8| {
                    let header = MavHeader {
                        system_id,
                        component_id,
                        sequence: *sequence,
                    };
                    *sequence = sequence.wrapping_add(1);
                    let count = encode(header, message, &mut buffer).unwrap();
                    let _ = socket.send_to(&buffer[..count], host);
                };
                while !stop.load(Ordering::SeqCst) {
                    // Position before heartbeat. The host commands only after it has heard a
                    // heartbeat, and a heartbeat with no fix yet is a real Revert. Sending the
                    // fix first keeps the start of every test nominal.
                    let (x, y, z) = *lock(&position);
                    if streaming.load(Ordering::SeqCst) {
                        send(
                            &MavMessage::LOCAL_POSITION_NED(LOCAL_POSITION_NED_DATA {
                                time_boot_ms: 0,
                                x,
                                y,
                                z,
                                vx: 0.0,
                                vy: 0.0,
                                vz: 0.0,
                            }),
                            &mut sequence,
                        );
                    }
                    let now_armed = armed.load(Ordering::SeqCst);
                    if now_armed != was_armed {
                        was_armed = now_armed;
                        last_beat = None;
                    }
                    if last_beat.is_none_or(|at| at.elapsed() >= Duration::from_millis(100)) {
                        send(
                            &MavMessage::HEARTBEAT(HEARTBEAT_DATA {
                                custom_mode: mode,
                                mavtype: MavType::MAV_TYPE_QUADROTOR,
                                autopilot: MavAutopilot::MAV_AUTOPILOT_PX4,
                                base_mode: base_mode(now_armed),
                                system_status: MavState::MAV_STATE_ACTIVE,
                                mavlink_version: 3,
                            }),
                            &mut sequence,
                        );
                        last_beat = Some(Instant::now());
                    }
                    if let Ok((count, _)) = socket.recv_from(&mut inbound) {
                        let mut answers: Vec<MavMessage> = Vec::new();
                        decode_each(&inbound[..count], |_, message| {
                            let mut seen = lock(&seen);
                            let command = match message {
                                MavMessage::COMMAND_LONG(data) => {
                                    seen.command_targets
                                        .push((data.target_system, data.target_component));
                                    data
                                }
                                MavMessage::SET_POSITION_TARGET_LOCAL_NED(data) => {
                                    seen.setpoints += 1;
                                    seen.setpoint_data.push(data.clone());
                                    return;
                                }
                                MavMessage::HEARTBEAT(_) => {
                                    seen.heartbeats += 1;
                                    return;
                                }
                                _ => return,
                            };
                            // The mode PX4 would switch to for this command.
                            let target = match command.command {
                                MavCmd::MAV_CMD_DO_SET_MODE => {
                                    seen.set_mode.push((
                                        command.param1,
                                        command.param2,
                                        command.param3,
                                        Instant::now(),
                                    ));
                                    (command.param2 == f32::from(PX4_MAIN_MODE_AUTO)).then(|| {
                                        ((command.param3 as u32) << 24)
                                            | (u32::from(PX4_MAIN_MODE_AUTO) << 16)
                                    })
                                }
                                MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH => {
                                    seen.nav.push((command.command, Instant::now()));
                                    Some(custom_mode(RecoveryMode::Rtl))
                                }
                                MavCmd::MAV_CMD_NAV_LAND => {
                                    seen.nav.push((command.command, Instant::now()));
                                    Some(custom_mode(RecoveryMode::Land))
                                }
                                _ => None,
                            };
                            let Some(target) = target else {
                                return;
                            };
                            let ack = |result| {
                                MavMessage::COMMAND_ACK(COMMAND_ACK_DATA {
                                    command: command.command,
                                    result,
                                })
                            };
                            let obey = match px4 {
                                Px4::Obey => true,
                                Px4::DropFirst(n) => {
                                    lost += 1;
                                    lost > n
                                }
                                Px4::Ignore(result) => {
                                    answers.extend(result.map(ack));
                                    false
                                }
                                Px4::DenySetMode => {
                                    let set_mode = command.command == MavCmd::MAV_CMD_DO_SET_MODE;
                                    if set_mode {
                                        answers.push(ack(MavResult::MAV_RESULT_DENIED));
                                    }
                                    !set_mode
                                }
                            };
                            if obey {
                                mode = target;
                                answers.push(ack(MavResult::MAV_RESULT_ACCEPTED));
                                // Report the new mode at once, as PX4 does on a mode change.
                                last_beat = None;
                            }
                        });
                        for answer in &answers {
                            send(answer, &mut sequence);
                        }
                    }
                    if last_beat.is_none() {
                        send(
                            &MavMessage::HEARTBEAT(HEARTBEAT_DATA {
                                custom_mode: mode,
                                mavtype: MavType::MAV_TYPE_QUADROTOR,
                                autopilot: MavAutopilot::MAV_AUTOPILOT_PX4,
                                base_mode: base_mode(now_armed),
                                system_status: MavState::MAV_STATE_ACTIVE,
                                mavlink_version: 3,
                            }),
                            &mut sequence,
                        );
                        last_beat = Some(Instant::now());
                    }
                    thread::sleep(Duration::from_millis(20));
                }
            })
        }
    }

    fn host_link(fc: &FakeFc) -> UdpLink {
        open(
            &Endpoint::Udp {
                host: "127.0.0.1".into(),
                port: fc.port(),
            },
            RECV_TIMEOUT,
        )
        .unwrap()
    }

    fn loopback(link: &UdpLink) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], link.local_addr().unwrap().port()))
    }

    fn spec_worker() -> EvalWorker {
        EvalWorker::start(|| {
            build_once(SPEC)
                .map(SpecEvaluator::new)
                .map_err(|err| format!("{err:?}"))
        })
        .unwrap()
    }

    fn options(ticks: u64) -> RunOptions {
        RunOptions {
            max_ticks: Some(ticks),
            stop: Arc::new(AtomicBool::new(false)),
            limits: crate::spec::limits(SPEC).unwrap(),
        }
    }

    #[test]
    fn loop_reverts_on_a_fence_breach_and_sends_px4_auto_rtl() {
        let fc = FakeFc::bind();
        let path = log_path("fence");
        let config = config(fc.port(), path.to_str().unwrap());
        let link = host_link(&fc);
        let fc_thread = fc.spawn(loopback(&link));
        let position = Arc::clone(&fc.position);
        let breach = thread::spawn(move || {
            thread::sleep(Duration::from_millis(900));
            *lock(&position) = (600.0, 600.0, -30.0);
        });
        let worker = spec_worker();
        let mut log = TickLog::open(&path, config.log.flush_every_n, 7).unwrap();
        let report = run(
            &config,
            worker,
            &mut log,
            7,
            link,
            &mut StubComplex,
            options(20),
        )
        .unwrap();
        breach.join().unwrap();
        thread::sleep(Duration::from_millis(100));
        fc.stop.store(true, Ordering::SeqCst);
        fc_thread.join().unwrap();

        assert_eq!(report.ticks, 20);
        assert!(report.recovery_sent);
        assert!(!report.watchdog_fired);
        let seen = lock(&fc.seen);
        assert!(seen.heartbeats >= 1, "{seen:?}");
        // StubComplex asks for nothing, so no zero setpoint is forwarded.
        assert_eq!(seen.setpoints, 0, "{seen:?}");
        // One recovery, AUTO + RTL sub-mode, then silence: the fake reports AUTO_RTL after.
        assert_eq!(seen.set_mode.len(), 1, "{seen:?}");
        let (param1, param2, param3, _) = seen.set_mode[0];
        assert_eq!(param1, 1.0);
        assert_eq!(param2, f32::from(PX4_MAIN_MODE_AUTO));
        assert_eq!(param3, f32::from(PX4_SUB_MODE_AUTO_RTL));

        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("# rta-log 1 spec_hash=0000000000000007"));
        assert_eq!(lines.len(), 21, "{text}");
        // Before the breach: fresh samples inside the fence, no tracker, so inhibit.
        assert!(
            lines.iter().any(|line| line.contains("fence_ok=1")
                && line.contains("verdict=inhibit")
                && line.contains("command=setpoints")),
            "{text}"
        );
        // The breach tick: fence_ok goes to 0, Revert, one mode command.
        let breach_line = lines
            .iter()
            .skip(1)
            .find(|line| line.contains("fence_ok=0") && !line.contains("no_fc"))
            .unwrap_or_else(|| panic!("{text}"));
        assert!(breach_line.contains("verdict=revert"), "{breach_line}");
        assert!(breach_line.contains("command=mode"), "{breach_line}");
        // After the flight controller reports AUTO_RTL, a revert tick sends nothing.
        assert!(lines.last().unwrap().contains("command=idle"), "{text}");
        let _ = std::fs::remove_file(&path);
    }

    /// A complex function that blocks the tick thread once.
    struct Stall {
        tick: u64,
        at: u64,
        hold: Duration,
        window: Arc<Mutex<Option<(Instant, Instant)>>>,
    }

    impl Complex for Stall {
        fn request(&mut self) -> Option<Request> {
            self.tick += 1;
            if self.tick == self.at {
                let started = Instant::now();
                thread::sleep(self.hold);
                *lock(&self.window) = Some((started, Instant::now()));
            }
            None
        }
    }

    #[test]
    fn watchdog_sends_recovery_while_the_tick_thread_is_stalled() {
        let fc = FakeFc::bind();
        let path = log_path("stall");
        let config = config(fc.port(), path.to_str().unwrap());
        let link = host_link(&fc);
        let fc_thread = fc.spawn(loopback(&link));
        let window = Arc::new(Mutex::new(None));
        let mut stall = Stall {
            tick: 0,
            at: 6,
            hold: Duration::from_millis(1_200),
            window: Arc::clone(&window),
        };
        let worker = spec_worker();
        let mut log = TickLog::open(&path, config.log.flush_every_n, 0).unwrap();
        let report = run(&config, worker, &mut log, 0, link, &mut stall, options(9)).unwrap();
        fc.stop.store(true, Ordering::SeqCst);
        fc_thread.join().unwrap();

        assert!(report.watchdog_fired, "{report:?}");
        assert!(report.recovery_sent);
        let (started, ended) = lock(&window).unwrap();
        let seen = lock(&fc.seen);
        assert_eq!(seen.set_mode.len(), 1, "{seen:?}");
        let (_, param2, param3, at) = seen.set_mode[0];
        assert_eq!(param2, f32::from(PX4_MAIN_MODE_AUTO));
        assert_eq!(param3, f32::from(PX4_SUB_MODE_AUTO_RTL));
        // The mode arrived while the tick thread was still blocked.
        assert!(
            at > started && at < ended,
            "{at:?} not in {started:?}..{ended:?}"
        );
        // After the stall the tick thread sees the latch and never resumes setpoints.
        let text = std::fs::read_to_string(&path).unwrap();
        let after: Vec<&str> = text
            .lines()
            .filter(|line| line.contains("latched"))
            .collect();
        assert!(!after.is_empty(), "{text}");
        assert!(
            after.iter().all(|line| line.contains("verdict=revert")),
            "{text}"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// The real spec until call `at`, then hung (never returns while `release` is unset) or
    /// slow by `slow`.
    struct Faulty {
        inner: SpecEvaluator,
        calls: u64,
        at: u64,
        slow: Option<Duration>,
        release: Arc<AtomicBool>,
        faulted_at: Arc<Mutex<Option<Instant>>>,
    }

    impl Evaluate for Faulty {
        fn evaluate(&mut self, record: &InputRecord, at_ms: u64) -> crate::eval::TickResult {
            self.calls += 1;
            if self.calls == self.at {
                *lock(&self.faulted_at) = Some(Instant::now());
                match self.slow {
                    Some(delay) => thread::sleep(delay),
                    None => {
                        while !self.release.load(Ordering::SeqCst) {
                            thread::sleep(Duration::from_millis(5));
                        }
                    }
                }
            }
            self.inner.evaluate(record, at_ms)
        }
    }

    /// Run the fake PX4 against an evaluator that faults on call 6, and return what it saw.
    fn run_with_faulty_eval(
        name: &str,
        slow: Option<Duration>,
    ) -> (RunReport, Vec<SetMode>, Instant, String) {
        let fc = FakeFc::bind();
        let path = log_path(name);
        let config = config(fc.port(), path.to_str().unwrap());
        let link = host_link(&fc);
        let fc_thread = fc.spawn(loopback(&link));
        let release = Arc::new(AtomicBool::new(false));
        let faulted_at = Arc::new(Mutex::new(None));
        let worker = {
            let release = Arc::clone(&release);
            let faulted_at = Arc::clone(&faulted_at);
            EvalWorker::start(move || {
                let inner = build_once(SPEC)
                    .map(SpecEvaluator::new)
                    .map_err(|err| format!("{err:?}"))?;
                Ok(Faulty {
                    inner,
                    calls: 0,
                    at: 6,
                    slow,
                    release,
                    faulted_at,
                })
            })
            .unwrap()
        };
        let mut log = TickLog::open(&path, 1, 0).unwrap();
        let started = Instant::now();
        let report = run(
            &config,
            worker,
            &mut log,
            0,
            link,
            &mut StubComplex,
            options(10),
        )
        .unwrap();
        // A hung evaluator must not hold up shutdown.
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "{:?}",
            started.elapsed()
        );
        release.store(true, Ordering::SeqCst);
        fc.stop.store(true, Ordering::SeqCst);
        fc_thread.join().unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        let faulted_at = lock(&faulted_at).unwrap_or_else(|| panic!("never faulted\n{text}"));
        let seen = lock(&fc.seen).set_mode.clone();
        (report, seen, faulted_at, text)
    }

    fn assert_reverted_on_the_faulted_tick(
        report: &RunReport,
        seen: &[SetMode],
        faulted_at: Instant,
        text: &str,
    ) {
        // The tick thread sent it, inside one deadline plus link slack. The watchdog did not
        // have to.
        assert!(report.recovery_sent, "{report:?}");
        assert!(!report.watchdog_fired, "{report:?}");
        assert_eq!(report.ticks, 10, "{report:?}");
        assert_eq!(seen.len(), 1, "{seen:?}");
        let (param1, param2, param3, at) = seen[0];
        assert_eq!((param1, param2, param3), (1.0, 4.0, 5.0));
        // Nothing was commanded before the fault, and the command followed it promptly. The
        // bound is loose for a loaded runner; the deadline itself is checked by eval_ms below.
        assert!(at >= faulted_at, "recovery before the fault\n{text}");
        let after = at.duration_since(faulted_at);
        assert!(after < Duration::from_millis(500), "{after:?}\n{text}");
        let lines: Vec<&str> = text.lines().filter(|l| l.starts_with("tick=")).collect();
        assert_eq!(lines.len(), 10, "{text}");
        // The first line that names the fault is the faulted tick, within the run.
        let faulted = lines
            .iter()
            .find(|line| line.contains("late"))
            .unwrap_or_else(|| panic!("{text}"));
        assert!(faulted.contains("latched"), "{text}");
        // No earlier tick reverted: the loop was nominal until the fault.
        for line in lines.iter().take_while(|line| *line != faulted) {
            assert!(
                line.contains("no_fc") || line.contains("verdict=inhibit"),
                "{line}\n{text}"
            );
        }
        assert!(faulted.contains("verdict=revert"), "{faulted}");
        assert!(faulted.contains("command=mode"), "{text}");
        let eval_ms: u64 = faulted
            .split_whitespace()
            .find_map(|field| field.strip_prefix("eval_ms="))
            .and_then(|value| value.parse().ok())
            .unwrap_or(u64::MAX);
        // The tick thread gave up at the deadline, not when the evaluator finished.
        assert!((80..300).contains(&eval_ms), "{faulted}");
        // Every later tick stays Revert without asking the worker again.
        let index = lines.iter().position(|line| line == faulted).unwrap();
        for line in &lines[index + 1..] {
            assert!(line.contains("verdict=revert"), "{line}");
            assert!(line.contains("eval_fault"), "{line}");
        }
    }

    #[test]
    fn a_hung_evaluator_reverts_within_one_deadline_and_the_fc_gets_recovery() {
        let (report, seen, faulted_at, text) = run_with_faulty_eval("hung", None);
        assert_reverted_on_the_faulted_tick(&report, &seen, faulted_at, &text);
    }

    #[test]
    fn a_slow_evaluator_past_the_deadline_reverts_and_latches() {
        let (report, seen, faulted_at, text) =
            run_with_faulty_eval("slow", Some(Duration::from_millis(200)));
        assert_reverted_on_the_faulted_tick(&report, &seen, faulted_at, &text);
    }

    /// Outside the 200 m square fence from the first fix, so the first tick after the first
    /// heartbeat reverts. No timed breach: nothing depends on when a tick lands.
    const OUTSIDE: (f32, f32, f32) = (600.0, 600.0, -30.0);

    /// Run the loop against a fake PX4 that starts outside the fence and answers recovery
    /// commands as `px4` says.
    fn run_outside(
        name: &str,
        px4: Px4,
        tune: impl FnOnce(&mut Config),
        ticks: u64,
        complex: &mut dyn Complex,
    ) -> (RunReport, Seen, String) {
        let mut fc = FakeFc::bind();
        fc.px4 = px4;
        *lock(&fc.position) = OUTSIDE;
        run_fc(name, &fc, tune, ticks, complex)
    }

    /// Run the loop against a prepared fake. Returns what the fake saw and the log.
    fn run_fc(
        name: &str,
        fc: &FakeFc,
        tune: impl FnOnce(&mut Config),
        ticks: u64,
        complex: &mut dyn Complex,
    ) -> (RunReport, Seen, String) {
        let path = log_path(name);
        let mut config = config(fc.port(), path.to_str().unwrap());
        tune(&mut config);
        let link = host_link(fc);
        let fc_thread = fc.spawn(loopback(&link));
        let mut log = TickLog::open(&path, 1, 0).unwrap();
        let report = run(
            &config,
            spec_worker(),
            &mut log,
            0,
            link,
            complex,
            options(ticks),
        )
        .unwrap();
        // Let a last datagram, such as a shutdown resend, land before the fake stops reading.
        thread::sleep(Duration::from_millis(200));
        fc.stop.store(true, Ordering::SeqCst);
        fc_thread.join().unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        let seen = lock(&fc.seen).clone();
        (report, seen, text)
    }

    /// Arrival jitter of the fake: it reads every 20 ms plus a 10 ms timeout, more on a
    /// loaded runner. A duplicate send shows as a gap near zero, far under any wait.
    const ARRIVAL_SLACK: Duration = Duration::from_millis(100);

    /// No resend arrived earlier than the policy allows after the one before it.
    fn assert_never_early(config_wait: impl Fn(u32) -> u64, sends: &[(MavCmd, Instant)]) {
        for (index, pair) in sends.windows(2).enumerate() {
            let gap = pair[1].1.duration_since(pair[0].1);
            let attempt = u32::try_from(index + 1).unwrap();
            let wait = Duration::from_millis(config_wait(attempt));
            assert!(
                gap + ARRIVAL_SLACK >= wait,
                "resend {attempt} after {gap:?}, policy wait {wait:?}: {sends:?}"
            );
        }
    }

    fn test_policy() -> Policy {
        policy_of(
            &config(1, "unused.log"),
            crate::spec::limits(SPEC).unwrap().heartbeat_stale_ms,
        )
    }

    fn tick_lines(text: &str) -> Vec<&str> {
        text.lines().filter(|l| l.starts_with("tick=")).collect()
    }

    #[test]
    fn a_dropped_first_recovery_command_is_resent_and_confirmed() {
        let (report, seen, text) =
            run_outside("dropped", Px4::DropFirst(1), |_| {}, 15, &mut StubComplex);
        let sends = seen.recovery();
        assert_eq!(sends.len(), 2, "{sends:?}\n{text}");
        assert!(sends
            .iter()
            .all(|(command, _)| *command == MavCmd::MAV_CMD_DO_SET_MODE));
        assert!(seen
            .set_mode
            .iter()
            .all(|(p1, p2, p3, _)| (*p1, *p2, *p3) == (1.0, 4.0, 5.0)));
        let policy = test_policy();
        assert_never_early(|n| policy.wait_after(n), &sends);
        assert!(report.recovery_confirmed, "{report:?}\n{text}");
        assert_eq!(report.recovery_attempts, 2, "{report:?}");
        assert!(!report.watchdog_fired);
        let lines = tick_lines(&text);
        assert!(
            lines.iter().any(|l| l.contains("recovery_confirmed")),
            "{text}"
        );
        assert!(lines.last().unwrap().contains("command=idle"), "{text}");
    }

    #[test]
    fn a_vehicle_that_never_switches_gets_the_documented_order_then_silence() {
        // 300 ms, 600 ms cap, two per step: six sends over about 2.7 s, used up at 3.3 s.
        let (report, seen, text) =
            run_outside("never", Px4::Ignore(None), |_| {}, 45, &mut StubComplex);
        let sends = seen.recovery();
        let commands: Vec<MavCmd> = sends.iter().map(|(command, _)| *command).collect();
        assert_eq!(
            commands,
            [
                MavCmd::MAV_CMD_DO_SET_MODE,
                MavCmd::MAV_CMD_DO_SET_MODE,
                MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH,
                MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH,
                MavCmd::MAV_CMD_NAV_LAND,
                MavCmd::MAV_CMD_NAV_LAND,
            ],
            "{text}"
        );
        let policy = test_policy();
        assert_never_early(|n| policy.wait_after(n), &sends);
        // The cadence is bounded above too: a resend is never held back by much more than
        // a tick plus the arrival jitter. Loose for a loaded runner.
        for (index, pair) in sends.windows(2).enumerate() {
            let gap = pair[1].1.duration_since(pair[0].1);
            let wait = policy.wait_after(u32::try_from(index + 1).unwrap());
            assert!(
                gap < Duration::from_millis(wait + 600),
                "{gap:?}: {sends:?}"
            );
        }
        assert!(report.recovery_sent);
        assert!(!report.recovery_confirmed);
        assert_eq!(report.recovery_attempts, 6);
        assert_eq!(seen.setpoints, 0);
        let lines = tick_lines(&text);
        let last = lines.last().unwrap();
        for item in [
            "recovery_unconfirmed",
            "attempts:6",
            "latched",
            "verdict=revert",
        ] {
            assert!(last.contains(item), "{item}: {last}");
        }
    }

    #[test]
    fn denied_set_mode_falls_back_to_return_to_launch_at_once() {
        // A long schedule, so a fallback that waited for it would be plain to see.
        let (report, seen, text) = run_outside(
            "denied",
            Px4::DenySetMode,
            |config| {
                config.recovery.retry_initial_ms = 2_000;
                config.recovery.retry_cap_ms = 2_000;
            },
            15,
            &mut StubComplex,
        );
        let sends = seen.recovery();
        let commands: Vec<MavCmd> = sends.iter().map(|(command, _)| *command).collect();
        assert_eq!(
            commands,
            [
                MavCmd::MAV_CMD_DO_SET_MODE,
                MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH
            ],
            "{text}"
        );
        let gap = sends[1].1.duration_since(sends[0].1);
        assert!(gap < Duration::from_millis(1_000), "{gap:?}\n{text}");
        assert!(report.recovery_confirmed, "{report:?}\n{text}");
        let lines = tick_lines(&text);
        assert!(
            lines
                .iter()
                .any(|l| l.contains("fallback:denied") && l.contains("step:nav_rtl")),
            "{text}"
        );
        assert!(
            lines.last().unwrap().contains("recovery_confirmed"),
            "{text}"
        );
    }

    #[test]
    fn an_accepted_ack_with_no_mode_report_keeps_resending() {
        let (report, seen, text) = run_outside(
            "accepted",
            Px4::Ignore(Some(MavResult::MAV_RESULT_ACCEPTED)),
            |_| {},
            50,
            &mut StubComplex,
        );
        let sends = seen.recovery();
        // An accepted ack is not a confirmed mode: the second DO_SET_MODE goes out, then
        // the order moves on.
        assert!(sends.len() >= 3, "{sends:?}\n{text}");
        assert_eq!(seen.set_mode.len(), 2, "{text}");
        let policy = test_policy();
        assert_never_early(|n| policy.wait_after(n), &sends);
        // Every send here was accepted, so each resend waited for a heartbeat to show the
        // mode: at least the hold, which is longer than one 1 Hz heartbeat period. PX4 SITL
        // reports a mode change in HEARTBEAT up to a second after it accepts it.
        assert!(policy.ack_hold_ms > 1_000, "{policy:?}");
        let hold = Duration::from_millis(policy.ack_hold_ms);
        for pair in sends.windows(2) {
            let gap = pair[1].1.duration_since(pair[0].1);
            assert!(
                gap + ARRIVAL_SLACK >= hold,
                "resend {gap:?} after an accepted ack: {sends:?}"
            );
        }
        assert!(!report.recovery_confirmed);
        let lines = tick_lines(&text);
        assert!(
            lines
                .iter()
                .any(|l| l.contains("ack:accepted") && l.contains("recovery_pending")),
            "{text}"
        );
    }

    #[test]
    fn the_tick_and_the_watchdog_never_send_the_same_attempt() {
        // The tick thread blocks for 1.5 s while resends are due. The watchdog takes over the
        // same latch, then the tick thread comes back to it.
        let window = Arc::new(Mutex::new(None));
        let mut stall = Stall {
            tick: 0,
            at: 6,
            hold: Duration::from_millis(1_500),
            window: Arc::clone(&window),
        };
        let (report, seen, text) = run_outside("shared", Px4::Ignore(None), |_| {}, 20, &mut stall);
        let sends = seen.recovery();
        assert!(report.watchdog_fired, "{report:?}\n{text}");
        // Every send the fake saw is one attempt the latch counted, and no two are close.
        assert_eq!(
            u32::try_from(sends.len()).unwrap(),
            report.recovery_attempts,
            "{sends:?}"
        );
        let policy = test_policy();
        assert_never_early(|n| policy.wait_after(n), &sends);
        let (started, ended) = lock(&window).unwrap();
        assert!(
            sends.iter().any(|(_, at)| *at > started && *at < ended),
            "no resend during the stall: {sends:?}"
        );
        assert!(
            sends.iter().any(|(_, at)| *at > ended),
            "the tick thread did not resume the resends: {sends:?}"
        );
    }

    /// A complex function that blocks for `hold` on a tick when, by what the fake has seen,
    /// a resend is due: after the tick has read the clock and before it drives the latch.
    /// It never blocks long enough to wake the watchdog the test configures.
    struct SlowWhenDue {
        seen: Arc<Mutex<Seen>>,
        policy: Policy,
        /// Attempts in the whole fallback order. Nothing is due once they are used up.
        attempts: u32,
        hold: Duration,
        stalls: u32,
    }

    impl Complex for SlowWhenDue {
        fn request(&mut self) -> Option<Request> {
            let sends = lock(&self.seen).recovery();
            // Sends leave on the tick grid and arrive up to 30 ms later, so a resend falls due
            // on the tick that sees about one wait, less half a period, since the last arrival.
            let attempt = u32::try_from(sends.len()).unwrap();
            let due = attempt < self.attempts
                && sends.last().is_some_and(|(_, last)| {
                    let wait = Duration::from_millis(self.policy.wait_after(attempt));
                    last.elapsed() + Duration::from_millis(50) >= wait
                });
            if due && self.stalls > 0 {
                self.stalls -= 1;
                thread::sleep(self.hold);
            }
            None
        }
    }

    #[test]
    fn a_slow_tick_does_not_pull_the_next_resend_early() {
        // A tick that reads the clock, then blocks for 400 ms before it sends a due resend.
        // The watchdog is held off, so only the tick thread sends. The next resend must wait
        // the full policy wait after the send that went on the wire. Timed from the clock
        // reading at the top of the slow tick, it falls due 400 ms early and the next, fast,
        // tick sends it about 200 ms after the one before.
        let mut fc = FakeFc::bind();
        fc.px4 = Px4::Ignore(None);
        *lock(&fc.position) = OUTSIDE;
        let policy = test_policy();
        let mut slow = SlowWhenDue {
            seen: Arc::clone(&fc.seen),
            policy,
            attempts: policy.attempts_per_step * 3,
            hold: Duration::from_millis(400),
            stalls: 3,
        };
        let (report, seen, text) = run_fc(
            "slow-tick",
            &fc,
            |config| config.tick.watchdog_misses = 50,
            40,
            &mut slow,
        );
        let sends = seen.recovery();
        assert!(!report.watchdog_fired, "{report:?}\n{text}");
        assert_eq!(slow.stalls, 0, "the slow ticks did not all run: {sends:?}");
        assert_eq!(
            u32::try_from(sends.len()).unwrap(),
            report.recovery_attempts,
            "{sends:?}"
        );
        assert_never_early(|n| policy.wait_after(n), &sends);
    }

    /// A complex function that asks for nothing and runs scripted actions on given ticks, on
    /// the tick thread. Actions keyed to ticks, not to wall time.
    struct Script {
        tick: u64,
        steps: Vec<(u64, Box<dyn FnMut()>)>,
    }

    impl Script {
        fn new() -> Self {
            Self {
                tick: 0,
                steps: Vec::new(),
            }
        }

        fn at(mut self, tick: u64, action: impl FnMut() + 'static) -> Self {
            self.steps.push((tick, Box::new(action)));
            self
        }
    }

    impl Complex for Script {
        fn request(&mut self) -> Option<Request> {
            self.tick += 1;
            for (at, action) in &mut self.steps {
                if *at == self.tick {
                    action();
                }
            }
            None
        }
    }

    fn set(flag: &Arc<AtomicBool>, value: bool) -> impl FnMut() + 'static {
        let flag = Arc::clone(flag);
        move || flag.store(value, Ordering::SeqCst)
    }

    /// Asks for one fixed setpoint every tick and moves the fake outside the fence at
    /// `breach_at`.
    struct Asks {
        tick: u64,
        request: Request,
        breach_at: u64,
        position: Arc<Mutex<(f32, f32, f32)>>,
    }

    impl Complex for Asks {
        fn request(&mut self) -> Option<Request> {
            self.tick += 1;
            if self.tick == self.breach_at {
                *lock(&self.position) = OUTSIDE;
            }
            Some(self.request)
        }
    }

    #[test]
    fn setpoints_and_recovery_go_to_the_heard_flight_controller_with_yaw_and_boot_time() {
        let mut fc = FakeFc::bind();
        // Not the configured companion id (1), and not the old hard-coded component (1).
        fc.ids = (42, 7);
        let request = Request {
            north: 120.0,
            east: 80.0,
            down: -25.0,
            yaw: 0.75,
            commit: true,
        };
        let mut asks = Asks {
            tick: 0,
            request,
            breach_at: 12,
            position: Arc::clone(&fc.position),
        };
        let started = Instant::now();
        let (report, seen, text) = run_fc("targets", &fc, |_| {}, 18, &mut asks);
        let elapsed_ms = u32::try_from(started.elapsed().as_millis()).unwrap();
        assert!(report.recovery_sent, "{text}");
        assert!(seen.setpoints >= 3, "{seen:?}\n{text}");
        for data in &seen.setpoint_data {
            assert_eq!(data.type_mask.bits(), 0x09F8, "{data:?}");
            assert_eq!((data.x, data.y, data.z), (120.0, 80.0, -25.0));
            assert_eq!(data.yaw, 0.75);
            assert_eq!((data.target_system, data.target_component), (42, 7));
            assert!(
                data.time_boot_ms <= elapsed_ms,
                "{data:?} after {elapsed_ms} ms"
            );
        }
        // The boot time is the run clock: it never goes back and it moves between ticks.
        let times: Vec<u32> = seen.setpoint_data.iter().map(|d| d.time_boot_ms).collect();
        assert!(times.windows(2).all(|pair| pair[0] <= pair[1]), "{times:?}");
        assert!(times.last() > times.first(), "{times:?}");
        assert!(!seen.command_targets.is_empty(), "{seen:?}");
        assert!(
            seen.command_targets.iter().all(|target| *target == (42, 7)),
            "{:?}",
            seen.command_targets
        );
        // The flight controller's ack, from (42, 7), confirmed nothing the heartbeat did not.
        assert!(report.recovery_confirmed, "{report:?}");
    }

    #[test]
    fn a_non_finite_setpoint_request_is_logged_and_not_forwarded() {
        let fc = FakeFc::bind();
        let mut asks = Asks {
            tick: 0,
            request: Request {
                north: 120.0,
                east: 80.0,
                down: -25.0,
                yaw: f64::NAN,
                commit: false,
            },
            breach_at: u64::MAX,
            position: Arc::clone(&fc.position),
        };
        let (_, seen, text) = run_fc("bad-setpoint", &fc, |_| {}, 8, &mut asks);
        assert_eq!(seen.setpoints, 0, "{seen:?}");
        let lines = heard_lines(&text);
        assert!(
            lines
                .iter()
                .any(|line| line.contains("bad_setpoint") && line.contains("command=setpoints")),
            "{text}"
        );
    }

    fn immediate(config: &mut Config) {
        config.startup.mode = config::StartupMode::Immediate;
    }

    /// Lines after the flight controller was first heard.
    fn heard_lines(text: &str) -> Vec<&str> {
        tick_lines(text)
            .into_iter()
            .filter(|line| !line.contains("no_fc"))
            .collect()
    }

    #[test]
    fn a_disarmed_vehicle_is_never_commanded_even_outside_the_fence() {
        let fc = FakeFc::bind();
        fc.armed.store(false, Ordering::SeqCst);
        *lock(&fc.position) = OUTSIDE;
        let (report, seen, text) = run_fc("disarmed", &fc, |_| {}, 15, &mut StubComplex);
        assert!(seen.recovery().is_empty(), "{text}");
        assert_eq!(seen.setpoints, 0);
        assert!(!report.recovery_sent, "{report:?}");
        let lines = heard_lines(&text);
        assert!(!lines.is_empty(), "{text}");
        for line in lines {
            // Monitored and logged: the fence Revert is there, and nothing went out.
            for item in ["disarmed", "verdict=revert", "fence_ok=0"] {
                assert!(line.contains(item), "{item}: {line}");
            }
            assert!(
                !line.contains("sent:") && !line.contains("recovery_"),
                "{line}"
            );
        }
    }

    #[test]
    fn arm_after_ready_then_a_breach_sends_one_auto_rtl_and_disarm_is_monitor_only() {
        let fc = FakeFc::bind();
        fc.armed.store(false, Ordering::SeqCst);
        let armed_at = Arc::new(Mutex::new(None));
        let mut script = Script::new()
            .at(4, {
                let armed = set(&fc.armed, true);
                let armed_at = Arc::clone(&armed_at);
                let mut armed = armed;
                move || {
                    *lock(&armed_at) = Some(Instant::now());
                    armed();
                }
            })
            .at(8, {
                let position = Arc::clone(&fc.position);
                move || *lock(&position) = OUTSIDE
            })
            .at(16, set(&fc.armed, false));
        let (report, seen, text) = run_fc("arm", &fc, |_| {}, 20, &mut script);
        assert_eq!(seen.set_mode.len(), 1, "{text}");
        assert!(seen.nav.is_empty());
        let (p1, p2, p3, at) = seen.set_mode[0];
        assert_eq!((p1, p2, p3), (1.0, 4.0, 5.0));
        assert!(
            at > lock(&armed_at).unwrap(),
            "commanded before the arm\n{text}"
        );
        assert!(report.recovery_confirmed, "{report:?}");
        let lines = heard_lines(&text);
        assert!(lines[0].contains("disarmed"), "{text}");
        assert!(!text.contains("not_ready_at_arm"), "{text}");
        let sent = lines
            .iter()
            .find(|line| line.contains("sent:set_mode"))
            .unwrap_or_else(|| panic!("{text}"));
        assert!(
            !sent.contains("disarmed") && sent.contains("verdict=revert"),
            "{sent}"
        );
        assert!(lines.last().unwrap().contains("disarmed"), "{text}");
    }

    #[test]
    fn arm_before_ready_reverts_at_once_and_latches_the_fault() {
        let fc = FakeFc::bind();
        fc.streaming.store(false, Ordering::SeqCst);
        let mut script = Script::new().at(6, set(&fc.streaming, true));
        let (report, seen, text) = run_fc("early-arm", &fc, |_| {}, 12, &mut script);
        assert_eq!(seen.set_mode.len(), 1, "{text}");
        assert!(report.recovery_sent);
        let lines = heard_lines(&text);
        for item in [
            "not_ready_at_arm",
            "verdict=revert",
            "command=mode",
            "sent:set_mode",
        ] {
            assert!(lines[0].contains(item), "{item}: {}\n{text}", lines[0]);
        }
        // The fix arriving later does not clear it.
        for line in &lines[1..] {
            assert!(line.contains("not_ready_at_arm"), "{line}");
            assert!(
                line.contains("latched") && line.contains("verdict=revert"),
                "{line}"
            );
        }
    }

    #[test]
    fn a_heartbeat_before_any_position_no_longer_sends_rtl_at_boot() {
        let fc = FakeFc::bind();
        fc.streaming.store(false, Ordering::SeqCst);
        let last_tick = Arc::new(Mutex::new(None));
        let mut script = Script::new().at(5, set(&fc.streaming, true)).at(12, {
            let last_tick = Arc::clone(&last_tick);
            move || *lock(&last_tick) = Some(Instant::now())
        });
        let (_, seen, text) = run_fc("boot", &fc, immediate, 12, &mut script);
        // The shutdown order still sends the recovery mode after the last tick. Nothing is
        // sent before it.
        let last_tick = lock(&last_tick).unwrap();
        assert!(
            seen.recovery().iter().all(|(_, at)| *at > last_tick),
            "{text}"
        );
        assert!(!text.contains("sent:"), "{text}");
        let lines = heard_lines(&text);
        assert!(lines[0].contains("not_ready"), "{text}");
        let first_ready = lines
            .iter()
            .position(|line| !line.contains("not_ready"))
            .unwrap_or_else(|| panic!("never ready\n{text}"));
        for line in &lines[first_ready..] {
            // Nominal: inside the fence, no tracker, so inhibit.
            assert!(line.contains("verdict=inhibit"), "{line}\n{text}");
            assert!(!line.contains("latched"), "{line}");
        }
    }

    #[test]
    fn immediate_mode_commands_a_disarmed_vehicle_once_ready() {
        let fc = FakeFc::bind();
        fc.armed.store(false, Ordering::SeqCst);
        *lock(&fc.position) = OUTSIDE;
        let (report, seen, text) = run_fc("immediate", &fc, immediate, 12, &mut StubComplex);
        assert_eq!(seen.set_mode.len(), 1, "{text}");
        assert!(report.recovery_confirmed);
        assert!(!text.contains("disarmed"), "{text}");
        assert!(heard_lines(&text)[0].contains("sent:set_mode"), "{text}");
    }

    #[test]
    fn the_watchdog_is_gated_while_disarmed() {
        let fc = FakeFc::bind();
        fc.armed.store(false, Ordering::SeqCst);
        *lock(&fc.position) = OUTSIDE;
        let window = Arc::new(Mutex::new(None));
        let mut stall = Stall {
            tick: 0,
            at: 4,
            hold: Duration::from_millis(1_200),
            window: Arc::clone(&window),
        };
        let (report, seen, text) = run_fc("dog-gated", &fc, |_| {}, 8, &mut stall);
        assert!(lock(&window).is_some());
        assert!(!report.watchdog_fired, "{report:?}\n{text}");
        assert!(seen.recovery().is_empty(), "{text}");
        assert!(!report.recovery_sent);
    }

    #[test]
    fn the_ready_timeout_is_logged_and_nothing_is_commanded_while_disarmed() {
        let fc = FakeFc::bind();
        fc.armed.store(false, Ordering::SeqCst);
        fc.streaming.store(false, Ordering::SeqCst);
        let (report, seen, text) = run_fc(
            "timeout",
            &fc,
            |config| config.startup.ready_timeout_ms = 300,
            10,
            &mut StubComplex,
        );
        assert!(seen.recovery().is_empty(), "{text}");
        assert!(!report.recovery_sent);
        let lines = tick_lines(&text);
        assert!(lines.last().unwrap().contains("ready_timeout"), "{text}");
        assert!(
            heard_lines(&text).iter().all(|l| l.contains("disarmed")),
            "{text}"
        );
    }

    /// A triangle: the lower-left half of the 200 m test square.
    const TRIANGLE: [[f64; 2]; 3] = [[0.0, 0.0], [200.0, 0.0], [0.0, 200.0]];

    fn with_polygon(config: &mut Config, polygon: &[[f64; 2]]) {
        if let config::Fence::Host {
            polygon: configured,
            ..
        } = &mut config.fence
        {
            *configured = polygon.to_vec();
        }
    }

    #[test]
    fn the_tick_fences_on_the_configured_polygon() {
        let mut vehicle = Vehicle {
            fix_ms: Some(0),
            alt_m: 30.0,
            ..Vehicle::default()
        };
        let square = config(1, "unused.log");
        let mut triangle = config(1, "unused.log");
        with_polygon(&mut triangle, &TRIANGLE);
        // Inside the square, outside the triangle: only the configured polygon decides.
        (vehicle.north_m, vehicle.east_m) = (155.0, 155.0);
        assert!(present(&square, &vehicle, 0).unwrap().fence_ok);
        assert!(!present(&triangle, &vehicle, 0).unwrap().fence_ok);
        // Deep inside both.
        (vehicle.north_m, vehicle.east_m) = (60.0, 60.0);
        assert!(present(&triangle, &vehicle, 0).unwrap().fence_ok);
        // An empty or degenerate polygon is never inside.
        for polygon in [&[][..], &TRIANGLE[..2]] {
            let mut bad = config(1, "unused.log");
            with_polygon(&mut bad, polygon);
            assert!(!present(&bad, &vehicle, 0).unwrap().fence_ok, "{polygon:?}");
        }
    }

    #[test]
    fn a_configured_polygon_trips_the_loop() {
        // The fake sits at (155, 155): inside the default square, outside the configured
        // triangle. The loop reverts and commands AUTO + RTL on the first heard tick.
        let fc = FakeFc::bind();
        *lock(&fc.position) = (155.0, 155.0, -30.0);
        let (report, seen, text) = run_fc(
            "triangle",
            &fc,
            |config| with_polygon(config, &TRIANGLE),
            8,
            &mut StubComplex,
        );
        assert_eq!(seen.set_mode.len(), 1, "{text}");
        assert!(report.recovery_confirmed, "{report:?}");
        let first = heard_lines(&text)[0];
        for item in ["fence_ok=0", "verdict=revert", "sent:set_mode"] {
            assert!(first.contains(item), "{item}: {first}");
        }
    }

    #[test]
    fn nothing_is_commanded_before_the_flight_controller_is_heard() {
        let silent = UdpSocket::bind("127.0.0.1:0").unwrap();
        silent
            .set_read_timeout(Some(Duration::from_millis(50)))
            .unwrap();
        let port = silent.local_addr().unwrap().port();
        let path = log_path("silent");
        let config = config(port, path.to_str().unwrap());
        let link = open(
            &Endpoint::Udp {
                host: "127.0.0.1".into(),
                port,
            },
            RECV_TIMEOUT,
        )
        .unwrap();
        let worker = spec_worker();
        let mut log = TickLog::open(&path, 1, 0).unwrap();
        let report = run(
            &config,
            worker,
            &mut log,
            0,
            link,
            &mut StubComplex,
            options(5),
        )
        .unwrap();
        assert!(!report.recovery_sent);
        // Only companion heartbeats reach the silent peer.
        let mut buffer = [0_u8; RECV_BUFFER];
        while let Ok((count, _)) = silent.recv_from(&mut buffer) {
            decode_each(&buffer[..count], |_, message| {
                assert!(matches!(message, MavMessage::HEARTBEAT(_)), "{message:?}");
            });
        }
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 6);
        for line in text.lines().skip(1) {
            assert!(
                line.contains("verdict=revert") && line.contains("no_fc"),
                "{line}"
            );
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_ground_station_heartbeat_is_not_the_flight_controller() {
        let mut vehicle = Vehicle::default();
        let header = MavHeader {
            system_id: 255,
            component_id: 190,
            sequence: 0,
        };
        apply_message(&mut vehicle, &header, &heartbeat(), 10);
        assert_eq!(vehicle.fc_heartbeat_ms, None);
        let px4 = MavMessage::HEARTBEAT(HEARTBEAT_DATA {
            custom_mode: custom_mode(RecoveryMode::Rtl),
            autopilot: MavAutopilot::MAV_AUTOPILOT_PX4,
            ..Default::default()
        });
        apply_message(&mut vehicle, &header, &px4, 20);
        assert_eq!(vehicle.fc_heartbeat_ms, Some(20));
        assert_eq!(
            vehicle.fc_custom_mode.and_then(reported_mode),
            Some(RecoveryMode::Rtl)
        );
    }

    #[test]
    fn a_non_finite_position_does_not_reset_the_fix_age() {
        let mut vehicle = Vehicle::default();
        let header = MavHeader::default();
        let bad = MavMessage::LOCAL_POSITION_NED(LOCAL_POSITION_NED_DATA {
            x: f32::NAN,
            ..Default::default()
        });
        apply_message(&mut vehicle, &header, &bad, 10);
        assert_eq!(vehicle.fix_ms, None);
        let config = config(1, "unused.log");
        let presented = present(&config, &vehicle, 10).unwrap();
        assert!(!presented.fence_ok);
        assert!(presented.record.fix_age_ms() > 1e300);
    }

    #[test]
    fn reported_or_other_never_equals_recovery_when_unknown() {
        for recovery in [RecoveryMode::Rtl, RecoveryMode::Loiter, RecoveryMode::Land] {
            assert_ne!(reported_or_other(None, recovery), recovery);
            assert_eq!(reported_or_other(Some(recovery), recovery), recovery);
        }
    }
}
