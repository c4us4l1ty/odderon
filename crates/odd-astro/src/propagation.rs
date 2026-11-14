//! Fixed-step velocity-Verlet propagation under a central point-mass gravity.
//!
//! This is a level-2 *two-body* numerical model, not a high-fidelity flight
//! model. The caller selects a time step, which must be tested for convergence.
//! Symplectic does not mean exact: energy oscillates and phase error accumulates.
//! No impact detection, atmosphere, thrust, or other bodies are modeled.

use odd_core::frames::State;

/// Invalid physical inputs, a central singularity, or floating-point overflow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropagationError {
    InvalidInput,
    CentralSingularity,
    NumericalRange,
}

/// Propagate one fixed velocity-Verlet step, in either direction in time.
///
/// `state` is measured relative to an inertially fixed central body; μ is its
/// gravitational parameter in m³/s² and `step_s` is a nonzero SI duration.
/// Position and velocity are evaluated at the same epoch. Returns an error
/// rather than an invalid state. The model does not detect surface crossings
/// or singularities occurring *between* step endpoints.
pub fn velocity_verlet_step(
    state: State,
    mu_m3_s2: f64,
    step_s: f64,
) -> Result<State, PropagationError> {
    if !mu_m3_s2.is_finite()
        || mu_m3_s2 <= 0.0
        || !step_s.is_finite()
        || step_s == 0.0
        || !state
            .position_m
            .iter()
            .chain(&state.velocity_m_s)
            .all(|v| v.is_finite())
    {
        return Err(PropagationError::InvalidInput);
    }
    let acceleration = gravity(state.position_m, mu_m3_s2)?;
    // Kick-drift-kick form avoids explicitly forming dt².
    let half_velocity: [f64; 3] =
        std::array::from_fn(|i| state.velocity_m_s[i] + 0.5 * step_s * acceleration[i]);
    let position: [f64; 3] =
        std::array::from_fn(|i| state.position_m[i] + step_s * half_velocity[i]);
    if !half_velocity.iter().chain(&position).all(|v| v.is_finite()) {
        return Err(PropagationError::NumericalRange);
    }
    let new_acceleration = gravity(position, mu_m3_s2)?;
    let velocity: [f64; 3] =
        std::array::from_fn(|i| half_velocity[i] + 0.5 * step_s * new_acceleration[i]);
    if !velocity.iter().all(|v| v.is_finite()) {
        return Err(PropagationError::NumericalRange);
    }
    Ok(State {
        position_m: position,
        velocity_m_s: velocity,
    })
}

fn gravity(position_m: [f64; 3], mu_m3_s2: f64) -> Result<[f64; 3], PropagationError> {
    let scale = position_m
        .iter()
        .fold(0.0_f64, |max, value| max.max(value.abs()));
    if scale == 0.0 {
        return Err(PropagationError::CentralSingularity);
    }
    let unit_scaled = position_m.map(|x| x / scale);
    let scaled_norm = unit_scaled.iter().map(|x| x * x).sum::<f64>().sqrt();
    let radius = scale * scaled_norm;
    if !radius.is_finite() || radius == 0.0 {
        return Err(PropagationError::NumericalRange);
    }
    let magnitude = (mu_m3_s2 / radius) / radius;
    if !magnitude.is_finite() || magnitude == 0.0 {
        return Err(PropagationError::NumericalRange);
    }
    Ok(unit_scaled.map(|x| -magnitude * (x / scaled_norm)))
}
