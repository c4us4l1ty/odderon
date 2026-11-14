//! Independent model comparison: point Earth and fixed inertial burn directions.
//! This is not yet the playable lunar contract or a lunar encounter simulation.
use odd_astro::{
    elements::Elements,
    force_model::{Environment, ForceModel},
    integrator::{IntegrationError, Options},
    spline::Keyframe,
};
use odd_core::frames::State;
use odd_sim::{compile_burn, CommandTimeline, Engine};

const MU: f64 = 3.986_004_418e14;
const R: f64 = 6_578_137.0;

fn engine() -> Engine {
    Engine {
        thrust_newtons: 18_000.0,
        specific_impulse_seconds: 450.0,
        stage: 0,
        engine: 0,
    }
}
fn model() -> ForceModel {
    ForceModel {
        mu: MU,
        zonal: None,
        drag: None,
        radiation: None,
        third_body: false,
        burn: None,
    }
}
fn options() -> Options {
    Options {
        relative_tolerance: 1e-11,
        absolute_tolerance: 1e-11,
        initial_step: 10.0,
        max_step: 60.0,
        max_steps: 100_000,
    }
}
fn no_environment(_: f64) -> Result<Environment, IntegrationError> {
    Err(IntegrationError::ForceFailure)
}
fn node() -> State {
    State {
        position_m: [R, 0.0, 0.0],
        velocity_m_s: [0.0, libm::sqrt(MU / R), 0.0],
    }
}
fn initial(time: f64) -> Keyframe {
    let speed = libm::sqrt(MU / R);
    let (s, c) = libm::sincos(time * speed / R);
    Keyframe {
        time,
        position: [R * c, R * s, 0.0],
        velocity: [-speed * s, speed * c, 0.0],
        mass: 18_000.0,
    }
}
fn run_burn(state: Keyframe, plan: odd_sim::CompiledBurn) -> Keyframe {
    let mut timeline = CommandTimeline::default();
    timeline.insert(plan.command).unwrap();
    let burns = timeline.burns(&[engine()]).unwrap();
    model()
        .propagate_with_burns(state, burns[0].end, options(), &burns, no_environment)
        .unwrap()
        .sample(burns[0].end)
        .unwrap()
}
fn apocenter(state: Keyframe) -> f64 {
    let el = Elements::from_state(
        State {
            position_m: state.position,
            velocity_m_s: state.velocity,
        },
        MU,
    )
    .unwrap();
    assert!(el.eccentricity < 1.0);
    el.semi_major_axis_m().unwrap() * (1.0 + el.eccentricity)
}
fn radial(s: Keyframe) -> f64 {
    s.position.iter().zip(s.velocity).map(|(r, v)| r * v).sum()
}
#[test]
fn two_equal_kicks_improve_point_earth_apoapsis_by_40000_km() {
    let one = compile_burn(node(), [3120.0, 0.0, 0.0], 0, 18_000.0, engine()).unwrap();
    let single = run_burn(initial(one.command.t_us as f64 / 1e6), one);
    let first = compile_burn(node(), [1560.0, 0.0, 0.0], 0, 18_000.0, engine()).unwrap();
    let after_first = run_burn(initial(first.command.t_us as f64 / 1e6), first);
    let coast = model()
        .propagate(after_first, 20_000.0, options(), no_environment)
        .unwrap();
    let mut bracket = None;
    for i in 1..coast.len() {
        let a = coast.keyframe(i - 1).unwrap();
        let b = coast.keyframe(i).unwrap();
        if radial(a) < 0.0 && radial(b) >= 0.0 {
            bracket = Some((a.time, b.time));
            break;
        }
    }
    let (mut lo, mut hi) = bracket.expect("next periapsis");
    for _ in 0..50 {
        let mid = (lo + hi) * 0.5;
        if radial(coast.sample(mid).unwrap()) < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let next_node = coast.sample((lo + hi) * 0.5).unwrap();
    let second = compile_burn(
        State {
            position_m: next_node.position,
            velocity_m_s: next_node.velocity,
        },
        [1560.0, 0.0, 0.0],
        libm::round(next_node.time * 1e6) as i64,
        next_node.mass,
        engine(),
    )
    .unwrap();
    let second_start = coast.sample(second.command.t_us as f64 / 1e6).unwrap();
    let split = run_burn(second_start, second);
    let (a, b) = (apocenter(single), apocenter(split));
    assert!(b - a > 40_000_000.0, "single={a}, split={b}");
    assert!(
        (single.mass - split.mass).abs() < 1e-5,
        "single={}, split={}",
        single.mass,
        split.mass
    );
    assert!((a / 1000.0 - 41_044.0).abs() < 100.0);
    assert!((b / 1000.0 - 109_315.0).abs() < 500.0);
}
