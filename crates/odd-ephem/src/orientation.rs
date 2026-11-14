//! Approximate IAU body orientation and IAU 2006 Earth mean sidereal angle.
//!
//! The Sun and Moon pole/prime-meridian series are from NAIF pck00011.tpc
//! (2022), IAU 2015/2009 respectively. The Moon series describes IAU_MOON,
//! not the higher-accuracy DE440 lunar principal-axes or mean-Earth frames.
//! Earth needs measured UT1 and terrestrial/celestial pole offsets for precision;
//! the functions below are not a full ITRF-to-ICRF transformation.

use odd_core::epoch::Epoch;
use std::f64::consts::{PI, TAU};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OrientationError {
    OutsideModelRange,
    NonFinite,
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
    let w = w_deg.to_radians().rem_euclid(TAU);
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
