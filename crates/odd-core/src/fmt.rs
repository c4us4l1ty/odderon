//! Display-only conversions from SI quantities.
//!
//! These values are for presentation, not internal simulation state. Formatting,
//! localization, and rounding are left to the user interface.

use crate::consts::{JULIAN_YEAR_S, SPEED_OF_LIGHT_M_S};
use crate::units::{Meters, Radians, RadiansPerSecond, Seconds};

pub fn kilometers(distance: Meters) -> f64 {
    distance.0 / 1_000.0
}

pub fn days(duration: Seconds) -> f64 {
    duration.0 / 86_400.0
}

pub fn light_years(distance: Meters) -> f64 {
    distance.0 / (SPEED_OF_LIGHT_M_S * JULIAN_YEAR_S)
}

pub fn degrees(angle: Radians) -> f64 {
    angle.0 * (180.0 / std::f64::consts::PI)
}

pub fn revolutions_per_minute(rate: RadiansPerSecond) -> f64 {
    rate.0 * (60.0 / std::f64::consts::TAU)
}
