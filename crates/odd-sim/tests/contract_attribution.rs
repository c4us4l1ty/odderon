//! Metric-specific counterfactual regression. These are numerical fixtures,
//! not browser mission acceptance tests or exact causal decompositions.
use odd_astro::{
    attribution::propagate_counterfactuals,
    elements::Elements,
    force_model::{Environment, ForceModel, Radiation, ThirdBody, Zonal},
    integrator::{IntegrationError, Options},
    perturb::drag::DragParameters,
    spline::Keyframe,
};
use odd_core::{epoch::Epoch, frames::State};
use odd_ephem::{Body, Ephemeris};
use odd_sim::{
    mission::metric_apoapsis,
    planner::{ManeuverNode, Plan},
    Engine,
};
const MU: f64 = 3.986_004_418e14;
const R: f64 = 6_378_137.0;
fn model(mass: f64) -> ForceModel {
    ForceModel {
        mu: MU,
        zonal: Some(Zonal {
            pole: [0.0, 0.0, 1.0],
            radius: R,
            j2: 1.082_626_68e-3,
        }),
        drag: Some(DragParameters {
            pole: [0.0, 0.0, 1.0],
            omega: [0.0, 0.0, 7.292115e-5],
            equatorial_radius: R,
            polar_radius: 6_356_752.314,
            cd: 2.2,
            area: 4.0,
            mass,
        }),
        radiation: Some(Radiation {
            cr: 1.3,
            area: 4.0,
            central_radius: R,
        }),
        third_body: true,
        burn: None,
    }
}
fn opts() -> Options {
    Options {
        relative_tolerance: 1e-10,
        absolute_tolerance: 1e-10,
        initial_step: 10.0,
        max_step: 120.0,
        max_steps: 1_000_000,
    }
}
fn env(eph: &Ephemeris, t: f64) -> Result<Environment, IntegrationError> {
    let epoch = Epoch::new(820_497_600, t).map_err(|_| IntegrationError::ForceFailure)?;
    let position = |body| {
        eph.state(body, Body::Earth, epoch)
            .map(|s| s.state.position_m)
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
fn node_angle(s: Keyframe) -> Result<f64, IntegrationError> {
    let el = Elements::from_state(
        State {
            position_m: s.position,
            velocity_m_s: s.velocity,
        },
        MU,
    )
    .map_err(|_| IntegrationError::ForceFailure)?;
    // This five-day fixture stays near zero; unwrap around the initial node.
    Ok(if el.ascending_node_rad > std::f64::consts::PI {
        el.ascending_node_rad - std::f64::consts::TAU
    } else {
        el.ascending_node_rad
    })
}
#[test]
fn sso_node_regression_is_dominated_by_j2_sensitivity() {
    let rp = R + 250_000.0;
    let ra = R + 800_000.0;
    let a = (rp + ra) / 2.0;
    let inc = 96.0 * std::f64::consts::PI / 180.0;
    let speed = libm::sqrt(MU * (2.0 / rp - 1.0 / a));
    let initial = Keyframe {
        time: 0.0,
        position: [rp, 0.0, 0.0],
        velocity: [0.0, speed * libm::cos(inc), speed * libm::sin(inc)],
        mass: 1200.0,
    };
    let engine = Engine {
        thrust_newtons: 2400.0,
        specific_impulse_seconds: 310.0,
        stage: 0,
        engine: 0,
    };
    let va = libm::sqrt(MU * (2.0 / ra - 1.0 / a));
    let vc = libm::sqrt(MU / ra);
    let delta = 2.6 * std::f64::consts::PI / 180.0;
    let time = std::f64::consts::PI * libm::sqrt(a * a * a / MU);
    let nodes = [ManeuverNode {
        epoch_us: libm::round(time * 1e6) as i64,
        tnw_m_s: [vc * libm::cos(delta) - va, 0.0, -vc * libm::sin(delta)],
    }];
    let plan = Plan::compile(initial, MU, 432_000_000_000, 1020.0, engine, &nodes).unwrap();
    let burns = plan.timeline.burns(&[engine]).unwrap();
    let eph = Ephemeris::bundled().unwrap();
    let runs = propagate_counterfactuals(
        initial,
        432_000.0,
        opts(),
        model(initial.mass),
        &burns,
        &plan.impulses,
        |t| env(&eph, t),
        node_angle,
    )
    .unwrap();
    let stats = runs.sensitivities().unwrap();
    let shares = stats.metric_shares.unwrap();
    assert!(
        shares[1] > 0.85,
        "nodal shares={shares:?}, deltas={:?}",
        stats.signed_metric_deltas
    );
    let drift = runs.full.metric * 180.0 / std::f64::consts::PI;
    assert!(
        (drift - 4.93).abs() < 0.15,
        "five-day drift {drift} degrees"
    );
}
#[test]
fn tug_apoapsis_sensitivity_is_dominated_by_burn_duration() {
    let r = R + 200_000.0;
    let initial = Keyframe {
        time: 0.0,
        position: [r, 0.0, 0.0],
        velocity: [0.0, libm::sqrt(MU / r), 0.0],
        mass: 18_000.0,
    };
    let engine = Engine {
        thrust_newtons: 18_000.0,
        specific_impulse_seconds: 450.0,
        stage: 0,
        engine: 0,
    };
    let nodes = [ManeuverNode {
        epoch_us: 1_600_000_000,
        tnw_m_s: [3120.0, 0.0, 0.0],
    }];
    let plan = Plan::compile(initial, MU, 20_000_000_000, 5200.0, engine, &nodes).unwrap();
    let burns = plan.timeline.burns(&[engine]).unwrap();
    let eph = Ephemeris::bundled().unwrap();
    let runs = propagate_counterfactuals(
        initial,
        20_000.0,
        opts(),
        model(initial.mass),
        &burns,
        &plan.impulses,
        |t| env(&eph, t),
        |s| metric_apoapsis(s, MU, R),
    )
    .unwrap();
    let stats = runs.sensitivities().unwrap();
    let shares = stats.metric_shares.unwrap();
    assert!(
        shares[0] > 0.7,
        "apoapsis shares={shares:?}, deltas={:?}",
        stats.signed_metric_deltas
    );
    assert!((shares.iter().sum::<f64>() - 1.0).abs() < 1e-12);
}
