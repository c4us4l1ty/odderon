use odd_astro::{
    elements::Elements,
    force_model::{Environment, ForceModel},
    integrator::{IntegrationError, Options},
    maneuver::apply_impulse,
    spline::Keyframe,
};
use odd_core::frames::State;
use odd_sim::{compile_burn, CommandKind, CommandTimeline, Engine};

#[test]
fn high_thrust_prograde_and_normal_approach_impulsive_elements() {
    let mu = 3.986_004_418e14;
    let r = 6_578_137.0;
    let speed = libm::sqrt(mu / r);
    let node = State { position_m: [r,0.0,0.0], velocity_m_s: [0.0,speed,0.0] };
    let engine = Engine { thrust_newtons: 1e6, specific_impulse_seconds: 450.0, stage: 0, engine: 0 };
    let model = ForceModel { mu, zonal: None, drag: None, radiation: None, third_body: false, burn: None };
    for dv in [[100.0,0.0,0.0],[0.0,0.0,100.0]] {
        let compiled = compile_burn(node, dv, 1_000_000, 1000.0, engine).unwrap();
        let CommandKind::Burn { dur_us, .. } = compiled.command.kind;
        let mut timeline = CommandTimeline::default();
        timeline.insert(compiled.command).unwrap();
        let burns = timeline.burns(&[engine]).unwrap();
        let start = burns[0].start;
        let end = burns[0].end;
        let theta = speed / r * (start - 1.0);
        let (s,c) = libm::sincos(theta);
        let initial = Keyframe { time: start, position: [r*c,r*s,0.0], velocity: [-speed*s,speed*c,0.0], mass: 1000.0 };
        let options = Options { relative_tolerance: 1e-12, absolute_tolerance: 1e-12, initial_step: 0.01, max_step: 1.0, max_steps: 100_000 };
        let result = model.propagate_with_burns(initial, end, options, &burns, |_: f64| -> Result<Environment,IntegrationError> { Err(IntegrationError::ForceFailure) }).unwrap().sample(end).unwrap();
        let finite = Elements::from_state(State { position_m: result.position, velocity_m_s: result.velocity }, mu).unwrap();
        let ideal = Elements::from_state(apply_impulse(node,dv).unwrap(),mu).unwrap();
        assert!((finite.semi_major_axis_m().unwrap() / ideal.semi_major_axis_m().unwrap() - 1.0).abs() < 1e-4);
        assert!((finite.eccentricity / ideal.eccentricity - 1.0).abs() < 1e-4);
        if dv[2] != 0.0 { assert!((finite.inclination_rad / ideal.inclination_rad - 1.0).abs() < 1e-4); }
        assert!((result.mass - (1000.0 - compiled.commanded_propellant_kg)).abs() < 1e-9);
        assert!((compiled.ideal_propellant_kg - 1000.0*(1.0-libm::exp(-100.0/(450.0*9.80665)))).abs() < 1e-9);
        assert!(dur_us > 0);
    }
}
