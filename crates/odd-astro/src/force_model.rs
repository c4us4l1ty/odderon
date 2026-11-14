//! Assembly of body-centered inertial point gravity and optional forces.
//! The environment provider must return geometric body-centered positions
//! at each integration stage; there is no hidden ephemeris extrapolation.
use crate::{
    integrator::{integrate, IntegrationError, Options},
    perturb::{
        burn::Burn,
        drag::{self, DragParameters},
        j2, srp, third_body,
    },
    spline::{Keyframe, TrajectorySpline},
};

#[derive(Clone, Copy, Debug)]
pub struct ThirdBody {
    pub position: [f64; 3],
    pub mu: f64,
    pub radius: f64,
}
#[derive(Clone, Copy, Debug)]
pub struct Environment {
    pub sun: ThirdBody,
    pub companion: ThirdBody,
}
#[derive(Clone, Copy, Debug)]
pub struct Zonal {
    pub pole: [f64; 3],
    pub radius: f64,
    pub j2: f64,
}
#[derive(Clone, Copy, Debug)]
pub struct Radiation {
    pub cr: f64,
    pub area: f64,
    pub central_radius: f64,
}
#[derive(Clone, Copy, Debug)]
pub struct ForceModel {
    pub mu: f64,
    pub zonal: Option<Zonal>,
    pub drag: Option<DragParameters>,
    pub radiation: Option<Radiation>,
    pub third_body: bool,
    pub burn: Option<Burn>,
}
impl ForceModel {
    /// Numerically propagate forward. Burn start and end are automatically
    /// supplied as integration events. The drag mass is the current state mass.
    pub fn propagate<F>(
        &self,
        initial: Keyframe,
        end: f64,
        options: Options,
        mut environment: F,
    ) -> Result<TrajectorySpline, IntegrationError>
    where
        F: FnMut(f64) -> Result<Environment, IntegrationError>,
    {
        let mut events = Vec::new();
        if let Some(b) = self.burn {
            events.extend([b.start, b.end]);
        }
        integrate(initial, end, &events, options, |t, r, v, m| {
            if !self.mu.is_finite() || self.mu <= 0.0 {
                return Err(IntegrationError::InvalidInput);
            }
            let radius = libm::sqrt(r.iter().map(|x| x * x).sum::<f64>());
            if !radius.is_finite() || radius <= 0.0 {
                return Err(IntegrationError::ForceFailure);
            }
            let mut acceleration = r.map(|x| -(self.mu / radius / radius) * (x / radius));
            let mut add = |a: [f64; 3]| {
                for i in 0..3 {
                    acceleration[i] += a[i];
                }
            };
            if let Some(z) = self.zonal {
                add(j2::acceleration(r, z.pole, self.mu, z.radius, z.j2)
                    .map_err(|_| IntegrationError::ForceFailure)?);
            }
            if let Some(mut d) = self.drag {
                d.mass = m;
                add(drag::acceleration(r, v, d).map_err(|_| IntegrationError::ForceFailure)?);
            }
            if self.third_body || self.radiation.is_some() {
                let env = environment(t)?;
                if self.third_body {
                    for body in [env.sun, env.companion] {
                        add(third_body::acceleration(r, body.position, body.mu)
                            .map_err(|_| IntegrationError::ForceFailure)?);
                    }
                }
                if let Some(s) = self.radiation {
                    let sun = std::array::from_fn(|i| env.sun.position[i] - r[i]);
                    let central = r.map(|x| -x);
                    let companion = std::array::from_fn(|i| env.companion.position[i] - r[i]);
                    let visible = srp::illumination_two(
                        sun,
                        env.sun.radius,
                        central,
                        s.central_radius,
                        companion,
                        env.companion.radius,
                    )
                    .map_err(|_| IntegrationError::ForceFailure)?;
                    add(srp::acceleration(sun, s.cr, s.area, m, visible)
                        .map_err(|_| IntegrationError::ForceFailure)?);
                }
            }
            let rate = if let Some(b) = self.burn {
                let (a, rate) = b
                    .acceleration_and_mass_rate(t, m)
                    .map_err(|_| IntegrationError::ForceFailure)?;
                add(a);
                rate
            } else {
                0.0
            };
            Ok((acceleration, rate))
        })
    }
}
