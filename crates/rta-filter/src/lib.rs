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

/// Backup-set inputs. The policy is the recovery the flight controller will fly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rally {
    pub north: f64,
    pub east: f64,
    pub down: f64,
}

/// Point-mass double integrator. Drag, wind, attitude, and actuator lag are not in this model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackupModel {
    pub v_max: f64,
    pub a_max: f64,
    pub dt_s: f64,
    pub horizon_steps: u32,
    pub deadline_steps: u32,
}

/// Velocity in the same axes as `State`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Velocity {
    pub north: f64,
    pub east: f64,
    pub down: f64,
}

/// Roll the backup policy forward. A rollout past the deadline is `Timeout`, not the proposed command.
///
/// Model mismatch (drag, wind, attitude dynamics, actuator lag, fence survey error) is unmeasured.
/// It is not covered by a hidden margin.
pub fn backup_apply(
    state: State,
    velocity: Velocity,
    proposed: Setpoint,
    constraints: Constraints,
    rally: Rally,
    model: BackupModel,
    polygon: &[[f64; 2]],
) -> Result<Filtered, FilterFault> {
    if model.horizon_steps > model.deadline_steps {
        return Err(FilterFault::Timeout);
    }
    if !model_ok(model) || polygon.len() < 3 {
        return Err(FilterFault::Failed);
    }
    if !finite(&proposed)
        || !constraints_finite(constraints)
        || !state_ok(state)
        || !vel_ok(velocity)
    {
        return Err(FilterFault::NonFinite);
    }
    let pos = [state.north, state.east, state.down];
    let vel = [velocity.north, velocity.east, velocity.down];
    if recoverable(pos, vel, proposed, constraints, rally, model, polygon) {
        return Ok(Filtered {
            setpoint: proposed,
            filter_intervened: false,
        });
    }
    let hold = Setpoint {
        north: state.north,
        east: state.east,
        down: state.down,
        yaw: proposed.yaw,
    };
    let backup = if rally.north.is_finite() && rally.east.is_finite() && rally.down.is_finite() {
        Setpoint {
            north: rally.north,
            east: rally.east,
            down: rally.down,
            yaw: proposed.yaw,
        }
    } else {
        hold
    };
    if recoverable(pos, vel, backup, constraints, rally, model, polygon) {
        return Ok(Filtered {
            setpoint: backup,
            filter_intervened: true,
        });
    }
    if backup != hold && recoverable(pos, vel, hold, constraints, rally, model, polygon) {
        return Ok(Filtered {
            setpoint: hold,
            filter_intervened: true,
        });
    }
    Err(FilterFault::Failed)
}

fn recoverable(
    pos: [f64; 3],
    vel: [f64; 3],
    command: Setpoint,
    constraints: Constraints,
    rally: Rally,
    model: BackupModel,
    polygon: &[[f64; 2]],
) -> bool {
    let target = [command.north, command.east, command.down];
    let (mut pos, mut vel) = step(pos, vel, target, model);
    if !in_set(pos, vel, constraints, polygon, model.v_max) {
        return false;
    }
    let backup = if rally.north.is_finite() && rally.east.is_finite() && rally.down.is_finite() {
        [rally.north, rally.east, rally.down]
    } else {
        pos
    };
    for _ in 0..model.horizon_steps {
        (pos, vel) = step(pos, vel, backup, model);
        if !in_set(pos, vel, constraints, polygon, model.v_max) {
            return false;
        }
    }
    true
}

fn step(
    pos: [f64; 3],
    vel: [f64; 3],
    target: [f64; 3],
    model: BackupModel,
) -> ([f64; 3], [f64; 3]) {
    let error = [target[0] - pos[0], target[1] - pos[1], target[2] - pos[2]];
    let v_des = clamp_norm(error, model.v_max);
    let accel = [
        (v_des[0] - vel[0]) / model.dt_s,
        (v_des[1] - vel[1]) / model.dt_s,
        (v_des[2] - vel[2]) / model.dt_s,
    ];
    let accel = clamp_norm(accel, model.a_max);
    let next_vel = clamp_norm(
        [
            vel[0] + accel[0] * model.dt_s,
            vel[1] + accel[1] * model.dt_s,
            vel[2] + accel[2] * model.dt_s,
        ],
        model.v_max,
    );
    let next_pos = [
        pos[0] + next_vel[0] * model.dt_s,
        pos[1] + next_vel[1] * model.dt_s,
        pos[2] + next_vel[2] * model.dt_s,
    ];
    (next_pos, next_vel)
}

fn in_set(
    pos: [f64; 3],
    vel: [f64; 3],
    constraints: Constraints,
    polygon: &[[f64; 2]],
    v_max: f64,
) -> bool {
    if !pos.iter().all(|v| v.is_finite()) || !vel.iter().all(|v| v.is_finite()) {
        return false;
    }
    if pos[2] < constraints.alt_floor || pos[2] > constraints.alt_cap {
        return false;
    }
    let speed = (vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]).sqrt();
    if speed > v_max + 1e-9 {
        return false;
    }
    inside([pos[0], pos[1]], polygon)
}

fn clamp_norm(value: [f64; 3], cap: f64) -> [f64; 3] {
    let norm = (value[0] * value[0] + value[1] * value[1] + value[2] * value[2]).sqrt();
    if norm <= cap || norm == 0.0 {
        return value;
    }
    let scale = cap / norm;
    [value[0] * scale, value[1] * scale, value[2] * scale]
}

fn inside(point: [f64; 2], polygon: &[[f64; 2]]) -> bool {
    let mut crossed = false;
    let n = polygon.len();
    for i in 0..n {
        let a = polygon[i];
        let b = polygon[(i + 1) % n];
        let intersect = ((a[1] > point[1]) != (b[1] > point[1]))
            && (point[0] < (b[0] - a[0]) * (point[1] - a[1]) / (b[1] - a[1]) + a[0]);
        if intersect {
            crossed = !crossed;
        }
    }
    crossed
}

fn model_ok(model: BackupModel) -> bool {
    model.v_max.is_finite()
        && model.a_max.is_finite()
        && model.dt_s.is_finite()
        && model.v_max > 0.0
        && model.a_max > 0.0
        && model.dt_s > 0.0
        && model.horizon_steps > 0
}

fn state_ok(state: State) -> bool {
    state.north.is_finite()
        && state.east.is_finite()
        && state.down.is_finite()
        && state.speed.is_finite()
}

fn vel_ok(velocity: Velocity) -> bool {
    velocity.north.is_finite() && velocity.east.is_finite() && velocity.down.is_finite()
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

    fn square() -> Vec<[f64; 2]> {
        vec![[0.0, 0.0], [200.0, 0.0], [200.0, 200.0], [0.0, 200.0]]
    }

    fn model(horizon: u32, deadline: u32) -> BackupModel {
        BackupModel {
            v_max: 10.0,
            a_max: 2.0,
            dt_s: 0.05,
            horizon_steps: horizon,
            deadline_steps: deadline,
        }
    }

    fn rally() -> Rally {
        Rally {
            north: 100.0,
            east: 100.0,
            down: 20.0,
        }
    }

    #[test]
    fn interior_command_is_unchanged() {
        let state = State {
            north: 100.0,
            east: 100.0,
            down: 20.0,
            speed: 1.0,
        };
        let proposed = Setpoint {
            north: 110.0,
            east: 100.0,
            down: 20.0,
            yaw: 0.0,
        };
        let got = backup_apply(
            state,
            Velocity {
                north: 0.0,
                east: 0.0,
                down: 0.0,
            },
            proposed,
            constraints(),
            rally(),
            model(4, 8),
            &square(),
        )
        .expect("interior");
        assert_eq!(got.setpoint, proposed);
        assert!(!got.filter_intervened);
    }

    #[test]
    fn command_through_the_margin_is_reshaped_or_rejected() {
        let state = State {
            north: 195.0,
            east: 100.0,
            down: 20.0,
            speed: 10.0,
        };
        let proposed = Setpoint {
            north: 400.0,
            east: 100.0,
            down: 20.0,
            yaw: 0.0,
        };
        let mut through = model(2, 4);
        through.dt_s = 1.0;
        let got = backup_apply(
            state,
            Velocity {
                north: 10.0,
                east: 0.0,
                down: 0.0,
            },
            proposed,
            constraints(),
            rally(),
            through,
            &square(),
        );
        match got {
            Ok(filtered) => {
                assert!(filtered.filter_intervened);
                assert_ne!(filtered.setpoint, proposed);
                assert!(filtered.setpoint.north < 200.0);
            }
            Err(FilterFault::Failed) => {}
            other => panic!("unfiltered or unexpected: {other:?}"),
        }
    }

    #[test]
    fn rollout_past_the_deadline_is_not_the_command() {
        let state = State {
            north: 100.0,
            east: 100.0,
            down: 20.0,
            speed: 0.0,
        };
        let proposed = Setpoint {
            north: 110.0,
            east: 100.0,
            down: 20.0,
            yaw: 0.0,
        };
        let got = backup_apply(
            state,
            Velocity {
                north: 0.0,
                east: 0.0,
                down: 0.0,
            },
            proposed,
            constraints(),
            rally(),
            model(20, 4),
            &square(),
        );
        assert_eq!(got, Err(FilterFault::Timeout));
    }
}
