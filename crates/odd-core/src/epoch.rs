//! Barycentric Dynamical Time (TDB), relative to J2000.0.
//!
//! UTC is deliberately not accepted: its leap seconds and TT–TDB conversion
//! need an explicitly versioned, independently validated time model.

use crate::consts::J2000_JULIAN_DATE_TDB;

/// Integer seconds plus a normalized fractional second on the TDB time scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Epoch {
    seconds: i64,
    fraction: f64,
}

/// A non-finite value or a time outside the representable integer-second range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EpochRangeError;

impl Epoch {
    pub const J2000: Self = Self { seconds: 0, fraction: 0.0 };

    /// Constructs an epoch, normalizing `fraction` into [0, 1).
    pub fn new(seconds: i64, fraction: f64) -> Result<Self, EpochRangeError> {
        if !fraction.is_finite() {
            return Err(EpochRangeError);
        }
        let whole = fraction.floor();
        // i64::MAX as f64 rounds to 2^63; the upper bound must be exclusive.
        if whole < i64::MIN as f64 || whole >= 9_223_372_036_854_775_808.0 {
            return Err(EpochRangeError);
        }
        let mut seconds = seconds.checked_add(whole as i64).ok_or(EpochRangeError)?;
        let mut fraction = fraction - whole;
        // Rounding can produce exactly 1 for a tiny negative fraction.
        if fraction >= 1.0 {
            seconds = seconds.checked_add(1).ok_or(EpochRangeError)?;
            fraction = 0.0;
        }
        // Canonicalize negative zero for stable future serialization.
        if fraction == 0.0 {
            fraction = 0.0;
        }
        Ok(Self { seconds, fraction })
    }

    /// A single f64 Julian date loses subsecond precision; use `new` when
    /// precision matters. The supplied date must already be in TDB, not UTC.
    pub fn from_julian_date_tdb(jd: f64) -> Result<Self, EpochRangeError> {
        Self::new(0, (jd - J2000_JULIAN_DATE_TDB) * 86_400.0)
    }

    pub fn seconds(self) -> i64 { self.seconds }
    pub fn fraction(self) -> f64 { self.fraction }

    /// Adds a duration in SI seconds. Non-finite values and overflow are errors.
    pub fn checked_add_seconds(self, duration: f64) -> Result<Self, EpochRangeError> {
        if !duration.is_finite() {
            return Err(EpochRangeError);
        }
        let whole = duration.floor();
        if whole < i64::MIN as f64 || whole >= 9_223_372_036_854_775_808.0 {
            return Err(EpochRangeError);
        }
        let fraction = self.fraction + (duration - whole);
        let carry = if fraction >= 1.0 { 1 } else { 0 };
        let seconds = i128::from(self.seconds) + i128::from(whole as i64) + carry;
        let seconds = i64::try_from(seconds).map_err(|_| EpochRangeError)?;
        Self::new(seconds, fraction - carry as f64)
    }

    /// Signed difference in seconds. Very large differences lose fractional
    /// precision when represented as a single f64.
    pub fn seconds_since(self, other: Self) -> f64 {
        (i128::from(self.seconds) - i128::from(other.seconds)) as f64
            + (self.fraction - other.fraction)
    }
}
