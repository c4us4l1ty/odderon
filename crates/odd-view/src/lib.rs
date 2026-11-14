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
