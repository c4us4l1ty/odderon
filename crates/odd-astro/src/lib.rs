//! Analytical, two-body astrodynamics for preliminary mission design.
//!
//! These calculations are not a flight simulation: burns are instantaneous,
//! bodies are point masses, and perturbations and operational constraints are
//! absent. All inputs and outputs use SI units.

/// Invalid physical inputs, or results outside finite floating-point range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrbitError {
    /// A required positive finite value is zero, negative, NaN, or infinite.
    InvalidInput,
    /// The given elliptical orbit cannot reach the requested radius.
    RadiusOutsideEllipse,
    /// An intermediate or result is not representable as a finite f64.
    NumericalRange,
}

fn positive(value: f64) -> Result<(), OrbitError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(OrbitError::InvalidInput)
    }
}

fn finite(value: f64) -> Result<f64, OrbitError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(OrbitError::NumericalRange)
    }
}

/// Speed in m/s for a circular two-body orbit at radius `radius_m`.
/// `mu_m3_s2` is the central body's standard gravitational parameter.
pub fn circular_speed(mu_m3_s2: f64, radius_m: f64) -> Result<f64, OrbitError> {
    positive(mu_m3_s2)?;
    positive(radius_m)?;
    let speed = finite((mu_m3_s2 / radius_m).sqrt())?;
    if speed == 0.0 {
        return Err(OrbitError::NumericalRange);
    }
    Ok(speed)
}

/// Vis-viva speed in m/s for an ellipse with the supplied semi-major axis.
/// Radius is measured from the central body, not above its surface. This
/// function deliberately excludes hyperbolic and parabolic trajectories.
pub fn elliptical_speed(
    mu_m3_s2: f64,
    radius_m: f64,
    semi_major_axis_m: f64,
) -> Result<f64, OrbitError> {
    positive(mu_m3_s2)?;
    positive(radius_m)?;
    positive(semi_major_axis_m)?;
    // Divide before multiplying by 2 to avoid overflowing 2 * a.
    if radius_m / semi_major_axis_m >= 2.0 {
        return Err(OrbitError::RadiusOutsideEllipse);
    }
    let factor = 2.0 / radius_m - 1.0 / semi_major_axis_m;
    let squared_speed = finite(mu_m3_s2 * factor)?;
    if squared_speed <= 0.0 {
        return Err(OrbitError::RadiusOutsideEllipse);
    }
    finite(squared_speed.sqrt())
}

/// An ideal coplanar circular-to-circular Hohmann maneuver. Signed burn values
/// are along the direction of motion: positive is prograde, negative retrograde.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HohmannTransfer {
    pub transfer_semi_major_axis_m: f64,
    pub departure_delta_v_m_s: f64,
    pub arrival_delta_v_m_s: f64,
    pub total_delta_v_m_s: f64,
    pub coast_time_s: f64,
}

/// Calculates an ideal two-impulse Hohmann transfer about one point mass.
/// It assumes circular, coplanar, same-direction orbits and ignores gravity
/// losses, plane changes, atmosphere, perturbations, thrust duration and phasing.
/// For equal radii no transfer is needed; both burns and coast time are zero.
pub fn hohmann_transfer(
    mu_m3_s2: f64,
    initial_radius_m: f64,
    final_radius_m: f64,
) -> Result<HohmannTransfer, OrbitError> {
    positive(mu_m3_s2)?;
    positive(initial_radius_m)?;
    positive(final_radius_m)?;
    if initial_radius_m == final_radius_m {
        return Ok(HohmannTransfer {
            transfer_semi_major_axis_m: initial_radius_m,
            departure_delta_v_m_s: 0.0,
            arrival_delta_v_m_s: 0.0,
            total_delta_v_m_s: 0.0,
            coast_time_s: 0.0,
        });
    }
    let a = finite(initial_radius_m * 0.5 + final_radius_m * 0.5)?;
    let initial_speed = circular_speed(mu_m3_s2, initial_radius_m)?;
    let final_speed = circular_speed(mu_m3_s2, final_radius_m)?;
    let departure = finite(elliptical_speed(mu_m3_s2, initial_radius_m, a)? - initial_speed)?;
    let arrival = finite(final_speed - elliptical_speed(mu_m3_s2, final_radius_m, a)?)?;
    let total = finite(departure.abs() + arrival.abs())?;
    // sqrt(a^3 / mu) = a / sqrt(mu / a); avoids forming a^3.
    let coast = finite(std::f64::consts::PI * (a / circular_speed(mu_m3_s2, a)?))?;
    Ok(HohmannTransfer {
        transfer_semi_major_axis_m: a,
        departure_delta_v_m_s: departure,
        arrival_delta_v_m_s: arrival,
        total_delta_v_m_s: total,
        coast_time_s: coast,
    })
}
