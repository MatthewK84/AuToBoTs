//! Allocations per steady-state tick, counted by a global allocator in this test binary.
//!
//! The tick thread (the caller of `runtime::run`) must allocate nothing once warmed up: the
//! reasons are a fixed `Reasons`, the log line is formatted into the log's own buffer, and
//! the fail-closed notes are a ring. `Complex::request` runs once per tick on that thread, so
//! the probe reads the thread's counter there; the difference between two calls is one whole
//! tick. The whole process is counted too and held to the RTLola interpreter's own cost for
//! one event, measured here on the test thread with the same spec and record shape: the
//! receive thread, the watchdog, and the worker's handoff add nothing on top
//! (`docs/runtime.md`, "Allocation").

use mavlink::dialects::common::{
    MavAutopilot, MavMessage, MavModeFlag, MavState, MavType, HEARTBEAT_DATA,
    LOCAL_POSITION_NED_DATA,
};
use mavlink::MavHeader;
use rta_host::command_out::custom_mode;
use rta_host::config::parse_str;
use rta_host::eval::Evaluator;
use rta_host::eval_worker::{EvalWorker, SpecEvaluator};
use rta_host::link::{open, Endpoint};
use rta_host::mavlink_link::{encode, MAX_FRAME};
use rta_host::monitor::build_once;
use rta_host::runtime::{run, RunOptions, RECV_TIMEOUT};
use rta_host::tick_log::TickLog;
use rta_spec::{InputRecord, RecoveryMode};
use rta_switch::{Complex, Request};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

struct Counting;

static TOTAL: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static MINE: Cell<u64> = const { Cell::new(0) };
}

fn count() {
    TOTAL.fetch_add(1, Ordering::Relaxed);
    let _ = MINE.try_with(|mine| mine.set(mine.get() + 1));
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const SPEC: &str = include_str!("../../../spec/monitor.lola");
const TICKS: usize = 40;
/// Ticks before the counts are held to the bound: gate opening, first heartbeat, first
/// datagram, and the first use of each channel and thread-local cache.
const WARM_UP: usize = 15;
/// Ceiling on the interpreter's own allocations for one event. `rtlola-interpreter` 0.11.0
/// makes 15 (inhibit) or 21 (revert) for this spec; an upgrade that grows past this fails.
const INTERPRETER_CEILING: u64 = 32;

/// Allocations the interpreter makes for one warmed-up event, counted on this thread.
fn interpreter_cost(fence_ok: bool) -> u64 {
    let mut monitor = build_once(SPEC).unwrap();
    let mut evaluator = Evaluator::default();
    // The record shape the runtime presents: fresh samples, no tracker.
    let record = InputRecord::try_new(30.0, fence_ok, 10.0, 0.0, 0.0, 10.0, 50.0).unwrap();
    for tick in 0..WARM_UP as u64 {
        evaluator.tick(&mut monitor, &record, tick * 50);
    }
    let before = MINE.with(Cell::get);
    evaluator.tick(&mut monitor, &record, WARM_UP as u64 * 50);
    MINE.with(Cell::get) - before
}

/// The test's PX4 stand-in: a heartbeat every 100 ms and a position every 20 ms, encoded into
/// a fixed buffer, and everything the host sends drained unread. It allocates nothing in its
/// loop, so the process count is the runtime's.
fn spawn_fc(
    socket: UdpSocket,
    host: SocketAddr,
    position: (f32, f32),
    custom_mode: u32,
    stop: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut buffer = [0_u8; MAX_FRAME];
        let mut inbound = [0_u8; 2_048];
        let mut sequence = 0_u8;
        let mut loops = 0_u32;
        let mut send = |message: &MavMessage, sequence: &mut u8| {
            let header = MavHeader {
                system_id: 1,
                component_id: 1,
                sequence: *sequence,
            };
            *sequence = sequence.wrapping_add(1);
            if let Ok(count) = encode(header, message, &mut buffer) {
                let _ = socket.send_to(&buffer[..count], host);
            }
        };
        while !stop.load(Ordering::SeqCst) {
            send(
                &MavMessage::LOCAL_POSITION_NED(LOCAL_POSITION_NED_DATA {
                    time_boot_ms: 0,
                    x: position.0,
                    y: position.1,
                    z: -30.0,
                    vx: 0.0,
                    vy: 0.0,
                    vz: 0.0,
                }),
                &mut sequence,
            );
            if loops.is_multiple_of(5) {
                send(
                    &MavMessage::HEARTBEAT(HEARTBEAT_DATA {
                        custom_mode,
                        mavtype: MavType::MAV_TYPE_QUADROTOR,
                        autopilot: MavAutopilot::MAV_AUTOPILOT_PX4,
                        base_mode: MavModeFlag::MAV_MODE_FLAG_CUSTOM_MODE_ENABLED
                            | MavModeFlag::MAV_MODE_FLAG_SAFETY_ARMED,
                        system_status: MavState::MAV_STATE_ACTIVE,
                        mavlink_version: 3,
                    }),
                    &mut sequence,
                );
            }
            loops = loops.wrapping_add(1);
            while socket.recv_from(&mut inbound).is_ok() {}
            thread::sleep(Duration::from_millis(20));
        }
    })
}

/// Reads both counters once per tick into pre-sized vectors, so the probe allocates nothing.
struct Probe {
    mine: Vec<u64>,
    total: Vec<u64>,
    request: Option<Request>,
}

impl Complex for Probe {
    fn request(&mut self) -> Option<Request> {
        if self.mine.len() < self.mine.capacity() {
            self.mine.push(MINE.with(Cell::get));
            self.total.push(TOTAL.load(Ordering::Relaxed));
        }
        self.request
    }
}

struct Counts {
    tick_thread: Vec<u64>,
    process: Vec<u64>,
    log: String,
}

/// Run `TICKS` ticks against the stand-in and return the per-tick allocation counts.
fn measure(name: &str, position: (f32, f32), custom_mode: u32, request: Option<Request>) -> Counts {
    let fc = UdpSocket::bind("127.0.0.1:0").unwrap();
    fc.set_read_timeout(Some(Duration::from_millis(1))).unwrap();
    let path = std::env::temp_dir().join(format!("rta-alloc-{}-{name}.log", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let config = parse_str(&format!(
        r#"
[link]
endpoint = "udp:127.0.0.1:{port}"
system_id = 1
component_id = 191

[tick]
period_ms = 100
deadline_ms = 80
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

[startup]
mode = "armed"
ready_timeout_ms = 30000
ready_fix_age_ms = 300
"#,
        port = fc.local_addr().unwrap().port(),
        log = path.display(),
    ))
    .unwrap();
    let link = open(
        &Endpoint::Udp {
            host: "127.0.0.1".into(),
            port: fc.local_addr().unwrap().port(),
        },
        RECV_TIMEOUT,
    )
    .unwrap();
    let host = SocketAddr::from(([127, 0, 0, 1], link.local_addr().unwrap().port()));
    let stop = Arc::new(AtomicBool::new(false));
    let fc_thread = spawn_fc(fc, host, position, custom_mode, Arc::clone(&stop));
    let worker = EvalWorker::start(|| {
        build_once(SPEC)
            .map(SpecEvaluator::new)
            .map_err(|err| format!("{err:?}"))
    })
    .unwrap();
    let mut log = TickLog::open(&path, config.log.flush_every_n, 1).unwrap();
    let mut probe = Probe {
        mine: Vec::with_capacity(TICKS),
        total: Vec::with_capacity(TICKS),
        request,
    };
    let report = run(
        &config,
        worker,
        &mut log,
        1,
        link,
        &mut probe,
        RunOptions {
            max_ticks: Some(TICKS as u64),
            stop: Arc::new(AtomicBool::new(false)),
            limits: rta_host::spec::limits(SPEC).unwrap(),
        },
    )
    .unwrap();
    stop.store(true, Ordering::SeqCst);
    fc_thread.join().unwrap();
    assert_eq!(report.ticks, TICKS as u64);
    let text = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    let deltas = |marks: &[u64]| marks.windows(2).map(|w| w[1] - w[0]).collect::<Vec<_>>();
    Counts {
        tick_thread: deltas(&probe.mine),
        process: deltas(&probe.total),
        log: text,
    }
}

fn assert_bounded(name: &str, counts: &Counts, interpreter: u64) {
    assert!(
        interpreter <= INTERPRETER_CEILING,
        "{name}: interpreter {interpreter}"
    );
    let steady = &counts.tick_thread[WARM_UP..];
    assert!(
        steady.iter().all(|count| *count == 0),
        "{name}: tick-thread allocations per tick {:?}",
        counts.tick_thread
    );
    let process: u64 = counts.process[WARM_UP..].iter().sum();
    let ticks = counts.process.len() - WARM_UP;
    assert!(
        process <= interpreter * ticks as u64,
        "{name}: process allocations {process} over {ticks} ticks, interpreter {interpreter} \
         per event: {:?}",
        counts.process
    );
    eprintln!(
        "{name}: interpreter {interpreter} per event; tick thread {:?}; process {:?}",
        &counts.tick_thread[WARM_UP..],
        &counts.process[WARM_UP..]
    );
}

/// Both scenarios run in one test so no other test thread allocates during a measurement. The
/// deadline is generous (80 ms of a 100 ms tick) so CPU load cannot turn a tick late.
#[test]
fn a_steady_state_tick_allocates_nothing_on_the_tick_thread() {
    // Nominal: armed, ready, inside the fence, no tracker, so the spec inhibits and the
    // requested setpoint goes out with the commit flag cleared.
    let nominal = measure(
        "nominal",
        (100.0, 100.0),
        0x0003_0000,
        Some(Request {
            north: 100.0,
            east: 100.0,
            down: -30.0,
            yaw: 0.0,
            commit: true,
        }),
    );
    let steady: Vec<&str> = nominal
        .log
        .lines()
        .filter(|line| line.starts_with("tick="))
        .skip(WARM_UP)
        .collect();
    assert!(
        steady
            .iter()
            .all(|line| line.contains("verdict=inhibit") && line.contains("command=setpoints")),
        "{}",
        nominal.log
    );
    assert_bounded("nominal", &nominal, interpreter_cost(true));

    // Revert: outside the fence with the flight controller already in AUTO_RTL, so the switch
    // holds Idle and the reasons name the spec outputs every tick.
    let revert = measure(
        "revert",
        (600.0, 600.0),
        custom_mode(RecoveryMode::Rtl),
        None,
    );
    let steady: Vec<&str> = revert
        .log
        .lines()
        .filter(|line| line.starts_with("tick="))
        .skip(WARM_UP)
        .collect();
    assert!(
        steady
            .iter()
            .all(|line| line.contains("verdict=revert") && line.contains("command=idle")),
        "{}",
        revert.log
    );
    assert_bounded("revert", &revert, interpreter_cost(false));
}
