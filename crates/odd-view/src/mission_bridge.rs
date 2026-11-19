//! Scalar WASM boundary for the three playable contracts. Heavy propagation
//! runs in a one-shot worker; this module schedules no frames or timers.
use super::{Ephemeris, EPHEMERIS};
use odd_astro::{
    bplane::target_bplane,
    patched_conics::sphere_of_influence,
    spline::{Keyframe, TrajectorySpline},
};
use odd_contracts::{environment, Contract, Objective};
use odd_core::frames::State;
use odd_sim::{
    encounter::LunarPatch,
    mission::{LockedMission, PredictionBaseline},
    planner::{ManeuverNode, Plan, PlanError},
    targeting::next_radial_turn,
    Engine,
};
// The flight evaluation and the replay live in a portable crate so a
// headless verifier runs the identical code path. This module only owns the
// browser's mutable session state and the scalar boundary.
use odd_verify::{moon_state, FlightOutcome as Outcome, FlightSettings, MOON_MU, REPORT_LEN};
use std::sync::Mutex;

const MU: f64 = odd_contracts::EARTH_MU;

#[derive(Clone)]
struct Settings {
    contract_id: u32,
    engine_spec: odd_propulsion::EngineSpec,
    epoch: i64,
    initial: Keyframe,
    duration_us: i64,
    dry_mass: f64,
    engine: Engine,
    nodes: Vec<ManeuverNode>,
    guidance: Vec<Option<State>>,
    node_metric: bool,
    /// Cryogenic fit-out. Editable by the player; signed into FDR v3.
    thermal_design: odd_contracts::ThermalDesign,
    /// Which disciplines the Chief-AI sized. Signed into FDR v3.
    delegation: odd_contracts::DelegationMask,
}
impl Settings {
    /// The tank model applies only to cryogenic pairs, and only when a load
    /// is actually aboard.
    /// The design that actually flies: the player's sliders with every
    /// delegated discipline replaced by its chief's textbook choice. The
    /// flight, the capture and the assembly readouts all resolve through
    /// here so they cannot disagree about what was flown.
    fn effective(&self) -> Option<odd_contracts::chief_ai::ChiefDesign> {
        let contract = Contract::get(self.contract_id)?;
        Some(odd_contracts::chief_ai::substitute(
            &contract,
            self.engine_spec,
            &self.thermal_design,
            self.delegation,
        ))
    }

    fn thermal_inputs(&self) -> Option<odd_verify::ThermalInputs> {
        let load = self.initial.mass - self.dry_mass;
        if load <= 0.0 {
            return None;
        }
        let effective = self.effective()?;
        Some(odd_verify::ThermalInputs {
            engine_spec: effective.engine,
            design: effective.thermal,
            load_kg: load,
        })
    }

    /// The physical inputs the portable evaluator needs. Node schedule and
    /// guidance travel in the compiled `Plan`, not in the settings.
    fn flight(&self) -> FlightSettings {
        FlightSettings {
            delegation: self.delegation,
            // The live designer draws and signs v4 records, so a new flight is
            // computed under the coupled, fitted-mass physics.
            revision: odd_core::PhysicsRevision::FittedScoring,
            thermal: self.thermal_inputs(),
            contract_id: self.contract_id,
            epoch: self.epoch,
            initial: self.initial,
            duration_us: self.duration_us,
            dry_mass: self.dry_mass,
            engine: self.engine,
            node_metric: self.node_metric,
        }
    }
}
#[derive(Clone)]
struct Execution {
    trajectory: TrajectorySpline,
    report: [f64; REPORT_LEN],
    final_state: Keyframe,
    burn_epochs: Vec<(f64, f64)>,
    burn_sweeps: Vec<f64>,
    thermal: odd_contracts::ThermalOutcome,
    /// v4: windows actually flown after any flameout, and where the engine
    /// died. Empty/`None` until the coupled replay fills them.
    flown_burn_windows: Vec<(f64, f64)>,
    flameout_time_s: Option<f64>,
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
/// Why the last `mission_compile` returned 0, as a stable code rather than a
/// translatable string. The browser maps codes to copy, so a new variant here
/// cannot silently become an unexplained sentence in front of a player (Law 6).
/// Zero means "no failure recorded".
static COMPILE_ERROR: Mutex<u8> = Mutex::new(0);

/// Stable codes for `mission_compile`'s refusal. `PlanError::Command` is folded
/// into `NUMERIC` because the browser has nothing different to say about it.
const COMPILE_REFUSAL_INVALID_INPUT: u8 = 1;
const COMPILE_REFUSAL_UNSUPPORTED_CONIC: u8 = 2;
const COMPILE_REFUSAL_NUMERIC: u8 = 3;
const COMPILE_REFUSAL_PROPELLANT: u8 = 4;
const COMPILE_REFUSAL_OUTSIDE_PLAN: u8 = 5;

fn refusal_code(error: PlanError) -> u8 {
    match error {
        PlanError::InvalidInput => COMPILE_REFUSAL_INVALID_INPUT,
        PlanError::UnsupportedConic => COMPILE_REFUSAL_UNSUPPORTED_CONIC,
        PlanError::NumericalRange | PlanError::Command(_) => COMPILE_REFUSAL_NUMERIC,
        PlanError::PropellantDeficit => COMPILE_REFUSAL_PROPELLANT,
        PlanError::BurnOutsidePlan => COMPILE_REFUSAL_OUTSIDE_PLAN,
    }
}

fn clear_results() {
    *COMPILE_ERROR.lock().unwrap() = 0;
    *PLAN.lock().unwrap() = None;
    *LOCKED.lock().unwrap() = None;
    *EXECUTION.lock().unwrap() = None;
    *PATCH.lock().unwrap() = None;
    *TARGET.lock().unwrap() = None;
    CAUSAL.lock().unwrap().clear();
}
/// Load a canonical ranked contract. All physics inputs come from odd-contracts.
#[no_mangle]
pub extern "C" fn mission_contract(id: u32) -> i32 {
    clear_results();
    *SETTINGS.lock().unwrap() = None;
    let Some(c) = Contract::get(id) else { return 0 };
    *SETTINGS.lock().unwrap() = Some(Settings {
        contract_id: id,
        engine_spec: c.engine_spec,
        thermal_design: odd_contracts::ThermalDesign::baseline(),
        delegation: odd_contracts::DelegationMask::MANUAL,
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
        // The free sandbox has no catalog stage, so it carries a placeholder
        // design and cannot produce a certified recorder.
        engine_spec: odd_contracts::STORABLE_BASELINE,
        thermal_design: odd_contracts::ThermalDesign::baseline(),
        delegation: odd_contracts::DelegationMask::MANUAL,
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
    let result = match Plan::compile_guided(
        s.initial,
        MU,
        s.duration_us,
        s.dry_mass,
        s.engine,
        &s.nodes,
        &s.guidance,
    ) {
        Ok(result) => result,
        Err(error) => {
            *COMPILE_ERROR.lock().unwrap() = refusal_code(error);
            return 0;
        }
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

/// Why the last `mission_compile` refused the plan, as a stable code:
/// 0 none, 1 invalid input, 2 unsupported conic, 3 numeric range,
/// 4 propellant deficit, 5 burn outside the mission window.
#[no_mangle]
pub extern "C" fn mission_compile_error() -> u32 {
    *COMPILE_ERROR.lock().unwrap() as u32
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

/// B-plane info at the lunar encounter.
/// 0 valid (1 if encounter with a computable B-plane), 1 miss distance m,
/// 2 B·T m (along-track), 3 B·R m (cross-track), 4-6 S unit vector (asymptote),
/// 7-9 R unit vector (cross-track basis), 10-12 T unit vector (along-track basis).
#[no_mangle]
pub extern "C" fn mission_bplane_info(index: u32) -> f64 {
    let patch = PATCH.lock().unwrap();
    let Some(p) = patch.as_ref() else {
        return if index == 0 { 0.0 } else { f64::NAN };
    };
    let Some(bp) = p.entry.bplane else {
        return if index == 0 { 0.0 } else { f64::NAN };
    };
    match index {
        0 => 1.0,
        1 => bp.miss_distance_m,
        2 => bp.b_t,
        3 => bp.b_r,
        4 => bp.s[0],
        5 => bp.s[1],
        6 => bp.s[2],
        7 => bp.r_hat[0],
        8 => bp.r_hat[1],
        9 => bp.r_hat[2],
        10 => bp.t_hat[0],
        11 => bp.t_hat[1],
        12 => bp.t_hat[2],
        _ => f64::NAN,
    }
}

/// Target the B-plane at the lunar encounter. Adjusts node TNW values so the
/// encounter's B-plane coordinates move toward (target_bt, target_br).
/// Sensitivity is computed by finite differences over the patched-conic preview:
/// each node component is perturbed by 0.1 m/s, the plan is recompiled, the
/// encounter is refound, and the B-plane is recomputed.
///
/// Returns 1 on success, 0 if there is no encounter, no nodes, or the system
/// is singular (insufficient sensitivity).
#[no_mangle]
pub extern "C" fn mission_target_bplane(target_bt: f64, target_br: f64) -> i32 {
    if !target_bt.is_finite() || !target_br.is_finite() {
        return 0;
    }
    let Some(s) = SETTINGS.lock().unwrap().clone() else {
        return 0;
    };
    if s.contract_id != 103 {
        return 0;
    }
    let Some(eph) = EPHEMERIS.get_or_init(|| Ephemeris::bundled().ok()).as_ref() else {
        return 0;
    };
    let Some(patch) = PATCH.lock().unwrap().clone() else {
        return 0;
    };
    let Some(current_bp) = patch.entry.bplane else {
        return 0;
    };

    let n = s.nodes.len();
    if n == 0 || n > 12 {
        return 0;
    }

    let current_bt = current_bp.b_t;
    let current_br = current_bp.b_r;

    // Sensitivity by finite differences: perturb each node component,
    // recompile the patched-conic plan, refind the encounter, recompute B-plane.
    const DELTA: f64 = 0.1;
    let mut sens = [[[0.0; 2]; 3]; 12];
    for (i, node) in s.nodes.iter().take(n).enumerate() {
        for (c, _) in node.tnw_m_s.iter().enumerate().take(3) {
            let mut perturbed = s.nodes.clone();
            perturbed[i].tnw_m_s[c] += DELTA;
            let Ok(perturbed_plan) = Plan::compile_guided(
                s.initial,
                MU,
                s.duration_us,
                s.dry_mass,
                s.engine,
                &perturbed,
                &s.guidance,
            ) else {
                continue;
            };
            let Ok(Some(perturbed_patch)) =
                LunarPatch::find(&perturbed_plan, MOON_MU, lunar_soi(), 300_000_000, |t| {
                    moon_state(eph, s.epoch, t)
                })
            else {
                continue;
            };
            let Some(bp) = perturbed_patch.entry.bplane else {
                continue;
            };
            sens[i][c][0] = (bp.b_t - current_bt) / DELTA;
            sens[i][c][1] = (bp.b_r - current_br) / DELTA;
        }
    }

    let mut any_sens = false;
    for node_sens in sens.iter().take(n) {
        for comp_sens in node_sens.iter() {
            if comp_sens[0].abs() > 1e-9 || comp_sens[1].abs() > 1e-9 {
                any_sens = true;
            }
        }
    }
    if !any_sens {
        return 0;
    }

    let node_tnw: Vec<[f64; 3]> = s.nodes.iter().map(|node| node.tnw_m_s).collect();
    let Some(adjustments) = target_bplane(&current_bp, target_bt, target_br, &node_tnw, &sens[..n])
    else {
        return 0;
    };

    let mut guard = SETTINGS.lock().unwrap();
    let Some(settings) = guard.as_mut() else {
        return 0;
    };
    for (i, adj) in adjustments.iter().enumerate() {
        if let Some(node) = settings.nodes.get_mut(i) {
            node.tnw_m_s[0] += adj[0];
            node.tnw_m_s[1] += adj[1];
            node.tnw_m_s[2] += adj[2];
        }
    }
    *PLAN.lock().unwrap() = None;
    *PATCH.lock().unwrap() = None;
    1
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

fn lunar_soi() -> f64 {
    sphere_of_influence(384_400_000.0, 4.902_800_118e12, MU).unwrap_or(66_100_000.0)
}
fn component_value(state: Keyframe, component: u32) -> f64 {
    match component {
        0..=2 => state.position[component as usize],
        3..=5 => state.velocity[(component - 3) as usize],
        6 => state.mass,
        _ => f64::NAN,
    }
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
    let Some(outcome) = odd_verify::compute(&s.flight(), &plan, locked.as_ref(), eph) else {
        return 0;
    };
    *EXECUTION.lock().unwrap() = Some(Execution {
        trajectory: outcome.trajectory,
        report: outcome.report,
        final_state: outcome.final_state,
        burn_epochs: outcome.burns.iter().map(|b| (b.start, b.end)).collect(),
        burn_sweeps: outcome.burn_sweeps,
        thermal: outcome.thermal,
        flown_burn_windows: outcome.flown_burn_windows,
        flameout_time_s: outcome.flameout_time_s,
    });
    1
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
    inclination_change_deg: f64,
    requested_inclination_change_deg: f64,
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
        inclination_change_deg,
        requested_inclination_change_deg,
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
    let Some(mut contract) = Contract::get(s.contract_id) else {
        return 0;
    };
    contract.initial = s.initial;
    contract.dry_mass_kg = s.dry_mass;
    contract.engine = s.engine;
    let Ok(split) =
        odd_contracts::split_tug_for_stage(&contract, kicks as usize, total_dv, first_node_s, eph)
    else {
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

pub(crate) struct CapturedCommand {
    pub epoch_us: i64,
    pub tnw_m_s: [f64; 3],
    pub guidance: Option<[f64; 6]>,
}

pub(crate) struct CapturedFlight {
    pub contract_id: u32,
    pub engine_spec: odd_propulsion::EngineSpec,
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
    /// The cryogenic fit-out that actually flew, and what it produced.
    pub thermal_design: odd_contracts::ThermalDesign,
    pub thermal: odd_contracts::ThermalOutcome,
    pub tank_volumes_m3: [f64; 2],
    pub delegation: odd_contracts::DelegationMask,
    /// v4: the burn windows the coupled flight actually flew, after any
    /// flameout truncated them, and where the engine died if it did.
    pub flown_burn_windows: Vec<(f64, f64)>,
    pub flameout_time_s: Option<f64>,
}

/// Snapshot the executed flight for signing. Requires a completed execution.
pub(crate) fn capture_flight() -> Option<CapturedFlight> {
    capture(false)
}

/// Snapshot exactly the same design and prediction before execution. The
/// outcome fields are placeholders and are never signed or submitted.
pub(crate) fn capture_preflight() -> Option<CapturedFlight> {
    capture(true)
}

fn capture(preflight: bool) -> Option<CapturedFlight> {
    let settings = SETTINGS.lock().unwrap().clone()?;
    let execution = EXECUTION.lock().unwrap().clone();
    let locked = LOCKED.lock().unwrap().clone();
    if preflight && (locked.is_none() || execution.is_some()) {
        return None;
    }
    if !preflight && execution.is_none() {
        return None;
    }
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
    let effective = settings.effective()?;
    let load = settings.initial.mass - settings.dry_mass;
    let contract = odd_contracts::Contract::get(settings.contract_id)?;
    let stage = effective.thermal.stage(&contract, &effective.engine, load);
    let volumes = stage
        .as_ref()
        .map(|stage| [stage.oxidizer.volume_m3, stage.fuel.volume_m3])
        .unwrap_or([0.0, 0.0]);
    let thermal = execution
        .as_ref()
        .map(|e| e.thermal.clone())
        .unwrap_or_else(|| {
            let mut placeholder = odd_contracts::ThermalOutcome::inert();
            placeholder.inert = stage.is_none();
            placeholder
        });
    Some(CapturedFlight {
        contract_id: settings.contract_id,
        engine_spec: effective.engine,
        epoch_s: settings.epoch,
        duration_us: settings.duration_us,
        initial: settings.initial,
        dry_mass: settings.dry_mass,
        thrust_n: settings.engine.thrust_newtons,
        specific_impulse_s: settings.engine.specific_impulse_seconds,
        node_metric: settings.node_metric,
        thermal_design: effective.thermal,
        tank_volumes_m3: volumes,
        thermal,
        delegation: settings.delegation,
        flown_burn_windows: execution
            .as_ref()
            .map(|e| e.flown_burn_windows.clone())
            .unwrap_or_else(|| settings.nodes.iter().map(|_| (0.0, 0.0)).collect()),
        flameout_time_s: execution.as_ref().and_then(|e| e.flameout_time_s),
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
        report: execution
            .as_ref()
            .map(|e| e.report)
            .unwrap_or([0.0; REPORT_LEN]),
        final_state: execution
            .as_ref()
            .map(|e| e.final_state)
            .unwrap_or(settings.initial),
    })
}

/// The catalog contract behind the current session, if any.
pub(crate) fn current_contract() -> Option<Contract> {
    let settings = SETTINGS.lock().unwrap();
    Contract::get(settings.as_ref()?.contract_id)
}

/// The engine design the current session flies.
pub(crate) fn current_engine_spec() -> Option<odd_propulsion::EngineSpec> {
    Some(SETTINGS.lock().unwrap().as_ref()?.engine_spec)
}

/// Rebuild the stage around a player-designed engine. Mass, thrust and Isp
/// all move together, and any compiled plan is discarded because its burn
/// durations were computed for the old engine.
pub(crate) fn apply_engine_design(spec: odd_propulsion::EngineSpec) -> i32 {
    let mut guard = SETTINGS.lock().unwrap();
    let Some(settings) = guard.as_mut() else {
        return 0;
    };
    let Some(contract) = Contract::get(settings.contract_id) else {
        return 0;
    };
    let vehicle =
        match contract.vehicle_delegated(spec, settings.delegation, &settings.thermal_design) {
            Ok(vehicle) => vehicle,
            Err(odd_propulsion::PropulsionError::ThermalLimitExceeded) => return -2,
            Err(_) => return -1,
        };
    if !valid_stage(&contract, &vehicle) {
        return -1;
    }
    settings.engine_spec = spec;
    settings.engine = vehicle.engine;
    settings.dry_mass = vehicle.dry_mass_kg;
    settings.initial.mass = vehicle.initial_mass_kg;
    settings.nodes.clear();
    settings.guidance.clear();
    drop(guard);
    clear_results();
    1
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
        thermal: execution.thermal,
        flown_burn_windows: execution.flown_burn_windows,
        flameout_time_s: execution.flameout_time_s,
        propellant_remaining_kg: None,
    };
    let commands: Vec<(i64, [f64; 3])> = settings
        .nodes
        .iter()
        .map(|node| (node.epoch_us, node.tnw_m_s))
        .collect();
    Some(odd_verify::geometry_from(
        &settings.initial,
        &plan,
        &outcome,
        &commands,
    ))
}

/// The recorder tests run real flights against the same process-wide statics,
/// so they must share this lock with the mission tests.
#[cfg(test)]
pub(crate) fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    tests::TEST_LOCK.lock().unwrap()
}

/// The value actually flown for a cryogenic/power field, after chief
/// substitution. Same field ids as `mission_thermal_get`. The assembly shows
/// this next to a locked slider so the player can see what the chief bought.
#[no_mangle]
pub extern "C" fn mission_thermal_effective(field: u32) -> f64 {
    let Some(settings) = SETTINGS.lock().unwrap().clone() else {
        return f64::NAN;
    };
    let Some(effective) = settings.effective() else {
        return f64::NAN;
    };
    let d = &effective.thermal;
    match field {
        0 => f64::from(d.mli_layers),
        1 => d.epsilon_eff,
        2 => d.cryocooler_lift_w,
        3 => d.radiator_area_m2,
        4 => d.strut_conductance_w_k,
        5 => d.relief_pressure_pa,
        6 => f64::from(d.shell),
        8 => d.bus.array_w_at_1au,
        9 => d.bus.avionics_w,
        _ => f64::NAN,
    }
}

/// Which department owns a field, so the assembly knows which slider to lock
/// when that department is delegated. Returns the department discriminant
/// (1 Thermal, 2 Power, 3 GNC) or u32::MAX when the field is unknown.
#[no_mangle]
pub extern "C" fn mission_thermal_owner(field: u32) -> u32 {
    match field {
        0..=6 => 1,
        8 => 2,
        9 => 3,
        _ => u32::MAX,
    }
}

/// The engine the stage actually flies after propulsion substitution:
/// 0 thrust N, 1 specific impulse s, 2 dry kg, 3 mixture ratio, 4 chamber Pa,
/// 5 throat m, 6 exit m, 7 bell length fraction, 8 propellant id.
#[no_mangle]
pub extern "C" fn mission_engine_effective(field: u32) -> f64 {
    let Some(settings) = SETTINGS.lock().unwrap().clone() else {
        return f64::NAN;
    };
    let Some(effective) = settings.effective() else {
        return f64::NAN;
    };
    let Some(contract) = Contract::get(settings.contract_id) else {
        return f64::NAN;
    };
    let Ok(vehicle) = contract.vehicle_delegated(
        settings.engine_spec,
        settings.delegation,
        &settings.thermal_design,
    ) else {
        return f64::NAN;
    };
    let spec = effective.engine;
    match field {
        0 => vehicle.engine.thrust_newtons,
        1 => vehicle.engine.specific_impulse_seconds,
        2 => vehicle.solution.dry_mass_kg,
        3 => spec.of_ratio,
        4 => spec.p_c_pa,
        5 => spec.r_throat_m,
        6 => spec.r_exit_m,
        7 => spec.length_fraction,
        8 => f64::from(spec.propellant as u8),
        _ => f64::NAN,
    }
}

// ---------------------------------------------------------------------------
// Vehicle assembly: Chief-AI delegation and the cryogenic design lever.
//
// These are the controls that make WP 1.6 and WP 1.7 reachable from the
// browser. Both rebuild the stage, because a chief's margin is real dry mass
// and a thicker blanket changes the hardware the chief would size.
// ---------------------------------------------------------------------------

/// Check the same thermal bounds used by the flight before publishing an
/// assembly edit. An engine solution alone does not validate its tank or bus.
fn valid_stage(contract: &Contract, vehicle: &odd_contracts::Vehicle) -> bool {
    if !vehicle.dry_mass_kg.is_finite()
        || vehicle.dry_mass_kg <= 0.0
        || !vehicle.propellant_kg.is_finite()
        || vehicle.propellant_kg <= 0.0
        || !vehicle.initial_mass_kg.is_finite()
        || vehicle.initial_mass_kg <= vehicle.dry_mass_kg
    {
        return false;
    }
    let flown = &vehicle.effective;
    if !odd_systems::needs_thermal_model(flown.engine.propellant) {
        return true;
    }
    flown
        .thermal
        .stage(contract, &flown.engine, vehicle.propellant_kg)
        .is_some_and(|stage| {
            stage.oxidizer.validate().is_ok()
                && stage.fuel.validate().is_ok()
                && stage.bus.validate().is_ok()
                && stage.structure_temperature_k.is_finite()
                && stage.structure_temperature_k > 0.0
                && stage.sink_temperature_k.is_finite()
                && stage.sink_temperature_k > 0.0
        })
}

/// Apply an edit to a candidate, then rebuild and validate it before touching
/// the live stage or its compiled plan. A rejected edit is a true no-op.
fn rebuild_stage(edit: impl FnOnce(&mut Settings) -> Result<(), i32>) -> i32 {
    let mut guard = SETTINGS.lock().unwrap();
    let Some(settings) = guard.as_mut() else {
        return 0;
    };
    let mut candidate = settings.clone();
    if let Err(code) = edit(&mut candidate) {
        return code;
    }
    let Some(contract) = Contract::get(candidate.contract_id) else {
        return 0;
    };
    let vehicle = match contract.vehicle_delegated(
        candidate.engine_spec,
        candidate.delegation,
        &candidate.thermal_design,
    ) {
        Ok(vehicle) => vehicle,
        Err(odd_propulsion::PropulsionError::ThermalLimitExceeded) => return -2,
        Err(_) => return -1,
    };
    if !valid_stage(&contract, &vehicle) {
        return 0;
    }
    candidate.engine = vehicle.engine;
    candidate.dry_mass = vehicle.dry_mass_kg;
    candidate.initial.mass = vehicle.initial_mass_kg;
    candidate.nodes.clear();
    candidate.guidance.clear();
    *settings = candidate;
    drop(guard);
    clear_results();
    1
}

/// The four discipline hardware masses and the margin each chief would add.
fn delegation_outcome() -> Option<odd_contracts::chief_ai::DelegationOutcome> {
    let settings = SETTINGS.lock().unwrap().clone()?;
    let contract = Contract::get(settings.contract_id)?;
    let vehicle = contract
        .vehicle_delegated(
            settings.engine_spec,
            settings.delegation,
            &settings.thermal_design,
        )
        .ok()?;
    let radius = libm::cbrt(3.0 * contract.tank_volume_m3 / (4.0 * std::f64::consts::PI));
    let area = 4.0 * std::f64::consts::PI * radius * radius;
    let flown = &vehicle.effective.thermal;
    let cryogenic = flown
        .stage(&contract, &vehicle.effective.engine, vehicle.propellant_kg)
        .is_some();
    let sizing = if cryogenic {
        *flown
    } else {
        let mut sanitized = *flown;
        sanitized.radiator_area_m2 = 0.0;
        sanitized.cryocooler_lift_w = 0.0;
        sanitized
    };
    let (volume, area) = if cryogenic {
        (contract.tank_volume_m3, area)
    } else {
        (0.0, 0.0)
    };
    let hardware = odd_contracts::chief_ai::hardware_mass(
        vehicle.solution.dry_mass_kg,
        volume,
        area,
        flown.relief_pressure_pa,
        &sizing,
    );
    Some(odd_contracts::chief_ai::apply(
        settings.delegation,
        &hardware,
        &[1.0; 4],
    ))
}

/// Set the delegation bitmask (bit 0 Propulsion, 1 Thermal, 2 Power, 3 GNC).
/// Rejects any bit outside the four departments. Rebuilds the stage, so the
/// margin immediately changes the mass that flies.
#[no_mangle]
pub extern "C" fn mission_delegation_set(mask: u32) -> i32 {
    let Ok(byte) = u8::try_from(mask) else {
        return 0;
    };
    let mask = odd_contracts::DelegationMask(byte);
    if !mask.is_valid() {
        return 0;
    }
    rebuild_stage(|settings| {
        settings.delegation = mask;
        Ok(())
    })
}

/// The current delegation bitmask.
#[no_mangle]
pub extern "C" fn mission_delegation_mask() -> u32 {
    SETTINGS
        .lock()
        .unwrap()
        .as_ref()
        .map_or(0, |s| u32::from(s.delegation.0))
}

/// One discipline's chief decision: field 0 is the kg the chief added, field 1
/// the discipline's own hardware mass. NaN when out of range.
#[no_mangle]
pub extern "C" fn mission_delegation_cell(department: u32, field: u32) -> f64 {
    let Some(outcome) = delegation_outcome() else {
        return f64::NAN;
    };
    let Ok(index) = usize::try_from(department) else {
        return f64::NAN;
    };
    let Some(decision) = outcome.decisions.get(index) else {
        return f64::NAN;
    };
    match field {
        0 => decision.margin_kg,
        1 => decision.base_kg,
        _ => f64::NAN,
    }
}

/// Assembly readout: 0 margin kg, 1 cost multiplier, 2 dry mass, 3 wet mass,
/// 4 efficiency cap. NaN when no stage is selected.
#[no_mangle]
pub extern "C" fn mission_delegation_readout(field: u32) -> f64 {
    let Some(settings) = SETTINGS.lock().unwrap().clone() else {
        return f64::NAN;
    };
    let Some(outcome) = delegation_outcome() else {
        return f64::NAN;
    };
    match field {
        0 => outcome.mass_penalty_kg,
        1 => outcome.cost_multiplier,
        2 => settings.dry_mass,
        3 => settings.initial.mass,
        4 => outcome.efficiency_cap,
        5 => Contract::get(settings.contract_id)
            .and_then(|c| {
                c.vehicle_delegated(
                    settings.engine_spec,
                    settings.delegation,
                    &settings.thermal_design,
                )
                .ok()
            })
            .map_or(f64::NAN, |v| v.thermal_fitout_kg),
        _ => f64::NAN,
    }
}

fn delegation_badge(department: u32) -> Option<String> {
    let department = odd_contracts::Department::from_u8(u8::try_from(department).ok()?)?;
    let outcome = delegation_outcome()?;
    outcome
        .decisions
        .iter()
        .find(|d| d.department == department)
        .map(|d| d.badge.clone())
}

/// UTF-8 length of a department's badge; 0 when out of range.
#[no_mangle]
pub extern "C" fn mission_delegation_badge_len(department: u32) -> u32 {
    delegation_badge(department).map_or(0, |s| s.len() as u32)
}

/// One UTF-8 byte of that badge; NaN when out of range.
#[no_mangle]
pub extern "C" fn mission_delegation_badge_byte(department: u32, index: u32) -> f64 {
    delegation_badge(department).map_or(f64::NAN, |s| {
        s.as_bytes()
            .get(index as usize)
            .map_or(f64::NAN, |b| *b as f64)
    })
}

/// Read a cryogenic design field. 0 MLI layers, 1 effective emittance,
/// 2 commanded cooler lift W, 3 radiator area m², 4 strut conductance W/K,
/// 5 relief pressure Pa, 6 shell id.
#[no_mangle]
pub extern "C" fn mission_thermal_get(field: u32) -> f64 {
    let Some(settings) = SETTINGS.lock().unwrap().clone() else {
        return f64::NAN;
    };
    let d = &settings.thermal_design;
    match field {
        0 => f64::from(d.mli_layers),
        1 => d.epsilon_eff,
        2 => d.cryocooler_lift_w,
        3 => d.radiator_area_m2,
        4 => d.strut_conductance_w_k,
        5 => d.relief_pressure_pa,
        6 => f64::from(d.shell),
        8 => d.bus.array_w_at_1au,
        // 7: does this stage actually carry cryogenic tanks? The assembly
        // disables the cryo knobs when it does not, instead of offering a
        // control that silently does nothing.
        7 => {
            let Some(contract) = Contract::get(settings.contract_id) else {
                return f64::NAN;
            };
            let Some(effective) = settings.effective() else {
                return f64::NAN;
            };
            let load = settings.initial.mass - settings.dry_mass;
            if effective
                .thermal
                .stage(&contract, &effective.engine, load)
                .is_some()
            {
                1.0
            } else {
                0.0
            }
        }
        _ => f64::NAN,
    }
}

/// Whether this contract and its current flown engine support the catalog's
/// recommended cryogenic card. A storable contract has no cryogenic card.
#[no_mangle]
pub extern "C" fn mission_thermal_card_available() -> i32 {
    let Some(settings) = SETTINGS.lock().unwrap().clone() else {
        return 0;
    };
    let Some(contract) = Contract::get(settings.contract_id) else {
        return 0;
    };
    let Some(flown) = settings.effective() else {
        return 0;
    };
    i32::from(
        odd_systems::needs_thermal_model(contract.engine_spec.propellant)
            && odd_systems::needs_thermal_model(flown.engine.propellant),
    )
}

/// Apply the contract's recommended *manual* cryogenic design. It remains a
/// paid, editable choice, not an automatic change to historical baselines.
/// Build the vehicle before changing the session so a rejected card cannot
/// leave partially updated hardware behind.
#[no_mangle]
pub extern "C" fn mission_thermal_apply_card() -> i32 {
    if mission_thermal_card_available() != 1 {
        return 0;
    }
    let mut guard = SETTINGS.lock().unwrap();
    let Some(settings) = guard.as_mut() else {
        return 0;
    };
    // Don't quietly replace a chief's flown hardware with a manual choice.
    if settings.delegation.0 & 0b0110 != 0 {
        return 0;
    }
    let Some(contract) = Contract::get(settings.contract_id) else {
        return 0;
    };
    let design = contract.thermal_card();
    let Ok(vehicle) =
        contract.vehicle_delegated(settings.engine_spec, settings.delegation, &design)
    else {
        return 0;
    };
    if !valid_stage(&contract, &vehicle) {
        return 0;
    }
    settings.thermal_design = design;
    settings.engine = vehicle.engine;
    settings.dry_mass = vehicle.dry_mass_kg;
    settings.initial.mass = vehicle.initial_mass_kg;
    settings.nodes.clear();
    settings.guidance.clear();
    drop(guard);
    clear_results();
    1
}

/// Set a cryogenic design field, then rebuild the stage. Returns 1 on
/// success, 0 for an unknown field or an unphysical value.
///
/// Changing the layer count re-derives the emittance floor (`4 × ideal`),
/// because a blanket's emittance is a consequence of its construction, not
/// an independent number the player may claim.
#[no_mangle]
pub extern "C" fn mission_thermal_set(field: u32, value: f64) -> i32 {
    if !value.is_finite() || value < 0.0 {
        return 0;
    }
    // Refuse cryogenic edits on a storable stage: there is no fit-out to
    // size, and accepting one would put hardware in the flown mass that the
    // record has no thermal block to prove.
    if mission_thermal_get(7) != 1.0 {
        return 0;
    }
    rebuild_stage(|settings| {
        let d = &mut settings.thermal_design;
        match field {
            0 => {
                let layers = value.round();
                if !(1.0..=100.0).contains(&layers) {
                    return Err(0);
                }
                d.mli_layers = layers as u32;
                d.epsilon_eff = odd_systems::ideal_epsilon_for_layers(d.mli_layers) * 4.0;
            }
            1 => {
                if !(0.0..1.0).contains(&value)
                    || value < odd_systems::ideal_epsilon_for_layers(d.mli_layers)
                {
                    return Err(0);
                }
                d.epsilon_eff = value;
            }
            2 => {
                // A cooler needs somewhere to reject its heat.
                if value > 0.0 && d.radiator_area_m2 <= 0.0 {
                    return Err(0);
                }
                d.cryocooler_lift_w = value;
            }
            3 => {
                if value == 0.0 && d.cryocooler_lift_w > 0.0 {
                    return Err(0);
                }
                d.radiator_area_m2 = value;
            }
            4 => d.strut_conductance_w_k = value,
            5 => {
                if value <= 0.0 {
                    return Err(0);
                }
                d.relief_pressure_pa = value;
            }
            8 => {
                // The array must at least cover the fixed avionics load, and
                // its mass is charged to the stage like any other hardware.
                if value < d.bus.avionics_w {
                    return Err(0);
                }
                d.bus.array_w_at_1au = value;
            }
            6 => {
                let Ok(id) = u8::try_from(value.round() as i64) else {
                    return Err(0);
                };
                if odd_contracts::thermal::shell_from_u8(id).is_none() {
                    return Err(0);
                }
                d.shell = id;
            }
            _ => return Err(0),
        }
        Ok(())
    })
}

/// Boil-off readout from the last execution. 0 total kg lost, 1 oxidizer kg,
/// 2 fuel kg, 3 cooler joules, 4 vent count, 5 inert flag. NaN before a flight.
#[no_mangle]
pub extern "C" fn mission_boil_off(field: u32) -> f64 {
    let Some(execution) = EXECUTION.lock().unwrap().clone() else {
        return f64::NAN;
    };
    let t = &execution.thermal;
    match field {
        0 => t.lost_kg,
        1 => t.tank_lost_kg[0],
        2 => t.tank_lost_kg[1],
        3 => t.cooler_energy_j,
        4 => t.vents.len() as f64,
        5 => {
            if t.inert {
                1.0
            } else {
                0.0
            }
        }
        _ => f64::NAN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // The scalar bridge owns process-wide statics; native tests must not
    // interleave setup/compile/lock calls the way separate workers would.
    pub(super) static TEST_LOCK: Mutex<()> = Mutex::new(());
    #[test]
    fn invalid_stage_edits_are_atomic_and_cannot_poison_the_plan() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(mission_contract(103), 1);
        assert_eq!(mission_add_node(1600.0, 200.0, 0.0, 0.0), 1);
        assert_eq!(mission_compile(), 1);
        let before = [
            mission_thermal_get(5),
            mission_thermal_get(3),
            mission_parameter(3),
            mission_preview(100.0, 0),
        ];
        assert!(before.iter().all(|v| v.is_finite()));
        // Liquid hydrogen cannot reach this modeled relief point while liquid.
        assert_eq!(mission_thermal_set(5, 2_000_000.0), 0);
        assert_eq!(mission_thermal_get(5), before[0]);
        assert_eq!(mission_thermal_set(3, f64::MAX), 0);
        assert_eq!(mission_thermal_get(3), before[1]);
        assert_eq!(mission_parameter(3).to_bits(), before[2].to_bits());
        assert_eq!(mission_node_count(), 1);
        assert_eq!(mission_preview(100.0, 0).to_bits(), before[3].to_bits());

        // Free sandbox setups have no catalog vehicle to rebuild. A failed
        // delegation request must not still change its stored mask.
        let r = odd_contracts::EARTH_EQUATORIAL_RADIUS + 260_000.0;
        assert_eq!(
            mission_setup(
                820_497_600.0,
                10_800.0,
                1200.0,
                1020.0,
                2400.0,
                310.0,
                r,
                0.0,
                0.0,
                0.0,
                (MU / r).sqrt(),
                0.0
            ),
            1
        );
        assert_eq!(mission_delegation_set(1), 0);
        assert_eq!(mission_delegation_mask(), 0);
    }

    #[test]
    fn recommended_card_is_real_paid_hardware_and_respects_delegation() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(mission_contract(101), 1);
        assert_eq!(mission_thermal_card_available(), 0);
        assert_eq!(mission_thermal_apply_card(), 0);
        assert_eq!(mission_contract(103), 1);
        let baseline = mission_parameter(3);
        assert_eq!(mission_thermal_card_available(), 1);
        assert_eq!(mission_thermal_apply_card(), 1);
        let card = Contract::get(103).unwrap().thermal_card();
        assert_eq!(mission_thermal_get(0), card.mli_layers as f64);
        assert_eq!(mission_thermal_get(1).to_bits(), card.epsilon_eff.to_bits());
        assert_eq!(mission_thermal_get(2), card.cryocooler_lift_w);
        assert_eq!(mission_thermal_get(8), card.bus.array_w_at_1au);
        assert!(mission_parameter(3) > baseline);
        assert_eq!(mission_delegation_set(2), 1);
        assert_eq!(mission_thermal_apply_card(), 0);
    }

    #[test]
    fn preview_and_failed_setup_clear_stale_state() {
        let _guard = TEST_LOCK.lock().unwrap();
        let radius = odd_contracts::EARTH_EQUATORIAL_RADIUS + 260_000.0;
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
    /// A refusal must say *why*. Before this, `mission_compile` returned 0 for
    /// five different failures and the browser turned all of them into "Maneuvers
    /// overlap, exceed the fuel budget, or fall outside the mission." — one
    /// sentence that named none of them.
    #[test]
    fn compile_refusals_report_their_reason() {
        let _guard = TEST_LOCK.lock().unwrap();

        // 101 is the storable rescue stage: 495.1 m/s of budget on 180 kg.
        assert_eq!(mission_contract(101), 1);
        // A burn past the budget that still fits inside the window: 600 m/s
        // needs ~216 kg from tanks that hold 180 kg, and lasts ~274 s of the
        // 10,800 s window, so the *tank* is what stops it.
        assert_eq!(mission_add_node(200.0, 600.0, 0.0, 0.0), 1);
        assert_eq!(mission_compile(), 0);
        assert_ne!(
            mission_compile_error(),
            0,
            "a refusal must carry a reason code"
        );
        let costly = mission_compile_error();
        assert_eq!(costly, 4, "an unaffordable burn must report a fuel refusal");

        // A modest burn, but burning it at t=20 s: burns are *centred* on their
        // node, so the first half of this one starts before the mission does.
        // This is the case that first showed up as a mystery `BurnOutsidePlan`.
        assert_eq!(mission_contract(101), 1);
        assert_eq!(mission_add_node(20.0, 100.0, 0.0, 0.0), 1);
        assert_eq!(mission_compile(), 0);
        let early = mission_compile_error();
        assert_eq!(
            early, 5,
            "a burn outside the window must report a window refusal"
        );
        assert_ne!(
            early, costly,
            "a burn outside the window must not report the same reason as an              unaffordable one ({early} vs {costly})"
        );

        // A clean plan clears the reason rather than leaving the last one behind.
        assert_eq!(mission_contract(101), 1);
        assert_eq!(mission_compile(), 1);
        assert_eq!(
            mission_compile_error(),
            0,
            "a successful compile must clear the refusal code"
        );
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
        let count = mission_diagnose(
            0.5, 50_000.0, 120_000.0, 0.30, 1.0, 0.10, 1.0, 40.0, 0.0089, 2.6,
        );
        assert_eq!(count, 8);
        assert_eq!(mission_causal_valid(), 1);
        assert_eq!(mission_causal_node_count(0), 4);
        assert_eq!(mission_causal_node_kind(0, 3), 3);
        assert!(mission_causal_demo_len(0) > 0);
    }
    #[test]
    fn canonical_contract_and_lock() {
        let _guard = TEST_LOCK.lock().unwrap();
        assert_eq!(mission_contract(103), 1);
        // Wet mass follows the solved engine mass, so it lands within grams
        // of the card's 18,000 kg rather than being the literal.
        assert!((mission_parameter(2) - 18_000.0).abs() < 0.05);
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
