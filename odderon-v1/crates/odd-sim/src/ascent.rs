//! Gravity-turn guidance for surface launch.
//!
//! A gravity turn starts with thrust vertical (along the local up vector)
//! and progressively tilts the thrust vector toward the velocity direction,
//! keeping angle of attack near zero. The tilt begins after a pitchover time
//! and proceeds at a bounded rate. This is the standard ascent guidance for
//! rockets launching from a planetary surface into orbit.
//!
//! All angles are radians; all times are seconds; all positions/velocities are
//! SI in a common inertial frame.
//!
//! # Limitations
//!
//! This is a *guidance law*, not a complete ascent program. It does not model
//! throttle profiles, staging, abort modes, range safety, or wind. It produces
//! a sequence of inertial thrust directions; converting them to quantized burn
//! commands is the caller's responsibility (see `odd_sim::compile_burn`).

use crate::{direction_quaternion, planner, Command, CommandTimeline, Engine};
use odd_core::frames::State;

/// Error computing a gravity-turn guidance profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GravityTurnError {
    InvalidInput,
    NumericalRange,
}

/// Gravity-turn guidance parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GravityTurn {
    /// Seconds after liftoff before tilting begins. Vehicle flies vertical
    /// until this time (clears the launch tower).
    pub pitchover_time_s: f64,
    /// Maximum tilt rate, rad/s. The thrust vector tilts no faster than this.
    pub tilt_rate_rad_s: f64,
    /// Maximum tilt angle from local vertical, rad. The turn ends when this
    /// angle is reached (typically near orbital insertion).
    pub max_tilt_angle_rad: f64,
}

impl GravityTurn {
    /// Compute the desired inertial thrust direction at `time_s` given the
    /// current state. Before `pitchover_time_s` the direction is local up
    /// (radial outward). Afterward it tilts toward the velocity direction at
    /// `tilt_rate_rad_s`, capped at `max_tilt_angle_rad`.
    ///
    /// Returns the unit thrust direction vector in the same inertial frame as
    /// `state`.
    pub fn thrust_direction(
        &self,
        state: &State,
        time_s: f64,
    ) -> Result<[f64; 3], GravityTurnError> {
        if !time_s.is_finite() || time_s < 0.0 {
            return Err(GravityTurnError::InvalidInput);
        }
        let r = state.position_m;
        let v = state.velocity_m_s;
        if !r.iter().all(|x| x.is_finite()) || !v.iter().all(|x| x.is_finite()) {
            return Err(GravityTurnError::InvalidInput);
        }

        // Local up = radial direction from planet center.
        let r_mag = libm::sqrt(r[0] * r[0] + r[1] * r[1] + r[2] * r[2]);
        if r_mag <= 0.0 {
            return Err(GravityTurnError::InvalidInput);
        }
        let up = [r[0] / r_mag, r[1] / r_mag, r[2] / r_mag];

        // Velocity component perpendicular to radial (horizontal in LVLH).
        let v_radial = v[0] * up[0] + v[1] * up[1] + v[2] * up[2];
        let v_perp = [
            v[0] - v_radial * up[0],
            v[1] - v_radial * up[1],
            v[2] - v_radial * up[2],
        ];
        let v_perp_mag =
            libm::sqrt(v_perp[0] * v_perp[0] + v_perp[1] * v_perp[1] + v_perp[2] * v_perp[2]);

        // Tilt angle: zero before pitchover, then grows at tilt_rate until cap.
        let tilt_angle = if time_s <= self.pitchover_time_s {
            0.0
        } else {
            let elapsed = time_s - self.pitchover_time_s;
            (self.tilt_rate_rad_s * elapsed).min(self.max_tilt_angle_rad)
        };

        if tilt_angle == 0.0 {
            return Ok(up);
        }

        if v_perp_mag < 1e-6 {
            // No horizontal velocity yet: cannot tilt toward velocity.
            // Stay vertical until velocity develops.
            return Ok(up);
        }

        let v_hat = [
            v_perp[0] / v_perp_mag,
            v_perp[1] / v_perp_mag,
            v_perp[2] / v_perp_mag,
        ];

        // Spherical interpolation between up and v_hat by tilt_angle.
        // up and v_hat are orthogonal (v_perp ⟂ up by construction).
        let cos_t = libm::cos(tilt_angle);
        let sin_t = libm::sin(tilt_angle);
        let direction = [
            up[0] * cos_t + v_hat[0] * sin_t,
            up[1] * cos_t + v_hat[1] * sin_t,
            up[2] * cos_t + v_hat[2] * sin_t,
        ];

        let mag = libm::sqrt(
            direction[0] * direction[0] + direction[1] * direction[1] + direction[2] * direction[2],
        );
        if mag <= 0.0 || !mag.is_finite() {
            return Err(GravityTurnError::NumericalRange);
        }
        Ok([direction[0] / mag, direction[1] / mag, direction[2] / mag])
    }

    /// Compile a gravity-turn ascent into a `CommandTimeline` of full-throttle
    /// burns centered on `dt_s`-spaced nodes from `t0` to `t1`. Each burn is
    /// `burn_dur_s` long and points along the guidance direction at its node.
    /// Mass is not tracked here; callers must ensure propellant suffices.
    #[allow(clippy::too_many_arguments)]
    pub fn compile_timeline(
        &self,
        initial: State,
        mu: f64,
        t0: f64,
        t1: f64,
        dt_s: f64,
        burn_dur_s: f64,
        engine: Engine,
    ) -> Result<CommandTimeline, GravityTurnError> {
        if !t0.is_finite() || !t1.is_finite() || t1 <= t0 {
            return Err(GravityTurnError::InvalidInput);
        }
        if dt_s <= 0.0 || burn_dur_s <= 0.0 || burn_dur_s > dt_s {
            return Err(GravityTurnError::InvalidInput);
        }

        let mut timeline = CommandTimeline::default();
        let mut time = t0;
        let mut state = initial;

        while time < t1 {
            let direction = self.thrust_direction(&state, time)?;
            let quat = direction_quaternion(direction);
            let start_us = libm::round(time * 1e6) as i64;
            let dur_us = libm::round(burn_dur_s * 1e6) as i64;
            if dur_us < 1 {
                return Err(GravityTurnError::NumericalRange);
            }

            let command = Command {
                t_us: start_us,
                kind: crate::CommandKind::Burn {
                    stage: engine.stage,
                    engine: engine.engine,
                    throttle_u16: u16::MAX,
                    dur_us,
                    quat_i16: quat,
                },
            };
            timeline
                .insert(command)
                .map_err(|_| GravityTurnError::NumericalRange)?;

            // Advance state by dt_s under two-body coast for the next guidance
            // evaluation. This is a preview, not a propagated trajectory.
            let step = dt_s.min(t1 - time);
            state = planner::kepler_coast(state, mu, step)
                .map_err(|_| GravityTurnError::NumericalRange)?;
            time += step;
        }

        Ok(timeline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Engine;
    use std::f64::consts::PI;

    fn earth_engine() -> Engine {
        Engine {
            thrust_newtons: 1_000_000.0,
            specific_impulse_seconds: 300.0,
            stage: 0,
            engine: 0,
        }
    }

    #[test]
    fn gravity_turn_parameters_validate() {
        // Valid parameters.
        let gt = GravityTurn {
            pitchover_time_s: 10.0,
            tilt_rate_rad_s: 0.05,
            max_tilt_angle_rad: PI / 2.0,
        };
        let state = State {
            position_m: [6_578_137.0, 0.0, 0.0],
            velocity_m_s: [0.0, 100.0, 0.0],
        };
        assert!(gt.thrust_direction(&state, 5.0).is_ok());

        // Invalid time.
        assert!(gt.thrust_direction(&state, f64::NAN).is_err());
        assert!(gt.thrust_direction(&state, -1.0).is_err());
    }

    #[test]
    fn vertical_before_pitchover() {
        let gt = GravityTurn {
            pitchover_time_s: 10.0,
            tilt_rate_rad_s: 0.1,
            max_tilt_angle_rad: PI / 2.0,
        };
        let state = State {
            position_m: [6_578_137.0, 0.0, 0.0],
            velocity_m_s: [0.0, 100.0, 0.0],
        };
        // Before pitchover: thrust is purely radial (local up = +X).
        let d = gt.thrust_direction(&state, 5.0).unwrap();
        assert!((d[0] - 1.0).abs() < 1e-12);
        assert!(d[1].abs() < 1e-12);
        assert!(d[2].abs() < 1e-12);
    }

    #[test]
    fn tilt_grows_after_pitchover() {
        let gt = GravityTurn {
            pitchover_time_s: 0.0,
            tilt_rate_rad_s: 0.1,
            max_tilt_angle_rad: PI / 2.0,
        };
        let state = State {
            position_m: [6_578_137.0, 0.0, 0.0],
            velocity_m_s: [0.0, 1000.0, 0.0],
        };
        // At t=0, tilt=0: direction is pure up (+X).
        let d0 = gt.thrust_direction(&state, 0.0).unwrap();
        assert!((d0[0] - 1.0).abs() < 1e-12);

        // At t=10, tilt=1.0 rad: direction has a significant +Y component.
        let d10 = gt.thrust_direction(&state, 10.0).unwrap();
        assert!(d10[1] > 0.5);
        assert!(d10[0] < 1.0);
        assert!(d10[0] > 0.0);

        // At t=100, tilt is capped at pi/2: direction is pure velocity (+Y).
        let d100 = gt.thrust_direction(&state, 100.0).unwrap();
        assert!((d100[1] - 1.0).abs() < 1e-6);
        assert!(d100[0].abs() < 1e-6);
    }

    #[test]
    fn max_tilt_angle_capped() {
        let gt = GravityTurn {
            pitchover_time_s: 0.0,
            tilt_rate_rad_s: 10.0,   // very fast
            max_tilt_angle_rad: 0.3, // small cap
        };
        let state = State {
            position_m: [6_578_137.0, 0.0, 0.0],
            velocity_m_s: [0.0, 1000.0, 0.0],
        };
        let d = gt.thrust_direction(&state, 100.0).unwrap();
        // Direction should be tilted 0.3 rad from +X toward +Y.
        // cos(0.3) ≈ 0.955, sin(0.3) ≈ 0.296
        assert!((d[0] - 0.9553).abs() < 1e-3);
        assert!((d[1] - 0.2955).abs() < 1e-3);
    }

    #[test]
    fn no_horizontal_velocity_stays_vertical() {
        let gt = GravityTurn {
            pitchover_time_s: 0.0,
            tilt_rate_rad_s: 0.1,
            max_tilt_angle_rad: PI / 2.0,
        };
        // Purely radial velocity: no horizontal component to tilt toward.
        let state = State {
            position_m: [6_578_137.0, 0.0, 0.0],
            velocity_m_s: [100.0, 0.0, 0.0],
        };
        let d = gt.thrust_direction(&state, 100.0).unwrap();
        assert!((d[0] - 1.0).abs() < 1e-12);
        assert!(d[1].abs() < 1e-12);
        assert!(d[2].abs() < 1e-12);
    }

    #[test]
    fn compile_timeline_produces_ordered_burns() {
        let gt = GravityTurn {
            pitchover_time_s: 5.0,
            tilt_rate_rad_s: 0.05,
            max_tilt_angle_rad: PI / 4.0,
        };
        let initial = State {
            position_m: [6_578_137.0, 0.0, 0.0],
            velocity_m_s: [0.0, 100.0, 0.0],
        };
        let engine = earth_engine();
        let timeline = gt
            .compile_timeline(initial, 3.986_004_418e14, 0.0, 60.0, 10.0, 5.0, engine)
            .unwrap();
        let commands = timeline.commands();
        assert!(!commands.is_empty());
        // Commands must be in time order and non-overlapping.
        for w in commands.windows(2) {
            assert!(w[0].t_us < w[1].t_us);
        }
    }

    #[test]
    fn kepler_coast_preview_conserves_energy() {
        let state = State {
            position_m: [6_578_137.0, 0.0, 0.0],
            velocity_m_s: [0.0, 7784.0, 0.0],
        };
        let mu = 3.986_004_418e14;
        let next = planner::kepler_coast(state, mu, 60.0).unwrap();

        // Specific energy should be conserved.
        let e0 = 0.5 * 7784.0_f64.powi(2) - mu / 6_578_137.0;
        let v_mag = libm::sqrt(
            next.velocity_m_s[0].powi(2)
                + next.velocity_m_s[1].powi(2)
                + next.velocity_m_s[2].powi(2),
        );
        let r_mag = libm::sqrt(
            next.position_m[0].powi(2) + next.position_m[1].powi(2) + next.position_m[2].powi(2),
        );
        let e1 = 0.5 * v_mag * v_mag - mu / r_mag;
        assert!((e1 - e0).abs() < 1e-3, "energy {e1} vs {e0}");
    }
}
