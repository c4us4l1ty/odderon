//! Small building blocks for ideal patched-conic planning, not a trajectory
//! encounter finder. A patch must occur at one common epoch in parallel inertial
//! axes; the caller supplies the secondary body's position and velocity.
use crate::{circular_speed, elliptical_speed, finite, positive, OrbitError};
use odd_core::frames::State;
use std::f64::consts::PI;

/// Laplace sphere-of-influence radius, a (m/M)^(2/5), in meters.
/// This approximation assumes a small secondary orbiting a dominant primary;
/// it is not a sharp boundary in actual gravity. Both masses are in kilograms.
pub fn sphere_of_influence(
    semi_major_axis_m: f64,
    secondary_mass_kg: f64,
    primary_mass_kg: f64,
) -> Result<f64, OrbitError> {
    positive(semi_major_axis_m)?;
    positive(secondary_mass_kg)?;
    positive(primary_mass_kg)?;
    if secondary_mass_kg > primary_mass_kg {
        return Err(OrbitError::InvalidInput);
    }
    let ratio_power = (0.4 * (secondary_mass_kg.ln() - primary_mass_kg.ln())).exp();
    let radius = finite(semi_major_axis_m * ratio_power)?;
    if radius <= 0.0 {
        return Err(OrbitError::NumericalRange);
    }
    Ok(radius)
}

/// Transform an instantaneous primary-relative state to a secondary-relative
/// state. Both supplied states must share the same epoch and non-rotating axes.
/// This only changes origin: it does not find an SOI crossing, apply a maneuver,
/// or model either body's gravitational influence during an encounter.
pub fn patch_to_secondary(vehicle: State, secondary: State) -> Result<State, OrbitError> {
    if !vehicle
        .position_m
        .iter()
        .chain(&vehicle.velocity_m_s)
        .chain(&secondary.position_m)
        .chain(&secondary.velocity_m_s)
        .all(|v| v.is_finite())
    {
        return Err(OrbitError::InvalidInput);
    }
    let position_m = std::array::from_fn(|i| vehicle.position_m[i] - secondary.position_m[i]);
    let velocity_m_s = std::array::from_fn(|i| vehicle.velocity_m_s[i] - secondary.velocity_m_s[i]);
    if !position_m
        .iter()
        .chain(&velocity_m_s)
        .all(|v| v.is_finite())
    {
        return Err(OrbitError::NumericalRange);
    }
    Ok(State {
        position_m,
        velocity_m_s,
    })
}

/// Two half-ellipses joined at a common apoapsis, with three tangential
/// impulsive burns between circular, coplanar, same-direction orbits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BiEllipticTransfer {
    pub first_semi_major_axis_m: f64,
    pub second_semi_major_axis_m: f64,
    pub departure_delta_v_m_s: f64,
    pub intermediate_delta_v_m_s: f64,
    pub arrival_delta_v_m_s: f64,
    pub total_delta_v_m_s: f64,
    pub coast_time_s: f64,
}

/// Calculate a specified finite-apoapsis bi-elliptic transfer; does not choose
/// the optimum apoapsis, test body impact, or account for burn duration,
/// phasing, perturbations, or plane changes. The common apoapsis may equal
/// either endpoint; in that case the corresponding leg is a half-circle.
pub fn bi_elliptic_transfer(
    mu_m3_s2: f64,
    initial_radius_m: f64,
    final_radius_m: f64,
    apoapsis_radius_m: f64,
) -> Result<BiEllipticTransfer, OrbitError> {
    positive(mu_m3_s2)?;
    positive(initial_radius_m)?;
    positive(final_radius_m)?;
    positive(apoapsis_radius_m)?;
    if apoapsis_radius_m < initial_radius_m.max(final_radius_m) {
        return Err(OrbitError::InvalidInput);
    }
    let a1 = finite(initial_radius_m * 0.5 + apoapsis_radius_m * 0.5)?;
    let a2 = finite(final_radius_m * 0.5 + apoapsis_radius_m * 0.5)?;
    let departure = finite(
        elliptical_speed(mu_m3_s2, initial_radius_m, a1)?
            - circular_speed(mu_m3_s2, initial_radius_m)?,
    )?;
    let intermediate = finite(
        elliptical_speed(mu_m3_s2, apoapsis_radius_m, a2)?
            - elliptical_speed(mu_m3_s2, apoapsis_radius_m, a1)?,
    )?;
    let arrival = finite(
        circular_speed(mu_m3_s2, final_radius_m)? - elliptical_speed(mu_m3_s2, final_radius_m, a2)?,
    )?;
    let total = finite(departure.abs() + intermediate.abs() + arrival.abs())?;
    let coast =
        finite(PI * (a1 / circular_speed(mu_m3_s2, a1)? + a2 / circular_speed(mu_m3_s2, a2)?))?;
    Ok(BiEllipticTransfer {
        first_semi_major_axis_m: a1,
        second_semi_major_axis_m: a2,
        departure_delta_v_m_s: departure,
        intermediate_delta_v_m_s: intermediate,
        arrival_delta_v_m_s: arrival,
        total_delta_v_m_s: total,
        coast_time_s: coast,
    })
}
