//! UTC calendar input to TDB seconds since J2000.0.
//!
//! The leap table is a fixed historical snapshot (NAIF naif0012). For dates
//! before 1972, ΔAT is clamped to 10 s; after the last entry it is held at
//! 37 s. These are extrapolations, NOT historical or future UTC truth. The
//! calendar is proleptic Gregorian with astronomical year numbering.
//!
//! The NAIF periodic TDB−TT approximation ignores small terms and is accurate
//! to about 30 microseconds, not nanoseconds. See the [SPICE time required
//! reading](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/req/time.html).

use crate::epoch::Epoch;

/// Gregorian UTC calendar fields. `second = 60` is accepted only during a
/// known positive leap second. Fraction must be finite and in [0, 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UtcDateTime {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub fraction: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UtcError {
    InvalidCalendar,
    InvalidLeapSecond,
    OutsideTableCoverage,
    NumericalRange,
}

/// Effective dates and TAI−UTC values, from NAIF naif0012.tls (2016-07-14).
const LEAPS: [(i32, u8, u8, i64); 28] = [
    (1972, 1, 1, 10),
    (1972, 7, 1, 11),
    (1973, 1, 1, 12),
    (1974, 1, 1, 13),
    (1975, 1, 1, 14),
    (1976, 1, 1, 15),
    (1977, 1, 1, 16),
    (1978, 1, 1, 17),
    (1979, 1, 1, 18),
    (1980, 1, 1, 19),
    (1981, 7, 1, 20),
    (1982, 7, 1, 21),
    (1983, 7, 1, 22),
    (1985, 7, 1, 23),
    (1988, 1, 1, 24),
    (1990, 1, 1, 25),
    (1991, 1, 1, 26),
    (1992, 7, 1, 27),
    (1993, 7, 1, 28),
    (1994, 7, 1, 29),
    (1996, 1, 1, 30),
    (1997, 7, 1, 31),
    (1999, 1, 1, 32),
    (2006, 1, 1, 33),
    (2009, 1, 1, 34),
    (2012, 7, 1, 35),
    (2015, 7, 1, 36),
    (2017, 1, 1, 37),
];

impl UtcDateTime {
    /// Convert using the documented extrapolated leap table and NAIF approximation.
    pub fn to_tdb(self) -> Result<Epoch, UtcError> {
        if !(1..=12).contains(&self.month)
            || self.day == 0
            || self.day > days_in_month(self.year, self.month)
            || self.hour > 23
            || self.minute > 59
            || self.second > 60
            || !self.fraction.is_finite()
            || !(0.0..1.0).contains(&self.fraction)
        {
            return Err(UtcError::InvalidCalendar);
        }
        let day = days_from_civil(self.year, self.month, self.day);
        let mut offset = 10;
        for &(y, m, d, tai_minus_utc) in &LEAPS {
            if days_from_civil(y, m, d) <= day {
                offset = tai_minus_utc;
            } else {
                break;
            }
        }
        if self.second == 60 {
            let valid = self.hour == 23
                && self.minute == 59
                && LEAPS.iter().any(|&(y, m, d, next)| {
                    days_from_civil(y, m, d) == day + 1 && next == offset + 1
                });
            if !valid {
                return Err(UtcError::InvalidLeapSecond);
            }
        }
        let days_since_2000 = day - days_from_civil(2000, 1, 1);
        let civil_seconds = days_since_2000 * 86_400 - 43_200
            + i64::from(self.hour) * 3_600
            + i64::from(self.minute) * 60
            + i64::from(self.second);
        // TAI = UTC + ΔAT; TT = TAI + 32.184 s. The leap second label
        // uses the preceding day's ΔAT, not the next day's offset.
        let tt = Epoch::new(civil_seconds + offset, self.fraction + 32.184)
            .map_err(|_| UtcError::NumericalRange)?;
        // NAIF: TDB − TT = K sin(M + EB sin(M)), M = M0 + M1*TDB.
        // Solve its implicit TDB argument by fixed-point iteration.
        let tt_seconds = tt.seconds() as f64 + tt.fraction();
        let mut correction = 0.0;
        for _ in 0..3 {
            let m = 6.239_996 + 1.990_968_71e-7 * (tt_seconds + correction);
            correction = 1.657e-3 * libm::sin(m + 1.671e-2 * libm::sin(m));
        }
        tt.checked_add_seconds(correction)
            .map_err(|_| UtcError::NumericalRange)
    }
}

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 0,
    }
}

// Proleptic Gregorian civil date -> days since 1970-01-01, integer-only.
// Algorithm: Howard Hinnant, https://howardhinnant.github.io/date_algorithms.html
fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let y = i64::from(year) - i64::from(month <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = i64::from(month) + if month > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}
