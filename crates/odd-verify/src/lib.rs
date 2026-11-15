//! Portable flight evaluation and proof-of-trajectory verification.
//!
//! This crate is the single replay path. It has no `web-sys`, no
//! `wasm-bindgen` and no browser globals: the browser bridge, a command-line
//! verifier and a future edge worker all call `verify_record` here, so a
//! flight cannot be checked by one implementation and flown by another.
//!
//! It lives outside `odd-sim` because `odd-contracts` already depends on
//! `odd-sim`; putting the replay there would close a dependency cycle.
use odd_astro::{
    elements::Elements,
    finite_burn::isolate_finite_burn,
    integrator::IntegrationError,
    patched_conics::sphere_of_influence,
    shapley::attribute_shapley,
    spline::{Keyframe, TrajectorySpline},
};
use odd_contracts::{environment, Contract, Objective};
use odd_core::{epoch::Epoch, frames::State};
use odd_ephem::{Body, Ephemeris};
use odd_propulsion::{solve_engine, EngineSolution};
use odd_report::{dossier::Geometry, Fdr, FdrError};
use odd_sim::{
    encounter::find_entry,
    mission::{apsides, metric_apoapsis, LockedMission, PredictionBaseline},
    planner::{ManeuverNode, Plan},
    Engine,
};
use std::sync::OnceLock;

const MU: f64 = odd_contracts::EARTH_MU;
const EARTH: f64 = odd_contracts::EARTH_EQUATORIAL_RADIUS;
pub const MOON_MU: f64 = 4.902_800_118e12;
/// Length of the deterministic mission report. Layout is documented in
/// docs/mission-planning-feasibility.md and mirrored by odd-report.
pub const REPORT_LEN: usize = 58;

static EPHEMERIS: OnceLock<Option<Ephemeris>> = OnceLock::new();

/// The bundled ephemeris, loaded once. A headless verifier gets exactly the
/// environment the browser flew with; there is no network fetch.
pub fn ephemeris() -> Option<&'static Ephemeris> {
    EPHEMERIS.get_or_init(|| Ephemeris::bundled().ok()).as_ref()
}

/// Physical setup of a flight. The command schedule lives in the `Plan`;
/// these are the quantities a recorder must carry to rebuild it.
#[derive(Clone, Copy, Debug)]
pub struct FlightSettings {
    pub contract_id: u32,
    pub epoch: i64,
    pub initial: Keyframe,
    pub duration_us: i64,
    pub dry_mass: f64,
    pub engine: Engine,
    pub node_metric: bool,
    /// Physical engine design and cryogenic fit-out. `None` means the stage
    /// carries no tank model (storables, or a legacy schema-1/2 record), so
    /// boil-off is identically zero and older certificates still replay.
    pub thermal: Option<ThermalInputs>,
    /// Which disciplines the Chief-AI sized. Caps Seff at 0.65 when non-zero.
    pub delegation: odd_contracts::DelegationMask,
}

/// The physical inputs the tank model needs, all of them design choices.
#[derive(Clone, Copy, Debug)]
pub struct ThermalInputs {
    pub engine_spec: odd_propulsion::EngineSpec,
    pub design: odd_contracts::ThermalDesign,
    pub load_kg: f64,
}

/// Everything a flight produced: the flown path, the 58-field report, the
/// terminal state and the realized burns.
pub struct FlightOutcome {
    pub trajectory: TrajectorySpline,
    pub report: [f64; REPORT_LEN],
    pub final_state: Keyframe,
    pub burns: Vec<odd_astro::perturb::burn::Burn>,
    /// Exact swept angle per burn, in command order. Their sum is report[57].
    pub burn_sweeps: Vec<f64>,
    /// Boil-off evaluated along the flown spline. Inert for storable stages.
    pub thermal: odd_contracts::ThermalOutcome,
}

/// Earth-relative Moon state at a mission time. Shared with the browser's
/// patched-conic preview so both use one environment convention.
pub fn moon_state(
    eph: &Ephemeris,
    epoch: i64,
    t: f64,
) -> Result<State, odd_sim::planner::PlanError> {
    let at = Epoch::new(epoch, t).map_err(|_| odd_sim::planner::PlanError::NumericalRange)?;
    eph.state(Body::Moon, Body::Earth, at)
        .map(|v| v.state)
        .map_err(|_| odd_sim::planner::PlanError::NumericalRange)
}
fn lunar_soi() -> f64 {
    sphere_of_influence(384_400_000.0, MOON_MU, MU).unwrap_or(66_100_000.0)
}

fn radial(s: Keyframe) -> f64 {
    s.position.iter().zip(s.velocity).map(|(r, v)| r * v).sum()
}
fn first_apo_after(path: &TrajectorySpline, after: f64) -> Option<f64> {
    let mut bracket = None;
    for i in 1..path.len() {
        let a = path.keyframe(i - 1)?;
        let b = path.keyframe(i)?;
        if a.time >= after && radial(a) > 0.0 && radial(b) <= 0.0 {
            bracket = Some((a.time, b.time));
            break;
        }
    }
    let (mut lo, mut hi) = bracket?;
    for _ in 0..48 {
        let mid = lo + (hi - lo) * 0.5;
        if mid <= lo || mid >= hi {
            break;
        }
        if radial(path.sample(mid).ok()?) > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some(lo + (hi - lo) * 0.5)
}

/// The whole flight evaluation, free of module state.
///
/// Replay verification calls this same function with settings rebuilt from a
/// signed recorder, so a verified flight is reproduced by the identical code
/// path rather than a second implementation that could agree by accident.
pub fn compute(
    s: &FlightSettings,
    plan: &Plan,
    locked: Option<&LockedMission>,
    eph: &Ephemeris,
) -> Option<FlightOutcome> {
    let Ok(burns) = plan.timeline.burns(&[s.engine]) else {
        return None;
    };
    let contract = Contract::get(s.contract_id).unwrap_or_else(|| Contract::get(101).unwrap());
    let model = contract.force_model();
    let options = contract.options();
    let limits = contract.limits();
    let end = s.duration_us as f64 / 1e6;
    let Ok(integrated) =
        model.propagate_with_limits(s.initial, end, options, &burns, limits, |t| {
            environment(eph, s.epoch, t)
        })
    else {
        return None;
    };
    let impacted = integrated.stopped.is_some();
    let mut evaluation_s = if let Some(stop) = integrated.stopped {
        stop.time
    } else {
        end
    };
    if !impacted && contract.objective == Objective::TransLunarApoapsis {
        let after = burns.last().map_or(0.0, |b| b.end);
        if let Some(apo) = first_apo_after(&integrated.trajectory, after) {
            evaluation_s = apo;
        }
    }
    let Ok(final_state) = integrated.trajectory.sample(evaluation_s) else {
        return None;
    };
    let el = Elements::from_state(
        State {
            position_m: final_state.position,
            velocity_m_s: final_state.velocity,
        },
        MU,
    )
    .ok();
    let ap = apsides(final_state, MU, EARTH).ok();
    let initial_el = Elements::from_state(
        State {
            position_m: s.initial.position,
            velocity_m_s: s.initial.velocity,
        },
        MU,
    )
    .ok();
    let node_change = |state: Keyframe| -> Result<f64, IntegrationError> {
        let initial = initial_el.ok_or(IntegrationError::ForceFailure)?;
        let el = Elements::from_state(
            State {
                position_m: state.position,
                velocity_m_s: state.velocity,
            },
            MU,
        )
        .map_err(|_| IntegrationError::ForceFailure)?;
        if libm::sin(el.inclination_rad).abs() < 1e-8 {
            return Err(IntegrationError::ForceFailure);
        }
        Ok(
            (el.ascending_node_rad - initial.ascending_node_rad + std::f64::consts::PI)
                .rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI,
        )
    };
    // Boil-off is integrated along the *flown* spline with the same adaptive
    // stepper and the same ephemeris, so replay reproduces the vent history.
    let thermal = match s.thermal {
        Some(inputs) => odd_contracts::thermal::evaluate(
            odd_contracts::thermal::ThermalStage {
                contract: &contract,
                spec: &inputs.engine_spec,
                load_kg: inputs.load_kg,
                design: &inputs.design,
            },
            &integrated.trajectory,
            odd_systems::ThermalClock {
                start_s: 0.0,
                // Boil-off accrues for the whole flight, not just up to the
                // scoring instant: a tug that reaches its apoapsis on day 3
                // keeps losing propellant until the contract window closes.
                end_s: if impacted { evaluation_s } else { end },
                options,
            },
            eph,
            s.epoch,
        )
        .unwrap_or_else(|_| odd_contracts::ThermalOutcome::inert()),
        None => odd_contracts::ThermalOutcome::inert(),
    };
    let mut report = [f64::NAN; REPORT_LEN];
    report[0] = final_state.mass;
    if let Some(ap) = ap {
        report[1] = ap.apoapsis_altitude_m;
        report[2] = ap.periapsis_altitude_m;
        report[3] = ap.eccentricity;
    }
    if let Some(el) = el {
        report[26] = el.inclination_rad;
        report[27] = el.ascending_node_rad;
    }
    report[5] = plan.requested_delta_v_m_s;
    report[29] = evaluation_s;
    report[30] = if impacted { 1.0 } else { 0.0 };
    report[31] = integrated
        .events
        .iter()
        .find(|e| e.name == "atmospheric_entry")
        .map_or(f64::NAN, |e| e.time);
    if let Ok(Some(enc)) = find_entry(&integrated.trajectory, lunar_soi(), |t| {
        moon_state(eph, s.epoch, t)
    }) {
        report[32] = enc.time_s;
        report[33] = enc.soi_radius_m;
    }
    let mission_score = locked.and_then(|lock| {
        odd_contracts::score_delegated(
            &contract,
            final_state,
            evaluation_s,
            lock.prediction(),
            plan.requested_delta_v_m_s,
            impacted,
            s.delegation,
        )
        .ok()
    });
    if let Some(score) = mission_score {
        report[4] = score.s;
        report[34] = score.prediction;
        report[35] = score.objectives;
        report[36] = score.efficiency;
    }
    if contract.objective == Objective::TransLunarApoapsis {
        if let Some(ap) = ap {
            report[40] = (ap.apoapsis_altitude_m + EARTH - 380_000_000.0).abs();
        }
    }
    // Exact finite-burn loss is isolated in the plain two-body model so J2,
    // third bodies, drag and SRP cannot contaminate the steering/gravity integrals.
    let mut burn_sweeps = vec![f64::NAN; burns.len()];
    if !burns.is_empty() {
        let loss_end = burns.last().map_or(evaluation_s, |b| b.end);
        if let Ok((isolated, loss)) = isolate_finite_burn(s.initial, loss_end, options, MU, &burns)
        {
            // Per-burn parts are measured on the same isolated trajectory, so
            // the printed schedule and the reported total cannot disagree.
            if let Ok(parts) = odd_astro::finite_burn::measure_burns_each(&isolated, &burns, MU) {
                for (slot, part) in burn_sweeps.iter_mut().zip(parts.iter()) {
                    *slot = part.sweep_rad;
                }
            }
            report[20] = loss.ideal_delta_v_m_s;
            report[21] = loss.steering_loss_m_s;
            report[22] = loss.gravity_loss_m_s;
            report[23] = loss.speed_deficit_m_s;
            report[24] = loss.identity_residual_m_s;
            report[25] = loss.burn_duration_s;
            report[57] = loss.sweep_rad;
        }
    }
    if ap.is_some() {
        let metric = |state| {
            if s.node_metric {
                node_change(state)
            } else {
                metric_apoapsis(state, MU, EARTH)
            }
        };
        // Apoapsis is a mission outcome; attribution is measured at a common
        // post-burn epoch. For the tug, the ideal multi-impulse preview can
        // impact on a later return even when the finite flight survives.
        let attribution_end = if contract.objective == Objective::TransLunarApoapsis {
            burns.last().map_or(evaluation_s, |b| b.end + 1.0)
        } else {
            evaluation_s
        };
        if let Ok(shapley) = attribute_shapley(
            s.initial,
            attribution_end,
            options,
            model,
            &burns,
            &plan.impulses,
            limits,
            |t| environment(eph, s.epoch, t),
            metric,
        ) {
            report[18] = shapley.common_end_s;
            report[19] = if shapley.truncated { 1.0 } else { 0.0 };
            report[37] = shapley.common_end_s;
            report[38] = report[19];
            report[14] = shapley.total_error;
            report[15] = shapley.reference_metric;
            report[16] = shapley.full_metric;
            report[17] = shapley.efficiency_residual;
            report[10..14].copy_from_slice(&shapley.contributions);
            if s.node_metric {
                report[28] = shapley.full_metric;
            }
            if !shapley.truncated && !impacted {
                if let Some(shares) = shapley.shares {
                    report[6..10].copy_from_slice(&shares);
                }
            }
            report[41..57].copy_from_slice(&shapley.coalition_metrics);
        }
    }
    Some(FlightOutcome {
        thermal,
        trajectory: integrated.trajectory,
        report,
        final_state,
        burns,
        burn_sweeps,
    })
}

/// Everything needed to re-run a flight, taken from a signed recorder.
#[derive(Clone)]
pub struct FlightSpec {
    pub contract_id: u32,
    pub epoch_s: i64,
    pub duration_us: i64,
    pub initial_mass_kg: f64,
    pub dry_mass_kg: f64,
    pub thrust_n: f64,
    pub specific_impulse_s: f64,
    pub initial_state: [f64; 6],
    pub node_metric: bool,
    pub prediction: [f64; 6],
    /// (epoch in microseconds, TNW delta-v, optional guidance state)
    pub commands: Vec<(i64, [f64; 3], Option<[f64; 6]>)>,
    /// The signed cryogenic fit-out, rebuilt from the record. `None` for a
    /// pre-v3 record or a storable stage.
    pub thermal: Option<ThermalInputs>,
    /// Signed delegation provenance, so a replay applies the same Seff cap.
    pub delegation: odd_contracts::DelegationMask,
}

pub struct ReplayOutcome {
    pub report: [f64; REPORT_LEN],
    pub final_state_hash: [u8; 32],
    pub geometry: Geometry,
    /// Recomputed relief-valve openings, for comparison with the signed list.
    pub vent_events: Vec<odd_report::VentEvent>,
    /// Recomputed loss per tank and cooler draw, compared bit-exactly.
    pub boil_off_kg: [f64; 2],
    pub cryocooler_energy_j: f64,
}

fn settings_from_spec(
    spec: &FlightSpec,
) -> Option<(FlightSettings, Vec<ManeuverNode>, Vec<Option<State>>)> {
    if spec.duration_us <= 0
        || spec.commands.len() > 12
        || !spec.initial_state.iter().all(|value| value.is_finite())
        || !spec.initial_mass_kg.is_finite()
        || !spec.dry_mass_kg.is_finite()
        || spec.dry_mass_kg <= 0.0
        || spec.initial_mass_kg < spec.dry_mass_kg
        || !spec.thrust_n.is_finite()
        || spec.thrust_n <= 0.0
        || !spec.specific_impulse_s.is_finite()
        || spec.specific_impulse_s <= 0.0
    {
        return None;
    }
    let mut nodes = Vec::with_capacity(spec.commands.len());
    let mut guidance = Vec::with_capacity(spec.commands.len());
    for (epoch_us, tnw, target) in &spec.commands {
        if *epoch_us <= 0 || *epoch_us > spec.duration_us || !tnw.iter().all(|v| v.is_finite()) {
            return None;
        }
        nodes.push(ManeuverNode {
            epoch_us: *epoch_us,
            tnw_m_s: *tnw,
        });
        guidance.push(target.map(|values| State {
            position_m: [values[0], values[1], values[2]],
            velocity_m_s: [values[3], values[4], values[5]],
        }));
    }
    Some((
        FlightSettings {
            // Schema 1/2 records predate the tank model; they replay with no
            // boil-off so their certificates remain valid.
            thermal: spec.thermal,
            delegation: spec.delegation,
            contract_id: spec.contract_id,
            epoch: spec.epoch_s,
            initial: Keyframe {
                time: 0.0,
                position: [
                    spec.initial_state[0],
                    spec.initial_state[1],
                    spec.initial_state[2],
                ],
                velocity: [
                    spec.initial_state[3],
                    spec.initial_state[4],
                    spec.initial_state[5],
                ],
                mass: spec.initial_mass_kg,
            },
            duration_us: spec.duration_us,
            dry_mass: spec.dry_mass_kg,
            engine: Engine {
                thrust_newtons: spec.thrust_n,
                specific_impulse_seconds: spec.specific_impulse_s,
                stage: 0,
                engine: 0,
            },
            node_metric: spec.node_metric,
        },
        nodes,
        guidance,
    ))
}

/// Project a state onto the initial orbital plane. The basis is fixed by the
/// initial position and angular momentum, so the figure is reproducible from
/// the recorder alone and does not depend on a camera.
fn plane_projection(initial: &Keyframe) -> (State, [f64; 3], [f64; 3]) {
    let r = initial.position;
    let v = initial.velocity;
    let norm = |a: [f64; 3]| (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-12);
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let rn = norm(r);
    let u = [r[0] / rn, r[1] / rn, r[2] / rn];
    let h = cross(r, v);
    let hn = norm(h);
    let w = [h[0] / hn, h[1] / hn, h[2] / hn];
    (
        State {
            position_m: r,
            velocity_m_s: v,
        },
        u,
        cross(w, u),
    )
}

const FIGURE_SAMPLES: usize = 480;

pub fn geometry_from(
    initial: &Keyframe,
    level1: &Plan,
    outcome: &FlightOutcome,
    commands: &[(i64, [f64; 3])],
) -> Geometry {
    let (_, u, v) = plane_projection(initial);
    let project = |state: Keyframe| {
        [
            state.position[0] * u[0] + state.position[1] * u[1] + state.position[2] * u[2],
            state.position[0] * v[0] + state.position[1] * v[1] + state.position[2] * v[2],
        ]
    };
    let end = outcome.report[29];
    let end = if end.is_finite() && end > 0.0 {
        end
    } else {
        1.0
    };
    let mut level1_points = Vec::with_capacity(FIGURE_SAMPLES + 1);
    let mut level3_points = Vec::with_capacity(FIGURE_SAMPLES + 1);
    for index in 0..=FIGURE_SAMPLES {
        let seconds = end * index as f64 / FIGURE_SAMPLES as f64;
        let micros = (seconds * 1e6).round() as i64;
        if let Ok(state) = level1.sample(micros) {
            level1_points.push(project(state));
        }
        if let Ok(state) = outcome.trajectory.sample(seconds) {
            level3_points.push(project(state));
        }
    }
    let burns = outcome
        .burns
        .iter()
        .enumerate()
        .map(|(index, burn)| odd_report::dossier::BurnRow {
            start_s: burn.start,
            duration_s: burn.end - burn.start,
            delta_v_tnw: commands
                .get(index)
                .map_or([f64::NAN; 3], |command| command.1),
            sweep_rad: outcome.burn_sweeps.get(index).copied().unwrap_or(f64::NAN),
        })
        .collect();
    Geometry {
        level1: level1_points,
        level3: level3_points,
        burns,
        earth_radius_m: EARTH,
    }
}

/// Re-run a recorded flight. Returns the report, the terminal-state hash and
/// the figure geometry, all from the same `compute` the live flight used.
pub fn replay_flight(spec: &FlightSpec) -> Option<ReplayOutcome> {
    let (settings, nodes, guidance) = settings_from_spec(spec)?;
    let plan = Plan::compile_guided(
        settings.initial,
        MU,
        settings.duration_us,
        settings.dry_mass,
        settings.engine,
        &nodes,
        &guidance,
    )
    .ok()?;
    // A recorded prediction means the flight was locked; without one the
    // score fields are undefined in both the record and the replay.
    let locked = if spec.prediction[..5].iter().all(|value| value.is_finite()) {
        let prediction = PredictionBaseline {
            delta_v_m_s: spec.prediction[0],
            apoapsis_altitude_m: spec.prediction[1],
            periapsis_altitude_m: spec.prediction[2],
            remaining_propellant_kg: spec.prediction[3],
            time_of_flight_s: spec.prediction[4],
            target_miss_distance_m: if spec.prediction[5].is_finite() && spec.prediction[5] >= 0.0 {
                Some(spec.prediction[5])
            } else {
                None
            },
        };
        Some(LockedMission::new(plan.clone(), prediction, settings.dry_mass).ok()?)
    } else {
        None
    };
    let eph = ephemeris()?;
    let outcome = compute(&settings, &plan, locked.as_ref(), eph)?;
    let state = outcome.final_state;
    let hash = odd_report::fdr::final_state_hash(
        outcome.report[29],
        &[
            state.position[0],
            state.position[1],
            state.position[2],
            state.velocity[0],
            state.velocity[1],
            state.velocity[2],
        ],
        state.mass,
    );
    let commands: Vec<(i64, [f64; 3])> = spec
        .commands
        .iter()
        .map(|(epoch, tnw, _)| (*epoch, *tnw))
        .collect();
    let geometry = geometry_from(&settings.initial, &plan, &outcome, &commands);
    Some(ReplayOutcome {
        report: outcome.report,
        final_state_hash: hash,
        geometry,
        boil_off_kg: outcome.thermal.tank_lost_kg,
        cryocooler_energy_j: outcome.thermal.cooler_energy_j,
        vent_events: outcome
            .thermal
            .vents
            .iter()
            .map(|event| odd_report::VentEvent {
                time_us: event.time_us,
                tank: event.tank,
                pressure_pa: event.pressure_pa,
                temperature_k: event.temperature_k,
            })
            .collect(),
    })
}

/// The complete proof-of-trajectory check, without browser state.
///
/// Two independent gates: the Ed25519 signature over canonical bytes rebuilt
/// from the typed fields, then a Level-3 replay through the same `compute`
/// the live flight used. Every recorded metric and the terminal-state hash
/// must reproduce bit for bit.
pub fn verify_record(fdr: &Fdr) -> Result<ReplayOutcome, FdrError> {
    fdr.verify_signature()?;
    // Gate zero: the signed engine design must actually produce the recorded
    // performance. Only then does the replay run, and it runs on the derived
    // numbers rather than on anything the client typed.
    certify_engine(fdr)?;
    let outcome = replay_flight(&spec_from_record(fdr))
        .ok_or(FdrError::Malformed("the command timeline does not compile"))?;
    fdr.compare_metrics(&outcome.report)?;
    fdr.compare_final_state(&outcome.final_state_hash)?;
    // Gate three (v3): the signed vent history must be what the signed
    // thermal spec actually produces. A client cannot claim less boil-off
    // than its own hardware causes.
    fdr.compare_vent_events(&outcome.vent_events)?;
    fdr.compare_boil_off(outcome.boil_off_kg, outcome.cryocooler_energy_j)?;
    Ok(outcome)
}

/// Recompute the engine a record signs and compare it with the record's
/// derived claims. A legacy schema-1 record has no design to check, so it
/// verifies against its recorded thrust and Isp and is labelled legacy by
/// the dossier.
/// The contract fixes the framing conditions of a flight: where it starts,
/// when, how long it runs and which metric it scores. Without this check a
/// player could sign a record that begins already on the target orbit, coast,
/// and collect a perfect score — the signature would be valid and the replay
/// would reproduce it exactly, because the replay honours the recorded start.
///
/// Compared bit-exactly: these are canonical constants, not measurements.
pub fn certify_contract(fdr: &Fdr, contract: &Contract) -> Result<(), FdrError> {
    let initial = &contract.initial;
    let expected = [
        initial.position[0],
        initial.position[1],
        initial.position[2],
        initial.velocity[0],
        initial.velocity[1],
        initial.velocity[2],
    ];
    for (index, (recorded, canonical)) in fdr.initial_state.iter().zip(expected).enumerate() {
        if recorded.to_bits() != canonical.to_bits() {
            let _ = index;
            return Err(FdrError::ContractMismatch("initial state"));
        }
    }
    if fdr.epoch_s != odd_contracts::START_EPOCH {
        return Err(FdrError::ContractMismatch("epoch"));
    }
    if fdr.duration_us != contract.duration_us {
        return Err(FdrError::ContractMismatch("duration"));
    }
    let node_metric = contract.objective == Objective::SunSynchronous;
    if fdr.node_metric != node_metric {
        return Err(FdrError::ContractMismatch("objective metric"));
    }
    if fdr.initial_mass_kg.to_bits() != initial.mass.to_bits() && fdr.engine.is_none() {
        return Err(FdrError::ContractMismatch("initial mass"));
    }
    Ok(())
}

pub fn certify_engine(fdr: &Fdr) -> Result<Option<EngineSolution>, FdrError> {
    let Some(spec) = &fdr.engine else {
        // A legacy record carries no design, but the contract still fixes its
        // framing conditions and its baseline engine.
        if let Some(contract) = Contract::get(fdr.contract_id) {
            certify_contract(fdr, &contract)?;
            if fdr.thrust_n.to_bits() != contract.engine.thrust_newtons.to_bits() {
                return Err(FdrError::EngineMismatch("thrust"));
            }
            if fdr.specific_impulse_s.to_bits()
                != contract.engine.specific_impulse_seconds.to_bits()
            {
                return Err(FdrError::EngineMismatch("specific impulse"));
            }
            if fdr.dry_mass_kg.to_bits() != contract.dry_mass_kg.to_bits() {
                return Err(FdrError::EngineMismatch("dry mass"));
            }
        }
        return Ok(None);
    };
    let solved = solve_engine(spec, 0.0).map_err(|_| FdrError::EngineUnusable)?;
    if !solved.flight_ready() {
        return Err(FdrError::EngineUnusable);
    }
    if fdr.thrust_n.to_bits() != solved.thrust_n.to_bits() {
        return Err(FdrError::EngineMismatch("thrust"));
    }
    if fdr.specific_impulse_s.to_bits() != solved.specific_impulse_s.to_bits() {
        return Err(FdrError::EngineMismatch("specific impulse"));
    }
    if let Some(contract) = Contract::get(fdr.contract_id) {
        certify_contract(fdr, &contract)?;
        let vehicle = contract
            .vehicle(*spec)
            .map_err(|_| FdrError::EngineUnusable)?;
        if fdr.dry_mass_kg.to_bits() != vehicle.dry_mass_kg.to_bits() {
            return Err(FdrError::EngineMismatch("dry mass"));
        }
        // The stage may be launched part-loaded, never over-loaded.
        if fdr.initial_mass_kg < vehicle.dry_mass_kg
            || fdr.initial_mass_kg > vehicle.dry_mass_kg + vehicle.propellant_kg
        {
            return Err(FdrError::EngineMismatch("propellant load"));
        }
    }
    Ok(Some(solved))
}

/// The engine a replay must fly: derived from the signed design when there
/// is one, otherwise the legacy record's own claim.
pub fn replay_engine(fdr: &Fdr) -> (f64, f64) {
    match fdr
        .engine
        .as_ref()
        .and_then(|spec| solve_engine(spec, 0.0).ok())
    {
        Some(solved) => (solved.thrust_n, solved.specific_impulse_s),
        None => (fdr.thrust_n, fdr.specific_impulse_s),
    }
}

/// Translate a recorder into the simulator's replay request.
pub fn spec_from_record(fdr: &Fdr) -> FlightSpec {
    let (thrust_n, specific_impulse_s) = replay_engine(fdr);
    FlightSpec {
        thermal: thermal_inputs_from_record(fdr),
        delegation: odd_contracts::DelegationMask(fdr.delegation_mask),
        contract_id: fdr.contract_id,
        epoch_s: fdr.epoch_s,
        duration_us: fdr.duration_us,
        initial_mass_kg: fdr.initial_mass_kg,
        dry_mass_kg: fdr.dry_mass_kg,
        thrust_n,
        specific_impulse_s,
        initial_state: fdr.initial_state,
        node_metric: fdr.node_metric,
        prediction: fdr.prediction_baseline,
        commands: fdr
            .commands
            .iter()
            .map(|command| (command.time_us, command.delta_v_tnw, command.guidance))
            .collect(),
    }
}

/// Hash a terminal keyframe the way the recorder does.
pub fn hash_final_state(report: &[f64; REPORT_LEN], state: &Keyframe) -> [u8; 32] {
    odd_report::fdr::final_state_hash(
        report[29],
        &[
            state.position[0],
            state.position[1],
            state.position[2],
            state.velocity[0],
            state.velocity[1],
            state.velocity[2],
        ],
        state.mass,
    )
}

/// Rebuild the tank model's inputs from the **signed** thermal spec.
///
/// Nothing here is taken on trust: the spec is physical hardware, and the
/// boil-off it implies is recomputed by the same integrator the flight used.
/// A record with no thermal block (schema 1/2, or a storable stage) has no
/// tank model, so its replay produces no vent events.
pub fn thermal_inputs_from_record(fdr: &Fdr) -> Option<ThermalInputs> {
    if fdr.schema_version < odd_report::SCHEMA_VERSION {
        return None;
    }
    let spec = fdr.thermal_specs.first()?;
    let engine = fdr.engine?;
    let design = odd_contracts::ThermalDesign {
        mli_layers: spec.mli_layers,
        epsilon_eff: spec.epsilon_eff,
        shell: spec.tank_shell_material,
        absorptivity: spec.absorptivity,
        outer_epsilon: spec.outer_epsilon,
        strut_conductance_w_k: spec.strut_conductance_w_k,
        relief_pressure_pa: spec.relief_pressure_pa,
        radiator_area_m2: spec.radiator_area_m2,
        radiator_epsilon: spec.radiator_epsilon,
        cryocooler_lift_w: spec.cryocooler_w,
        cryocooler_carnot_fraction: spec.cryocooler_carnot_fraction,
        bus: odd_systems::BusSpec {
            array_w_at_1au: spec.array_w_at_1au,
            avionics_w: spec.avionics_w,
            electric_propulsion_w: spec.electric_propulsion_w,
        },
        structure_temperature_k: spec.structure_temperature_k,
        sink_temperature_k: spec.sink_temperature_k,
    };
    Some(ThermalInputs {
        engine_spec: engine,
        design,
        load_kg: fdr.initial_mass_kg - fdr.dry_mass_kg,
    })
}
