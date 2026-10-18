//! Property tests for the parsing and decision paths. Every test runs on a fixed seed with
//! no failure file, so a run is the same on every machine; widen `CASES` locally to search
//! harder. A failure prints the minimal input.

use mavlink::dialects::common::{MavCmd, MavMessage, MavResult, COMMAND_ACK_DATA};
use mavlink::MavHeader;
use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};
use rta_host::command_out::{self, Target, POSITION_AND_YAW};
use rta_host::config;
use rta_host::gate::{Gate, Inputs, Mode};
use rta_host::link::{parse_endpoint, Endpoint};
use rta_host::mavlink_link::{self, CommandAck, Frame, MAX_FRAMES_PER_DATAGRAM};
use rta_host::read::{self, PartialRecord};
use rta_host::reason::Reasons;
use rta_host::ring::STATUS_LOG_CAPACITY;
use rta_spec::{RecoveryMode, Verdict};
use rta_switch::{decide, Request, SwitchCommand};

const CASES: u32 = 512;

fn fixed(seed: u64) -> Config {
    Config {
        cases: CASES,
        rng_seed: RngSeed::Fixed(seed),
        failure_persistence: None,
        ..Config::default()
    }
}

fn recovery_mode() -> impl Strategy<Value = RecoveryMode> {
    prop_oneof![
        Just(RecoveryMode::Rtl),
        Just(RecoveryMode::Loiter),
        Just(RecoveryMode::Land)
    ]
}

fn verdict() -> impl Strategy<Value = Verdict> {
    prop_oneof![
        Just(Verdict::Pass),
        Just(Verdict::Inhibit),
        Just(Verdict::Revert)
    ]
}

fn request() -> impl Strategy<Value = Option<Request>> {
    proptest::option::of(
        (
            any::<f64>(),
            any::<f64>(),
            any::<f64>(),
            any::<f64>(),
            any::<bool>(),
        )
            .prop_map(|(north, east, down, yaw, commit)| Request {
                north,
                east,
                down,
                yaw,
                commit,
            }),
    )
}

fn ack_frame(command: MavCmd, result: MavResult, system: u8, component: u8) -> Vec<u8> {
    let header = MavHeader {
        system_id: system,
        component_id: component,
        sequence: 0,
    };
    let message = MavMessage::COMMAND_ACK(COMMAND_ACK_DATA { command, result });
    let mut buffer = [0u8; mavlink_link::MAX_FRAME];
    let length = mavlink_link::encode(header, &message, &mut buffer).unwrap();
    buffer[..length].to_vec()
}

proptest! {
    #![proptest_config(fixed(0x5EED_0001))]

    /// `read::apply` never panics, never touches the fields it does not own, and changes
    /// nothing when it refuses a blob.
    #[test]
    fn read_apply_changes_only_its_own_field(
        bytes in proptest::collection::vec(any::<u8>(), 0..80),
        now_ms in any::<u64>(),
        conf in proptest::option::of(any::<f64>()),
    ) {
        let mut record = PartialRecord { track_conf: conf, range_m: Some(7.0), ..PartialRecord::default() };
        let before = record.clone();
        let result = read::apply(&bytes, now_ms, &mut record);
        prop_assert_eq!(record.track_conf.map(f64::to_bits), before.track_conf.map(f64::to_bits));
        prop_assert_eq!(record.range_m, before.range_m);
        match (bytes.first(), result) {
            (Some(1), Ok(())) => {
                prop_assert_eq!(bytes.len(), 5);
                let alt_mm = i32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
                prop_assert_eq!(record.alt_m, Some(f64::from(alt_mm) / 1_000.0));
                prop_assert_eq!(record.fix_received_ms, Some(now_ms));
                prop_assert_eq!(record.heartbeat_received_ms, None);
            }
            (Some(2), Ok(())) => {
                prop_assert_eq!(record.heartbeat_received_ms, Some(now_ms));
                prop_assert_eq!(record.alt_m, None);
                prop_assert_eq!(record.fix_received_ms, None);
            }
            (Some(3), Ok(())) => {
                prop_assert_eq!(record.status_log.len(), 1);
                prop_assert!(record.status_log.len() <= STATUS_LOG_CAPACITY);
                prop_assert_eq!(record.alt_m, None);
                prop_assert_eq!(record.heartbeat_received_ms, None);
            }
            (_, Ok(())) => prop_assert!(false, "accepted {:?}", bytes),
            (_, Err(_)) => {
                prop_assert!(!matches!(bytes.first(), Some(2) | Some(3)));
                prop_assert_eq!(&record, &before);
            }
        }
    }

    /// The raw `COMMAND_ACK` reader is total: command is bytes 0 and 1 little-endian, result
    /// is byte 2, and MAVLink 2's trimmed trailing zeros read as zero.
    #[test]
    fn command_ack_payload_reads_command_and_result(payload in proptest::collection::vec(any::<u8>(), 0..12)) {
        let at = |index: usize| payload.get(index).copied().unwrap_or(0);
        prop_assert_eq!(
            mavlink_link::command_ack(&payload),
            CommandAck { command: u16::from_le_bytes([at(0), at(1)]), result: at(2) }
        );
    }

    /// Any datagram decodes without a panic and yields at most the per-datagram bound.
    #[test]
    fn any_datagram_decodes_within_the_frame_bound(datagram in proptest::collection::vec(any::<u8>(), 0..2048)) {
        let mut frames = 0;
        mavlink_link::decode_frames(&datagram, |_, _| frames += 1);
        prop_assert!(frames <= MAX_FRAMES_PER_DATAGRAM);
    }

    /// An encoded `COMMAND_ACK`, alone or behind garbage-free repeats, comes back with its
    /// command, result, and sender, and through the raw path, not the typed one.
    #[test]
    fn an_encoded_ack_round_trips_through_the_raw_reader(
        command in prop_oneof![
            Just(MavCmd::MAV_CMD_DO_SET_MODE),
            Just(MavCmd::MAV_CMD_NAV_RETURN_TO_LAUNCH),
            Just(MavCmd::MAV_CMD_NAV_LAND),
            Just(MavCmd::MAV_CMD_COMPONENT_ARM_DISARM),
        ],
        result in 0_u8..=4,
        system in any::<u8>(),
        component in any::<u8>(),
        repeats in 1_usize..20,
    ) {
        let typed = match result {
            0 => MavResult::MAV_RESULT_ACCEPTED,
            1 => MavResult::MAV_RESULT_TEMPORARILY_REJECTED,
            2 => MavResult::MAV_RESULT_DENIED,
            3 => MavResult::MAV_RESULT_UNSUPPORTED,
            _ => MavResult::MAV_RESULT_FAILED,
        };
        let frame = ack_frame(command, typed, system, component);
        let datagram: Vec<u8> = std::iter::repeat_n(frame, repeats).flatten().collect();
        let mut seen = Vec::new();
        mavlink_link::decode_frames(&datagram, |header, frame| match frame {
            Frame::CommandAck(ack) => seen.push((header.system_id, header.component_id, ack)),
            Frame::Message(message) => panic!("typed {message:?}"),
        });
        prop_assert_eq!(seen.len(), repeats.min(MAX_FRAMES_PER_DATAGRAM));
        for item in seen {
            prop_assert_eq!(item, (system, component, CommandAck { command: command as u16, result }));
        }
    }
}

proptest! {
    #![proptest_config(fixed(0x5EED_0002))]

    /// Any text is a config or an error, never a panic.
    #[test]
    fn any_text_parses_or_is_refused(text in "\\PC{0,400}") {
        let _ = config::parse_str(&text);
    }

    /// For the numeric fields, `parse_str` accepts exactly the configs that meet the
    /// documented cross-field rules.
    #[test]
    fn config_accepts_exactly_the_valid_numbers(
        // Each rule is broken in roughly one case in ten, so valid and invalid configs both
        // come up often: offsets from the value a rule compares against.
        deadline in 0_u64..100,
        period_over in -10_i64..90,
        misses in 0_u32..10,
        latency in 0_u64..300,
        initial_over in -30_i64..300,
        cap_over in -30_i64..300,
        attempts in 0_u32..12,
        fix_age_over in -30_i64..300,
        timeout in 0_u64..10,
        flush in 0_u64..10,
        budget_under in -10_i64..90,
    ) {
        let offset = |base: u64, by: i64| base.saturating_add_signed(by);
        let period = offset(deadline, period_over);
        let initial = offset(latency, initial_over);
        let cap = offset(initial, cap_over);
        let fix_age = offset(period, fix_age_over);
        let budget = offset(deadline, -budget_under);
        let text = format!(r#"
[link]
endpoint = "udp:127.0.0.1:14540"
system_id = 1
component_id = 191
[tick]
period_ms = {period}
deadline_ms = {deadline}
watchdog_misses = {misses}
mode_change_latency_ms = {latency}
tracker_hold_ms = 200
[recovery]
mode = "rtl"
retry_initial_ms = {initial}
retry_cap_ms = {cap}
attempts_per_step = {attempts}
[spec]
path = "spec/monitor.lola"
[log]
path = "rta.log"
flush_every_n = {flush}
[fence]
source = "host"
polygon = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]
vertical_cap_m = 120.0
v_max = 15.0
a_max = 3.0
[shadow]
enabled = false
inference_budget_ms = {budget}
weights_path = ""
weights_hash = ""
silence_ticks = 3
[startup]
mode = "armed"
ready_timeout_ms = {timeout}
ready_fix_age_ms = {fix_age}
"#);
        let valid = period > deadline
            && misses >= 1
            && latency >= 1
            && initial >= latency
            && cap >= initial
            && (1..=10).contains(&attempts)
            && fix_age >= period
            && timeout >= 1
            && flush >= 1
            && budget >= 1
            && budget < deadline;
        let parsed = config::parse_str(&text);
        prop_assert_eq!(parsed.is_ok(), valid, "{:?}", parsed.err());
    }

    /// Endpoint parsing is total, and every well-formed UDP form round-trips.
    #[test]
    fn endpoints_round_trip_and_nothing_panics(
        junk in "\\PC{0,60}",
        host in "[a-z0-9][a-z0-9.\\-]{0,20}",
        port in any::<u16>(),
        inbound in any::<bool>(),
    ) {
        let _ = parse_endpoint(&junk);
        let scheme = if inbound { "udpin" } else { "udp" };
        let parsed = parse_endpoint(&format!("{scheme}:{host}:{port}"));
        let expected = if inbound {
            Endpoint::UdpIn { host: host.clone(), port }
        } else {
            Endpoint::Udp { host: host.clone(), port }
        };
        prop_assert_eq!(parsed, Ok(expected));
    }
}

proptest! {
    #![proptest_config(fixed(0x5EED_0003))]

    /// The switch: Revert never yields setpoints, only Pass with a committing request
    /// commits, and Revert in the recovery mode already reported is Idle.
    #[test]
    fn the_switch_never_commits_off_pass_and_never_steers_on_revert(
        verdict in verdict(),
        request in request(),
        reported in recovery_mode(),
        recovery in recovery_mode(),
    ) {
        let command = decide(verdict, request, reported, recovery);
        match (verdict, command) {
            (Verdict::Revert, SwitchCommand::Idle) => prop_assert_eq!(reported, recovery),
            (Verdict::Revert, SwitchCommand::Mode { recovery: sent }) => {
                prop_assert_ne!(reported, recovery);
                prop_assert_eq!(sent, recovery);
            }
            (Verdict::Revert, other) => prop_assert!(false, "{:?}", other),
            (_, SwitchCommand::Setpoints { commit, .. }) => {
                let may = verdict == Verdict::Pass && request.is_some_and(|item| item.commit);
                prop_assert_eq!(commit, may);
            }
            (_, other) => prop_assert!(false, "{:?} on {:?}", other, verdict),
        }
    }

    /// The startup gate, over any input sequence: in `armed` mode nothing is commanded
    /// while disarmed or unheard, and the arm-before-ready fault never clears.
    #[test]
    fn the_armed_gate_never_acts_disarmed_and_never_clears_its_fault(
        steps in proptest::collection::vec((any::<bool>(), any::<bool>(), any::<bool>(), 0_u64..200), 1..60),
    ) {
        let mut gate = Gate::new(Mode::Armed, 1_000);
        let mut now_ms = 0;
        let mut faulted = false;
        for (heard, armed, fresh, advance) in steps {
            now_ms += advance;
            let inputs = Inputs { heard, armed, ready: heard && fresh };
            let mut reasons = Reasons::default();
            let decision = gate.step(now_ms, inputs, &mut reasons);
            if !(heard && armed) {
                prop_assert!(!decision.act, "{:?}", inputs);
            }
            prop_assert!(!faulted || gate.faulted(), "the fault cleared");
            faulted = gate.faulted();
            prop_assert_eq!(decision.force_revert, faulted);
        }
    }

    /// Setpoint encoding: an `f32`-finite request goes out as position and yaw to the target
    /// with the wrapped time; anything else is not forwarded.
    #[test]
    fn a_setpoint_is_position_and_yaw_or_nothing(
        north in any::<f64>(),
        east in any::<f64>(),
        down in any::<f64>(),
        yaw in any::<f64>(),
        system in any::<u8>(),
        component in any::<u8>(),
        now_ms in any::<u64>(),
    ) {
        let command = SwitchCommand::Setpoints { north, east, down, yaw, commit: false };
        let target = Target { system, component };
        let time = command_out::time_boot_ms(now_ms);
        prop_assert_eq!(u64::from(time), now_ms % (1_u64 << 32));
        let finite = [north, east, down, yaw].iter().all(|value| (*value as f32).is_finite());
        match command_out::to_message(&command, target, time) {
            Some(MavMessage::SET_POSITION_TARGET_LOCAL_NED(data)) => {
                prop_assert!(finite);
                prop_assert_eq!(data.type_mask, POSITION_AND_YAW);
                prop_assert_eq!((data.target_system, data.target_component), (system, component));
                prop_assert_eq!(data.time_boot_ms, time);
                prop_assert_eq!(data.yaw.to_bits(), (yaw as f32).to_bits());
            }
            None => prop_assert!(!finite),
            Some(other) => prop_assert!(false, "{:?}", other),
        }
    }

    /// Only AUTO with the RTL, loiter, or land sub-mode decodes as a recovery mode, and each
    /// recovery mode's encoding decodes to itself.
    #[test]
    fn only_the_three_auto_sub_modes_are_recovery(custom in any::<u32>(), mode in recovery_mode()) {
        prop_assert_eq!(command_out::reported_mode(command_out::custom_mode(mode)), Some(mode));
        let main = (custom >> 16) & 0xFF;
        let sub = (custom >> 24) & 0xFF;
        let expected = match (main, sub) {
            (4, 5) => Some(RecoveryMode::Rtl),
            (4, 3) => Some(RecoveryMode::Loiter),
            (4, 6) => Some(RecoveryMode::Land),
            _ => None,
        };
        prop_assert_eq!(command_out::reported_mode(custom), expected);
    }
}
