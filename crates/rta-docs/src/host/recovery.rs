//! Document and source-text checks for `rta-host/src/recovery.rs`.

use rta_host::config;
use rta_host::recovery::*;
use rta_spec::RecoveryMode;

#[test]
fn the_latch_holds_no_growing_state() {
    let source = include_str!("../../../rta-host/src/recovery.rs");
    let body = source.split("#[cfg(test)]").next().unwrap_or_default();
    for word in [
        "Vec<Step", "VecDeque", "push(", "HashMap", "panic!", "unwrap()",
    ] {
        let found = body
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .any(|line| line.contains(word) && !line.contains("out.push("));
        assert!(!found, "{word}");
    }
}

#[test]
fn the_recovery_note_matches_the_order_the_policy_and_the_ack_table() {
    let note = include_str!("../../../../docs/recovery.md");
    for (recovery, name) in [
        (RecoveryMode::Rtl, "rtl"),
        (RecoveryMode::Loiter, "loiter"),
        (RecoveryMode::Land, "land"),
    ] {
        let cells: Vec<String> = fallback_order(recovery)
            .iter()
            .map(|step| match step {
                Step::SetMode(mode) => format!(
                    "`MAV_CMD_DO_SET_MODE` AUTO_{}",
                    format!("{mode:?}").to_uppercase()
                ),
                other => format!("`{:?}`", other.command()),
            })
            .collect();
        let mut row = format!("| `{name}` | {} |", cells.join(" | "));
        if cells.len() == 2 {
            row.push_str(" none |");
        }
        assert!(note.contains(&row), "{row}");
    }
    let example = config::parse_str(include_str!("../../../../config/example.toml")).unwrap();
    for row in [
        format!(
            "| `recovery.retry_initial_ms` | {} |",
            example.recovery.retry_initial_ms
        ),
        format!(
            "| `recovery.retry_cap_ms` | {} |",
            example.recovery.retry_cap_ms
        ),
        format!(
            "| `recovery.attempts_per_step` | {} |",
            example.recovery.attempts_per_step
        ),
    ] {
        assert!(note.contains(&row), "{row}");
    }
    for value in 0..=6 {
        let code = AckResult::from_raw(value).code();
        assert!(note.contains(&format!("`ack:{code}`")), "{code}");
    }
    for item in [
        "never sent twice",
        "## When the vehicle never confirms",
        "the latch stops sending",
        "Revert stays latched until restart",
        "`recovery_unconfirmed`",
        "An ack never does.",
        "No PX4 SITL run of the retry or the fallback is recorded.",
        "is an owner decision",
    ] {
        assert!(note.contains(item), "{item}");
    }
    let requirements = include_str!("../../../../docs/requirements.md");
    assert!(requirements.contains("| R-R1 | recovery |"));
    assert!(requirements.contains("send no attempt twice from the tick thread and the watchdog"));
}

#[test]
fn the_default_order_gives_up_when_the_note_says() {
    let example = config::parse_str(include_str!("../../../../config/example.toml")).unwrap();
    let policy = Policy {
        initial_ms: example.recovery.retry_initial_ms,
        cap_ms: example.recovery.retry_cap_ms,
        attempts_per_step: example.recovery.attempts_per_step,
        ack_hold_ms: 0,
    };
    let sends =
        u32::try_from(fallback_order(RecoveryMode::Rtl).len()).unwrap() * policy.attempts_per_step;
    let last: u64 = (1..sends).map(|n| policy.wait_after(n)).sum();
    let gives_up = last + policy.wait_after(sends);
    let note = include_str!("../../../../docs/recovery.md");
    let line = format!(
        "there are {} sends over about {:.1} s. The latch gives up about {:.1} s after the first send.",
        ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine"]
            [sends as usize],
        last as f64 / 1_000.0,
        gives_up as f64 / 1_000.0
    );
    assert!(note.contains(&line), "{line}");
}
