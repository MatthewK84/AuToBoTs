//! Safety filter contract. It reshapes a setpoint. It does not build a command.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct State {
    pub north: f64,
    pub east: f64,
    pub down: f64,
    pub speed: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Setpoint {
    pub north: f64,
    pub east: f64,
    pub down: f64,
    pub yaw: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Constraints {
    pub speed_cap: f64,
    pub alt_floor: f64,
    pub alt_cap: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Filtered {
    pub setpoint: Setpoint,
    pub filter_intervened: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterFault {
    Timeout,
    NonFinite,
    Failed,
}

pub fn apply(
    _state: State,
    proposed: Setpoint,
    constraints: Constraints,
    timed_out: bool,
    failed: bool,
) -> Result<Filtered, FilterFault> {
    if timed_out {
        return Err(FilterFault::Timeout);
    }
    if failed {
        return Err(FilterFault::Failed);
    }
    if !finite(&proposed) || !constraints_finite(constraints) {
        return Err(FilterFault::NonFinite);
    }
    let mut out = proposed;
    let mut intervened = false;
    if proposed.down < constraints.alt_floor {
        out.down = constraints.alt_floor;
        intervened = true;
    }
    if proposed.down > constraints.alt_cap {
        out.down = constraints.alt_cap;
        intervened = true;
    }
    let speed = (proposed.north.hypot(proposed.east)).abs();
    if constraints.speed_cap >= 0.0 && speed > constraints.speed_cap && speed > 0.0 {
        let scale = constraints.speed_cap / speed;
        out.north = proposed.north * scale;
        out.east = proposed.east * scale;
        intervened = true;
    }
    if !finite(&out) {
        return Err(FilterFault::NonFinite);
    }
    Ok(Filtered {
        setpoint: out,
        filter_intervened: intervened,
    })
}

fn finite(setpoint: &Setpoint) -> bool {
    setpoint.north.is_finite()
        && setpoint.east.is_finite()
        && setpoint.down.is_finite()
        && setpoint.yaw.is_finite()
}

fn constraints_finite(constraints: Constraints) -> bool {
    constraints.speed_cap.is_finite()
        && constraints.alt_floor.is_finite()
        && constraints.alt_cap.is_finite()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> State {
        State {
            north: 0.0,
            east: 0.0,
            down: 10.0,
            speed: 1.0,
        }
    }

    fn constraints() -> Constraints {
        Constraints {
            speed_cap: 5.0,
            alt_floor: 0.0,
            alt_cap: 40.0,
        }
    }

    #[test]
    fn timeout_is_not_a_setpoint() {
        let proposed = Setpoint {
            north: 1.0,
            east: 0.0,
            down: 10.0,
            yaw: 0.0,
        };
        assert_eq!(
            apply(state(), proposed, constraints(), true, false),
            Err(FilterFault::Timeout)
        );
    }

    #[test]
    fn non_finite_is_not_a_setpoint() {
        let proposed = Setpoint {
            north: f64::NAN,
            east: 0.0,
            down: 10.0,
            yaw: 0.0,
        };
        assert_eq!(
            apply(state(), proposed, constraints(), false, false),
            Err(FilterFault::NonFinite)
        );
    }

    #[test]
    fn filter_does_not_name_a_command() {
        let source = include_str!("lib.rs");
        let link = ["MAV", "Link"].concat();
        let spec = ["rt", "lola"].concat();
        let command = ["Switch", "Command"].concat();
        let decide = ["fn ", "decide"].concat();
        assert!(!source.contains(&link));
        assert!(!source.contains(&spec));
        assert!(!source.contains(&command));
        assert!(!source.contains(&decide));
    }
}
