//! The v4 flight path: coupling venting and burning into one live mass.
//!
//! v3 propagated with the plain force model and then integrated the tanks
//! *beside* the flight, so a tank could vent a tonne and the trajectory never
//! noticed. v4 flies the other way round: the tank columns are part of the
//! integrator state, the engine and the relief valves draw from one propellant
//! pool, and every acceleration sees `m_dry,fitted + m_prop(t)`.
//!
//! This module is the bridge, not a second implementation. It calls
//! `odd_astro::coupled::integrate_coupled` with the contract's own force field
//! (through `ForceModel::field`, the same body the uncoupled propagator uses)
//! and the real `StageTankModel`, so the live flight and the authority's replay
//! reach the same numbers by construction.

use odd_astro::coupled::{integrate_coupled, CoupledCheckpoint, CoupledOutcome, CoupledSetup};
use odd_astro::force_model::Environment;
use odd_astro::integrator::{IntegrationError, Options};
use odd_astro::perturb::burn::Burn;
use odd_astro::perturb::pool::PropellantPool;
use odd_astro::spline::{Keyframe, TrajectorySpline};
use odd_contracts::Contract;
use odd_systems::{StageTankModel, ThermalContext, ThermalEnvironment};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::{FlightSettings, ThermalInputs};

/// Fly a v4 mission under the coupled law.
///
/// Returns `None` when the stage has no tank model (a storable vehicle has
/// nothing to couple and must use the plain propagator), when the signed tank
/// design is not physical, or when the integration fails.
pub fn fly_coupled<E>(
    settings: &FlightSettings,
    contract: &Contract,
    burns: &[Burn],
    environment: E,
) -> Option<CoupledFlight>
where
    E: Fn(f64) -> Result<Environment, IntegrationError> + Copy,
{
    let inputs = settings.thermal?;
    fly_segment(
        settings,
        contract,
        inputs,
        None,
        burns,
        settings.duration_us as f64 / 1e6,
        environment,
    )
}

/// Advance a fitted cryogenic stage from an optional complete checkpoint.
/// A manual cockpit changes the force between segments, never the tank law.
pub fn fly_segment<E>(
    settings: &FlightSettings,
    contract: &Contract,
    inputs: ThermalInputs,
    resumed: Option<&CoupledCheckpoint>,
    burns: &[Burn],
    end_s: f64,
    environment: E,
) -> Option<CoupledFlight>
where
    E: Fn(f64) -> Result<Environment, IntegrationError> + Copy,
{
    let options = contract.options();
    let limits = contract.limits();
    let stage = inputs.design.stage_at(
        contract,
        &inputs.engine_spec,
        inputs.load_kg,
        settings.revision,
    )?;
    let pool = PropellantPool::new(inputs.load_kg, inputs.engine_spec.of_ratio)?;

    // The tank model is only handed the time, so it reads the vehicle position
    // and the ephemeris state that the acceleration closure published for the
    // same instant. Both closures run inside one right-hand-side evaluation, so
    // this is deterministic — and it has to be, because the authority repeats
    // the flight from the signed record and compares the metrics bit for bit.
    //
    // Vectors run from the tank *to* the body, matching
    // `odd_contracts::thermal::thermal_environment`.
    // The tank model is evaluated *before* the force field within each
    // right-hand-side call, so it cannot read a position the force field just
    // published. It reads the last accepted position instead — one step stale,
    // which is the same discretization the integrator already applies to the
    // state — and samples the ephemeris itself. `environment` is a pure
    // function of flight time, so both closures see identical Sun/Earth/Moon
    // geometry at the same instant and the flight reproduces exactly on replay.
    let position = Rc::new(Cell::new(
        resumed.map_or(settings.initial.position, |c| c.position),
    ));
    let for_tanks = Rc::clone(&position);
    // The ephemeris is by far the most expensive part of a right-hand-side
    // evaluation, and both tanks and the force field ask for it at the *same*
    // instant within one evaluation. One entry of cache removes two thirds of
    // those samples without changing a single number: `environment` is a pure
    // function of flight time.
    let clock = Rc::new(RefCell::new(None::<(f64, Environment)>));

    let sample = {
        let clock = Rc::clone(&clock);
        move |t: f64| sample_environment(&clock, environment, t)
    };
    let mut tanks = StageTankModel::new(
        stage.oxidizer,
        stage.fuel,
        stage.bus,
        ThermalContext {
            structure_temperature_k: stage.structure_temperature_k,
            sink_temperature_k: stage.sink_temperature_k,
            bus_w: stage.bus.discretionary_capacity(),
        },
        {
            let sample = &sample;
            move |t: f64| {
                let env = sample(t)?;
                let r = for_tanks.get();
                let to = |body: [f64; 3]| [body[0] - r[0], body[1] - r[1], body[2] - r[2]];
                Ok(ThermalEnvironment {
                    sun: to(env.sun.position),
                    earth: [-r[0], -r[1], -r[2]],
                    moon: to(env.companion.position),
                })
            }
        },
    )
    .ok()?;

    let force = contract.force_model();
    let setup = CoupledSetup {
        dry_mass_kg: settings.dry_mass,
        pool,
        burns,
        options,
    };
    let outcome = integrate_coupled(
        &setup,
        settings.initial,
        resumed.map_or(0.0, |c| c.time_s),
        end_s,
        |t, r, v, mass| {
            position.set(r);
            let env = sample(t)?;
            force.field(t, r, v, mass, Some(limits), &mut |_| Ok(env))
        },
        &mut tanks,
        resumed,
    )
    .ok()?;
    Some(CoupledFlight::new(outcome, tanks.vents().to_vec()))
}

/// The coupled flight, presented in the shape the rest of the verifier expects.
pub struct CoupledFlight {
    pub trajectory: TrajectorySpline,
    /// Relief-valve openings, recorded as the coupled loop opened them.
    pub vents: Vec<odd_systems::VentEvent>,
    /// Windows actually flown, seconds. A flameout shortens an entry.
    pub flown_burns: Vec<(f64, f64)>,
    pub flameout_s: Option<f64>,
    /// Propellant still aboard at the end of the propagated window.
    pub remaining_kg: f64,
    pub outcome: CoupledOutcome,
}

impl CoupledFlight {
    /// The final keyframe of the coupled trajectory.
    pub fn terminal(&self) -> Option<Keyframe> {
        self.trajectory
            .keyframe(self.trajectory.len().checked_sub(1)?)
    }

    /// Wrap a raw outcome and the vent history the tank model recorded.
    pub fn new(outcome: CoupledOutcome, vents: Vec<odd_systems::VentEvent>) -> Self {
        let trajectory = outcome.trajectory.clone();
        Self {
            trajectory,
            vents,
            flown_burns: outcome.flown_burns.clone(),
            flameout_s: outcome.flameout,
            remaining_kg: outcome.pool.remaining_kg(),
            outcome,
        }
    }
}

/// Options a coupled flight inherits from its contract, so a caller cannot
/// quietly run v4 at v3's tolerances.
pub fn coupled_options(contract: &Contract) -> Options {
    contract.options()
}

/// Sample the ephemeris once per flight instant and reuse it within one
/// right-hand-side evaluation. `environment` is a pure function of time, so
/// the cache can never change a result.
fn sample_environment<E>(
    cache: &RefCell<Option<(f64, Environment)>>,
    environment: E,
    t: f64,
) -> Result<Environment, IntegrationError>
where
    E: Fn(f64) -> Result<Environment, IntegrationError>,
{
    if let Some((cached, env)) = *cache.borrow() {
        if cached.to_bits() == t.to_bits() {
            return Ok(env);
        }
    }
    let env = environment(t)?;
    *cache.borrow_mut() = Some((t, env));
    Ok(env)
}
