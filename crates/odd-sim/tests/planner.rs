use odd_astro::{elements::Elements, hohmann_transfer, spline::Keyframe};
use odd_core::frames::State;
use odd_sim::{planner::{kepler_coast, ManeuverNode, Plan, PlanError}, Engine};
const MU: f64 = 3.986_004_418e14;
const R: f64 = 6_638_137.0;
fn engine() -> Engine { Engine { thrust_newtons: 2400.0, specific_impulse_seconds: 310.0, stage: 0, engine: 0 } }
fn initial() -> Keyframe { Keyframe { time: 0.0, position: [R,0.0,0.0], velocity: [0.0,libm::sqrt(MU/R),0.0], mass: 1200.0 } }
#[test]
fn analytical_two_node_hohmann_and_quantized_timeline() {
    let transfer = hohmann_transfer(MU,R,6_928_137.0).unwrap();
    let first = 200_000_000_i64;
    let second = first + libm::round(transfer.coast_time_s*1e6) as i64;
    let end = second + 200_000_000;
    let nodes = [ManeuverNode { epoch_us:first, tnw_m_s:[transfer.departure_delta_v_m_s,0.0,0.0] }, ManeuverNode { epoch_us:second, tnw_m_s:[transfer.arrival_delta_v_m_s,0.0,0.0] }];
    let plan = Plan::compile(initial(),MU,end,1020.0,engine(),&nodes).unwrap();
    assert_eq!(plan.timeline.commands().len(),2);
    assert_eq!(plan.timeline.burns(&[engine()]).unwrap().len(),2);
    let final_state = plan.sample(end).unwrap();
    let el = Elements::from_state(State { position_m: final_state.position, velocity_m_s: final_state.velocity }, MU).unwrap();
    assert!((el.semi_major_axis_m().unwrap()-6_928_137.0).abs()<0.01);
    assert!(el.eccentricity < 1e-8);
    assert!((final_state.mass-(1200.0-plan.commanded_propellant_kg)).abs()<1e-9);
    let before = plan.sample(first-1).unwrap();
    let after = plan.sample(first).unwrap();
    assert!(before.mass > after.mass);
    assert!(matches!(Plan::compile(initial(),MU,end,1199.9,engine(),&nodes),Err(PlanError::PropellantDeficit)));
}
#[test]
fn analytical_coast_returns_after_period_and_supports_hyperbolas() {
    let state = State { position_m: initial().position, velocity_m_s: initial().velocity };
    let period = std::f64::consts::TAU*R/libm::sqrt(MU/R);
    let result = kepler_coast(state,MU,period).unwrap();
    for i in 0..3 { assert!((state.position_m[i]-result.position_m[i]).abs()<1e-6); }
    let escape = State { velocity_m_s:[0.0,12000.0,0.0], ..state };
    let future = kepler_coast(escape,MU,1000.0).unwrap();
    let restored = kepler_coast(future,MU,-1000.0).unwrap();
    for i in 0..3 { assert!((state.position_m[i]-restored.position_m[i]).abs()<1e-5); }
}
