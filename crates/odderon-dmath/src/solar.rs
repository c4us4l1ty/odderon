//! Analytical solar ephemeris & atmospheric radiative transfer for coastal launch sites.
//! Room orientation: window wall is at +Z facing South-East (azimuth 132 deg, overlooking Pacific pad).

use crate::trig::{clamp_f64, dasin, datan2, dcos, dexp, dsin, dsqrt, PI};
use crate::vec::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct SolarState {
    /// Solar elevation above horizon (radians).
    pub elevation_rad: f64,
    /// Solar azimuth clockwise from North (radians).
    pub azimuth_rad: f64,
    /// Unit vector pointing toward the sun in room coordinates (+Y up, +Z out window SE, +X right SW).
    pub sun_dir_room: Vec3,
    /// Direct Normal Irradiance (W/m^2) after Rayleigh/aerosol airmass extinction.
    pub dni_wm2: f64,
    /// Diffuse sky horizontal irradiance (W/m^2).
    pub diffuse_wm2: f64,
    /// Solar flux incident on the vertical South-East apartment window plane (W/m^2).
    pub window_incident_wm2: f64,
    /// Correlated effective warmth factor [0 = twilight, 1 = high summer noon].
    pub warmth: f32,
}

/// Computes analytical summer solar geometry (July 21, DOY = 202) at Uchinoura (31.251° N, 131.079° E).
/// `local_hour_jst` is local Japan Standard Time in hours (e.g. 12.25 = 12:15 PM summer noon).
pub fn sun_state_uchinoura(local_hour_jst: f64) -> SolarState {
    let lat = 31.2510 * PI / 180.0;
    let lon_deg = 131.0791;
    // July 21 solar declination ~ +20.44 deg
    let decl = 20.44 * PI / 180.0;
    // JST central meridian is 135 E -> solar noon is shifted by ~(135 - 131.079)/15 = +0.261 h
    let solar_hour = local_hour_jst - 0.2614;
    let hour_angle = (solar_hour - 12.0) * (15.0 * PI / 180.0);

    let sin_el = clamp_f64(
        dsin(lat) * dsin(decl) + dcos(lat) * dcos(decl) * dcos(hour_angle),
        -1.0,
        1.0,
    );
    let el = dasin(sin_el);
    let cos_el = dsqrt(clamp_f64(1.0 - sin_el * sin_el, 1e-8, 1.0));

    // Azimuth from North clockwise
    let sin_az = -dcos(decl) * dsin(hour_angle) / cos_el;
    let cos_az = (dsin(decl) - dsin(lat) * sin_el) / (dcos(lat) * cos_el);
    let mut az = datan2(sin_az, cos_az);
    if az < 0.0 {
        az += 2.0 * PI;
    }

    // Room +Z faces coastal South-East (azimuth_window = 128 deg so noon & morning sun slant across desk & tatami!)
    let win_az = 128.0 * PI / 180.0;
    let rel_az = az - win_az;
    // In room coords: +Z is outward along win_az, +X is +90 deg clockwise from win_az, +Y is zenith
    let dir_z = (cos_el * dcos(rel_az)) as f32;
    let dir_x = (cos_el * dsin(rel_az)) as f32;
    let dir_y = sin_el as f32;
    let sun_dir_room = Vec3::new(dir_x, dir_y, dir_z).normalized();

    // Atmospheric airmass & Meinel clear-sky DNI model (solar constant 1361 W/m^2)
    let (dni, diffuse) = if sin_el > 0.01 {
        let airmass = 1.0 / (sin_el + 0.15 * clamp_f64(el * 180.0 / PI + 3.885, 0.5, 95.0).recip());
        let tau = -0.145 * clamp_f64(airmass, 1.0, 28.0);
        let dni_val = 1361.0 * dexp(tau);
        let diff_val = 95.0 * sin_el + 38.0;
        (dni_val, diff_val)
    } else {
        let twilight = clamp_f64((sin_el + 0.12) / 0.13, 0.0, 1.0);
        (0.0, 24.0 * twilight)
    };

    // Flux striking the vertical +Z window (normal = [0, 0, 1] outward) + ground/ocean albedo bounce
    let cos_inc = clamp_f64(sun_dir_room.z as f64, 0.0, 1.0);
    let ground_reflected = 0.22 * (dni * clamp_f64(sin_el, 0.0, 1.0) + diffuse) * 0.5;
    let window_incident_wm2 = dni * cos_inc + 0.5 * diffuse + ground_reflected;

    let warmth = clamp_f64((sin_el + 0.04) / 0.45, 0.0, 1.0) as f32;
    let _ = lon_deg;

    SolarState {
        elevation_rad: el,
        azimuth_rad: az,
        sun_dir_room,
        dni_wm2: dni,
        diffuse_wm2: diffuse,
        window_incident_wm2,
        warmth,
    }
}
