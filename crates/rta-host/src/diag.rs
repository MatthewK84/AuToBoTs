//! Process diagnostics: startup errors, the run banner, the run summary. One line each, with
//! a level and a component, on stderr for `error` and `warn` and stdout for `info`. Not the
//! tick log: the tick log format and its version are unchanged (`docs/log-format.md`).
//!
//! `RTA_LOG` sets the lowest level shown: `error`, `warn`, or `info` (the default). An
//! unknown value shows everything, so a typo never hides an error.

use std::fmt::Write as _;
use std::io::Write as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Error,
    Warn,
    Info,
}

impl Level {
    pub fn name(self) -> &'static str {
        match self {
            Level::Error => "error",
            Level::Warn => "warn",
            Level::Info => "info",
        }
    }

    /// The lowest level `RTA_LOG` lets through.
    pub fn threshold(setting: Option<&str>) -> Level {
        match setting {
            Some("error") => Level::Error,
            Some("warn") => Level::Warn,
            _ => Level::Info,
        }
    }
}

/// One diagnostic line: `rta-host level=<level> component=<component> msg="<message>"`.
/// Quotes, backslashes, and control characters in the message are escaped, so a line is one
/// line and the fields stay parseable.
pub fn line(level: Level, component: &str, message: &str) -> String {
    let mut out = format!(
        "rta-host level={} component={component} msg=\"",
        level.name()
    );
    for c in message.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{{{:x}}}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Write one line if `RTA_LOG` lets the level through. A failed write is ignored: there is
/// nowhere left to report it.
pub fn emit(level: Level, component: &str, message: &str) {
    let threshold = Level::threshold(std::env::var("RTA_LOG").ok().as_deref());
    if level > threshold {
        return;
    }
    let text = line(level, component, message);
    let _ = match level {
        Level::Error | Level::Warn => writeln!(std::io::stderr(), "{text}"),
        Level::Info => writeln!(std::io::stdout(), "{text}"),
    };
}

pub fn error(component: &str, message: &str) {
    emit(Level::Error, component, message);
}

pub fn warn(component: &str, message: &str) {
    emit(Level::Warn, component, message);
}

pub fn info(component: &str, message: &str) {
    emit(Level::Info, component, message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_has_the_level_the_component_and_an_escaped_message() {
        assert_eq!(
            line(Level::Error, "config", "bad \"x\"\nline\\"),
            "rta-host level=error component=config msg=\"bad \\\"x\\\"\\nline\\\\\""
        );
        assert_eq!(
            line(Level::Info, "runtime", "tab\there"),
            "rta-host level=info component=runtime msg=\"tab\\u{9}here\""
        );
    }

    #[test]
    fn the_threshold_never_hides_an_error() {
        for setting in [
            None,
            Some("info"),
            Some("warn"),
            Some("error"),
            Some("ERROR"),
            Some(""),
        ] {
            assert!(Level::Error <= Level::threshold(setting), "{setting:?}");
        }
        assert_eq!(Level::threshold(Some("warn")), Level::Warn);
        assert!(Level::Info > Level::threshold(Some("warn")));
        assert_eq!(Level::threshold(Some("verbose")), Level::Info);
    }
}
