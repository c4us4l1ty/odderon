//! Scalar WASM boundary for multi-node preview and the Earth force model.
//! Separate worker instances isolate each run. No simulation or redraw loop is
//! scheduled here; callers request a completed trajectory once.
use super::{Body, Ephemeris, Epoch, EPHEMERIS};
use odd_astro::{
    attribution::propagate_counterfactuals,
    force_model::{Environment, ForceModel, Radiation, ThirdBody, Zonal},
    integrator::{IntegrationError, Options},
    perturb::drag::DragParameters,
    spline::{Keyframe, TrajectorySpline},
};
use odd_sim::{
    mission::{apsides, metric_apoapsis, score_rescue},
    planner::{ManeuverNode, Plan},
    Engine,
};
use std::sync::Mutex;
const MU: f64 = 3.986_004_418e14;
const EARTH: f64 = 6_378_137.0;
#[derive(Clone)]
struct Settings {
    epoch: i64,
    initial: Keyframe,
    duration_us: i64,
    dry_mass: f64,
    engine: Engine,
    nodes: Vec<ManeuverNode>,
}
struct Execution {
    trajectory: TrajectorySpline,
    report: [f64; 26],
}
static SETTINGS: Mutex<Option<Settings>> = Mutex::new(None);
static PLAN: Mutex<Option<Plan>> = Mutex::new(None);
static EXECUTION: Mutex<Option<Execution>> = Mutex::new(None);
fn microseconds(seconds: f64) -> Option<i64> {
    if !seconds.is_finite() || !(0.0..=864_000.0).contains(&seconds) {
        return None;
    }
    Some((seconds * 1e6).round() as i64)
}
/// Reset the mission. Initial state is Earth-centered J2000 in SI units.
#[no_mangle]
#[allow(clippy::too_many_arguments)] // Stable scalar WASM boundary.
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
    *PLAN.lock().unwrap() = None;
    *EXECUTION.lock().unwrap() = None;
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
    });
    1
}
#[no_mangle]
pub extern "C" fn mission_add_node(seconds: f64, t: f64, n: f64, w: f64) -> i32 {
    *PLAN.lock().unwrap() = None;
    *EXECUTION.lock().unwrap() = None;
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
    1
}
#[no_mangle]
pub extern "C" fn mission_compile() -> i32 {
    *PLAN.lock().unwrap() = None;
    *EXECUTION.lock().unwrap() = None;
    let Some(s) = SETTINGS.lock().unwrap().clone() else {
        return 0;
    };
    match Plan::compile(s.initial, MU, s.duration_us, s.dry_mass, s.engine, &s.nodes) {
        Ok(result) => {
            *PLAN.lock().unwrap() = Some(result);
            1
        }
        Err(_) => 0,
    }
}
/// Components 0..2 position, 3..5 velocity, 6 mass. Time is seconds.
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
fn component_value(state: Keyframe, component: u32) -> f64 {
    match component {
        0..=2 => state.position[component as usize],
        3..=5 => state.velocity[(component - 3) as usize],
        6 => state.mass,
        _ => f64::NAN,
    }
}
fn environment(eph: &Ephemeris, epoch: i64, t: f64) -> Result<Environment, IntegrationError> {
    let at = Epoch::new(epoch, t).map_err(|_| IntegrationError::ForceFailure)?;
    let position = |body| {
        eph.state(body, Body::Earth, at)
            .map(|v| v.state.position_m)
            .map_err(|_| IntegrationError::ForceFailure)
    };
    Ok(Environment {
        sun: ThirdBody {
            position: position(Body::Sun)?,
            mu: 1.327_124_400_412_794_2e20,
            radius: 695_700_000.0,
        },
        companion: ThirdBody {
            position: position(Body::Moon)?,
            mu: 4.902_800_118e12,
            radius: 1_737_400.0,
        },
    })
}
/// Propagate one full trajectory and seven counterfactual runs. Returns zero
/// for failed propagation or undefined apoapsis; never exposes stale results.
#[no_mangle]
pub extern "C" fn mission_execute() -> i32 {
    *EXECUTION.lock().unwrap() = None;
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
    let end = s.duration_us as f64 / 1e6;
    let model = ForceModel {
        mu: MU,
        zonal: Some(Zonal {
            pole: [0.0, 0.0, 1.0],
            radius: EARTH,
            j2: 1.082_626_68e-3,
        }),
        drag: Some(DragParameters {
            pole: [0.0, 0.0, 1.0],
            omega: [0.0, 0.0, 7.292_115e-5],
            equatorial_radius: EARTH,
            polar_radius: 6_356_752.314,
            cd: 2.2,
            area: 4.0,
            mass: s.initial.mass,
        }),
        radiation: Some(Radiation {
            cr: 1.3,
            area: 4.0,
            central_radius: EARTH,
        }),
        third_body: true,
        burn: None,
    };
    let options = Options {
        relative_tolerance: 1e-10,
        absolute_tolerance: 1e-10,
        initial_step: 10.0,
        max_step: 120.0,
        max_steps: 1_000_000,
    };
    let Ok(trajectory) = model.propagate_with_burns(s.initial, end, options, &burns, |t| {
        environment(eph, s.epoch, t)
    }) else {
        return 0;
    };
    let Ok(final_state) = trajectory.sample(end) else {
        return 0;
    };
    let Ok(orb) = apsides(final_state, MU, EARTH) else {
        return 0;
    };
    let Ok(runs) = propagate_counterfactuals(
        s.initial,
        end,
        options,
        model,
        &burns,
        &plan.impulses,
        |t| environment(eph, s.epoch, t),
        |state| metric_apoapsis(state, MU, EARTH),
    ) else {
        return 0;
    };
    let Ok(sensitivity) = runs.sensitivities() else {
        return 0;
    };
    let mut report = [f64::NAN; 26];
    report[0] = final_state.mass;
    report[1] = orb.apoapsis_altitude_m;
    report[2] = orb.periapsis_altitude_m;
    report[3] = orb.eccentricity;
    report[4] = score_rescue(final_state, s.dry_mass, end, MU, EARTH).unwrap_or(f64::NAN);
    report[5] = plan.requested_delta_v_m_s;
    if let Some(shares) = sensitivity.position_shares {
        report[6..11].copy_from_slice(&shares);
    }
    if let Some(shares) = sensitivity.metric_shares {
        report[11..16].copy_from_slice(&shares);
    }
    report[16..21].copy_from_slice(&sensitivity.signed_metric_deltas);
    report[21..26].copy_from_slice(&sensitivity.position_distances_m);
    *EXECUTION.lock().unwrap() = Some(Execution { trajectory, report });
    1
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_and_failed_setup_clear_stale_state() {
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
}
