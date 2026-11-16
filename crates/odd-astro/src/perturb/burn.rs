//! Constant inertial-force finite burn. Supply start/end as integrator events
//! so RK steps do not straddle the discontinuities.
use super::{norm, PerturbationError};
/// Standard gravity, the constant that converts specific impulse to exhaust
/// velocity. Public so the propellant pool can draw the same mass flow a burn
/// consumes rather than re-deriving it.
pub const G0: f64 = 9.80665;
#[derive(Clone, Copy, Debug)]
pub struct Burn {
    pub start: f64,
    pub end: f64,
    pub force_newtons: [f64; 3],
    pub specific_impulse_seconds: f64,
}
impl Burn {
    /// Propellant mass flow while thrusting, kg/s: `F / (Isp · g₀)`.
    /// Returns 0 for a burn that cannot be evaluated, so a caller drawing from
    /// a pool never has to duplicate the validity rules below.
    pub fn mass_flow_kg_s(&self) -> f64 {
        if !self.specific_impulse_seconds.is_finite()
            || self.specific_impulse_seconds <= 0.0
            || !self.force_newtons.iter().all(|x| x.is_finite())
        {
            return 0.0;
        }
        norm(self.force_newtons) / (self.specific_impulse_seconds * G0)
    }

    pub fn acceleration_and_mass_rate(
        self,
        time: f64,
        mass: f64,
    ) -> Result<([f64; 3], f64), PerturbationError> {
        if !self.start.is_finite()
            || !self.end.is_finite()
            || self.end <= self.start
            || !time.is_finite()
            || !mass.is_finite()
            || mass <= 0.0
            || !self.specific_impulse_seconds.is_finite()
            || self.specific_impulse_seconds <= 0.0
            || !self.force_newtons.iter().all(|x| x.is_finite())
        {
            return Err(PerturbationError::InvalidInput);
        }
        if time < self.start || time >= self.end {
            return Ok(([0.0; 3], 0.0));
        }
        let thrust = norm(self.force_newtons);
        let acceleration = self.force_newtons.map(|f| f / mass);
        let rate = -thrust / (self.specific_impulse_seconds * G0);
        if !rate.is_finite() || !acceleration.iter().all(|x| x.is_finite()) {
            return Err(PerturbationError::NumericalRange);
        }
        Ok((acceleration, rate))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        integrator::{integrate, Options},
        spline::Keyframe,
    };
    #[test]
    fn burn_duration_and_rocket_equation() {
        let burn = Burn {
            start: 100.0,
            end: 1100.0,
            force_newtons: [1500.0, 0.0, 0.0],
            specific_impulse_seconds: 450.0,
        };
        let initial = Keyframe {
            time: 0.0,
            position: [0.0; 3],
            velocity: [0.0; 3],
            mass: 1000.0,
        };
        let opts = Options {
            relative_tolerance: 1e-12,
            absolute_tolerance: 1e-12,
            initial_step: 300.0,
            max_step: 300.0,
            max_steps: 100_000,
        };
        let spline = integrate(
            initial,
            1200.0,
            &[burn.start, burn.end],
            opts,
            |t, _, _, m| {
                burn.acceleration_and_mass_rate(t, m)
                    .map_err(|_| crate::integrator::IntegrationError::ForceFailure)
            },
        )
        .unwrap();
        let final_state = spline.sample(1200.0).unwrap();
        let expected = 1000.0 - 1000.0 * 1500.0 / (450.0 * G0);
        assert!((final_state.mass - expected).abs() < 1e-10);
        let dv = 450.0 * G0 * libm::log(1000.0 / expected);
        assert!((final_state.velocity[0] - dv).abs() < 1e-8);
    }
}
