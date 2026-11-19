//! V3 local flight instrument. Each tick advances the fitted v1 stage from
//! its last accepted checkpoint. No target orbit is ever used as a command.
use odd_astro::perturb::drag::surface_radius;
use odd_astro::{
    coupled::{checkpoint_at_end, integrate_coupled, CoupledCheckpoint, CoupledSetup, InertTanks},
    elements::Elements,
    perturb::{
        burn::{Burn, G0},
        pool::PropellantPool,
    },
    spline::Keyframe,
};
use odd_contracts::{environment, Contract, EARTH_EQUATORIAL_RADIUS as R, EARTH_MU};
use odd_core::frames::State;
use odd_sim::mission::{apsides, PredictionBaseline};
use odd_verify::FlightSettings;
use std::sync::Mutex;

struct Flight {
    setup: FlightSettings,
    contract: Contract,
    state: Keyframe,
    checkpoint: Option<CoupledCheckpoint>,
    dv: f64,
    pitch: f64,
    yaw: f64,
    roll: f64,
    impacted: bool,
    ended: bool,
    prediction: Option<PredictionBaseline>,
}
static FLIGHT: Mutex<Option<Flight>> = Mutex::new(None);
fn norm(v: [f64; 3]) -> f64 {
    libm::sqrt(v.iter().map(|x| x * x).sum())
}
fn unit(v: [f64; 3]) -> [f64; 3] {
    let n = norm(v);
    v.map(|x| x / n)
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
impl Flight {
    fn nose(&self, pitch: f64, yaw: f64) -> [f64; 3] {
        let t = unit(self.state.velocity);
        let n = unit(cross(self.state.position, t));
        let r = cross(t, n);
        std::array::from_fn(|i| {
            libm::cos(pitch) * (libm::cos(yaw) * t[i] + libm::sin(yaw) * n[i])
                + libm::sin(pitch) * r[i]
        })
    }
    fn surface_altitude(&self, r: [f64; 3]) -> Option<f64> {
        let l = self.contract.limits();
        Some(norm(r) - surface_radius(r, l.pole, l.equatorial_radius_m, l.polar_radius_m).ok()?)
    }
    fn elements(&self) -> Option<Elements> {
        Elements::from_state(
            State {
                position_m: self.state.position,
                velocity_m_s: self.state.velocity,
            },
            EARTH_MU,
        )
        .ok()
    }
    fn time_to_apsis(&self, target: f64) -> Option<f64> {
        let e = self.elements()?;
        if e.eccentricity < 1e-7 || e.eccentricity >= 1. {
            return None;
        }
        let eccentric =
            odd_astro::anomaly::elliptic_from_true(e.eccentricity, e.true_anomaly_rad).ok()?;
        let m = odd_astro::anomaly::elliptic_mean(e.eccentricity, eccentric).ok()?;
        let a = e.semi_major_axis_m()?;
        Some((target - m).rem_euclid(std::f64::consts::TAU) * libm::sqrt(a * a * a / EARTH_MU))
    }
    fn score(&self) -> Option<odd_contracts::MissionScore> {
        // Objective attainment is independent of the prediction component.
        // Never report a composite score for a pilot who did not predict.
        let prediction = self.prediction.unwrap_or(PredictionBaseline {
            delta_v_m_s: 0.,
            apoapsis_altitude_m: 0.,
            periapsis_altitude_m: 0.,
            remaining_propellant_kg: 0.,
            time_of_flight_s: 1.,
            target_miss_distance_m: None,
        });
        odd_contracts::score_delegated_fitted(
            &self.contract,
            self.state,
            self.state.time,
            prediction,
            self.dv,
            self.impacted,
            self.setup.delegation,
            self.setup.dry_mass,
        )
        .ok()
    }
    fn advance(&self, end: f64, force: [f64; 3]) -> Option<odd_astro::coupled::CoupledOutcome> {
        let s = &self.setup;
        let c = &self.contract;
        let start = self.state.time;
        let mut burn = Burn {
            start,
            end,
            force_newtons: force,
            specific_impulse_seconds: s.engine.specific_impulse_seconds,
        };
        let flow = burn.mass_flow_kg_s();
        // Stop the powered arc at the lean tank's remaining mixture, even if
        // the commanded interval straddles depletion. The remainder coasts.
        if flow > 0. {
            if let Some(cp) = self.checkpoint {
                burn.end = end.min(start + cp.pool.burnable_while_mixed() / flow);
            }
        }
        let burns = if flow > 0. && burn.end > start {
            std::slice::from_ref(&burn)
        } else {
            &[]
        };
        let inputs = s.thermal?;
        let eph = odd_verify::ephemeris()?;
        let env = |t| environment(eph, s.epoch, t);
        let resumed = self.checkpoint.as_ref();
        let outcome = if odd_systems::needs_thermal_model(inputs.engine_spec.propellant) {
            odd_verify::coupled_flight::fly_segment(s, c, inputs, resumed, burns, end, env)?.outcome
        } else {
            let setup = CoupledSetup {
                dry_mass_kg: s.dry_mass,
                pool: PropellantPool::new(inputs.load_kg, inputs.engine_spec.of_ratio)?,
                burns,
                options: c.options(),
            };
            let force = c.force_model();
            integrate_coupled(
                &setup,
                self.state,
                start,
                end,
                |t, r, v, m| force.field(t, r, v, m, Some(c.limits()), &mut |t| env(t)),
                &mut InertTanks,
                resumed,
            )
            .ok()?
        };
        Some(outcome)
    }
    fn step(
        &mut self,
        dt: f64,
        throttle: f64,
        pitch_axis: f64,
        yaw_axis: f64,
        roll_axis: f64,
    ) -> Option<()> {
        let start = self.state.time;
        let end = (start + dt).min(self.setup.duration_us as f64 / 1e6);
        if self.ended || end <= start {
            return None;
        }
        let wrap = |angle: f64| {
            (angle + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
        };
        // Rate-command attitude relative to the instantaneous velocity frame;
        // not a rigid-body/wheel torque model. Resolve pilot axes after roll.
        let dt = end - start;
        let roll = wrap(self.roll + 0.65 * dt * roll_axis);
        let pitch = (self.pitch
            + 0.65 * dt * (pitch_axis * libm::cos(roll) - yaw_axis * libm::sin(roll)))
        .clamp(-std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
        let yaw = wrap(
            self.yaw + 0.65 * dt * (yaw_axis * libm::cos(roll) + pitch_axis * libm::sin(roll)),
        );
        let force = self
            .nose(pitch, yaw)
            .map(|v| v * throttle * self.setup.engine.thrust_newtons);
        let mut outcome = self.advance(end, force)?;
        // Search accepted knots, not just the end of a long warp command.
        // Reintegrate the final interval so tanks and orbit stop together.
        let mut lo = start;
        let mut impact = None;
        for i in 1..outcome.trajectory.len() {
            let k = outcome.trajectory.keyframe(i)?;
            if self.surface_altitude(k.position)? <= 0. {
                impact = Some(k.time);
                break;
            }
            lo = k.time;
        }
        if let Some(mut hi) = impact {
            while hi - lo > 0.001 {
                let mid = (hi + lo) * 0.5;
                let trial = self.advance(mid, force)?;
                let cp = checkpoint_at_end(&trial)?;
                if self.surface_altitude(cp.position)? <= 0. {
                    hi = mid
                } else {
                    lo = mid
                }
            }
            outcome = self.advance(hi, force)?;
        }
        let s = &self.setup;
        let resumed = self.checkpoint.as_ref();
        let cp = checkpoint_at_end(&outcome)?;
        let prev = resumed.map_or(0., |x| x.pool.burned_kg());
        let burned = (cp.pool.burned_kg() - prev).max(0.);
        self.dv += s.engine.specific_impulse_seconds
            * G0
            * libm::log(self.state.mass / (self.state.mass - burned));
        self.state = Keyframe {
            time: cp.time_s,
            position: cp.position,
            velocity: cp.velocity,
            mass: s.dry_mass + cp.pool.remaining_kg(),
        };
        self.pitch = pitch;
        self.yaw = yaw;
        self.roll = roll;
        self.impacted = impact.is_some();
        self.ended = end >= s.duration_us as f64 / 1e6 || self.impacted;
        self.checkpoint = Some(cp);
        Some(())
    }
}
#[no_mangle]
pub extern "C" fn pilot_start() -> i32 {
    let Some(setup) = super::mission_bridge::fitted_settings() else {
        return 0;
    };
    let Some(contract) = Contract::get(setup.contract_id) else {
        return 0;
    };
    *FLIGHT.lock().unwrap() = Some(Flight {
        state: setup.initial,
        setup,
        contract,
        checkpoint: None,
        dv: 0.,
        pitch: 0.,
        yaw: 0.,
        roll: 0.,
        impacted: false,
        ended: false,
        prediction: None,
    });
    1
}
#[no_mangle]
pub extern "C" fn pilot_step(dt: f64, throttle: f64, pitch: f64, yaw: f64, roll: f64) -> i32 {
    if !dt.is_finite()
        || dt <= 0.
        || dt > 120.
        || !(0.0..=1.0).contains(&throttle)
        || !(-1.0..=1.0).contains(&pitch)
        || !(-1.0..=1.0).contains(&yaw)
        || !(-1.0..=1.0).contains(&roll)
        || (dt > 1.0 && (throttle != 0. || pitch != 0. || yaw != 0. || roll != 0.))
    {
        return 0;
    }
    FLIGHT
        .lock()
        .unwrap()
        .as_mut()
        .and_then(|f| f.step(dt, throttle, pitch, yaw, roll))
        .is_some() as i32
}
#[no_mangle]
pub extern "C" fn pilot_predict(ap: f64, pe: f64, fuel: f64, dv: f64, time: f64) -> i32 {
    let mut g = FLIGHT.lock().unwrap();
    let Some(f) = g.as_mut() else { return 0 };
    if f.state.time != 0.
        || ![ap, pe, fuel, dv, time].iter().all(|x| x.is_finite())
        || fuel < 0.
        || dv < 0.
        || time <= 0.
    {
        return 0;
    }
    f.prediction = Some(PredictionBaseline {
        delta_v_m_s: dv,
        apoapsis_altitude_m: ap,
        periapsis_altitude_m: pe,
        remaining_propellant_kg: fuel,
        time_of_flight_s: time,
        target_miss_distance_m: None,
    });
    1
}
/// SI telemetry. Angular controls/readouts in radians, epochs in seconds.
#[no_mangle]
pub extern "C" fn pilot_read(i: u32) -> f64 {
    let g = FLIGHT.lock().unwrap();
    let Some(f) = g.as_ref() else { return f64::NAN };
    let k = f.state;
    match i {
        0 => k.time,
        1..=3 => k.position[i as usize - 1],
        4..=6 => k.velocity[i as usize - 4],
        7 => k.mass - f.setup.dry_mass,
        8 => f.surface_altitude(k.position).unwrap_or(f64::NAN),
        9 => norm(k.velocity),
        10 | 11 => apsides(k, EARTH_MU, R).map_or(f64::NAN, |a| {
            if i == 10 {
                a.apoapsis_altitude_m
            } else {
                a.periapsis_altitude_m
            }
        }),
        12 => f.dv,
        13 => f.pitch,
        14 => f.yaw,
        15 => f.ended as u32 as f64,
        16 => f.setup.engine.specific_impulse_seconds * G0 * libm::log(k.mass / f.setup.dry_mass),
        17 => f.elements().map_or(f64::NAN, |e| e.inclination_rad),
        18..=20 => f.nose(f.pitch, f.yaw)[i as usize - 18],
        21 => f.checkpoint.map_or(0., |c| c.pool.vented_kg().iter().sum()),
        22 => f
            .prediction
            .and_then(|_| f.score())
            .map_or(f64::NAN, |s| s.s),
        23 => f.elements().map_or(f64::NAN, |e| e.true_anomaly_rad),
        24 => f.roll,
        25 => f.score().map_or(0., |s| s.objectives),
        26 => f.impacted as u8 as f64,
        27 => f.checkpoint.is_none_or(|cp| cp.pool.has_both_fluids()) as u8 as f64,
        28 => f.time_to_apsis(std::f64::consts::PI).unwrap_or(f64::NAN),
        29 => f.time_to_apsis(0.).unwrap_or(f64::NAN),
        30 => f.elements().map_or(f64::NAN, |e| e.ascending_node_rad),
        31 => f.elements().map_or(f64::NAN, |e| e.eccentricity),
        _ => f64::NAN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cockpit_rejects_invalid_input_without_advancing_then_flies_and_replays() {
        // Use a private flight rather than the process-global bridge: other
        // workspace tests exercise the mission station in parallel.
        let contract = Contract::get(101).unwrap();
        let setup = FlightSettings {
            contract_id: 101,
            epoch: odd_contracts::START_EPOCH,
            initial: contract.initial,
            duration_us: contract.duration_us,
            dry_mass: contract.dry_mass_kg,
            engine: contract.engine,
            node_metric: false,
            thermal: Some(odd_verify::ThermalInputs {
                engine_spec: contract.engine_spec,
                design: odd_contracts::ThermalDesign::baseline(),
                load_kg: contract.initial.mass - contract.dry_mass_kg,
            }),
            delegation: odd_contracts::DelegationMask::MANUAL,
            revision: odd_core::PhysicsRevision::FittedScoring,
        };
        let fresh = || Flight {
            setup,
            contract,
            state: setup.initial,
            checkpoint: None,
            dv: 0.,
            pitch: 0.,
            yaw: 0.,
            roll: 0.,
            impacted: false,
            ended: false,
            prediction: None,
        };
        let (mut flight, mut twin, mut coast) = (fresh(), fresh(), fresh());
        assert!(flight.step(0., 1., 0., 0., 0.).is_none());
        assert_eq!(flight.state.time, 0.);
        for _ in 0..10 {
            flight.step(0.2, 0.6, 0., 0., 0.).unwrap();
            twin.step(0.2, 0.6, 0., 0., 0.).unwrap();
            coast.step(0.2, 0., 0., 0., 0.).unwrap()
        }
        assert_eq!(flight.checkpoint, twin.checkpoint);
        assert!(flight.state.mass < coast.state.mass);
        assert!(norm(flight.state.velocity) > norm(coast.state.velocity));
    }
}
