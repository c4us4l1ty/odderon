//! Scalar WASM boundary for the three playable contracts. Heavy propagation
//! runs in a one-shot worker; this module schedules no frames or timers.
use super::{Body, Ephemeris, Epoch, EPHEMERIS};
use odd_astro::{
    elements::Elements,
    finite_burn::isolate_finite_burn,
    integrator::IntegrationError,
    patched_conics::sphere_of_influence,
    shapley::attribute_shapley,
    spline::{Keyframe, TrajectorySpline},
};
use odd_contracts::{environment, Contract, Objective};
use odd_core::frames::State;
use odd_sim::{
    encounter::{find_entry, LunarPatch},
    mission::{apsides, metric_apoapsis, LockedMission, PredictionBaseline},
    planner::{ManeuverNode, Plan},
    targeting::next_radial_turn,
    Engine,
};
use std::sync::Mutex;

const MU: f64 = odd_contracts::EARTH_MU;
const EARTH: f64 = odd_contracts::EARTH_EQUATORIAL_RADIUS;
const MOON_MU: f64 = 4.902_800_118e12;
pub const REPORT_LEN: usize = 58;

#[derive(Clone)]
struct Settings {
    contract_id: u32,
    epoch: i64,
    initial: Keyframe,
    duration_us: i64,
    dry_mass: f64,
    engine: Engine,
    nodes: Vec<ManeuverNode>,
    guidance: Vec<Option<State>>,
    node_metric: bool,
}
#[derive(Clone)]
struct Execution {
    trajectory: TrajectorySpline,
    report: [f64; REPORT_LEN],
    final_state: Keyframe,
    burn_epochs: Vec<(f64, f64)>,
    burn_sweeps: Vec<f64>,
}
struct Outcome {
    trajectory: TrajectorySpline,
    report: [f64; REPORT_LEN],
    final_state: Keyframe,
    burns: Vec<odd_astro::perturb::burn::Burn>,
    /// Exact swept angle per burn, in command order. Their sum is report[57].
    burn_sweeps: Vec<f64>,
}
static SETTINGS: Mutex<Option<Settings>> = Mutex::new(None);
static PLAN: Mutex<Option<Plan>> = Mutex::new(None);
static LOCKED: Mutex<Option<LockedMission>> = Mutex::new(None);
static EXECUTION: Mutex<Option<Execution>> = Mutex::new(None);
static PATCH: Mutex<Option<LunarPatch>> = Mutex::new(None);
static TARGET: Mutex<Option<Keyframe>> = Mutex::new(None);
static CAUSAL: Mutex<Vec<odd_app::causal::CausalChain>> = Mutex::new(Vec::new());

fn microseconds(seconds: f64) -> Option<i64> {
    if !seconds.is_finite() || !(0.0..=864_000.0).contains(&seconds) {
        return None;
    }
    Some((seconds * 1e6).round() as i64)
}
fn clear_results() {
    *PLAN.lock().unwrap() = None;
    *LOCKED.lock().unwrap() = None;
    *EXECUTION.lock().unwrap() = None;
    *PATCH.lock().unwrap() = None;
    *TARGET.lock().unwrap() = None;
    CAUSAL.lock().unwrap().clear();
}
fn moon_state(eph: &Ephemeris, epoch: i64, t: f64) -> Result<State, odd_sim::planner::PlanError> {
    let at = Epoch::new(epoch, t).map_err(|_| odd_sim::planner::PlanError::NumericalRange)?;
    eph.state(Body::Moon, Body::Earth, at)
        .map(|v| v.state)
        .map_err(|_| odd_sim::planner::PlanError::NumericalRange)
}
fn lunar_soi() -> f64 {
    sphere_of_influence(384_400_000.0, MOON_MU, MU).unwrap_or(66_100_000.0)
}

/// Load a canonical ranked contract. All physics inputs come from odd-contracts.
#[no_mangle]
pub extern "C" fn mission_contract(id: u32) -> i32 {
    clear_results();
    *SETTINGS.lock().unwrap() = None;
    let Some(c) = Contract::get(id) else { return 0 };
    *SETTINGS.lock().unwrap() = Some(Settings {
        contract_id: id,
        epoch: odd_contracts::START_EPOCH,
        initial: c.initial,
        duration_us: c.duration_us,
        dry_mass: c.dry_mass_kg,
        engine: c.engine,
        nodes: Vec::new(),
        guidance: Vec::new(),
        node_metric: c.objective == Objective::SunSynchronous,
    });
    1
}

/// Read canonical setup/display parameters after mission_contract.
/// 0 epoch, 1 duration_s, 2 mass, 3 dry, 4 thrust, 5 isp, 6..11 initial state,
/// 12 challenge rating, 13 reference delta-v, 14 objective id.
#[no_mangle]
pub extern "C" fn mission_parameter(index: u32) -> f64 {
    let settings = SETTINGS.lock().unwrap();
    let Some(s) = settings.as_ref() else {
        return f64::NAN;
    };
    let Some(c) = Contract::get(s.contract_id) else {
        return f64::NAN;
    };
    match index {
        0 => s.epoch as f64,
        1 => s.duration_us as f64 / 1e6,
        2 => s.initial.mass,
        3 => s.dry_mass,
        4 => s.engine.thrust_newtons,
        5 => s.engine.specific_impulse_seconds,
        6..=8 => s.initial.position[(index - 6) as usize],
        9..=11 => s.initial.velocity[(index - 9) as usize],
        12 => c.rating,
        13 => c.reference_delta_v_m_s,
        14 => match c.objective {
            Objective::CircularRescue => 101.0,
            Objective::SunSynchronous => 102.0,
            Objective::TransLunarApoapsis => 103.0,
        },
        _ => f64::NAN,
    }
}

/// Custom setup retained for native parity tests. Earth-centered J2000, SI.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn mission_setup(
    epoch: f64,
    duration: f64,
    mass: f64,
    dry: f64,
    thrust: f64,
    isp: f64,
    x: f64,
    y: f64,
    z: f64,
    vx: f64,
    vy: f64,
    vz: f64,
) -> i32 {
    clear_results();
    let mut settings = SETTINGS.lock().unwrap();
    *settings = None;
    let Some(duration_us) = microseconds(duration) else {
        return 0;
    };
    if !epoch.is_finite()
        || epoch.fract() != 0.0
        || !(631_108_800.0..1_136_030_400.0).contains(&epoch)
        || duration_us <= 0
        || !mass.is_finite()
        || !dry.is_finite()
        || dry <= 0.0
        || mass < dry
        || !thrust.is_finite()
        || thrust <= 0.0
        || !isp.is_finite()
        || isp <= 0.0
        || ![x, y, z, vx, vy, vz].iter().all(|x| x.is_finite())
    {
        return 0;
    }
    *settings = Some(Settings {
        contract_id: 0,
        epoch: epoch as i64,
        initial: Keyframe {
            time: 0.0,
            position: [x, y, z],
            velocity: [vx, vy, vz],
            mass,
        },
        duration_us,
        dry_mass: dry,
        engine: Engine {
            thrust_newtons: thrust,
            specific_impulse_seconds: isp,
            stage: 0,
            engine: 0,
        },
        nodes: Vec::new(),
        guidance: Vec::new(),
        node_metric: false,
    });
    1
}

#[no_mangle]
pub extern "C" fn mission_metric(metric: u32) -> i32 {
    if metric > 1 {
        return 0;
    }
    let mut settings = SETTINGS.lock().unwrap();
    let Some(settings) = settings.as_mut() else {
        return 0;
    };
    settings.node_metric = metric == 1;
    *EXECUTION.lock().unwrap() = None;
    *LOCKED.lock().unwrap() = None;
    1
}

#[no_mangle]
pub extern "C" fn mission_add_node(seconds: f64, t: f64, n: f64, w: f64) -> i32 {
    clear_results();
    let Some(epoch_us) = microseconds(seconds) else {
        return 0;
    };
    if ![t, n, w].iter().all(|x| x.is_finite()) {
        return 0;
    }
    let mut settings = SETTINGS.lock().unwrap();
    let Some(settings) = settings.as_mut() else {
        return 0;
    };
    if settings.nodes.len() >= 12 {
        return 0;
    }
    settings.nodes.push(ManeuverNode {
        epoch_us,
        tnw_m_s: [t, n, w],
    });
    settings.guidance.push(None);
    1
}

/// Attach a numerical pre-burn reference state to a node. The TNW command is
/// frozen in inertial axes from this state, allowing actual-periapsis targeting.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn mission_add_guidance(
    index: u32,
    x: f64,
    y: f64,
    z: f64,
    vx: f64,
    vy: f64,
    vz: f64,
) -> i32 {
    clear_results();
    if ![x, y, z, vx, vy, vz].iter().all(|v| v.is_finite()) {
        return 0;
    }
    let mut settings = SETTINGS.lock().unwrap();
    let Some(settings) = settings.as_mut() else {
        return 0;
    };
    let Some(slot) = settings.guidance.get_mut(index as usize) else {
        return 0;
    };
    *slot = Some(State {
        position_m: [x, y, z],
        velocity_m_s: [vx, vy, vz],
    });
    1
}

#[no_mangle]
pub extern "C" fn mission_compile() -> i32 {
    clear_results();
    let Some(s) = SETTINGS.lock().unwrap().clone() else {
        return 0;
    };
    let Ok(result) = Plan::compile_guided(
        s.initial,
        MU,
        s.duration_us,
        s.dry_mass,
        s.engine,
        &s.nodes,
        &s.guidance,
    ) else {
        return 0;
    };
    if s.contract_id == 103 {
        if let Some(eph) = EPHEMERIS.get_or_init(|| Ephemeris::bundled().ok()).as_ref() {
            if let Ok(patch) = LunarPatch::find(&result, MOON_MU, lunar_soi(), 300_000_000, |t| {
                moon_state(eph, s.epoch, t)
            }) {
                *PATCH.lock().unwrap() = patch;
            }
        }
    }
    *PLAN.lock().unwrap() = Some(result);
    1
}

#[no_mangle]
pub extern "C" fn mission_preview(seconds: f64, component: u32) -> f64 {
    let Some(time) = microseconds(seconds) else {
        return f64::NAN;
    };
    let plan = PLAN.lock().unwrap();
    let Some(state) = plan.as_ref().and_then(|p| p.sample(time).ok()) else {
        return f64::NAN;
    };
    component_value(state, component)
}

/// Level-1 patched lunar preview, translated back to Earth-centered axes.
#[no_mangle]
pub extern "C" fn mission_lunar_preview(seconds: f64, component: u32) -> f64 {
    let Some(time) = microseconds(seconds) else {
        return f64::NAN;
    };
    let Some(s) = SETTINGS.lock().unwrap().clone() else {
        return f64::NAN;
    };
    let plan = PLAN.lock().unwrap();
    let Some(plan) = plan.as_ref() else {
        return f64::NAN;
    };
    let patch = PATCH.lock().unwrap();
    let state = if let Some(patch) = patch.as_ref() {
        let Some(eph) = EPHEMERIS.get_or_init(|| Ephemeris::bundled().ok()).as_ref() else {
            return f64::NAN;
        };
        patch
            .sample(plan, time, |t| moon_state(eph, s.epoch, t))
            .ok()
    } else {
        plan.sample(time).ok()
    };
    state.map_or(f64::NAN, |s| component_value(s, component))
}

/// 0 valid, 1 entry time s, 2 SOI radius m, 3 entry vehicle-Moon center distance m.
#[no_mangle]
pub extern "C" fn mission_lunar_info(index: u32) -> f64 {
    let patch = PATCH.lock().unwrap();
    let Some(p) = patch.as_ref() else {
        return if index == 0 { 0.0 } else { f64::NAN };
    };
    match index {
        0 => 1.0,
        1 => p.entry.time_s,
        2 => p.entry.soi_radius_m,
        3 => {
            let d = std::array::from_fn::<_, 3, _>(|i| {
                p.entry.vehicle_primary.position_m[i] - p.entry.secondary.position_m[i]
            });
            (d.iter().map(|x| x * x).sum::<f64>()).sqrt()
        }
        _ => f64::NAN,
    }
}

/// Lock the compiled plan and player prediction before execution. SI units.
#[no_mangle]
pub extern "C" fn mission_lock(
    dv: f64,
    apo_alt: f64,
    peri_alt: f64,
    fuel: f64,
    tof: f64,
    miss: f64,
) -> i32 {
    *EXECUTION.lock().unwrap() = None;
    let Some(plan) = PLAN.lock().unwrap().clone() else {
        return 0;
    };
    let Some(s) = SETTINGS.lock().unwrap().clone() else {
        return 0;
    };
    let prediction = PredictionBaseline {
        delta_v_m_s: dv,
        apoapsis_altitude_m: apo_alt,
        periapsis_altitude_m: peri_alt,
        remaining_propellant_kg: fuel,
        time_of_flight_s: tof,
        target_miss_distance_m: if miss.is_finite() && miss >= 0.0 {
            Some(miss)
        } else {
            None
        },
    };
    match LockedMission::new(plan, prediction, s.dry_mass) {
        Ok(lock) => {
            *LOCKED.lock().unwrap() = Some(lock);
            1
        }
        Err(_) => 0,
    }
}

fn component_value(state: Keyframe, component: u32) -> f64 {
    match component {
        0..=2 => state.position[component as usize],
        3..=5 => state.velocity[(component - 3) as usize],
        6 => state.mass,
        _ => f64::NAN,
    }
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

/// Execute the locked mission, or an unlocked background preview. Returns 1
/// even for a handled impact so the debrief can explain the terminal event.
/// Report layout is documented in docs/mission-planning-feasibility.md.
#[no_mangle]
pub extern "C" fn mission_execute() -> i32 {
    *EXECUTION.lock().unwrap() = None;
    let Some(s) = SETTINGS.lock().unwrap().clone() else {
        return 0;
    };
    let locked = LOCKED.lock().unwrap().clone();
    let Some(plan) = locked
        .as_ref()
        .map(|l| l.plan().clone())
        .or_else(|| PLAN.lock().unwrap().clone())
    else {
        return 0;
    };
    let Some(eph) = EPHEMERIS.get_or_init(|| Ephemeris::bundled().ok()).as_ref() else {
        return 0;
    };
    let Some(outcome) = compute(&s, &plan, locked.as_ref(), eph) else {
        return 0;
    };
    *EXECUTION.lock().unwrap() = Some(Execution {
        trajectory: outcome.trajectory,
        report: outcome.report,
        final_state: outcome.final_state,
        burn_epochs: outcome.burns.iter().map(|b| (b.start, b.end)).collect(),
        burn_sweeps: outcome.burn_sweeps,
    });
    1
}

/// The whole flight evaluation, free of module state.
///
/// Replay verification calls this same function with settings rebuilt from a
/// signed recorder, so a verified flight is reproduced by the identical code
/// path rather than a second implementation that could agree by accident.
fn compute(
    s: &Settings,
    plan: &Plan,
    locked: Option<&LockedMission>,
    eph: &Ephemeris,
) -> Option<Outcome> {
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
        if el.inclination_rad.sin().abs() < 1e-8 {
            return Err(IntegrationError::ForceFailure);
        }
        Ok(
            (el.ascending_node_rad - initial.ascending_node_rad + std::f64::consts::PI)
                .rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI,
        )
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
        odd_contracts::score(
            &contract,
            final_state,
            evaluation_s,
            lock.prediction(),
            plan.requested_delta_v_m_s,
            impacted,
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
    Some(Outcome {
        trajectory: integrated.trajectory,
        report,
        final_state,
        burns,
        burn_sweeps,
    })
}

/// Number of registered acronyms. The browser builds its hover registry from
/// this same odd-app table, so a new term cannot drift into an unexplained UI.
#[no_mangle]
pub extern "C" fn mission_acronym_count() -> u32 {
    odd_app::lens::ACRONYMS.len() as u32
}
fn acronym_field(term: u32, field: u32) -> Option<&'static str> {
    let a = odd_app::lens::ACRONYMS.get(term as usize)?;
    Some(match field {
        0 => a.term,
        1 => a.expansion,
        2 => a.plain,
        _ => return None,
    })
}
/// UTF-8 byte length of a registry field (0 term, 1 expansion, 2 plain text).
#[no_mangle]
pub extern "C" fn mission_acronym_len(term: u32, field: u32) -> u32 {
    acronym_field(term, field).map_or(0, |s| s.len() as u32)
}
/// One UTF-8 byte of a registry field; NaN when out of range.
#[no_mangle]
pub extern "C" fn mission_acronym_byte(term: u32, field: u32, index: u32) -> f64 {
    acronym_field(term, field).map_or(f64::NAN, |s| {
        s.as_bytes()
            .get(index as usize)
            .map_or(f64::NAN, |b| *b as f64)
    })
}
/// UTF-8 length of the browser art key for a registry term's projection.
#[no_mangle]
pub extern "C" fn mission_acronym_key_len(term: u32) -> u32 {
    odd_app::lens::ACRONYMS
        .get(term as usize)
        .map_or(0, |a| a.projection.key().len() as u32)
}
/// One UTF-8 byte of that art key; NaN when out of range.
#[no_mangle]
pub extern "C" fn mission_acronym_key_byte(term: u32, index: u32) -> f64 {
    odd_app::lens::ACRONYMS
        .get(term as usize)
        .and_then(|a| a.projection.key().as_bytes().get(index as usize))
        .map_or(f64::NAN, |b| *b as f64)
}
/// odd_app::lens::Projection discriminant for a registry term.
#[no_mangle]
pub extern "C" fn mission_acronym_projection(term: u32) -> u32 {
    odd_app::lens::ACRONYMS
        .get(term as usize)
        .map_or(u32::MAX, |a| a.projection as u32)
}

/// Run the deterministic odd-app causal DAG engine. Inputs are SI altitudes,
/// fractions, degrees and 0/1 flags. Returns the number of triggered chains.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn mission_diagnose(
    thrust_to_weight: f64,
    periapsis_altitude_m: f64,
    atmosphere_altitude_m: f64,
    gravity_loss_fraction: f64,
    propellant_deficit: f64,
    apoapsis_shortfall_fraction: f64,
    impacted: f64,
    burn_sweep_deg: f64,
) -> u32 {
    let inputs = odd_app::causal::Inputs {
        thrust_to_weight,
        periapsis_altitude_m,
        atmosphere_altitude_m,
        gravity_loss_fraction,
        propellant_deficit: propellant_deficit != 0.0,
        apoapsis_shortfall_fraction,
        impacted: impacted != 0.0,
        burn_sweep_deg,
    };
    let chains = odd_app::causal::diagnose(&inputs);
    let len = chains.len() as u32;
    *CAUSAL.lock().unwrap() = chains;
    len
}
#[no_mangle]
pub extern "C" fn mission_causal_count() -> u32 {
    CAUSAL.lock().unwrap().len() as u32
}
/// Warning discriminant in odd_app::causal::Warning declaration order.
#[no_mangle]
pub extern "C" fn mission_causal_warning(chain: u32) -> u32 {
    CAUSAL
        .lock()
        .unwrap()
        .get(chain as usize)
        .map_or(u32::MAX, |c| c.warning as u32)
}
#[no_mangle]
pub extern "C" fn mission_causal_node_count(chain: u32) -> u32 {
    CAUSAL
        .lock()
        .unwrap()
        .get(chain as usize)
        .map_or(0, |c| c.nodes.len() as u32)
}
#[no_mangle]
pub extern "C" fn mission_causal_node_kind(chain: u32, node: u32) -> u32 {
    CAUSAL
        .lock()
        .unwrap()
        .get(chain as usize)
        .and_then(|c| c.nodes.get(node as usize))
        .map_or(u32::MAX, |n| n.kind as u32)
}
fn causal_node_text(chain: u32, node: u32) -> Option<String> {
    let guard = CAUSAL.lock().unwrap();
    let c = guard.get(chain as usize)?;
    Some(c.nodes.get(node as usize)?.text.to_string())
}
#[no_mangle]
pub extern "C" fn mission_causal_node_len(chain: u32, node: u32) -> u32 {
    causal_node_text(chain, node).map_or(0, |s| s.len() as u32)
}
#[no_mangle]
pub extern "C" fn mission_causal_node_byte(chain: u32, node: u32, index: u32) -> f64 {
    causal_node_text(chain, node).map_or(f64::NAN, |s| {
        s.as_bytes()
            .get(index as usize)
            .map_or(f64::NAN, |b| *b as f64)
    })
}
fn causal_demo(chain: u32) -> Option<String> {
    let guard = CAUSAL.lock().unwrap();
    Some(guard.get(chain as usize)?.demo.to_string())
}
#[no_mangle]
pub extern "C" fn mission_causal_demo_len(chain: u32) -> u32 {
    causal_demo(chain).map_or(0, |s| s.len() as u32)
}
#[no_mangle]
pub extern "C" fn mission_causal_demo_byte(chain: u32, index: u32) -> f64 {
    causal_demo(chain).map_or(f64::NAN, |s| {
        s.as_bytes()
            .get(index as usize)
            .map_or(f64::NAN, |b| *b as f64)
    })
}
/// Kahn acyclicity check over the currently stored chains.
#[no_mangle]
pub extern "C" fn mission_causal_valid() -> i32 {
    odd_app::causal::is_valid_dag(&CAUSAL.lock().unwrap()) as i32
}

/// Live Visual Physics Lens quantities for a node's pre-burn state.
/// kind 0 tangential: 0 KE change J, 1/2 apo before/after m, 3/4 peri before/after m.
/// kind 1 normal: 0..2 torque N·m, 3 plane tilt rad.
/// kind 2 radial: 0..2 eccentricity vector before, 3..5 after, 6 rotation rad.
#[no_mangle]
pub extern "C" fn mission_lens(node: u32, kind: u32, index: u32) -> f64 {
    let settings = SETTINGS.lock().unwrap();
    let Some(s) = settings.as_ref() else {
        return f64::NAN;
    };
    let Some(n) = s.nodes.get(node as usize) else {
        return f64::NAN;
    };
    let plan = PLAN.lock().unwrap();
    let Some(plan) = plan.as_ref() else {
        return f64::NAN;
    };
    let Ok(pre) = plan.sample(n.epoch_us.saturating_sub(1)) else {
        return f64::NAN;
    };
    let state = State {
        position_m: pre.position,
        velocity_m_s: pre.velocity,
    };
    match kind {
        0 => odd_app::lens::tangential(state, MU, pre.mass, n.tnw_m_s[0]).map_or(f64::NAN, |p| {
            match index {
                0 => p.kinetic_energy_change_j,
                1 => p.apoapsis_before_m,
                2 => p.apoapsis_after_m,
                3 => p.periapsis_before_m,
                4 => p.periapsis_after_m,
                _ => f64::NAN,
            }
        }),
        1 => odd_app::lens::normal(state, s.engine.thrust_newtons, n.tnw_m_s[2]).map_or(
            f64::NAN,
            |p| match index {
                0..=2 => p.torque_n_m[index as usize],
                3 => p.plane_tilt_rad,
                _ => f64::NAN,
            },
        ),
        2 => odd_app::lens::radial(state, MU, n.tnw_m_s[1]).map_or(f64::NAN, |p| match index {
            0..=2 => p.eccentricity_vector_before[index as usize],
            3..=5 => p.eccentricity_vector_after[(index - 3) as usize],
            6 => p.rotation_rad,
            _ => f64::NAN,
        }),
        _ => f64::NAN,
    }
}

/// Local practice Astro-Elo update. index 0 player, 1 mission.
#[no_mangle]
pub extern "C" fn mission_elo(
    player: f64,
    mission: f64,
    score: f64,
    k_player: f64,
    index: u32,
) -> f64 {
    if ![player, mission, score, k_player]
        .iter()
        .all(|x| x.is_finite())
        || !(0.0..=1.0).contains(&score)
        || k_player < 0.0
    {
        return f64::NAN;
    }
    let (p, m) = odd_app::debrief::elo_update(player, mission, score, k_player, 8.0);
    match index {
        0 => p,
        1 => m,
        _ => f64::NAN,
    }
}

#[no_mangle]
pub extern "C" fn mission_actual(seconds: f64, component: u32) -> f64 {
    let execution = EXECUTION.lock().unwrap();
    let Some(state) = execution
        .as_ref()
        .and_then(|e| e.trajectory.sample(seconds).ok())
    else {
        return f64::NAN;
    };
    component_value(state, component)
}
#[no_mangle]
pub extern "C" fn mission_report(index: u32) -> f64 {
    EXECUTION
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|e| e.report.get(index as usize))
        .copied()
        .unwrap_or(f64::NAN)
}
#[no_mangle]
pub extern "C" fn mission_report_len() -> u32 {
    REPORT_LEN as u32
}

/// Replace the current plan with an equal-delta-v perigee-kick sequence.
/// Contract 103 only; `kicks` is 2 or 3. The worker calls this off the UI thread.
#[no_mangle]
pub extern "C" fn mission_split_tug(kicks: u32, total_dv: f64, first_node_s: f64) -> i32 {
    clear_results();
    let Some(s) = SETTINGS.lock().unwrap().clone() else {
        return 0;
    };
    if s.contract_id != 103 {
        return 0;
    }
    let Some(eph) = EPHEMERIS.get_or_init(|| Ephemeris::bundled().ok()).as_ref() else {
        return 0;
    };
    let Ok(split) = odd_contracts::split_tug(kicks as usize, total_dv, first_node_s, eph) else {
        return 0;
    };
    {
        let mut settings = SETTINGS.lock().unwrap();
        let Some(settings) = settings.as_mut() else {
            return 0;
        };
        settings.nodes = split.nodes;
        settings.guidance = split.guidance;
    }
    if let Ok(patch) = LunarPatch::find(&split.plan, MOON_MU, lunar_soi(), 300_000_000, |t| {
        moon_state(eph, s.epoch, t)
    }) {
        *PATCH.lock().unwrap() = patch;
    }
    *PLAN.lock().unwrap() = Some(split.plan);
    1
}

/// Number of nodes in the current settings (after mission_split_tug).
#[no_mangle]
pub extern "C" fn mission_node_count() -> u32 {
    SETTINGS
        .lock()
        .unwrap()
        .as_ref()
        .map_or(0, |s| s.nodes.len() as u32)
}
/// Node epoch seconds by index.
#[no_mangle]
pub extern "C" fn mission_node_time(index: u32) -> f64 {
    SETTINGS
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|s| s.nodes.get(index as usize))
        .map_or(f64::NAN, |n| n.epoch_us as f64 / 1e6)
}
/// Node TNW component by index (0..2).
#[no_mangle]
pub extern "C" fn mission_node_dv(index: u32, component: u32) -> f64 {
    SETTINGS
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|s| s.nodes.get(index as usize))
        .and_then(|n| n.tnw_m_s.get(component as usize))
        .copied()
        .unwrap_or(f64::NAN)
}
/// Whether a node has numerical guidance (1) or uses the ideal frame (0).
#[no_mangle]
pub extern "C" fn mission_node_guided(index: u32) -> f64 {
    SETTINGS
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|s| s.guidance.get(index as usize))
        .map_or(f64::NAN, |g| if g.is_some() { 1.0 } else { 0.0 })
}
/// Guidance state component (0..5) for a node, if present.
#[no_mangle]
pub extern "C" fn mission_node_guidance(index: u32, component: u32) -> f64 {
    let settings = SETTINGS.lock().unwrap();
    let Some(s) = settings.as_ref() else {
        return f64::NAN;
    };
    let Some(Some(g)) = s.guidance.get(index as usize) else {
        return f64::NAN;
    };
    match component {
        0..=2 => g.position_m[component as usize],
        3..=5 => g.velocity_m_s[(component - 3) as usize],
        _ => f64::NAN,
    }
}

/// Target the next full-force periapsis after `after_s` using the currently
/// compiled prefix burns. Stores the state for mission_target_* accessors.
#[no_mangle]
pub extern "C" fn mission_next_perigee(after_s: f64) -> i32 {
    *TARGET.lock().unwrap() = None;
    let Some(s) = SETTINGS.lock().unwrap().clone() else {
        return 0;
    };
    let Some(plan) = PLAN.lock().unwrap().clone() else {
        return 0;
    };
    let Some(eph) = EPHEMERIS.get_or_init(|| Ephemeris::bundled().ok()).as_ref() else {
        return 0;
    };
    let Ok(burns) = plan.timeline.burns(&[s.engine]) else {
        return 0;
    };
    let contract = Contract::get(s.contract_id).unwrap_or_else(|| Contract::get(101).unwrap());
    let end = (after_s + 200_000.0).min(s.duration_us as f64 / 1e6);
    match next_radial_turn(
        s.initial,
        after_s,
        end,
        true,
        &contract.force_model(),
        &burns,
        contract.limits(),
        contract.options(),
        |t| environment(eph, s.epoch, t),
    ) {
        Ok(state) => {
            *TARGET.lock().unwrap() = Some(state);
            1
        }
        Err(_) => 0,
    }
}
#[no_mangle]
pub extern "C" fn mission_target_time() -> f64 {
    TARGET.lock().unwrap().map_or(f64::NAN, |s| s.time)
}
#[no_mangle]
pub extern "C" fn mission_target_state(component: u32) -> f64 {
    TARGET
        .lock()
        .unwrap()
        .map_or(f64::NAN, |s| component_value(s, component))
}

// ---------------------------------------------------------------------------
// Signed flight recorder support.
//
// These helpers expose the executed flight to fdr_bridge without letting the
// recorder path touch module state: a replay builds its own Settings and calls
// the same `compute` the live flight used.
// ---------------------------------------------------------------------------

/// Everything needed to re-run a flight, taken from a signed recorder.
pub(crate) struct FlightSpec {
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
}

pub(crate) struct CapturedCommand {
    pub epoch_us: i64,
    pub tnw_m_s: [f64; 3],
    pub guidance: Option<[f64; 6]>,
}

pub(crate) struct CapturedFlight {
    pub contract_id: u32,
    pub epoch_s: i64,
    pub duration_us: i64,
    pub initial: Keyframe,
    pub dry_mass: f64,
    pub thrust_n: f64,
    pub specific_impulse_s: f64,
    pub node_metric: bool,
    pub prediction: [f64; 6],
    pub commands: Vec<CapturedCommand>,
    pub report: [f64; REPORT_LEN],
    pub final_state: Keyframe,
}

pub(crate) struct ReplayOutcome {
    pub report: [f64; REPORT_LEN],
    pub final_state_hash: [u8; 32],
    pub geometry: odd_report::dossier::Geometry,
}

/// Snapshot the executed flight for signing. Requires a completed execution.
pub(crate) fn capture_flight() -> Option<CapturedFlight> {
    let settings = SETTINGS.lock().unwrap().clone()?;
    let execution = EXECUTION.lock().unwrap().clone()?;
    let locked = LOCKED.lock().unwrap().clone();
    let prediction = match locked.as_ref() {
        Some(lock) => {
            let p = lock.prediction();
            [
                p.delta_v_m_s,
                p.apoapsis_altitude_m,
                p.periapsis_altitude_m,
                p.remaining_propellant_kg,
                p.time_of_flight_s,
                p.target_miss_distance_m.unwrap_or(f64::NAN),
            ]
        }
        None => [f64::NAN; 6],
    };
    Some(CapturedFlight {
        contract_id: settings.contract_id,
        epoch_s: settings.epoch,
        duration_us: settings.duration_us,
        initial: settings.initial,
        dry_mass: settings.dry_mass,
        thrust_n: settings.engine.thrust_newtons,
        specific_impulse_s: settings.engine.specific_impulse_seconds,
        node_metric: settings.node_metric,
        prediction,
        commands: settings
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| CapturedCommand {
                epoch_us: node.epoch_us,
                tnw_m_s: node.tnw_m_s,
                guidance: settings
                    .guidance
                    .get(index)
                    .copied()
                    .flatten()
                    .map(|state| {
                        [
                            state.position_m[0],
                            state.position_m[1],
                            state.position_m[2],
                            state.velocity_m_s[0],
                            state.velocity_m_s[1],
                            state.velocity_m_s[2],
                        ]
                    }),
            })
            .collect(),
        report: execution.report,
        final_state: execution.final_state,
    })
}

fn settings_from_spec(
    spec: &FlightSpec,
) -> Option<(Settings, Vec<ManeuverNode>, Vec<Option<State>>)> {
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
        Settings {
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
            nodes: nodes.clone(),
            guidance: guidance.clone(),
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

fn geometry_from(
    initial: &Keyframe,
    level1: &Plan,
    outcome: &Outcome,
    commands: &[(i64, [f64; 3])],
) -> odd_report::dossier::Geometry {
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
    odd_report::dossier::Geometry {
        level1: level1_points,
        level3: level3_points,
        burns,
        earth_radius_m: EARTH,
    }
}

/// Re-run a recorded flight. Returns the report, the terminal-state hash and
/// the figure geometry, all from the same `compute` the live flight used.
pub(crate) fn replay_flight(spec: &FlightSpec) -> Option<ReplayOutcome> {
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
    let eph = EPHEMERIS
        .get_or_init(|| Ephemeris::bundled().ok())
        .as_ref()?;
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
    })
}

/// Figure geometry for the flight currently on screen.
pub(crate) fn dossier_geometry() -> Option<odd_report::dossier::Geometry> {
    let settings = SETTINGS.lock().unwrap().clone()?;
    let execution = EXECUTION.lock().unwrap().clone()?;
    let locked = LOCKED.lock().unwrap().clone();
    let plan = locked
        .as_ref()
        .map(|lock| lock.plan().clone())
        .or_else(|| PLAN.lock().unwrap().clone())?;
    let outcome = Outcome {
        trajectory: execution.trajectory,
        report: execution.report,
        final_state: execution.final_state,
        burns: execution
            .burn_epochs
            .iter()
            .map(|(start, end)| odd_astro::perturb::burn::Burn {
                start: *start,
                end: *end,
                force_newtons: [0.0; 3],
                specific_impulse_seconds: settings.engine.specific_impulse_seconds,
            })
            .collect(),
        burn_sweeps: execution.burn_sweeps,
    };
    let commands: Vec<(i64, [f64; 3])> = settings
        .nodes
        .iter()
        .map(|node| (node.epoch_us, node.tnw_m_s))
        .collect();
    Some(geometry_from(&settings.initial, &plan, &outcome, &commands))
}

/// The recorder tests run real flights against the same process-wide statics,
/// so they must share this lock with the mission tests.
#[cfg(test)]
pub(crate) fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    tests::TEST_LOCK.lock().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    // The scalar bridge owns process-wide statics; native tests must not
    // interleave setup/compile/lock calls the way separate workers would.
    pub(super) static TEST_LOCK: Mutex<()> = Mutex::new(());
    #[test]
    fn preview_and_failed_setup_clear_stale_state() {
        let _guard = TEST_LOCK.lock().unwrap();
        let radius = EARTH + 260_000.0;
        let speed = (MU / radius).sqrt();
        assert_eq!(
            mission_setup(
                820_497_600.0,
                10_800.0,
                1200.0,
                1020.0,
                2400.0,
                310.0,
                radius,
                0.0,
                0.0,
                0.0,
                speed,
                0.0
            ),
            1
        );
        assert_eq!(mission_add_node(200.0, 82.0, 0.0, 0.0), 1);
        assert_eq!(mission_compile(), 1);
        assert!(mission_preview(1000.0, 0).is_finite());
        assert_eq!(
            mission_setup(
                f64::NAN,
                10_800.0,
                1200.0,
                1020.0,
                2400.0,
                310.0,
                radius,
                0.0,
                0.0,
                0.0,
                speed,
                0.0
            ),
            0
        );
        assert!(mission_preview(1000.0, 0).is_nan());
    }
    #[test]
    fn contract_103_loss_and_shapley_report() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(mission_contract(103), 1);
        assert_eq!(mission_split_tug(3, 3240.0, 1600.0), 1);
        assert_eq!(mission_execute(), 1);
        assert_eq!(mission_report_len(), REPORT_LEN as u32);
        assert_eq!(mission_report(30), 0.0);
        assert_eq!(mission_report(19), 0.0);
        let shares: f64 = (6..10).map(|i| mission_report(i)).sum();
        assert!((shares - 1.0).abs() < 1e-12);
        assert!(mission_report(6) > 0.70);
        assert!(mission_report(17).abs() < 1e-6);
        let residual = mission_report(24);
        println!("Contract 103 bridge finite-burn identity residual: {residual:.12} m/s");
        assert!(residual.abs() < 1e-4, "identity residual {residual}");
        assert!(mission_report(57) > 0.0);
    }
    #[test]
    fn acronym_registry_and_causal_dag_bridge() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert!(mission_acronym_count() >= 13);
        let mut found_twr = false;
        for i in 0..mission_acronym_count() {
            let len = mission_acronym_len(i, 0);
            let term: String = (0..len)
                .map(|j| mission_acronym_byte(i, 0, j) as u8 as char)
                .collect();
            // The browser art key must round-trip for every term, so a new
            // Projection variant cannot render the wrong demonstration.
            let key_len = mission_acronym_key_len(i);
            assert!(key_len > 0, "{term} has no art key");
            let key: String = (0..key_len)
                .map(|j| mission_acronym_key_byte(i, j) as u8 as char)
                .collect();
            assert_eq!(
                key,
                odd_app::lens::find(&term).unwrap().projection.key(),
                "{term} art key did not round-trip"
            );
            assert!(mission_acronym_key_byte(i, key_len).is_nan());
            if term == "TWR" {
                found_twr = true;
                assert!(mission_acronym_len(i, 1) > 0);
                assert!(mission_acronym_len(i, 2) > 0);
                assert_eq!(key, "twr");
            }
        }
        assert!(found_twr);
        // Out-of-range terms stay inert rather than reading adjacent memory.
        let past = mission_acronym_count();
        assert_eq!(mission_acronym_key_len(past), 0);
        assert!(mission_acronym_key_byte(past, 0).is_nan());
        let count = mission_diagnose(0.5, 50_000.0, 120_000.0, 0.30, 1.0, 0.10, 1.0, 40.0);
        assert_eq!(count, 7);
        assert_eq!(mission_causal_valid(), 1);
        assert_eq!(mission_causal_node_count(0), 4);
        assert_eq!(mission_causal_node_kind(0, 3), 3);
        assert!(mission_causal_demo_len(0) > 0);
    }
    #[test]
    fn canonical_contract_and_lock() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(mission_contract(103), 1);
        assert_eq!(mission_parameter(2), 18_000.0);
        assert_eq!(mission_add_node(1600.0, 1080.0, 0.0, 0.0), 1);
        assert_eq!(mission_compile(), 1);
        assert_eq!(
            mission_lock(
                1080.0,
                10_000_000.0,
                200_000.0,
                12_000.0,
                600_000.0,
                f64::NAN
            ),
            1
        );
    }
}
