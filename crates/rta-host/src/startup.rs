//! Startup and shutdown order. A failed step does not open the link.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Config,
    Spec,
    Log,
    Socket,
}

#[derive(Debug)]
pub struct Startup {
    steps: Vec<Step>,
    link_opened: bool,
}

impl Startup {
    pub fn run(config_ok: bool, spec_ok: bool, log_ok: bool) -> Self {
        let mut steps = vec![Step::Config];
        if !config_ok {
            return Self {
                steps,
                link_opened: false,
            };
        }
        steps.push(Step::Spec);
        if !spec_ok {
            return Self {
                steps,
                link_opened: false,
            };
        }
        steps.push(Step::Log);
        if !log_ok {
            return Self {
                steps,
                link_opened: false,
            };
        }
        steps.push(Step::Socket);
        Self {
            steps,
            link_opened: true,
        }
    }

    pub fn link_opened(&self) -> bool {
        self.link_opened
    }

    pub fn steps(&self) -> &[Step] {
        &self.steps
    }
}

#[derive(Debug)]
pub struct Shutdown {
    order: Vec<&'static str>,
}

impl Shutdown {
    pub fn run(mode_sent: bool, flush: &mut dyn FnMut()) -> Self {
        let mut order = vec!["stop_setpoints"];
        if mode_sent {
            order.push("mode");
        }
        flush();
        order.push("flush");
        order.push("exit");
        Self { order }
    }

    pub fn order(&self) -> &[&'static str] {
        &self.order
    }
}

#[cfg(test)]
mod tests {
    use super::{Shutdown, Startup, Step};

    #[test]
    fn a_spec_load_error_does_not_construct_the_link() {
        let startup = Startup::run(true, false, true);
        assert!(!startup.link_opened());
        assert!(!startup.steps().contains(&Step::Socket));
        assert_eq!(startup.steps(), vec![Step::Config, Step::Spec]);
    }

    #[test]
    fn shutdown_sends_the_mode_command_before_the_log_flush() {
        let mut saw_mode_before_flush = false;
        let mode_sent = true;
        let shutdown = Shutdown::run(mode_sent, &mut || {
            saw_mode_before_flush = mode_sent;
        });
        assert!(saw_mode_before_flush);
        let mode_at = shutdown.order().iter().position(|s| *s == "mode").unwrap();
        let flush_at = shutdown.order().iter().position(|s| *s == "flush").unwrap();
        assert!(mode_at < flush_at);
    }
}
