//! Small presentation boundary for the bounded ephemeris. Values remain SI/J2000.
//! The browser owns camera/projection, never the science model.
use odd_core::epoch::Epoch;
use odd_ephem::{
    orientation::{earth_approx_orientation, moon_orientation},
    Body, Ephemeris,
};
use std::sync::{Mutex, OnceLock};

// position, velocity, Sun relative Earth, Earth and Moon body-to-J2000 matrices,
// then a count and time-tagged Earth-relative Moon samples (seconds,x,y,z).
const HEADER: usize = 28;
const MAX_SAMPLES: usize = 57;
const SIZE: usize = HEADER + 4 * MAX_SAMPLES;
static DATA: Mutex<[f64; SIZE]> = Mutex::new([0.0; SIZE]);
static EPHEMERIS: OnceLock<Option<Ephemeris>> = OnceLock::new();

fn snapshot(seconds: i64) -> Option<[f64; SIZE]> {
    let eph = EPHEMERIS
        .get_or_init(|| Ephemeris::bundled().ok())
        .as_ref()?;
    let epoch = Epoch::new(seconds, 0.0).ok()?;
    let moon = eph.state(Body::Moon, Body::Earth, epoch).ok()?.state;
    let sun = eph.state(Body::Sun, Body::Earth, epoch).ok()?.state;
    let earth_orientation = earth_approx_orientation(epoch).ok()?.body_to_j2000;
    let moon_orientation = moon_orientation(epoch).ok()?.body_to_j2000;
    let mut out = [0.0; SIZE];
    out[..3].copy_from_slice(&moon.position_m);
    out[3..6].copy_from_slice(&moon.velocity_m_s);
    out[6..9].copy_from_slice(&sun.position_m);
    for i in 0..3 {
        out[9 + i * 3..12 + i * 3].copy_from_slice(&earth_orientation[i]);
        out[18 + i * 3..21 + i * 3].copy_from_slice(&moon_orientation[i]);
    }
    let (start, end) = eph.coverage();
    let mut n = 0;
    // Half-day samples are actual geometric states, not a fitted ellipse.
    for offset in -28_i64..=28 {
        let time = seconds + offset * 43_200;
        if time < start || time >= end {
            continue;
        }
        let state = eph
            .state(Body::Moon, Body::Earth, Epoch::new(time, 0.0).ok()?)
            .ok()?
            .state;
        let i = HEADER + n * 4;
        out[i] = time as f64;
        out[i + 1..i + 4].copy_from_slice(&state.position_m);
        n += 1;
    }
    out[27] = n as f64;
    Some(out)
}

/// Returns 1 on success, 0 on invalid/out-of-range input. A failed query clears
/// the previous snapshot so callers cannot mistake stale data for a new epoch.
#[no_mangle]
pub extern "C" fn view_load(seconds: f64) -> i32 {
    let mut data = DATA.lock().unwrap();
    *data = [0.0; SIZE];
    if !seconds.is_finite()
        || seconds.fract() != 0.0
        || !(631_108_800.0..1_136_030_400.0).contains(&seconds)
    {
        return 0;
    }
    match snapshot(seconds as i64) {
        Some(result) => {
            *data = result;
            1
        }
        None => 0,
    }
}

/// Access values after successful `view_load`; invalid indices return NaN.
#[no_mangle]
pub extern "C" fn view_value(index: u32) -> f64 {
    DATA.lock()
        .unwrap()
        .get(index as usize)
        .copied()
        .unwrap_or(f64::NAN)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_tracks_oracle_and_clips_trail() {
        let s = snapshot(820_497_600).unwrap();
        assert!((s[0] - 144_325_727.0).abs() < 4.0);
        assert_eq!(s[27], 57.0);
        assert_eq!(s[HEADER + 28 * 4], 820_497_600.0);
        assert!((s[HEADER + 28 * 4 + 1] - s[0]).abs() < 1e-6);
        assert_eq!(snapshot(631_108_800).unwrap()[27], 29.0);
        assert!(snapshot(1_136_030_400).is_none());
    }
    #[test]
    fn invalid_input_clears_snapshot() {
        assert_eq!(view_load(f64::NAN), 0);
        assert_eq!(view_value(0), 0.0);
        assert!(view_value(SIZE as u32).is_nan());
    }
}

// Orbit output: signed sample count (positive = closed), 180 positions, then four [valid,x,y,z]
// marker records in PE, AP, AN, DN order. Marker validity is explicit.
const ORBIT_SAMPLES: usize = 180;
const ORBIT_MARKERS: usize = 1 + 3 * ORBIT_SAMPLES;
const ORBIT_SIZE: usize = ORBIT_MARKERS + 4 * 4;
static ORBIT: Mutex<[f64; ORBIT_SIZE]> = Mutex::new([0.0; ORBIT_SIZE]);

fn orbit_snapshot(state: odd_core::frames::State, mu: f64) -> Option<[f64; ORBIT_SIZE]> {
    use odd_astro::{
        anomaly::{true_from_elliptic, true_from_hyperbolic},
        elements::{Elements, SINGULAR_TOLERANCE},
    };
    use std::f64::consts::{PI, TAU};
    let el = Elements::from_state(state, mu).ok()?;
    let mut out = [0.0; ORBIT_SIZE];
    out[0] = if el.eccentricity < 1.0 {
        ORBIT_SAMPLES as f64
    } else {
        -(ORBIT_SAMPLES as f64)
    };
    for i in 0..ORBIT_SAMPLES {
        let fraction = i as f64 / (ORBIT_SAMPLES - 1) as f64;
        let nu = if el.eccentricity < 1.0 {
            true_from_elliptic(el.eccentricity, TAU * i as f64 / ORBIT_SAMPLES as f64).ok()?
        } else if el.eccentricity > 1.0 {
            // Hyperbolas extend to infinity. Show a finite F∈[-3,3] segment.
            true_from_hyperbolic(el.eccentricity, 6.0 * fraction - 3.0).ok()?
        } else {
            // Parabolic Barker parameter D is similarly bounded for display.
            2.0 * (40.0 * fraction - 20.0).atan()
        };
        let position = Elements {
            true_anomaly_rad: nu,
            ..el
        }
        .to_state(mu)
        .ok()?
        .position_m;
        out[1 + 3 * i..4 + 3 * i].copy_from_slice(&position);
    }
    // Peri/apo are undefined for a circular orbit, and node markers are
    // undefined for an equatorial plane; omit instead of inventing them.
    let markers = [
        if el.eccentricity >= SINGULAR_TOLERANCE {
            Some(0.0)
        } else {
            None
        },
        if el.eccentricity >= SINGULAR_TOLERANCE && el.eccentricity < 1.0 {
            Some(PI)
        } else {
            None
        },
        if el.inclination_rad.sin().abs() >= SINGULAR_TOLERANCE {
            Some(-el.argument_of_periapsis_rad)
        } else {
            None
        },
        if el.inclination_rad.sin().abs() >= SINGULAR_TOLERANCE {
            Some(PI - el.argument_of_periapsis_rad)
        } else {
            None
        },
    ];
    for (i, angle) in markers.into_iter().enumerate() {
        if let Some(angle) = angle {
            if let Ok(s) = (Elements {
                true_anomaly_rad: angle,
                ..el
            })
            .to_state(mu)
            {
                let index = ORBIT_MARKERS + i * 4;
                out[index] = 1.0;
                out[index + 1..index + 4].copy_from_slice(&s.position_m);
            }
        }
    }
    Some(out)
}

/// Set the active central-body-relative test orbit in inertial SI coordinates.
/// Invalid states clear the previous orbit. This computes only ideal conics,
/// not a propagated spacecraft trajectory or an impact check.
#[no_mangle]
pub extern "C" fn orbit_load(mu: f64, x: f64, y: f64, z: f64, vx: f64, vy: f64, vz: f64) -> i32 {
    let mut out = ORBIT.lock().unwrap();
    *out = [0.0; ORBIT_SIZE];
    let state = odd_core::frames::State {
        position_m: [x, y, z],
        velocity_m_s: [vx, vy, vz],
    };
    match orbit_snapshot(state, mu) {
        Some(result) => {
            *out = result;
            1
        }
        None => 0,
    }
}

#[no_mangle]
pub extern "C" fn orbit_value(index: u32) -> f64 {
    ORBIT
        .lock()
        .unwrap()
        .get(index as usize)
        .copied()
        .unwrap_or(f64::NAN)
}

#[cfg(test)]
mod orbit_tests {
    use super::*;
    #[test]
    fn computes_real_orbit_samples_and_nodes() {
        let mu = 3.986_004_418e14;
        let state = odd_core::frames::State {
            position_m: [7e6, 0.0, 0.0],
            velocity_m_s: [0.0, 7600.0, 2000.0],
        };
        let out = orbit_snapshot(state, mu).unwrap();
        assert_eq!(out[0], 180.0);
        assert!((out[1] - 7e6).abs() < 1e-7);
        for index in 0..4 {
            assert_eq!(out[ORBIT_MARKERS + 4 * index], 1.0);
        }
        for index in [2, 3] {
            assert!(out[ORBIT_MARKERS + 4 * index + 3].abs() < 1e-7);
        }
        let r = (out[ORBIT_MARKERS + 1].powi(2)
            + out[ORBIT_MARKERS + 2].powi(2)
            + out[ORBIT_MARKERS + 3].powi(2))
        .sqrt();
        assert!(r > 6_378_137.0);
    }
    #[test]
    fn invalid_orbit_clears_output() {
        assert_eq!(orbit_load(0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0), 0);
        assert_eq!(orbit_value(0), 0.0);
    }
}
