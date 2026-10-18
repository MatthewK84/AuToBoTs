//! Integrated runtime. One tick thread, one receive thread, one watchdog thread.
//!
//! The tick thread ages samples, builds the record, evaluates the spec, passes the verdict
//! through `FailClosed` and `decide`, sends the outbound message, and writes the tick log.
//! The receive thread applies inbound MAVLink to the shared vehicle state. The watchdog thread
//! reads the published tick only and sends the recovery mode on its own socket handle.

use crate::clock::age_ms;
use crate::command_out::{reported_mode, to_message};
use crate::config::{self, Config};
use crate::eval_worker::{EvalWorker, Outcome};
use crate::fail_closed::{FailClosed, TickCase};
use crate::fence::{fence_ok, FenceInput};
use crate::link::{LinkError, UdpLink};
use crate::link_age::{LinkAge, LinkMessage};
use crate::mavlink_link::{decode_each, encode, heartbeat, MAX_FRAME};
use crate::mode::ModeSender;
use crate::stale::present_track;
use crate::tick_log::{command_name, TickLog, TickRecord};
use crate::watchdog::Watchdog;
use mavlink::dialects::common::{MavAutopilot, MavMessage};
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
    pub fix_ms: Option<u64>,
    pub north_m: f64,
    pub east_m: f64,
    pub alt_m: f64,
    pub link: LinkAge,
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
    /// Recovery is sent once per process (`mode.rs`). Either thread may claim it.
    recovery_sent: AtomicBool,
    watchdog_fired: AtomicBool,
}

impl Shared {
    fn snapshot(&self) -> Vehicle {
        *lock(&self.vehicle)
    }

    /// Claim the one recovery send and put it on the wire. A local send error releases the
    /// claim so the next tick or the watchdog can send. A lost datagram is not seen here.
    fn send_recovery(&self, sender: &mut Sender, recovery: RecoveryMode, target: u8) -> bool {
        if self.recovery_sent.swap(true, Ordering::SeqCst) {
            return false;
        }
        let sent = to_message(&SwitchCommand::Mode { recovery }, target)
            .map(|message| sender.send(&message).is_ok())
            .unwrap_or(false);
        if !sent {
            self.recovery_sent.store(false, Ordering::SeqCst);
        }
        sent
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
    pub watchdog_fired: bool,
}

pub struct RunOptions {
    /// Stop after this many ticks. `None` runs until `stop` is set.
    pub max_ticks: Option<u64>,
    /// Set by SIGINT, or by a caller, to begin the shutdown order.
    pub stop: Arc<AtomicBool>,
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
        recovery_sent: AtomicBool::new(false),
        watchdog_fired: AtomicBool::new(false),
    });

    let receiver = spawn_receiver(link.try_clone().map_err(RunError::Link)?, &shared, clock)?;
    let watchdog = spawn_watchdog(
        Sender::new(link.try_clone().map_err(RunError::Link)?, config),
        &shared,
        clock,
        config,
        recovery,
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
    let mut closed = FailClosed::default();
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
        let mut reasons = Vec::new();
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
                        reasons.push("eval_fault".to_string());
                        (None, false, 0, Some(presented))
                    }
                }
            }
            Err(_) => {
                reasons.push("record".to_string());
                (None, false, 0, None)
            }
        };
        if late {
            reasons.push("late".to_string());
        }
        // A sent recovery, a failed log write, or an evaluation fault latches Revert until
        // restart.
        let latched = shared.recovery_sent.load(Ordering::SeqCst) || log_failed || worker.faulted();
        if latched {
            reasons.push("latched".to_string());
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
        });
        // The logged verdict is the one the switch acted on. Mode and Idle come only from
        // Revert; a forced Revert shows here even when the spec said Pass.
        let logged_verdict = match command {
            SwitchCommand::Mode { .. } | SwitchCommand::Idle => Verdict::Revert,
            SwitchCommand::Setpoints { .. } => verdict.unwrap_or(Verdict::Revert),
        };

        let target = vehicle.fc_system.unwrap_or(config.link.system_id);
        if vehicle.fc_heartbeat_ms.is_none() {
            // Nothing is commanded to a flight controller that has not been heard.
            reasons.push("no_fc".to_string());
        } else {
            match command {
                SwitchCommand::Mode { recovery } => {
                    shared.send_recovery(&mut sender, recovery, target);
                }
                SwitchCommand::Setpoints { .. } if request.is_none() => {
                    // The switch fills a missing request with zeros. A zero setpoint is a
                    // real position, so nothing is forwarded when nothing was asked.
                    reasons.push("no_request".to_string());
                }
                SwitchCommand::Setpoints { .. } => {
                    if let Some(message) = to_message(&command, target) {
                        let _ = sender.send(&message);
                    }
                }
                SwitchCommand::Idle => {}
            }
        }

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
    if vehicle.fc_heartbeat_ms.is_some() && reported != Some(recovery) {
        let target = vehicle.fc_system.unwrap_or(config.link.system_id);
        shared.send_recovery(&mut sender, recovery, target);
    }
    let _ = log.flush();
    worker.shutdown();
    let _ = receiver.join();
    let _ = watchdog.join();
    report.recovery_sent = shared.recovery_sent.load(Ordering::SeqCst);
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
                        let mut vehicle = lock(&shared.vehicle);
                        decode_each(datagram, |header, message| {
                            apply_message(&mut vehicle, header, message, now_ms);
                        });
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
    recovery: RecoveryMode,
) -> Result<thread::JoinHandle<()>, RunError> {
    let shared = Arc::clone(shared);
    let mut dog = Watchdog::new(config.tick.period_ms, config.tick.watchdog_misses);
    let poll = Duration::from_millis((config.tick.period_ms / 2).max(1));
    let fallback_target = config.link.system_id;
    thread::Builder::new()
        .name("rta-watchdog".into())
        .spawn(move || {
            let mut sink = Vec::with_capacity(1);
            while !shared.stop.load(Ordering::SeqCst) {
                thread::sleep(poll);
                let now_ms = clock.now_ms();
                dog.publish(shared.published_ms.load(Ordering::SeqCst));
                let vehicle = shared.snapshot();
                if vehicle.fc_heartbeat_ms.is_none() {
                    continue;
                }
                let reported = vehicle.fc_custom_mode.and_then(reported_mode);
                sink.clear();
                let mut once = ModeSender::default();
                dog.check(
                    now_ms,
                    reported_or_other(reported, recovery),
                    recovery,
                    &mut once,
                    &mut sink,
                );
                if !sink.is_empty() && !shared.stop.load(Ordering::SeqCst) {
                    let target = vehicle.fc_system.unwrap_or(fallback_target);
                    if shared.send_recovery(&mut sender, recovery, target) {
                        shared.watchdog_fired.store(true, Ordering::SeqCst);
                    }
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
    use crate::monitor::build_once;
    use mavlink::dialects::common::{
        MavCmd, MavModeFlag, MavState, MavType, HEARTBEAT_DATA, LOCAL_POSITION_NED_DATA,
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
deadline_ms = 50
watchdog_misses = 3
mode_change_latency_ms = 200
tracker_hold_ms = 200

[recovery]
mode = "rtl"

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
    #[derive(Debug, Default)]
    struct Seen {
        set_mode: Vec<SetMode>,
        setpoints: usize,
        heartbeats: usize,
    }

    /// A PX4-like peer on loopback. It heartbeats with its custom mode, streams
    /// `LOCAL_POSITION_NED` from `position`, and adopts a commanded AUTO sub-mode the way PX4
    /// reports it. It stops on `stop`.
    struct FakeFc {
        socket: UdpSocket,
        position: Arc<Mutex<(f32, f32, f32)>>,
        seen: Arc<Mutex<Seen>>,
        stop: Arc<AtomicBool>,
    }

    impl FakeFc {
        fn bind() -> Self {
            let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
            socket
                .set_read_timeout(Some(Duration::from_millis(10)))
                .unwrap();
            Self {
                socket,
                position: Arc::new(Mutex::new((100.0, 100.0, -30.0))),
                seen: Arc::new(Mutex::new(Seen::default())),
                stop: Arc::new(AtomicBool::new(false)),
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
            thread::spawn(move || {
                let mut mode = PX4_POSCTL;
                let mut sequence = 0_u8;
                let mut buffer = [0_u8; MAX_FRAME];
                let mut inbound = [0_u8; RECV_BUFFER];
                let mut last_beat: Option<Instant> = None;
                let mut send = |message: &MavMessage, sequence: &mut u8| {
                    let header = MavHeader {
                        system_id: 1,
                        component_id: 1,
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
                    if last_beat.is_none_or(|at| at.elapsed() >= Duration::from_millis(100)) {
                        send(
                            &MavMessage::HEARTBEAT(HEARTBEAT_DATA {
                                custom_mode: mode,
                                mavtype: MavType::MAV_TYPE_QUADROTOR,
                                autopilot: MavAutopilot::MAV_AUTOPILOT_PX4,
                                base_mode: MavModeFlag::MAV_MODE_FLAG_CUSTOM_MODE_ENABLED,
                                system_status: MavState::MAV_STATE_ACTIVE,
                                mavlink_version: 3,
                            }),
                            &mut sequence,
                        );
                        last_beat = Some(Instant::now());
                    }
                    if let Ok((count, _)) = socket.recv_from(&mut inbound) {
                        decode_each(&inbound[..count], |_, message| {
                            let mut seen = lock(&seen);
                            match message {
                                MavMessage::COMMAND_LONG(data)
                                    if data.command == MavCmd::MAV_CMD_DO_SET_MODE =>
                                {
                                    seen.set_mode.push((
                                        data.param1,
                                        data.param2,
                                        data.param3,
                                        Instant::now(),
                                    ));
                                    if data.param2 == f32::from(PX4_MAIN_MODE_AUTO) {
                                        mode = ((data.param3 as u32) << 24)
                                            | (u32::from(PX4_MAIN_MODE_AUTO) << 16);
                                    }
                                }
                                MavMessage::SET_POSITION_TARGET_LOCAL_NED(_) => seen.setpoints += 1,
                                MavMessage::HEARTBEAT(_) => seen.heartbeats += 1,
                                _ => {}
                            }
                        });
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
        assert!((50..250).contains(&eval_ms), "{faulted}");
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
            run_with_faulty_eval("slow", Some(Duration::from_millis(80)));
        assert_reverted_on_the_faulted_tick(&report, &seen, faulted_at, &text);
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

#[cfg(test)]
mod doc_tests {
    #[test]
    fn the_runtime_note_names_the_threads_the_order_and_what_is_not_shown() {
        let note = include_str!("../../../docs/runtime.md");
        for item in [
            "Tick",
            "Receive",
            "Watchdog",
            "stop setpoints",
            "flush the log",
            "no_fc",
            "no_request",
            "latched",
            "The deadline is preemptive.",
            "udpin:host:port",
            "No PX4 SITL run of this loop is recorded.",
        ] {
            assert!(note.contains(item), "{item}");
        }
        let deps = include_str!("../../../docs/dependencies.md");
        let manifest = include_str!("../Cargo.toml");
        assert!(deps.contains("| ctrlc | 3.5.2 |"));
        assert!(manifest.contains("ctrlc = \"=3.5.2\""));
    }
}
