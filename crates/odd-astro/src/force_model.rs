//! Assembly of body-centered inertial point gravity and optional forces.
//! The environment provider must return geometric body-centered positions
//! at each integration stage; there is no hidden ephemeris extrapolation.
use crate::{
    integrator::{
        integrate_with_events, EventDirection, Integrated, IntegrationError, Options, StateEvent,
    },
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

/// A geometric central-body reference surface and optional entry altitude.
/// Impact terminates propagation at the surface; entry is recorded but does
/// not stop the flight. Neither event models landing, terrain or breakup.
#[derive(Clone, Copy, Debug)]
pub struct FlightLimits {
    pub pole: [f64; 3],
    pub equatorial_radius_m: f64,
    pub polar_radius_m: f64,
    pub entry_altitude_m: Option<f64>,
}

impl ForceModel {
    /// Numerically propagate forward. Burn start and end are automatically
    /// supplied as integration events. The drag mass is the current state mass.
    pub fn propagate<F>(
        &self,
        initial: Keyframe,
        end: f64,
        options: Options,
        environment: F,
    ) -> Result<TrajectorySpline, IntegrationError>
    where
        F: FnMut(f64) -> Result<Environment, IntegrationError>,
    {
        let burns: Vec<Burn> = self.burn.into_iter().collect();
        self.propagate_with_burns(initial, end, options, &burns, environment)
    }

    /// Propagate a chronological, non-overlapping schedule of constant-force
    /// burns. This replaces `self.burn`; events include every start and end.
    /// Dry mass and engine availability remain the caller's responsibility.
    /// Without flight limits, below-surface drag inputs fail as before.
    pub fn propagate_with_burns<F>(
        &self,
        initial: Keyframe,
        end: f64,
        options: Options,
        burns: &[Burn],
        environment: F,
    ) -> Result<TrajectorySpline, IntegrationError>
    where
        F: FnMut(f64) -> Result<Environment, IntegrationError>,
    {
        self.propagate_internal(initial, end, options, burns, None, environment)
            .map(|result| result.trajectory)
    }

    /// Propagate with a terminal central-body impact event and optional
    /// non-terminal atmospheric-entry event. Trial Runge–Kutta stages may
    /// probe just below the surface while localizing the crossing; drag there
    /// is evaluated at the reference surface solely to keep the trial finite.
    /// Accepted flight is truncated at impact and never continues underground.
    pub fn propagate_with_limits<F>(
        &self,
        initial: Keyframe,
        end: f64,
        options: Options,
        burns: &[Burn],
        limits: FlightLimits,
        environment: F,
    ) -> Result<Integrated, IntegrationError>
    where
        F: FnMut(f64) -> Result<Environment, IntegrationError>,
    {
        if ![limits.equatorial_radius_m, limits.polar_radius_m]
            .iter()
            .all(|x| x.is_finite() && *x > 0.0)
            || !limits.pole.iter().all(|x| x.is_finite())
            || limits
                .entry_altitude_m
                .is_some_and(|x| !x.is_finite() || x < 0.0)
        {
            return Err(IntegrationError::InvalidInput);
        }
        let surface = drag::surface_radius(
            initial.position,
            limits.pole,
            limits.equatorial_radius_m,
            limits.polar_radius_m,
        )
        .map_err(|_| IntegrationError::InvalidInput)?;
        if libm::sqrt(initial.position.iter().map(|x| x * x).sum::<f64>()) <= surface {
            return Err(IntegrationError::InvalidInput);
        }
        self.propagate_internal(initial, end, options, burns, Some(limits), environment)
    }

    fn propagate_internal<F>(
        &self,
        initial: Keyframe,
        end: f64,
        options: Options,
        burns: &[Burn],
        limits: Option<FlightLimits>,
        mut environment: F,
    ) -> Result<Integrated, IntegrationError>
    where
        F: FnMut(f64) -> Result<Environment, IntegrationError>,
    {
        let mut events = Vec::with_capacity(burns.len() * 2);
        for (i, b) in burns.iter().enumerate() {
            b.acceleration_and_mass_rate(b.start, initial.mass)
                .map_err(|_| IntegrationError::InvalidInput)?;
            if i > 0 && burns[i - 1].end > b.start {
                return Err(IntegrationError::InvalidInput);
            }
            events.extend([b.start, b.end]);
        }
        events.dedup();
        let altitude = |r: [f64; 3], limits: FlightLimits| -> f64 {
            match drag::surface_radius(
                r,
                limits.pole,
                limits.equatorial_radius_m,
                limits.polar_radius_m,
            ) {
                Ok(surface) => libm::sqrt(r.iter().map(|x| x * x).sum::<f64>()) - surface,
                Err(_) => f64::NAN,
            }
        };
        let impact = move |_: f64, r: [f64; 3], _: [f64; 3], _: f64| -> f64 {
            limits.map_or(f64::NAN, |l| altitude(r, l))
        };
        let entry = move |_: f64, r: [f64; 3], _: [f64; 3], _: f64| -> f64 {
            limits.map_or(f64::NAN, |l| {
                altitude(r, l) - l.entry_altitude_m.unwrap_or(0.0)
            })
        };
        let mut state_events = Vec::new();
        if limits.is_some() {
            state_events.push(StateEvent {
                name: "impact",
                direction: EventDirection::Descending,
                terminal: true,
                value: &impact,
            });
        }
        if limits.is_some_and(|l| l.entry_altitude_m.is_some()) {
            state_events.push(StateEvent {
                name: "atmospheric_entry",
                direction: EventDirection::Descending,
                terminal: false,
                value: &entry,
            });
        }
        integrate_with_events(
            initial,
            end,
            &events,
            &state_events,
            options,
            |t, r, v, m| {
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
                    let mut drag_r = r;
                    if let Some(l) = limits {
                        let surface = drag::surface_radius(
                            r,
                            l.pole,
                            l.equatorial_radius_m,
                            l.polar_radius_m,
                        )
                        .map_err(|_| IntegrationError::ForceFailure)?;
                        if radius < surface {
                            // Clamp a few meters below the ellipsoid for trial
                            // stages only; floating-point surface_radius can
                            // otherwise leave a tiny negative altitude after a
                            // nominal scale-to-surface correction.
                            let scale = (surface + 1.0) / radius;
                            drag_r = r.map(|x| x * scale);
                        }
                    }
                    add(drag::acceleration(drag_r, v, d)
                        .map_err(|_| IntegrationError::ForceFailure)?);
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
                        // Event localization may trial a point just below the
                        // reference surface. Earth then fully occults the Sun in
                        // this model; do not ask the angular-disk routine to
                        // evaluate an observer inside an occulter.
                        let visible = if radius <= s.central_radius {
                            0.0
                        } else {
                            srp::illumination_two(
                                sun,
                                env.sun.radius,
                                central,
                                s.central_radius,
                                companion,
                                env.companion.radius,
                            )
                            .map_err(|_| IntegrationError::ForceFailure)?
                        };
                        add(srp::acceleration(sun, s.cr, s.area, m, visible)
                            .map_err(|_| IntegrationError::ForceFailure)?);
                    }
                }
                let active = burns.iter().find(|b| t >= b.start && t < b.end);
                let rate = if let Some(b) = active {
                    let (a, rate) = b
                        .acceleration_and_mass_rate(t, m)
                        .map_err(|_| IntegrationError::ForceFailure)?;
                    add(a);
                    rate
                } else {
                    0.0
                };
                Ok((acceleration, rate))
            },
        )
    }

    /// The gravitational and non-gravitational field alone: no thrust.
    ///
    /// This is the same body as the propagator's right-hand side above, so a
    /// coupled integrator that supplies its own thrust cannot drift from the
    /// force law the uncoupled path uses. `burns` is empty by construction
    /// here, which is why the mass argument only reaches drag and SRP.
    pub fn field<F>(
        &self,
        t: f64,
        r: [f64; 3],
        v: [f64; 3],
        m: f64,
        limits: Option<FlightLimits>,
        environment: &mut F,
    ) -> Result<[f64; 3], IntegrationError>
    where
        F: FnMut(f64) -> Result<Environment, IntegrationError>,
    {
        if !self.mu.is_finite() || self.mu <= 0.0 || !m.is_finite() || m <= 0.0 {
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
            let mut drag_r = r;
            if let Some(l) = limits {
                let surface =
                    drag::surface_radius(r, l.pole, l.equatorial_radius_m, l.polar_radius_m)
                        .map_err(|_| IntegrationError::ForceFailure)?;
                if radius < surface {
                    let scale = (surface + 1.0) / radius;
                    drag_r = r.map(|x| x * scale);
                }
            }
            add(drag::acceleration(drag_r, v, d).map_err(|_| IntegrationError::ForceFailure)?);
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
                let visible = if radius <= s.central_radius {
                    0.0
                } else {
                    srp::illumination_two(
                        sun,
                        env.sun.radius,
                        central,
                        s.central_radius,
                        companion,
                        env.companion.radius,
                    )
                    .map_err(|_| IntegrationError::ForceFailure)?
                };
                add(srp::acceleration(sun, s.cr, s.area, m, visible)
                    .map_err(|_| IntegrationError::ForceFailure)?);
            }
        }
        Ok(acceleration)
    }
}
