//! End-to-end numerical physics fixture, independent of browser UI.
use odd_astro::{
    attribution::propagate_counterfactuals,
    force_model::{Environment, ForceModel, Radiation, ThirdBody, Zonal},
    hohmann_transfer,
    integrator::{IntegrationError, Options},
    perturb::drag::DragParameters,
    spline::Keyframe,
};
use odd_core::epoch::Epoch;
use odd_ephem::{Body, Ephemeris};
use odd_sim::{mission::{metric_apoapsis, score_rescue}, planner::{ManeuverNode, Plan}, Engine};
const MU: f64 = 3.986_004_418e14;
const EARTH: f64 = 6_378_137.0;
#[test]
fn rescue_hohmann_finite_burns_in_full_force_model() {
    let r = EARTH+260_000.0;
    let initial = Keyframe { time:0.0, position:[r,0.0,0.0], velocity:[0.0,libm::sqrt(MU/r),0.0], mass:1200.0 };
    let engine = Engine { thrust_newtons:2400.0, specific_impulse_seconds:310.0, stage:0, engine:0 };
    let h = hohmann_transfer(MU,r,EARTH+550_000.0).unwrap();
    let nodes = [ManeuverNode { epoch_us:200_000_000, tnw_m_s:[h.departure_delta_v_m_s,0.0,0.0] }, ManeuverNode { epoch_us:200_000_000+libm::round(h.coast_time_s*1e6) as i64, tnw_m_s:[h.arrival_delta_v_m_s,0.0,0.0] }];
    let plan = Plan::compile(initial,MU,10_800_000_000,1020.0,engine,&nodes).unwrap();
    let burns = plan.timeline.burns(&[engine]).unwrap();
    let model = ForceModel { mu: MU, zonal:Some(Zonal { pole:[0.0,0.0,1.0], radius:EARTH, j2:1.082_626_68e-3 }), drag:Some(DragParameters { pole:[0.0,0.0,1.0], omega:[0.0,0.0,7.292115e-5], equatorial_radius:EARTH, polar_radius:6_356_752.314, cd:2.2, area:4.0, mass:1200.0 }), radiation:Some(Radiation { cr:1.3, area:4.0, central_radius:EARTH }), third_body:true, burn:None };
    let eph = Ephemeris::bundled().unwrap();
    let environment = |t| -> Result<Environment, IntegrationError> {
        let epoch = Epoch::new(820_497_600,t).map_err(|_| IntegrationError::ForceFailure)?;
        let position = |body| eph.state(body,Body::Earth,epoch).map(|s| s.state.position_m).map_err(|_| IntegrationError::ForceFailure);
        Ok(Environment { sun:ThirdBody { position:position(Body::Sun)?, mu:1.327_124_400_412_794_2e20, radius:695_700_000.0 }, companion:ThirdBody { position:position(Body::Moon)?, mu:4.902_800_118e12, radius:1_737_400.0 } })
    };
    let opts=Options { relative_tolerance:1e-10, absolute_tolerance:1e-10, initial_step:10.0, max_step:60.0, max_steps:200_000 };
    let out=model.propagate_with_burns(initial,10_800.0,opts,&burns,environment).unwrap();
    let final_state=out.sample(10_800.0).unwrap();
    let score=score_rescue(final_state,1020.0,10_800.0,MU,EARTH).unwrap();
    assert!(score>0.8,"score={score}, final={final_state:?}");
    assert!((initial.mass-final_state.mass-plan.commanded_propellant_kg).abs()<1e-8);
    let runs=propagate_counterfactuals(initial,10_800.0,opts,model,&burns,&plan.impulses,environment,|s| metric_apoapsis(s,MU,EARTH)).unwrap();
    assert!((runs.full.position_m[0]-final_state.position[0]).abs()<1e-9);
    let stats=runs.sensitivities().unwrap();
    assert!((stats.metric_shares.unwrap().iter().sum::<f64>()-1.0).abs()<1e-12);
}
