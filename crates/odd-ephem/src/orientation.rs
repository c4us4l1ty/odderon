//! Body orientation for the Sun, Moon and Earth.
//!
//! The Sun and Moon pole/prime-meridian series are from NAIF pck00011.tpc
//! (2022), IAU 2015/2009 respectively. The Moon series describes IAU_MOON,
//! not the higher-accuracy DE440 lunar principal-axes or mean-Earth frames.
//!
//! Earth orientation is offered at two fidelities:
//!   - `earth_approx_orientation`: IAU 2009 mean model, no UT1 or polar motion.
//!     Suitable for visualization; NAIF warns its prime-meridian error is at
//!     least 150 arcseconds.
//!   - `earth_orientation`: full ITRF-to-ICRF transformation using measured
//!     Earth Orientation Parameters (UT1-UTC and polar motion x_pole, y_pole).
//!     Callers supply EOP values from IERS bulletins; this crate does not ship
//!     EOP data.

use odd_core::epoch::Epoch;
use std::f64::consts::{PI, TAU};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrientationError {
    OutsideModelRange,
    NonFinite,
}

/// Earth Orientation Parameters for a given epoch.
///
/// Callers obtain UT1-UTC, x_pole and y_pole from IERS Bulletins A/B and
/// supply them here. Values are in seconds and radians respectively.
/// When all fields are zero, `earth_orientation` reduces to the mean model
/// (equivalent to `earth_approx_orientation` within numerical noise).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EarthOrientationParams {
    /// UT1 minus UTC, in seconds. Positive means UT1 runs ahead of UTC.
    pub ut1_minus_utc_s: f64,
    /// Polar motion x-coordinate (toward the Greenwich meridian), radians.
    pub x_pole_rad: f64,
    /// Polar motion y-coordinate (toward 90° west longitude), radians.
    pub y_pole_rad: f64,
}

impl Default for EarthOrientationParams {
    fn default() -> Self {
        Self {
            ut1_minus_utc_s: 0.0,
            x_pole_rad: 0.0,
            y_pole_rad: 0.0,
        }
    }
}

/// Approximate IAU orientation, with columns equal to the body-fixed unit
/// axes expressed in ICRF-aligned SPICE J2000 coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyOrientation {
    pub pole_ra_rad: f64,
    pub pole_dec_rad: f64,
    pub prime_meridian_rad: f64,
    pub body_to_j2000: [[f64; 3]; 3],
}

const MOON_ANGLES: [(f64, f64); 13] = [
    (125.045, -0.0529921),
    (250.089, -0.1059842),
    (260.008, 13.0120009),
    (176.625, 13.3407154),
    (357.529, 0.9856003),
    (311.589, 26.4057084),
    (134.963, 13.0649930),
    (276.617, 0.3287146),
    (34.226, 1.7484877),
    (15.134, -0.1589763),
    (119.743, 0.0036096),
    (239.961, 0.1643573),
    (25.053, 12.9590088),
];
const MOON_RA: [f64; 13] = [
    -3.8787, -0.1204, 0.0700, -0.0172, 0.0, 0.0072, 0.0, 0.0, 0.0, -0.0052, 0.0, 0.0, 0.0043,
];
const MOON_DEC: [f64; 13] = [
    1.5419, 0.0239, -0.0278, 0.0068, 0.0, -0.0029, 0.0009, 0.0, 0.0, 0.0008, 0.0, 0.0, -0.0009,
];
const MOON_W: [f64; 13] = [
    3.5610, 0.1208, -0.0642, 0.0158, 0.0252, -0.0066, -0.0047, -0.0046, 0.0028, 0.0052, 0.0040,
    0.0019, -0.0044,
];

fn bounded_days(epoch: Epoch) -> Result<f64, OrientationError> {
    // Bound the nominal model to the shipped ephemeris, rather than implying
    // that its rotational elements remain accurate indefinitely.
    if !(631_108_800..1_136_030_400).contains(&epoch.seconds()) {
        return Err(OrientationError::OutsideModelRange);
    }
    Ok(epoch.seconds() as f64 / 86_400.0 + epoch.fraction() / 86_400.0)
}
fn orientation(ra_deg: f64, dec_deg: f64, w_deg: f64) -> BodyOrientation {
    let ra = ra_deg.to_radians().rem_euclid(TAU);
    let dec = dec_deg.to_radians();
    let w = w_deg.rem_euclid(360.0).to_radians();
    let (sa, ca) = (libm::sin(ra), libm::cos(ra));
    let (sd, cd) = (libm::sin(dec), libm::cos(dec));
    let (sw, cw) = (libm::sin(w), libm::cos(w));
    let node = [-sa, ca, 0.0];
    let meridian = [-sd * ca, -sd * sa, cd];
    let pole = [cd * ca, cd * sa, sd];
    BodyOrientation {
        pole_ra_rad: ra,
        pole_dec_rad: dec,
        prime_meridian_rad: w,
        body_to_j2000: std::array::from_fn(|i| {
            [
                cw * node[i] + sw * meridian[i],
                -sw * node[i] + cw * meridian[i],
                pole[i],
            ]
        }),
    }
}
/// IAU_SUN orientation for epochs within the bundled ephemeris coverage.
/// The adopted prime meridian is a conventional rigid rotation; the solar
/// surface rotates differentially and is not a solid body.
pub fn sun_orientation(epoch: Epoch) -> Result<BodyOrientation, OrientationError> {
    let d = bounded_days(epoch)?;
    Ok(orientation(286.13, 63.87, 84.176 + 14.18440 * d))
}
/// Approximate IAU_EARTH orientation from the 2009 model in NAIF pck00011.
/// This is a low-accuracy visualization model, NOT a substitute for an
/// Earth-orientation-parameter-based ITRF transformation. NAIF warns its
/// prime-meridian error is at least 150 arcseconds; it does not use UT1.
pub fn earth_approx_orientation(epoch: Epoch) -> Result<BodyOrientation, OrientationError> {
    let d = bounded_days(epoch)?;
    let t = d / 36_525.0;
    Ok(orientation(
        -0.641 * t,
        90.0 - 0.557 * t,
        190.147 + 360.9856235 * d,
    ))
}

/// IAU_MOON orientation for epochs within the bundled ephemeris coverage.
pub fn moon_orientation(epoch: Epoch) -> Result<BodyOrientation, OrientationError> {
    let d = bounded_days(epoch)?;
    let t = d / 36_525.0;
    let mut ra = 269.9949 + 0.0031 * t;
    let mut dec = 66.5392 + 0.0130 * t;
    let mut w = 38.3213 + 13.17635815 * d - 1.4e-12 * d * d;
    for i in 0..13 {
        let angle = (MOON_ANGLES[i].0 + MOON_ANGLES[i].1 * d).to_radians();
        let sin = libm::sin(angle);
        let cos = libm::cos(angle);
        ra += MOON_RA[i] * sin;
        dec += MOON_DEC[i] * cos;
        w += MOON_W[i] * sin;
    }
    Ok(orientation(ra, dec, w))
}

/// IAU 2006 Greenwich *mean* sidereal time and mean obliquity, radians.
/// Inputs are separately measured UT1 and TT days since J2000 (JD 2451545.0
/// in their respective time scales), not TDB. Obtain UT1 from observed EOP;
/// substituting UTC or TDB silently introduces orientation error. GMST is
/// not apparent sidereal time and this is not a full Earth orientation matrix.
/// Restrict the polynomial to 1900–2100 approximately.
pub fn earth_mean_angles(ut1_days: f64, tt_days: f64) -> Result<(f64, f64), OrientationError> {
    if !ut1_days.is_finite() || !tt_days.is_finite() {
        return Err(OrientationError::NonFinite);
    }
    if ut1_days.abs() > 36_525.0 || tt_days.abs() > 36_525.0 {
        return Err(OrientationError::OutsideModelRange);
    }
    let t = tt_days / 36_525.0;
    let era = TAU
        * (0.779_057_273_264_0 + 0.002_737_811_911_354_48 * ut1_days + ut1_days.rem_euclid(1.0));
    let gmst_arcsec = 0.014506
        + t * (4612.156534
            + t * (1.3915817 + t * (-0.00000044 + t * (-0.000029956 + t * (-0.0000000368)))));
    let obliq_arcsec = 84_381.406
        + t * (-46.836769
            + t * (-0.0001831 + t * (0.00200340 + t * (-0.000000576 + t * (-0.0000000434)))));
    Ok((
        (era + gmst_arcsec * PI / (180.0 * 3600.0)).rem_euclid(TAU),
        obliq_arcsec * PI / (180.0 * 3600.0),
    ))
}

/// Multiply two 3x3 matrices (row-major).
fn mat3_mul(a: [[f64; 3]; 3], b: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    std::array::from_fn(|i| {
        std::array::from_fn(|j| a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j])
    })
}

/// Rotation about the z-axis by `angle` radians.
fn rot3_z(angle: f64) -> [[f64; 3]; 3] {
    let (s, c) = (libm::sin(angle), libm::cos(angle));
    [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]
}

/// Polar motion matrix: ITRF to mean pole system, from x_pole and y_pole.
/// First-order IERS convention; x_pole and y_pole in radians.
fn polar_motion(x_pole_rad: f64, y_pole_rad: f64) -> [[f64; 3]; 3] {
    let (cx, sx) = (libm::cos(x_pole_rad), libm::sin(x_pole_rad));
    let (cy, sy) = (libm::cos(y_pole_rad), libm::sin(y_pole_rad));
    // R_y(-y_p) * R_x(-x_p)
    [
        [cy, 0.0, -sy],
        [sx * sy, cx, sx * cy],
        [cx * sy, -sx, cx * cy],
    ]
}

/// Full ITRF-to-ICRF Earth orientation using Earth Orientation Parameters.
///
/// Combines the IAU 2009 mean celestial pole direction with a diurnal
/// rotation determined by UT1 (via the Earth Rotation Angle) and a polar
/// motion correction from x_pole and y_pole. Callers supply measured EOP
/// from IERS Bulletins A/B.
///
/// When `params` are all zero, the result matches `earth_approx_orientation`
/// to within numerical noise (both use the same mean pole model).
pub fn earth_orientation(
    epoch: Epoch,
    params: EarthOrientationParams,
) -> Result<BodyOrientation, OrientationError> {
    if !params.ut1_minus_utc_s.is_finite()
        || !params.x_pole_rad.is_finite()
        || !params.y_pole_rad.is_finite()
    {
        return Err(OrientationError::NonFinite);
    }

    // Mean orientation: pole direction (RA/Dec) and approximate prime meridian.
    let mean = earth_approx_orientation(epoch)?;
    let d = bounded_days(epoch)?;

    // Mean prime meridian angle from the IAU 2009 model, in radians.
    let w_mean = (190.147 + 360.9856235 * d).to_radians();

    // Earth Rotation Angle from UT1 = UTC + (UT1 - UTC).
    let ut1_days = d + params.ut1_minus_utc_s / 86_400.0;
    let era = TAU
        * (0.779_057_273_264_0 + 0.002_737_811_911_354_48 * ut1_days + ut1_days.rem_euclid(1.0));

    // Replace the mean-model prime-meridian rotation with the UT1-based ERA:
    //   body_to_j2000 = M_mean × R3_z(ERA - w_mean) × M_polar
    let delta_w = era - w_mean;
    let body_to_j2000 = mat3_mul(
        mat3_mul(mean.body_to_j2000, rot3_z(delta_w)),
        polar_motion(params.x_pole_rad, params.y_pole_rad),
    );

    Ok(BodyOrientation {
        pole_ra_rad: mean.pole_ra_rad,
        pole_dec_rad: mean.pole_dec_rad,
        prime_meridian_rad: era,
        body_to_j2000,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn earth_orientation_zero_params_preserves_pole() {
        // With zero EOP, earth_orientation and earth_approx_orientation share
        // the same celestial pole direction (column 2 of body_to_j2000).
        // The rotation about the pole differs because ERA (UT1-based) and the
        // mean-model prime meridian angle have different zero points.
        let epoch = Epoch::new(820_497_600, 0.0).unwrap();
        let approx = earth_approx_orientation(epoch).unwrap();
        let eop = earth_orientation(epoch, EarthOrientationParams::default()).unwrap();
        for i in 0..3 {
            assert!(
                (approx.body_to_j2000[i][2] - eop.body_to_j2000[i][2]).abs() < 1e-12,
                "pole component {i} differs: {} vs {}",
                approx.body_to_j2000[i][2],
                eop.body_to_j2000[i][2]
            );
        }
    }

    #[test]
    fn earth_orientation_is_valid_rotation() {
        // The output matrix must be orthonormal with determinant +1.
        let epoch = Epoch::new(820_497_600, 0.0).unwrap();
        let params = EarthOrientationParams {
            ut1_minus_utc_s: 0.42,
            x_pole_rad: 0.1 * PI / 180.0 / 3600.0, // 0.1 arcsec
            y_pole_rad: -0.2 * PI / 180.0 / 3600.0, // -0.2 arcsec
        };
        let ori = earth_orientation(epoch, params).unwrap();
        let m = ori.body_to_j2000;

        // Columns must be unit vectors.
        for col in [0, 1, 2] {
            let mag =
                (m[0][col] * m[0][col] + m[1][col] * m[1][col] + m[2][col] * m[2][col]).sqrt();
            assert!((mag - 1.0).abs() < 1e-12, "column {col} not unit: {mag}");
        }

        // Columns must be mutually orthogonal.
        for (a, b) in [(0, 1), (0, 2), (1, 2)] {
            let dot = m[0][a] * m[0][b] + m[1][a] * m[1][b] + m[2][a] * m[2][b];
            assert!(dot.abs() < 1e-12, "columns {a},{b} not orthogonal: {dot}");
        }

        // Determinant must be +1.
        let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
        assert!((det - 1.0).abs() < 1e-12, "determinant {det} not +1");
    }

    #[test]
    fn earth_orientation_ut1_changes_rotation() {
        // Changing UT1-UTC must rotate the body-fixed axes about the pole.
        let epoch = Epoch::new(820_497_600, 0.0).unwrap();
        let base = earth_orientation(epoch, EarthOrientationParams::default()).unwrap();
        let shifted = earth_orientation(
            epoch,
            EarthOrientationParams {
                ut1_minus_utc_s: 1.0,
                ..Default::default()
            },
        )
        .unwrap();
        // The matrices must differ (1 s of UT1 changes ERA by ~15 microradians).
        let mut differ = false;
        for i in 0..3 {
            for j in 0..3 {
                if (base.body_to_j2000[i][j] - shifted.body_to_j2000[i][j]).abs() > 1e-9 {
                    differ = true;
                }
            }
        }
        assert!(differ, "UT1 shift did not change orientation");
    }

    #[test]
    fn earth_orientation_polar_motion_changes_matrix() {
        // Non-zero polar motion must change the matrix.
        let epoch = Epoch::new(820_497_600, 0.0).unwrap();
        let base = earth_orientation(epoch, EarthOrientationParams::default()).unwrap();
        let shifted = earth_orientation(
            epoch,
            EarthOrientationParams {
                x_pole_rad: 0.5 * PI / 180.0 / 3600.0, // 0.5 arcsec
                ..Default::default()
            },
        )
        .unwrap();
        let mut differ = false;
        for i in 0..3 {
            for j in 0..3 {
                if (base.body_to_j2000[i][j] - shifted.body_to_j2000[i][j]).abs() > 1e-12 {
                    differ = true;
                }
            }
        }
        assert!(differ, "polar motion did not change orientation");
    }

    #[test]
    fn earth_orientation_rejects_non_finite() {
        let epoch = Epoch::new(820_497_600, 0.0).unwrap();
        assert_eq!(
            earth_orientation(
                epoch,
                EarthOrientationParams {
                    ut1_minus_utc_s: f64::NAN,
                    ..Default::default()
                },
            ),
            Err(OrientationError::NonFinite)
        );
    }

    #[test]
    fn earth_orientation_outside_range() {
        let before = Epoch::new(631_108_799, 0.999).unwrap();
        let end = Epoch::new(1_136_030_400, 0.0).unwrap();
        for epoch in [before, end] {
            assert_eq!(
                earth_orientation(epoch, EarthOrientationParams::default()),
                Err(OrientationError::OutsideModelRange)
            );
        }
    }
}
