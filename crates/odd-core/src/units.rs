//! Strongly typed SI quantities used at simulation boundaries.
//!
//! These wrappers prevent adding unlike units, but do not impose physical
//! validity constraints: algorithms must validate finiteness and ranges.

use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Div, Mul, Sub, SubAssign};

macro_rules! si_quantity {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd, Serialize, Deserialize)]
        #[repr(transparent)]
        pub struct $name(pub f64);
        impl From<f64> for $name { fn from(value: f64) -> Self { Self(value) } }
        impl From<$name> for f64 { fn from(value: $name) -> Self { value.0 } }
        impl Add for $name { type Output = Self; fn add(self, rhs: Self) -> Self { Self(self.0 + rhs.0) } }
        impl Sub for $name { type Output = Self; fn sub(self, rhs: Self) -> Self { Self(self.0 - rhs.0) } }
        impl AddAssign for $name { fn add_assign(&mut self, rhs: Self) { self.0 += rhs.0; } }
        impl SubAssign for $name { fn sub_assign(&mut self, rhs: Self) { self.0 -= rhs.0; } }
        impl Mul<f64> for $name { type Output = Self; fn mul(self, rhs: f64) -> Self { Self(self.0 * rhs) } }
        impl Div<f64> for $name { type Output = Self; fn div(self, rhs: f64) -> Self { Self(self.0 / rhs) } }
    };
}

si_quantity!(
    /// Distance in meters.
    Meters
);
si_quantity!(
    /// Mass in kilograms.
    Kilograms
);
si_quantity!(
    /// Duration in SI seconds.
    Seconds
);
si_quantity!(
    /// Thermodynamic temperature in kelvins.
    Kelvin
);
si_quantity!(
    /// Pressure in pascals.
    Pascals
);
si_quantity!(
    /// Power in watts.
    Watts
);
si_quantity!(
    /// Energy in joules.
    Joules
);
si_quantity!(
    /// Angle in radians.
    Radians
);
si_quantity!(
    /// Speed in meters per second.
    MetersPerSecond
);
