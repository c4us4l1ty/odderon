//! Long-range, low-accuracy analytical lunar and planetary approximations.
//! Meeus chapter 47 terms adapted from Sonia Keys' MIT-licensed implementation;
//! see data/attribution/meeus-soniakeys-MIT.txt.
const TA: [[f64; 6]; 40] = [
    [0.0, 0.0, 1.0, 0.0, 6288774.0, -20905355.0],
    [2.0, 0.0, -1.0, 0.0, 1274027.0, -3699111.0],
    [2.0, 0.0, 0.0, 0.0, 658314.0, -2955968.0],
    [0.0, 0.0, 2.0, 0.0, 213618.0, -569925.0],
    [0.0, 1.0, 0.0, 0.0, -185116.0, 48888.0],
    [0.0, 0.0, 0.0, 2.0, -114332.0, -3149.0],
    [2.0, 0.0, -2.0, 0.0, 58793.0, 246158.0],
    [2.0, -1.0, -1.0, 0.0, 57066.0, -152138.0],
    [2.0, 0.0, 1.0, 0.0, 53322.0, -170733.0],
    [2.0, -1.0, 0.0, 0.0, 45758.0, -204586.0],
    [0.0, 1.0, -1.0, 0.0, -40923.0, -129620.0],
    [1.0, 0.0, 0.0, 0.0, -34720.0, 108743.0],
    [0.0, 1.0, 1.0, 0.0, -30383.0, 104755.0],
    [2.0, 0.0, 0.0, -2.0, 15327.0, 10321.0],
    [0.0, 0.0, 1.0, 2.0, -12528.0, 0.0],
    [0.0, 0.0, 1.0, -2.0, 10980.0, 79661.0],
    [4.0, 0.0, -1.0, 0.0, 10675.0, -34782.0],
    [0.0, 0.0, 3.0, 0.0, 10034.0, -23210.0],
    [4.0, 0.0, -2.0, 0.0, 8548.0, -21636.0],
    [2.0, 1.0, -1.0, 0.0, -7888.0, 24208.0],
    [2.0, 1.0, 0.0, 0.0, -6766.0, 30824.0],
    [1.0, 0.0, -1.0, 0.0, -5163.0, -8379.0],
    [1.0, 1.0, 0.0, 0.0, 4987.0, -16675.0],
    [2.0, -1.0, 1.0, 0.0, 4036.0, -12831.0],
    [2.0, 0.0, 2.0, 0.0, 3994.0, -10445.0],
    [4.0, 0.0, 0.0, 0.0, 3861.0, -11650.0],
    [2.0, 0.0, -3.0, 0.0, 3665.0, 14403.0],
    [0.0, 1.0, -2.0, 0.0, -2689.0, -7003.0],
    [2.0, 0.0, -1.0, 2.0, -2602.0, 0.0],
    [2.0, -1.0, -2.0, 0.0, 2390.0, 10056.0],
    [1.0, 0.0, 1.0, 0.0, -2348.0, 6322.0],
    [2.0, -2.0, 0.0, 0.0, 2236.0, -9884.0],
    [0.0, 1.0, 2.0, 0.0, -2120.0, 5751.0],
    [0.0, 2.0, 0.0, 0.0, -2069.0, 0.0],
    [2.0, -2.0, -1.0, 0.0, 2048.0, -4950.0],
    [2.0, 0.0, 1.0, -2.0, -1773.0, 4130.0],
    [2.0, 0.0, 0.0, 2.0, -1595.0, 0.0],
    [4.0, -1.0, -1.0, 0.0, 1215.0, -3958.0],
    [0.0, 0.0, 2.0, 2.0, -1110.0, 0.0],
    [3.0, 0.0, -1.0, 0.0, -892.0, 3258.0],
];
const TB: [[f64; 5]; 30] = [
    [0.0, 0.0, 0.0, 1.0, 5128122.0],
    [0.0, 0.0, 1.0, 1.0, 280602.0],
    [0.0, 0.0, 1.0, -1.0, 277693.0],
    [2.0, 0.0, 0.0, -1.0, 173237.0],
    [2.0, 0.0, -1.0, 1.0, 55413.0],
    [2.0, 0.0, -1.0, -1.0, 46271.0],
    [2.0, 0.0, 0.0, 1.0, 32573.0],
    [0.0, 0.0, 2.0, 1.0, 17198.0],
    [2.0, 0.0, 1.0, -1.0, 9266.0],
    [0.0, 0.0, 2.0, -1.0, 8822.0],
    [2.0, -1.0, 0.0, -1.0, 8216.0],
    [2.0, 0.0, -2.0, -1.0, 4324.0],
    [2.0, 0.0, 1.0, 1.0, 4200.0],
    [2.0, 1.0, 0.0, -1.0, -3359.0],
    [2.0, -1.0, -1.0, 1.0, 2463.0],
    [2.0, -1.0, 0.0, 1.0, 2211.0],
    [2.0, -1.0, -1.0, -1.0, 2065.0],
    [0.0, 1.0, -1.0, -1.0, -1870.0],
    [4.0, 0.0, -1.0, -1.0, 1828.0],
    [0.0, 1.0, 0.0, 1.0, -1794.0],
    [0.0, 0.0, 0.0, 3.0, -1749.0],
    [0.0, 1.0, -1.0, 1.0, -1565.0],
    [1.0, 0.0, 0.0, 1.0, -1491.0],
    [0.0, 1.0, 1.0, 1.0, -1475.0],
    [0.0, 1.0, 1.0, -1.0, -1410.0],
    [0.0, 1.0, 0.0, -1.0, -1344.0],
    [1.0, 0.0, 0.0, -1.0, -1335.0],
    [0.0, 0.0, 3.0, 1.0, 1107.0],
    [4.0, 0.0, 0.0, -1.0, 1021.0],
    [4.0, 0.0, -1.0, 1.0, 833.0],
];
use odd_core::{epoch::Epoch, frames::State};
use std::f64::consts::PI;

const CENTURY: f64 = 36525.0 * 86400.0;
const RAD: f64 = PI / 180.0;
fn poly(t: f64, c: &[f64]) -> (f64, f64) {
    let mut v = *c.last().unwrap();
    let mut d = 0.0;
    for &a in c[..c.len() - 1].iter().rev() {
        d = d * t + v;
        v = v * t + a;
    }
    (v, d)
}
fn sin_term(sum: &mut f64, derivative: &mut f64, coefficient: f64, angle: f64, rate: f64) {
    let (s, c) = angle.sin_cos();
    *sum += coefficient * s;
    *derivative += coefficient * c * rate;
}
fn rotate_date(t: f64, v: [f64; 3]) -> [f64; 3] {
    // Meeus mean ecliptic/equinox of date to J2000 using IAU 1976
    // precession and mean obliquity. Far from J2000 this is approximate.
    let eps =
        (23.4392911111 - (46.8150 * t + 0.00059 * t * t - 0.001813 * t * t * t) / 3600.0) * RAD;
    let (se, ce) = eps.sin_cos();
    let q = [v[0], ce * v[1] - se * v[2], se * v[1] + ce * v[2]];
    let zeta = (2306.2181 * t + 0.30188 * t * t + 0.017998 * t * t * t) * RAD / 3600.0;
    let z = (2306.2181 * t + 1.09468 * t * t + 0.018203 * t * t * t) * RAD / 3600.0;
    let theta = (2004.3109 * t - 0.42665 * t * t - 0.041833 * t * t * t) * RAD / 3600.0;
    let (sa, ca) = zeta.sin_cos();
    let (sb, cb) = z.sin_cos();
    let (st, ct) = theta.sin_cos();
    let p = [
        [ca * ct * cb - sa * sb, -sa * ct * cb - ca * sb, -st * cb],
        [ca * ct * sb + sa * cb, -sa * ct * sb + ca * cb, -st * sb],
        [ca * st, -sa * st, ct],
    ];
    std::array::from_fn(|i| p[0][i] * q[0] + p[1][i] * q[1] + p[2][i] * q[2])
}
/// Truncated Meeus ELP2000/82 Moon relative to Earth, in J2000 axes.
fn lunar(t: f64) -> State {
    let (lp, lpd) = poly(
        t,
        &[
            218.3164477,
            481267.88123421,
            -0.0015786,
            1.0 / 538841.0,
            -1.0 / 65194000.0,
        ],
    );
    let coeffs: [&[f64]; 4] = [
        &[
            297.8501921,
            445267.1114034,
            -0.0018819,
            1.0 / 545868.0,
            -1.0 / 113065000.0,
        ],
        &[357.5291092, 35999.0502909, -0.0001536, 1.0 / 24490000.0],
        &[
            134.9633964,
            477198.8675055,
            0.0087414,
            1.0 / 69699.0,
            -1.0 / 14712000.0,
        ],
        &[
            93.272095,
            483202.0175233,
            -0.0036539,
            -1.0 / 3526000.0,
            1.0 / 863310000.0,
        ],
    ];
    let mut a = [0.0; 4];
    let mut d = [0.0; 4];
    for j in 0..4 {
        let (v, rate) = poly(t, coeffs[j]);
        a[j] = v * RAD;
        d[j] = rate * RAD;
    }
    let lp = lp * RAD;
    let lpd = lpd * RAD;
    let (e, ed) = poly(t, &[1.0, -0.002516, -0.0000074]);
    let (a1, a1d) = ((119.75 + 131.849 * t) * RAD, 131.849 * RAD);
    let (a2, a2d) = ((53.09 + 479264.290 * t) * RAD, 479264.290 * RAD);
    let (a3, a3d) = ((313.45 + 481266.484 * t) * RAD, 481266.484 * RAD);
    let (mut longitude, mut dl, mut distance, mut dr, mut latitude, mut db) =
        (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for (c, x, dx) in [
        (3958.0, a1, a1d),
        (1962.0, lp - a[3], lpd - d[3]),
        (318.0, a2, a2d),
    ] {
        sin_term(&mut longitude, &mut dl, c, x, dx);
    }
    for (c, x, dx) in [
        (-2235.0, lp, lpd),
        (382.0, a3, a3d),
        (175.0, a1 - a[3], a1d - d[3]),
        (175.0, a1 + a[3], a1d + d[3]),
        (127.0, lp - a[2], lpd - d[2]),
        (-115.0, lp + a[2], lpd + d[2]),
    ] {
        sin_term(&mut latitude, &mut db, c, x, dx);
    }
    for row in TA {
        let angle = (0..4).map(|i| row[i] * a[i]).sum::<f64>();
        let rate = (0..4).map(|i| row[i] * d[i]).sum::<f64>();
        let (s, c) = angle.sin_cos();
        let (factor, df) = match row[1].abs() as u8 {
            1 => (e, ed),
            2 => (e * e, 2.0 * e * ed),
            _ => (1.0, 0.0),
        };
        longitude += row[4] * factor * s;
        dl += row[4] * (df * s + factor * c * rate);
        distance += row[5] * factor * c;
        dr += row[5] * (df * c - factor * s * rate);
    }
    for row in TB {
        let angle = (0..4).map(|i| row[i] * a[i]).sum::<f64>();
        let rate = (0..4).map(|i| row[i] * d[i]).sum::<f64>();
        let (s, c) = angle.sin_cos();
        let (factor, df) = match row[1].abs() as u8 {
            1 => (e, ed),
            2 => (e * e, 2.0 * e * ed),
            _ => (1.0, 0.0),
        };
        latitude += row[4] * factor * s;
        db += row[4] * (df * s + factor * c * rate);
    }
    let lon = lp + longitude * 1e-6 * RAD;
    let lat = latitude * 1e-6 * RAD;
    let r = 385_000_560.0 + distance;
    let lonrate = (lpd + dl * 1e-6 * RAD) / CENTURY;
    let latrate = db * 1e-6 * RAD / CENTURY;
    let rrate = dr / CENTURY;
    let (sl, cl) = lon.sin_cos();
    let (sb, cb) = lat.sin_cos();
    let unit = [cb * cl, cb * sl, sb];
    let dunit = [
        -sb * latrate * cl - cb * sl * lonrate,
        -sb * latrate * sl + cb * cl * lonrate,
        cb * latrate,
    ];
    let pos = unit.map(|x| x * r);
    let vel = std::array::from_fn(|i| rrate * unit[i] + r * dunit[i]);
    let position_m = rotate_date(t, pos);
    let mut velocity_m_s = rotate_date(t, vel);
    let dt = 1e-4;
    let plus = rotate_date(t + dt, pos);
    let minus = rotate_date(t - dt, pos);
    for i in 0..3 {
        velocity_m_s[i] += (plus[i] - minus[i]) / (2.0 * dt * CENTURY);
    }
    State {
        position_m,
        velocity_m_s,
    }
}

// JPL table 2a/2b: long-range heliocentric approximate elements. Planetary
// masses are used only to estimate the otherwise unavailable solar barycenter;
// this is not the DE440 barycenter and includes no asteroid/TNO corrections.
const PLANETS: [([f64; 6], [f64; 6], [f64; 4], f64); 8] = [
    (
        [
            0.38709843,
            0.20563661,
            7.00559432,
            252.25166724,
            77.45771895,
            48.33961819,
        ],
        [
            0.0,
            0.00002123,
            -0.00590158,
            149472.67486623,
            0.15940013,
            -0.12214182,
        ],
        [0.0; 4],
        1.6601e-7,
    ),
    (
        [
            0.72332102,
            0.00676399,
            3.39777545,
            181.97970850,
            131.76755713,
            76.67261496,
        ],
        [
            -0.00000026,
            -0.00005107,
            0.00043494,
            58517.81560260,
            0.05679648,
            -0.27274174,
        ],
        [0.0; 4],
        2.4478e-6,
    ),
    (
        [
            1.00000018,
            0.01673163,
            -0.00054346,
            100.46691572,
            102.93005885,
            -5.11260389,
        ],
        [
            -0.00000003,
            -0.00003661,
            -0.01337178,
            35999.37306329,
            0.31795260,
            -0.24123856,
        ],
        [0.0; 4],
        3.0404e-6,
    ),
    (
        [
            1.52371243,
            0.09336511,
            1.85181869,
            -4.56813164,
            -23.91744784,
            49.71320984,
        ],
        [
            0.00000097,
            0.00009149,
            -0.00724757,
            19140.29934243,
            0.45223625,
            -0.26852431,
        ],
        [0.0; 4],
        3.2272e-7,
    ),
    (
        [
            5.20248019,
            0.04853590,
            1.29861416,
            34.33479152,
            14.27495244,
            100.29282654,
        ],
        [
            -0.00002864,
            0.00018026,
            -0.00322699,
            3034.90371757,
            0.18199196,
            0.13024619,
        ],
        [-0.00012452, 0.06064060, -0.35635438, 38.35125],
        9.5479e-4,
    ),
    (
        [
            9.54149883,
            0.05550825,
            2.49424102,
            50.07571329,
            92.86136063,
            113.63998702,
        ],
        [
            -0.00003065,
            -0.00032044,
            0.00451969,
            1222.11494724,
            0.54179478,
            -0.25015002,
        ],
        [0.00025899, -0.13434469, 0.87320147, 38.35125],
        2.8588e-4,
    ),
    (
        [
            19.18797948,
            0.04685740,
            0.77298127,
            314.20276625,
            172.43404441,
            73.96250215,
        ],
        [
            -0.00020455,
            -0.00001550,
            -0.00180155,
            428.49512595,
            0.09266985,
            0.05739699,
        ],
        [0.00058331, -0.97731848, 0.17689245, 7.67025],
        4.3662e-5,
    ),
    (
        [
            30.06952752,
            0.00895439,
            1.77005520,
            304.22289287,
            46.68158724,
            131.78635853,
        ],
        [
            0.00006447,
            0.00000818,
            0.00022400,
            218.46515314,
            0.01009938,
            -0.00606302,
        ],
        [-0.00041348, 0.68346318, -0.10162547, 7.67025],
        5.1514e-5,
    ),
];
fn planet(index: usize, t: f64) -> [f64; 3] {
    let (base, rate, extra, _) = PLANETS[index];
    let el: [f64; 6] = std::array::from_fn(|i| base[i] + rate[i] * t);
    let (a, e, inc, long, peri, node) = (
        el[0] * 149_597_870_700.0,
        el[1],
        el[2] * RAD,
        el[3],
        el[4] * RAD,
        el[5] * RAD,
    );
    let q = extra[3] * RAD * t;
    let mean = ((long - el[4] + extra[0] * t * t + extra[1] * q.cos() + extra[2] * q.sin()) * RAD)
        .rem_euclid(2.0 * PI);
    let mut ecc = mean;
    for _ in 0..6 {
        ecc -= (ecc - e * ecc.sin() - mean) / (1.0 - e * ecc.cos());
    }
    let (sin_e, cos_e) = ecc.sin_cos();
    let x = a * (cos_e - e);
    let y = a * (1.0 - e * e).sqrt() * sin_e;
    let (sw, cw) = (peri - node).sin_cos();
    let (sn, cn) = node.sin_cos();
    let (si, ci) = inc.sin_cos();
    let xe = (cw * cn - sw * sn * ci) * x + (-sw * cn - cw * sn * ci) * y;
    let ye = (cw * sn + sw * cn * ci) * x + (-sw * sn + cw * cn * ci) * y;
    let ze = sw * si * x + cw * si * y;
    let (s, c) = (23.43928 * RAD).sin_cos();
    [xe, c * ye - s * ze, s * ye + c * ze]
}
fn planetary_state(index: usize, t: f64) -> State {
    let position_m = planet(index, t);
    let dt = 4.0 / CENTURY;
    let plus = planet(index, t + dt);
    let minus = planet(index, t - dt);
    State {
        position_m,
        velocity_m_s: std::array::from_fn(|i| (plus[i] - minus[i]) / 8.0),
    }
}
fn add(a: State, b: State, scale: f64) -> State {
    State {
        position_m: std::array::from_fn(|i| a.position_m[i] + scale * b.position_m[i]),
        velocity_m_s: std::array::from_fn(|i| a.velocity_m_s[i] + scale * b.velocity_m_s[i]),
    }
}
fn zero() -> State {
    State {
        position_m: [0.0; 3],
        velocity_m_s: [0.0; 3],
    }
}

/// Approximate relative state. The long-range fit is only valid from
/// astronomical year -2999 (3000 BC) through year 3000 inclusive.
pub(crate) fn state(
    target: crate::Body,
    center: crate::Body,
    epoch: Epoch,
) -> Result<State, crate::EphemerisError> {
    use crate::{Body, EphemerisError};
    // Half-open TDB coverage, proleptic Gregorian 3000 BC through AD 3000.
    const START: i64 = -157_753_224_000;
    const END: i64 = 31_588_488_000;
    if epoch.seconds() < START || epoch.seconds() >= END {
        return Err(EphemerisError::OutsideCoverage);
    }
    let t = (epoch.seconds() as f64 + epoch.fraction()) / CENTURY;
    if target == center {
        return Ok(zero());
    }
    let need_moon =
        matches!(target, Body::Earth | Body::Moon) || matches!(center, Body::Earth | Body::Moon);
    let moon = if need_moon { lunar(t) } else { zero() };
    // Avoid heliocentric summation when both states share the EMB origin.
    let is_local = |b| matches!(b, Body::Earth | Body::Moon | Body::EarthMoonBarycenter);
    let emb = if is_local(target) && is_local(center) {
        zero()
    } else {
        planetary_state(2, t)
    };
    let bary = if target == Body::SolarSystemBarycenter || center == Body::SolarSystemBarycenter {
        let mut sum = zero();
        let mut mass = 1.0;
        for (i, planet) in PLANETS.iter().enumerate() {
            sum = add(sum, planetary_state(i, t), planet.3);
            mass += planet.3;
        }
        add(zero(), sum, 1.0 / mass)
    } else {
        zero()
    };
    let moon_fraction = 4.902_800_118e12 / (3.986_004_355_07e14 + 4.902_800_118e12);
    let body = |id| match id {
        Body::Sun => zero(),
        Body::SolarSystemBarycenter => bary,
        Body::EarthMoonBarycenter => emb,
        Body::Earth => add(emb, moon, -moon_fraction),
        Body::Moon => add(emb, moon, 1.0 - moon_fraction),
    };
    Ok(add(body(target), body(center), -1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Body, Ephemeris};
    #[test]
    fn lunar_against_de440_and_long_range_distance() {
        let eph = Ephemeris::bundled().unwrap();
        let epoch = Epoch::new(820_497_600, 0.0).unwrap();
        let actual = state(Body::Moon, Body::Earth, epoch).unwrap();
        let reference = eph.state(Body::Moon, Body::Earth, epoch).unwrap().state;
        let dist = |a: [f64; 3], b: [f64; 3]| {
            a.iter()
                .zip(b)
                .map(|(x, y)| (x - y) * (x - y))
                .sum::<f64>()
                .sqrt()
        };
        let pe = dist(actual.position_m, reference.position_m);
        let ve = dist(actual.velocity_m_s, reference.velocity_m_s);
        println!("2026 lunar error {pe} m, {ve} m/s");
        assert!(pe < 500_000.0 && ve < 5.0);
        let future = Epoch::from_utc(2150, 6, 15, 0, 0, 0.0).unwrap();
        let moon = eph.state(Body::Moon, Body::Earth, future).unwrap();
        assert_eq!(moon.model, crate::EphemerisModel::Analytical);
        let distance = moon
            .state
            .position_m
            .iter()
            .map(|x| x * x)
            .sum::<f64>()
            .sqrt();
        println!("2150 lunar distance {distance} m");
        assert!((356e6..=407e6).contains(&distance));
    }
    #[test]
    fn compares_analytical_to_packed_across_years_and_velocity_derivative() {
        let eph = Ephemeris::bundled().unwrap();
        let mut max_pos = 0.0_f64;
        let mut max_vel = 0.0_f64;
        let mut max_emb = 0.0_f64;
        let norm = |a: [f64; 3]| a.iter().map(|x| x * x).sum::<f64>().sqrt();
        let diff = |a: [f64; 3], b: [f64; 3]| std::array::from_fn(|i| a[i] - b[i]);
        for day in (0..16 * 365).step_by(37) {
            let epoch = Epoch::new(631_108_800 + i64::from(day) * 86400, 0.0).unwrap();
            let lunar = state(Body::Moon, Body::Earth, epoch).unwrap();
            let reference = eph.state(Body::Moon, Body::Earth, epoch).unwrap().state;
            max_pos = max_pos.max(norm(diff(lunar.position_m, reference.position_m)));
            max_vel = max_vel.max(norm(diff(lunar.velocity_m_s, reference.velocity_m_s)));
            let em = state(Body::EarthMoonBarycenter, Body::Sun, epoch).unwrap();
            let emref = eph
                .state(Body::EarthMoonBarycenter, Body::Sun, epoch)
                .unwrap()
                .state;
            max_emb = max_emb.max(norm(diff(em.position_m, emref.position_m)));
            let before = state(
                Body::Moon,
                Body::Earth,
                epoch.checked_sub_seconds(1.0).unwrap(),
            )
            .unwrap();
            let after = state(
                Body::Moon,
                Body::Earth,
                epoch.checked_add_seconds(1.0).unwrap(),
            )
            .unwrap();
            let numerical =
                std::array::from_fn(|i| (after.position_m[i] - before.position_m[i]) / 2.0);
            assert!(norm(diff(numerical, lunar.velocity_m_s)) < 0.01);
        }
        println!("Sampled 2020–2035 max Moon position {max_pos} m velocity {max_vel} m/s; EMB/Sun position {max_emb} m");
        assert!(max_pos < 500_000.0 && max_vel < 5.0 && max_emb < 40_000_000.0);
    }
    #[test]
    fn analytical_range_is_half_open_and_body_pairs_are_consistent() {
        let e = Ephemeris::bundled().unwrap();
        let start = Epoch::new(-157_753_224_000, 0.0).unwrap();
        let end = Epoch::new(31_588_488_000, 0.0).unwrap();
        assert!(e.state(Body::Moon, Body::Earth, start).is_ok());
        assert!(e
            .state(
                Body::Moon,
                Body::Earth,
                end.checked_sub_seconds(1.0).unwrap()
            )
            .is_ok());
        assert_eq!(
            e.state(
                Body::Moon,
                Body::Earth,
                start.checked_sub_seconds(1.0).unwrap()
            ),
            Err(crate::EphemerisError::OutsideCoverage)
        );
        assert_eq!(
            e.state(Body::Moon, Body::Earth, end),
            Err(crate::EphemerisError::OutsideCoverage)
        );
        let epoch = Epoch::from_utc(2150, 6, 15, 0, 0, 0.0).unwrap();
        for a in [
            Body::Sun,
            Body::SolarSystemBarycenter,
            Body::EarthMoonBarycenter,
            Body::Earth,
            Body::Moon,
        ] {
            for b in [
                Body::Sun,
                Body::SolarSystemBarycenter,
                Body::EarthMoonBarycenter,
                Body::Earth,
                Body::Moon,
            ] {
                let ab = e.state(a, b, epoch).unwrap().state;
                let ba = e.state(b, a, epoch).unwrap().state;
                for i in 0..3 {
                    assert_eq!(ab.position_m[i], -ba.position_m[i]);
                    assert_eq!(ab.velocity_m_s[i], -ba.velocity_m_s[i]);
                }
            }
        }
    }
    #[test]
    fn independent_de440s_cspice_outside_packed_table() {
        // CSPICE N0067, JPL de440s.bsp, geometric J2000, SI; epochs are TDB.
        let cases = [
            (
                -1_577_880_000,
                [
                    186_511_671.0644489,
                    312_836_790.560312,
                    164_402_440.08560172,
                ],
                [-886.4614315785211, 368.89650558180665, 219.06582529940417],
            ),
            (
                -43_200,
                [
                    -317_650_241.68312335,
                    -236_464_546.14542875,
                    -62_676_289.75542066,
                ],
                [560.7870647144779, -733.2714113269337, -319.69883830384384],
            ),
            (
                4_734_763_200,
                [
                    -259_848_915.81221125,
                    233_558_903.26688057,
                    121_520_985.45822763,
                ],
                [-791.346033266093, -683.8993200418148, -203.7582739763241],
            ),
        ];
        for (seconds, p, v) in cases {
            let got = state(Body::Moon, Body::Earth, Epoch::new(seconds, 0.0).unwrap()).unwrap();
            let error = |a: [f64; 3], b: [f64; 3]| {
                a.iter()
                    .zip(b)
                    .map(|(x, y)| (x - y) * (x - y))
                    .sum::<f64>()
                    .sqrt()
            };
            println!(
                "{seconds}: {} m / {} m/s",
                error(got.position_m, p),
                error(got.velocity_m_s, v)
            );
            assert!(error(got.position_m, p) < 100_000.0);
            assert!(error(got.velocity_m_s, v) < 1.0);
        }
    }
}
