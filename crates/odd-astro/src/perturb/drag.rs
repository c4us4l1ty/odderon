//! Earth reference atmosphere and drag. The seven-layer USSA76 lower
//! atmosphere ends at 86 km geometric altitude, not 1000 km. Above that,
//! this module uses a fixed exponential reference-density table, NOT the
//! full USSA76 upper-atmosphere physics or a space-weather model. High-altitude
//! drag estimates are illustrative and can differ greatly from reality.
use super::{dot, norm, PerturbationError};

/// Approximate density in kg/m³, using geometric altitude in meters.
/// Returns zero above 1000 km and rejects negative altitude.
pub fn earth_density(height: f64) -> Result<f64, PerturbationError> {
    if !height.is_finite() || height < 0.0 {
        return Err(PerturbationError::InvalidInput);
    }
    if height > 1_000_000.0 {
        return Ok(0.0);
    }
    if height >= 86_000.0 {
        // Reference exponential thermosphere, geometric altitude (m), base
        // density (kg/m³), scale height (m). This is not a time-varying model.
        const TABLE: [(f64, f64, f64); 15] = [
            (86e3, 6.957e-6, 5.799e3),
            (100e3, 5.297e-7, 5.877e3),
            (110e3, 9.661e-8, 7.263e3),
            (120e3, 2.438e-8, 9.473e3),
            (130e3, 8.484e-9, 12.636e3),
            (140e3, 3.845e-9, 16.149e3),
            (150e3, 2.070e-9, 22.523e3),
            (180e3, 5.464e-10, 29.740e3),
            (200e3, 2.789e-10, 37.105e3),
            (250e3, 7.248e-11, 45.546e3),
            (300e3, 2.418e-11, 53.628e3),
            (350e3, 9.518e-12, 53.298e3),
            (400e3, 3.725e-12, 58.515e3),
            (500e3, 6.967e-13, 63.822e3),
            (600e3, 1.454e-13, 71.835e3),
        ];
        const TOP: [(f64, f64, f64); 4] = [
            (700e3, 3.614e-14, 88.667e3),
            (800e3, 1.170e-14, 124.640e3),
            (900e3, 5.245e-15, 181.05e3),
            (1000e3, 3.019e-15, 268e3),
        ];
        let mut layer = TABLE[0];
        for row in TABLE.into_iter().chain(TOP) {
            if height >= row.0 {
                layer = row;
            }
        }
        return Ok(layer.1 * libm::exp(-(height - layer.0) / layer.2));
    }
    // USSA76 standard geopotential layers: base height, temperature,
    // pressure, temperature lapse rate. SI values.
    const H: [f64; 7] = [0.0, 11e3, 20e3, 32e3, 47e3, 51e3, 71e3];
    const T: [f64; 7] = [288.15, 216.65, 216.65, 228.65, 270.65, 270.65, 214.65];
    const P: [f64; 7] = [
        101325.0, 22632.06, 5474.889, 868.0187, 110.9063, 66.93887, 3.956420,
    ];
    const L: [f64; 7] = [-0.0065, 0.0, 0.001, 0.0028, 0.0, -0.0028, -0.002];
    let h = 6_356_766.0 * height / (6_356_766.0 + height);
    let mut i = 0;
    for (j, &base) in H.iter().enumerate().skip(1) {
        if h >= base {
            i = j;
        }
    }
    let dt = h - H[i];
    let temp = T[i] + L[i] * dt;
    let pressure = if L[i] == 0.0 {
        P[i] * libm::exp(-9.80665 * dt / (287.05287 * T[i]))
    } else {
        P[i] * libm::pow(T[i] / temp, 9.80665 / (287.05287 * L[i]))
    };
    Ok(pressure / (287.05287 * temp))
}

/// Parameters of the reference body and spacecraft, in SI units.
#[derive(Clone, Copy, Debug)]
pub struct DragParameters {
    pub pole: [f64; 3],
    pub omega: [f64; 3],
    pub equatorial_radius: f64,
    pub polar_radius: f64,
    pub cd: f64,
    pub area: f64,
    pub mass: f64,
}

/// Drag for an oblate body whose symmetry axis is `pole`. Altitude is the
/// geocentric radial distance above the ellipsoid in that direction, not
/// terrain or geodetic height. `omega` is inertial angular velocity (rad/s).
pub fn acceleration(
    r: [f64; 3],
    v: [f64; 3],
    params: DragParameters,
) -> Result<[f64; 3], PerturbationError> {
    let DragParameters {
        pole,
        omega,
        equatorial_radius,
        polar_radius,
        cd,
        area,
        mass,
    } = params;
    if !r
        .iter()
        .chain(v.iter())
        .chain(pole.iter())
        .chain(omega.iter())
        .all(|x| x.is_finite())
        || ![equatorial_radius, polar_radius, mass]
            .iter()
            .all(|x| x.is_finite() && *x > 0.0)
        || ![cd, area].iter().all(|x| x.is_finite() && *x >= 0.0)
    {
        return Err(PerturbationError::InvalidInput);
    }
    let distance = norm(r);
    let pole_length = norm(pole);
    if distance <= 0.0 || pole_length <= 0.0 || !distance.is_finite() || !pole_length.is_finite() {
        return Err(PerturbationError::InvalidInput);
    }
    let sine = dot(r, pole) / (distance * pole_length);
    let surface = 1.0
        / libm::sqrt(
            (1.0 - sine * sine) / (equatorial_radius * equatorial_radius)
                + sine * sine / (polar_radius * polar_radius),
        );
    let rho = earth_density(distance - surface)?;
    let rotation = [
        omega[1] * r[2] - omega[2] * r[1],
        omega[2] * r[0] - omega[0] * r[2],
        omega[0] * r[1] - omega[1] * r[0],
    ];
    let relative = std::array::from_fn(|i| v[i] - rotation[i]);
    let speed = norm(relative);
    let factor = -0.5 * rho * speed * cd * area / mass;
    let result = relative.map(|x| factor * x);
    if result.iter().all(|x| x.is_finite()) {
        Ok(result)
    } else {
        Err(PerturbationError::NumericalRange)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reference_densities_and_corotation() {
        assert!((earth_density(0.0).unwrap() - 1.225).abs() < 1e-4);
        assert!((earth_density(300e3).unwrap() / 2.418e-11 - 1.0).abs() < 1e-14);
        assert_eq!(earth_density(1_000_001.0).unwrap(), 0.0);
        let a = acceleration(
            [6_678_137.0, 0.0, 0.0],
            [0.0, 486.9, 0.0],
            DragParameters {
                pole: [0.0, 0.0, 1.0],
                omega: [0.0, 0.0, 486.9 / 6_678_137.0],
                equatorial_radius: 6_378_137.0,
                polar_radius: 6_356_752.314,
                cd: 2.2,
                area: 4.0,
                mass: 500.0,
            },
        )
        .unwrap();
        assert!(norm(a) < 1e-25);
    }
}
