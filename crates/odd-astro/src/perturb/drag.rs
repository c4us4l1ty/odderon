//! Earth reference atmosphere and drag. The seven-layer USSA76 lower
//! atmosphere ends at 86 km geometric altitude, not 1000 km. Above that,
//! this module uses published piecewise polynomial fits to the USSA76 upper
//! density tables, not the full species model or a space-weather model.
//! High-altitude density varies greatly with solar activity.
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
        // Braeunig Table 8, polynomial fits to USSA76 density tables. Heights
        // are geometric km, coefficients approximate the table (~4 digits).
        const FIT: [(f64, [f64; 5]); 10] = [
            (86.0, [0.0, -3.322622e-6, 9.111460e-4, -0.2609971, 5.944694]),
            (91.0, [0.0, 2.873405e-5, -0.008492037, 0.6541179, -23.62010]),
            (
                100.0,
                [-1.240774e-5, 0.005162063, -0.8048342, 55.55996, -1443.338],
            ),
            (110.0, [0.0, -8.854164e-5, 0.03373254, -4.390837, 176.5294]),
            (
                120.0,
                [3.661771e-7, -2.154344e-4, 0.04809214, -4.884744, 172.3597],
            ),
            (
                150.0,
                [1.906032e-8, -1.527799e-5, 0.004724294, -0.6992340, 20.50921],
            ),
            (
                200.0,
                [
                    1.199282e-9,
                    -1.451051e-6,
                    6.910474e-4,
                    -0.1736220,
                    -5.321644,
                ],
            ),
            (
                300.0,
                [
                    1.140564e-10,
                    -2.130756e-7,
                    1.570762e-4,
                    -0.07029296,
                    -12.89844,
                ],
            ),
            (
                500.0,
                [
                    8.105631e-12,
                    -2.358417e-9,
                    -2.635110e-6,
                    -0.01562608,
                    -20.02246,
                ],
            ),
            (
                750.0,
                [
                    -3.701195e-12,
                    -8.608611e-9,
                    5.118829e-5,
                    -0.06600998,
                    -6.137674,
                ],
            ),
        ];
        let z = height / 1000.0;
        let mut coeff = FIT[0].1;
        for (base, values) in FIT {
            if z >= base {
                coeff = values;
            }
        }
        let [a, b, c, d, e] = coeff;
        return Ok(libm::exp((((a * z + b) * z + c) * z + d) * z + e));
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

/// Radial distance to the reference ellipsoid along `r`, in meters.
/// This is a geometric reference surface, not terrain or geodetic height.
pub fn surface_radius(
    r: [f64; 3],
    pole: [f64; 3],
    equatorial_radius: f64,
    polar_radius: f64,
) -> Result<f64, PerturbationError> {
    if !r.iter().chain(pole.iter()).all(|x| x.is_finite())
        || ![equatorial_radius, polar_radius]
            .iter()
            .all(|x| x.is_finite() && *x > 0.0)
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
    if surface.is_finite() && surface > 0.0 {
        Ok(surface)
    } else {
        Err(PerturbationError::NumericalRange)
    }
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
    let surface = surface_radius(r, pole, equatorial_radius, polar_radius)?;
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
        assert!((earth_density(300e3).unwrap() / 1.915_12e-11 - 1.0).abs() < 1e-5);
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
