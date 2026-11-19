//! Propagation with a live propellant pool and a venting tank model.
//!
//! This is the v4 flight law. v3's `ForceModel::propagate_with_limits` flies a
//! mass that only decreases by burning; the tank model then reports, after the
//! fact, how much propellant would have boiled away if the tanks had been real.
//! That is fine for a physics exhibit and wrong for a game: a tug whose blanket
//! is bare reaches its last perigee kick carrying propellant the vehicle no
//! longer has, and a burn that should flame out completes.
//!
//! Here the two are one system. Every derivative evaluation does:
//!
//! ```text
//! tank ODE at (T_ox, T_fu) → rates
//! m = dry + pool.remaining − (what this step is carrying away)
//! a = gravity + J2 + drag(m) + SRP(m) + F/m   (F = 0 once the lean tank is dry)
//! ```
//!
//! Mass is **derived, never integrated**. The state vector carries position,
//! velocity and the two tank columns (temperature, liquid, cumulative vented);
//! mass is read back from the pool. An integrated mass column could drift from
//! the pool's own accounting, and a signed record must not be able to disagree
//! with itself about how much propellant was aboard.
//!
//! # Why the pool is stepped, not integrated
//!
//! `dp45::trial` runs seven stages, and a stage must not mutate anything. The
//! mass at stage *i* is therefore the pool at the start of the step minus the
//! burn and vent mass that stage implies. The committed step then advances the
//! pool exactly once, from the *accepted* end-of-step tank columns. The pool
//! stays a single source of truth and every stage still sees the mass it should.
//!
//! Two consequences worth naming:
//!
//! * **Flameout.** Thrust stops when the lean tank empties, inside the step
//!   where that happens. `flown_burns` reports the window actually flown, so a
//!   dossier can print a burn shorter than the plan, and `flameout` carries the
//!   instant it died.
//! * **Speed.** The tank model evaluates its environment at every stage of
//!   every substep. The contracts' 120 s maximum step keeps that bounded; a
//!   longer flight costs more steps, never a coarser model.

use crate::dp45;
use crate::integrator::IntegrationError;
use crate::perturb::burn::Burn;
use crate::perturb::pool::PropellantPool;
use crate::spline::{Keyframe, TrajectorySpline};

/// Number of state variables the coupled ODE carries.
pub const COUPLED_STATE: usize = 6 + 2 * TANK_STRIDE;
/// Per tank: temperature K, liquid kg, cumulative vented kg.
pub const TANK_STRIDE: usize = 3;

/// One tank's contribution to the coupled derivative.
#[derive(Clone, Copy, Debug, Default)]
pub struct TankRate {
    /// dT/dt, K/s. Zero while venting, when temperature is pinned.
    pub temperature_k_s: f64,
    /// d(liquid)/dt, kg/s. Never positive.
    pub liquid_kg_s: f64,
    /// d(vented)/dt, kg/s. Never negative.
    pub vented_kg_s: f64,
    /// Electrical draw of this tank's cooler, W.
    pub cooler_w: f64,
}

/// The tank model, as the coupled integrator sees it.
///
/// Implementors own the heat balance. The integrator owns the flight: it
/// passes the state it is holding and takes back rates, and the tank never
/// moves the vehicle.
pub trait TankModel {
    /// The temperature a tank starts at: its relief setting, saturated.
    fn initial_temperature_k(&mut self, index: usize) -> Result<f64, IntegrationError>;

    /// Rate form for one tank. `index` is 0 for oxidizer, 1 for fuel and
    /// `state` is that tank's live column: `[K, kg, kg]`.
    fn rates(
        &mut self,
        time_s: f64,
        index: usize,
        state: [f64; TANK_STRIDE],
    ) -> Result<TankRate, IntegrationError>;
}

/// A stage with no tank model (a storable, or a pre-cryogenic schema).
/// Nothing boils, so the coupled path degenerates to plain burning.
#[derive(Clone, Copy, Debug, Default)]
pub struct InertTanks;

impl TankModel for InertTanks {
    fn initial_temperature_k(&mut self, _index: usize) -> Result<f64, IntegrationError> {
        // No tank exists, so nothing reads this; a positive finite value keeps
        // the state vector valid.
        Ok(300.0)
    }

    fn rates(
        &mut self,
        _time_s: f64,
        _index: usize,
        _state: [f64; TANK_STRIDE],
    ) -> Result<TankRate, IntegrationError> {
        Ok(TankRate::default())
    }
}

/// The complete resumable state at an instant.
///
/// This is exactly what a v4 segment checkpoint signs, so a verifier can start
/// here instead of replaying from the pad. Every field is a state variable of
/// the coupled ODE or of the pool; none is a derived convenience.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoupledCheckpoint {
    pub time_s: f64,
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub pool: PropellantPool,
    /// Tank columns, `[K, kg, kg]` per tank, oxidizer then fuel.
    pub tanks: [[f64; TANK_STRIDE]; 2],
    /// Cumulative cooler energy, J.
    pub cooler_joules: f64,
}

/// What the coupled propagation produced.
#[derive(Clone, Debug)]
pub struct CoupledOutcome {
    pub trajectory: TrajectorySpline,
    /// Live propellant state at the end of the propagated window.
    pub pool: PropellantPool,
    /// Tank temperature columns at the end, K, oxidizer then fuel.
    pub tank_temperatures_k: [f64; 2],
    /// Tank liquid columns at the end, kg, oxidizer then fuel.
    pub tank_liquid_kg: [f64; 2],
    /// Cumulative vent loss at the end, kg, oxidizer then fuel.
    pub vented_kg: [f64; 2],
    /// Cumulative cooler draw at the end, J.
    pub cooler_joules: f64,
    /// Windows actually flown per scheduled burn, seconds. A flameout
    /// shortens an entry; it never removes one.
    pub flown_burns: Vec<(f64, f64)>,
    /// Time at which a tank emptied while a burn was still scheduled.
    pub flameout: Option<f64>,
    /// Right-hand-side evaluations actually performed. The resumption oracle
    /// counts these so "started at day 4" is measured, not asserted.
    pub rhs_evaluations: usize,
}

/// Everything the coupled loop needs that the caller owns.
pub struct CoupledSetup<'a> {
    /// Structural mass plus everything that neither boils nor burns, kg.
    pub dry_mass_kg: f64,
    /// The one pool both processes draw from.
    pub pool: PropellantPool,
    /// Scheduled constant-force segments, chronological, non-overlapping.
    pub burns: &'a [Burn],
    pub options: crate::integrator::Options,
}

/// Integrate the flight and the tanks together over `[start_s, end_s]`.
///
/// `initial` is the position, velocity and mass at `start_s`. A resumed
/// segment passes the checkpoint it is resuming from and nothing about
/// `[0, start_s]` is recomputed.
#[allow(clippy::too_many_arguments)]
pub fn integrate_coupled<M, A>(
    setup: &CoupledSetup<'_>,
    initial: Keyframe,
    start_s: f64,
    end_s: f64,
    accelerate: A,
    tanks: &mut M,
    resumed: Option<&CoupledCheckpoint>,
) -> Result<CoupledOutcome, IntegrationError>
where
    M: TankModel,
    A: Fn(f64, [f64; 3], [f64; 3], f64) -> Result<[f64; 3], IntegrationError>,
{
    let CoupledSetup {
        dry_mass_kg,
        pool,
        burns,
        options,
    } = setup;
    let (dry_mass_kg, burns, options) = (*dry_mass_kg, *burns, *options);
    if !start_s.is_finite()
        || !end_s.is_finite()
        || end_s < start_s
        || options.max_steps == 0
        || !dry_mass_kg.is_finite()
        || dry_mass_kg <= 0.0
        || pool.remaining_kg() <= 0.0
        || !initial.mass.is_finite()
    {
        return Err(IntegrationError::InvalidInput);
    }
    let _ = initial.mass;

    // The pool is part of the resumable state. Restarting it from the launch
    // value would silently fly the resumed leg with propellant the previous
    // segment already burned and vented.
    let mut live = resumed.map_or(*pool, |c| c.pool);
    let mut t = start_s;
    let mut position = resumed.map_or(initial.position, |c| c.position);
    let mut velocity = resumed.map_or(initial.velocity, |c| c.velocity);
    let mut cooler_joules = resumed.map_or(0.0, |c| c.cooler_joules);
    let mut columns: [[f64; TANK_STRIDE]; 2] = match resumed {
        Some(checkpoint) => checkpoint.tanks,
        None => {
            let mut fresh = [[0.0; TANK_STRIDE]; 2];
            for (index, column) in fresh.iter_mut().enumerate() {
                column[0] = tanks.initial_temperature_k(index)?;
                column[1] = live.remaining_kg_for(index);
                column[2] = 0.0;
                if !column[0].is_finite() || column[0] <= 0.0 {
                    return Err(IntegrationError::NumericalRange);
                }
            }
            fresh
        }
    };

    let mut trajectory = TrajectorySpline::new();
    trajectory
        .push(Keyframe {
            time: t,
            position,
            velocity,
            mass: dry_mass_kg + live.remaining_kg(),
        })
        .map_err(|_| IntegrationError::InvalidInput)?;

    let mut rhs = 0usize;
    let mut flown: Vec<(f64, f64)> = burns.iter().map(|b| (b.start, b.start)).collect();
    let mut flameout: Option<f64> = None;
    let mut h = options.initial_step.min(options.max_step);
    let mut attempts = 0usize;

    while t < end_s {
        attempts += 1;
        if attempts > options.max_steps {
            return Err(IntegrationError::StepLimit);
        }
        // Never step across a burn boundary: thrust is discontinuous there.
        let boundary = burns
            .iter()
            .flat_map(|b| [b.start, b.end])
            .filter(|edge| *edge > t)
            .fold(end_s, f64::min);
        let remaining = boundary - t;
        h = h.min(options.max_step).min(remaining);
        let next_t = if h >= remaining { boundary } else { t + h };
        if next_t <= t {
            return Err(IntegrationError::StepUnderflow);
        }
        let width = next_t - t;
        let step_mass = dry_mass_kg + live.remaining_kg();
        // The scheduled segment, whether or not the engine can still light.
        // A burn that cannot start at all is a flameout, not a missing burn, so
        // it stays in the schedule and reports a zero-length flown window.
        let scheduled: Option<Burn> = burns.iter().find(|b| t >= b.start && t < b.end).copied();
        // What actually thrusts. A dry lean tank means no thrust even though
        // the window is still open.
        let lit = scheduled.filter(|_| live.has_both_fluids());
        let y0 = state_vector(position, velocity, &columns);
        let flow = lit.map_or(0.0, |b| b.mass_flow_kg_s());

        let trial = dp45::trial(
            t,
            y0,
            width,
            options.relative_tolerance,
            options.absolute_tolerance,
            |time, z| {
                let mut tank_rates = [0.0; 2 * TANK_STRIDE];
                let mut vented_so_far = 0.0;
                for index in 0..2 {
                    let base = 6 + index * TANK_STRIDE;
                    let column = [z[base], z[base + 1], z[base + 2]];
                    let rate = tanks.rates(time, index, column)?;
                    if rate.liquid_kg_s > 0.0 {
                        // Neither a vent nor a heater can create liquid on the
                        // step the pool is accounting for; treat it as a fault.
                        return Err(IntegrationError::ForceFailure);
                    }
                    vented_so_far += -rate.liquid_kg_s * (time - t);
                    tank_rates[index * TANK_STRIDE] = rate.temperature_k_s;
                    tank_rates[index * TANK_STRIDE + 1] = rate.liquid_kg_s;
                    tank_rates[index * TANK_STRIDE + 2] = rate.vented_kg_s;
                }
                // The mass this stage should see: start-of-step mass less what
                // the step has already carried away.
                let burned = flow * (time - t).max(0.0);
                let stage_mass = (step_mass - burned - vented_so_far).max(dry_mass_kg);
                let mut acceleration =
                    accelerate(time, [z[0], z[1], z[2]], [z[3], z[4], z[5]], stage_mass)?;
                if let Some(burn) = lit {
                    if time >= burn.start && time < burn.end {
                        let thrust = burn
                            .acceleration_and_mass_rate(time, stage_mass)
                            .map_err(|_| IntegrationError::ForceFailure)?
                            .0;
                        for i in 0..3 {
                            acceleration[i] += thrust[i];
                        }
                    }
                }
                let mut out = [0.0; COUPLED_STATE];
                out[0..3].copy_from_slice(&[z[3], z[4], z[5]]);
                out[3..6].copy_from_slice(&acceleration);
                out[6..].copy_from_slice(&tank_rates);
                Ok(out)
            },
        );
        rhs += 7;
        let (y1, err) = match trial {
            Ok(value) => value,
            Err(IntegrationError::StepUnderflow) => return Err(IntegrationError::StepUnderflow),
            Err(_) if width > 1e-6 => {
                h *= 0.5;
                continue;
            }
            Err(error) => return Err(error),
        };
        if !err.is_finite() || err > 1.0 {
            h *= dp45::controller(err.max(f64::MIN_POSITIVE));
            continue;
        }

        // --- Commit. The pool advances exactly once, here. ---
        position = [y1[0], y1[1], y1[2]];
        velocity = [y1[3], y1[4], y1[5]];
        let previous_t = t;
        t = next_t;
        for (index, column) in columns.iter_mut().enumerate() {
            let base = 6 + index * TANK_STRIDE;
            *column = [y1[base], y1[base + 1].max(0.0), y1[base + 2].max(0.0)];
            // The tank ODE's own running vent total is authoritative.
            live.set_vented(index, column[2]);
        }
        if let Some(burn) = scheduled {
            // How much of this window the engine could actually fly. With
            // `flow` kilograms per second and `reachable` kilograms the lean
            // tank can still support, the burn dies after that many seconds.
            // A pool with no mixture left flies none of it.
            let reachable = if live.has_both_fluids() {
                live.burnable_while_mixed()
            } else {
                0.0
            };
            let flow = burn.mass_flow_kg_s();
            let wanted = flow * width;
            let flew = if wanted <= reachable {
                width
            } else if flow > 0.0 {
                (reachable / flow).clamp(0.0, width)
            } else {
                0.0
            };
            let flown_end = previous_t + flew;
            if let Some(entry) = flown.iter_mut().find(|(start, _)| *start == burn.start) {
                entry.1 = entry.1.max(flown_end);
            }
            live.draw_burn(&burn, flew);
            if flew < width && flameout.is_none() {
                flameout = Some(flown_end);
            }
        }
        // Cooler energy accumulates from the accepted tank state.
        let mut draw_w = 0.0;
        for (index, column) in columns.iter().enumerate() {
            draw_w += tanks.rates(t, index, *column)?.cooler_w;
        }
        cooler_joules += draw_w * width;

        trajectory
            .push(Keyframe {
                time: t,
                position,
                velocity,
                mass: dry_mass_kg + live.remaining_kg(),
            })
            .map_err(|_| IntegrationError::NumericalRange)?;
        h = width * dp45::controller(err);
    }

    Ok(CoupledOutcome {
        trajectory,
        pool: live,
        tank_temperatures_k: [columns[0][0], columns[1][0]],
        tank_liquid_kg: [columns[0][1], columns[1][1]],
        vented_kg: [columns[0][2], columns[1][2]],
        cooler_joules,
        flown_burns: flown,
        flameout,
        rhs_evaluations: rhs,
    })
}

/// The signed checkpoint at the end of a propagated window.
///
/// Every field must describe the *same* instant. The terminal state and the
/// pool do — the integrator stops at `end_s` and reports both from there — so
/// this takes no time argument. It used to accept one, which let a caller
/// build a checkpoint whose position came from an interpolated spline instant
/// while its propellant, tank columns and cooler draw came from the end of the
/// leg. That is not a state the vehicle was ever in, and it is exactly the
/// kind of "two truths about one vehicle" that `validate_resumable` rejects
/// from a client.
///
/// A checkpoint at an interior instant is therefore not produced here at all:
/// a segment boundary is defined by where propagation stopped.
pub fn checkpoint_at_end(outcome: &CoupledOutcome) -> Option<CoupledCheckpoint> {
    let last = outcome.trajectory.len().checked_sub(1)?;
    let frame = outcome.trajectory.keyframe(last)?;
    Some(CoupledCheckpoint {
        time_s: frame.time,
        position: frame.position,
        velocity: frame.velocity,
        pool: outcome.pool,
        tanks: [
            [
                outcome.tank_temperatures_k[0],
                outcome.tank_liquid_kg[0],
                outcome.vented_kg[0],
            ],
            [
                outcome.tank_temperatures_k[1],
                outcome.tank_liquid_kg[1],
                outcome.vented_kg[1],
            ],
        ],
        cooler_joules: outcome.cooler_joules,
    })
}

fn state_vector(
    position: [f64; 3],
    velocity: [f64; 3],
    columns: &[[f64; TANK_STRIDE]; 2],
) -> [f64; COUPLED_STATE] {
    let mut out = [0.0; COUPLED_STATE];
    out[0..3].copy_from_slice(&position);
    out[3..6].copy_from_slice(&velocity);
    for (index, column) in columns.iter().enumerate() {
        let base = 6 + index * TANK_STRIDE;
        out[base..base + TANK_STRIDE].copy_from_slice(column);
    }
    out
}
