use odd_astro::circular_speed;
use odd_astro::propagation::{velocity_verlet_step, PropagationError};
use odd_core::frames::State;

const MU: f64 = 3.986_004_418e14;
const RADIUS: f64 = 7_000_000.0;

fn initial_state() -> State {
    State {
        position_m: [RADIUS, 0.0, 0.0],
        velocity_m_s: [0.0, circular_speed(MU, RADIUS).unwrap(), 0.0],
    }
}

fn integrate(mut state: State, dt: f64, steps: usize) -> State {
    for _ in 0..steps {
        state = velocity_verlet_step(state, MU, dt).unwrap();
    }
    state
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[test]
fn circular_orbit_tracks_independent_two_body_solution_and_converges() {
    // Circular orbit analytic quarter period: x=0, y=r, v=(-sqrt(mu/r),0,0).
    // Kepler's third law, https://science.nasa.gov/solar-system/orbits-and-keplers-laws/
    let start = initial_state();
    let speed = start.velocity_m_s[1];
    let quarter_period = std::f64::consts::FRAC_PI_2 * RADIUS / speed;
    let coarse = integrate(start, quarter_period / 128.0, 128);
    let fine = integrate(start, quarter_period / 256.0, 256);
    let expected = [0.0, RADIUS, 0.0];
    let coarse_error = distance(coarse.position_m, expected);
    let fine_error = distance(fine.position_m, expected);
    assert!(fine_error < 1_000.0, "quarter-orbit error {fine_error} m");
    assert!(coarse_error / fine_error > 3.8 && coarse_error / fine_error < 4.2);
    assert!(distance(fine.velocity_m_s, [-speed, 0.0, 0.0]) < 1.0);
}

#[test]
fn circular_orbit_energy_and_reversibility() {
    let start = initial_state();
    let period = std::f64::consts::TAU * RADIUS / start.velocity_m_s[1];
    let dt = period / 1024.0;
    let end = integrate(start, dt, 1024 * 5);
    let specific_energy = |s: State| {
        let r = (s.position_m.iter().map(|x| x * x).sum::<f64>()).sqrt();
        0.5 * s.velocity_m_s.iter().map(|v| v * v).sum::<f64>() - MU / r
    };
    let relative_error =
        ((specific_energy(end) - specific_energy(start)) / specific_energy(start)).abs();
    assert!(
        relative_error < 1e-8,
        "relative energy error {relative_error}"
    );
    let back = integrate(end, -dt, 1024 * 5);
    assert!(distance(back.position_m, start.position_m) < 1e-3);
    assert!(distance(back.velocity_m_s, start.velocity_m_s) < 1e-6);
}

#[test]
fn rejects_bad_inputs_and_singularity() {
    let zero_position = State {
        position_m: [0.0; 3],
        velocity_m_s: [1.0, 0.0, 0.0],
    };
    assert_eq!(
        velocity_verlet_step(zero_position, MU, 1.0),
        Err(PropagationError::CentralSingularity)
    );
    assert_eq!(
        velocity_verlet_step(initial_state(), MU, 0.0),
        Err(PropagationError::InvalidInput)
    );
    assert_eq!(
        velocity_verlet_step(initial_state(), -1.0, 1.0),
        Err(PropagationError::InvalidInput)
    );
    assert_eq!(
        velocity_verlet_step(initial_state(), MU, f64::NAN),
        Err(PropagationError::InvalidInput)
    );
    assert_eq!(
        velocity_verlet_step(initial_state(), MU, f64::MAX),
        Err(PropagationError::NumericalRange)
    );
}

#[test]
fn hohmann_prediction_matches_two_body_propagation() {
    // Compare independently implemented analytical and numerical models, not
    // a claim of agreement with a real-world perturbed mission.
    let target = 20_000_000.0;
    let plan = odd_astro::hohmann_transfer(MU, RADIUS, target).unwrap();
    let mut spacecraft = initial_state();
    spacecraft.velocity_m_s[1] += plan.departure_delta_v_m_s;
    let arrival = integrate(spacecraft, plan.coast_time_s / 4096.0, 4096);
    assert!(distance(arrival.position_m, [-target, 0.0, 0.0]) < 100.0);
    let target_speed = circular_speed(MU, target).unwrap();
    // At apoapsis velocity is toward -y, and the prograde arrival burn
    // increases its magnitude to the circular value.
    assert!((arrival.velocity_m_s[1] - plan.arrival_delta_v_m_s + target_speed).abs() < 0.1);
}
