use odd_astro::{
    attribution::propagate_counterfactuals,
    elements::Elements,
    force_model::{Environment, ForceModel, Zonal},
    integrator::{IntegrationError, Options},
    spline::Keyframe,
};
use odd_core::frames::State;

#[test]
fn j2_counterfactual_nodal_sensitivity_is_computed_from_actual_propagations() {
    let mu = 3.986_004_418e14;
    let radius = 7_178_137.0;
    let inc = 98.6 * std::f64::consts::PI / 180.0;
    let speed = libm::sqrt(mu / radius);
    let initial = Keyframe { time: 0.0, position: [radius,0.0,0.0], velocity: [0.0,speed*libm::cos(inc),speed*libm::sin(inc)], mass: 500.0 };
    let full = ForceModel { mu, zonal: Some(Zonal { pole: [0.0,0.0,1.0], radius: 6_378_137.0, j2: 1.082_626_68e-3 }), drag: None, radiation: None, third_body: false, burn: None };
    let opts = Options { relative_tolerance: 1e-10, absolute_tolerance: 1e-10, initial_step: 10.0, max_step: 120.0, max_steps: 100_000 };
    let data = propagate_counterfactuals(initial, 86400.0, opts, full, &[], &[], |_: f64| -> Result<Environment,IntegrationError> { Err(IntegrationError::ForceFailure) }, |s| {
        let el = Elements::from_state(State { position_m: s.position, velocity_m_s: s.velocity }, mu).map_err(|_| IntegrationError::ForceFailure)?;
        Ok(el.ascending_node_rad)
    }).unwrap();
    let result = data.sensitivities().unwrap();
    assert!(result.signed_metric_deltas[1] > 0.0);
    assert!(result.metric_shares.unwrap()[1] > 0.999_999);
    assert!(result.position_distances_m[1] > 0.0);
}
