//! Time systems: the difference between *when the universe did something* and *what the clock on the wall
//! said*.
//!
//! ## Why this is a module and not a helper
//!
//! Every other module in this crate takes `dt` in seconds and returns `dt` in seconds, deliberately: a
//! propagator that never sees a date cannot get a date wrong. But a pass prediction is a **schedule**, and a
//! schedule is read off a clock. The moment this project prints "rise at 14:31:20" it has made a claim about
//! which 14:31:20 — and there are four of them, differing by up to 69 seconds in 2026.
//!
//! The four, in the order they must be converted:
//!
//! ```text
//! UTC  ── leap seconds ──►  TAI  ── 32.184 s ──►  TT  ── ≤1.658 ms periodic ──►  TDB
//! ```
//!
//! * **UTC** is what a clock reads. It has 60-second minutes and, historically, 61-second minutes.
//! * **TAI** is a uniform atomic count with no leap seconds. `TAI − UTC` is 37 s as of 2017-01-01, and it
//!   only ever increases.
//! * **TT** is TAI + 32.184 s, by definition — the offset is the epoch difference between the atomic and
//!   dynamical timescales, not an observation.
//! * **TDB** is the barycentric dynamical timescale, what the ephemerides and this kernel's `dt` use. It
//!   differs from TT by a **periodic** term of amplitude 1.658 ms caused by the Earth's elliptical orbit
//!   through the Sun's potential.
//!
//! ## The magnitude, because "it's only milliseconds" is the trap
//!
//! 1.658 ms sounds negligible. At low Earth orbit a satellite covers **12.7 m** in that time. A leap second
//! is **7.7 km** of along-track position. And the four time systems differ by 69 s in total, which is a
//! **different pass** rather than a late one.
//!
//! For comparison, the constant-precision residual documented in `tests/observe.rs` is 4.7 m. So the TDB
//! correction is `2.7×` the size of the error the project already went to some trouble to *state*. Applying
//! the big corrections and skipping the small one is not a coherent position; both are here.
//!
//! ## What is modelled, and what is refused
//!
//! * **Leap seconds: modelled exactly**, from a table that ships as data with its range declared. Before
//!   1972 UTC had no leap seconds and drifted against atomic time by an amount that is *not* a table of
//!   integer steps — it was a rubber-second system with fractional offsets. This module therefore
//!   **refuses** to convert dates before 1972 rather than extrapolate the table backwards and produce a
//!   number that looks authoritative.
//! * **TDB − TT: modelled to 1.658 ms** with the leading periodic term. The full expression is a
//!   Fairhead–Bretagnon series; the leading term captures the amplitude to a few tens of microseconds, and
//!   the residual is stated rather than implied.
//! * **UT1 − UTC is not modelled at all, and cannot be.** UT1 is the Earth's actual rotation angle; it is
//!   *measured*, published in advance by the IERS as a prediction, and revised afterwards. It differs from
//!   UTC by up to 0.9 s. This crate's frames rotate at a constant sidereal rate, so they are not UT1-aligned
//!   either — see `frames.rs`. The honest statement is: **this module does not convert to UT1, so it cannot
//!   be used to point a telescope.** Anything that needs Earth's true rotation angle needs a table that
//!   arrives with the data and expires.
//!
//! ## The future, which is the interesting part
//!
//! A game clock runs into the future, and leap seconds are not scheduled — they are announced about six
//! months ahead. So a conversion for a date after the table's end is not a computation, it is a **guess**,
//! and this module says so in the type: [`TimeError::BeyondLeapSecondTable`]. The caller can then decide
//! whether to present a time as provisional. A simulation that quietly extrapolated would show a player a
//! mission scheduled to the second, 40 years out, whose error is unknown and growing.

use odderon_dmath::sin;

/// A date and time in a calendar, before any timescale is applied.
///
/// Second `60` is a legal value: it is how a leap second is written (`23:59:60`), and refusing it would make
/// the one date this module exists for unrepresentable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Civil {
    /// Year, proleptic Gregorian. Negative for BCE, and `0` is 1 BCE — the astronomical convention, stated
    /// because the historical one has no year zero and the difference is a silent off-by-one.
    pub year: i32,
    /// Month, 1–12.
    pub month: u8,
    /// Day, 1–31.
    pub day: u8,
    /// Hour, 0–23.
    pub hour: u8,
    /// Minute, 0–59.
    pub minute: u8,
    /// Second, 0–60. `60` only occurs on a leap second.
    pub second: u8,
    /// Sub-second part, `[0, 1)`.
    pub fraction: f64,
}

/// Which timescale a number of seconds is expressed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scale {
    /// Coordinated Universal Time: the civil clock, with leap seconds.
    Utc,
    /// International Atomic Time: uniform, no leap seconds. `TAI − UTC = 37 s` in 2026.
    Tai,
    /// Terrestrial Time: `TAI + 32.184 s`. What the geocentric ephemerides are tabulated in.
    Tt,
    /// Barycentric Dynamical Time: `TT` plus a periodic ≤1.658 ms term. **This is the scale the kernel's
    /// `dt` is in**, per `VISION.md` §5.2.
    Tdb,
}

/// Why a conversion could not be made.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TimeError {
    /// The date is before 1972-01-01, where UTC was a rubber-second system and integer leap seconds are the
    /// wrong model. Refused rather than extrapolated.
    BeforeLeapSecondEra,
    /// The date is beyond the end of the shipped leap-second table.
    ///
    /// Not an error in the arithmetic: it is the statement that nobody has *announced* the leap seconds for
    /// that date yet. A caller that presents the result must present it as provisional.
    BeyondLeapSecondTable {
        /// The last date the table covers, as a `Civil`.
        table_ends: Civil,
    },
    /// A field is out of range: month 13, day 32, minute 61, non-finite fraction.
    BadCivil,
    /// `second == 60` on a day where the table records no leap second.
    ///
    /// Accepted on a leap day, refused otherwise: an invented `23:59:60` would be a *second* instant with no
    /// second in it, and the conversion would silently overlap the following midnight.
    NoLeapSecondOnThisDay,
    /// `TAI − UTC` would be negative, which no published table has ever produced.
    NegativeOffset,
}

// ---------------------------------------------------------------------------------------------
// The leap-second table
// ---------------------------------------------------------------------------------------------

/// One row of the leap-second table: from this UTC date onwards, `TAI − UTC` is [`tai_minus_utc`].
///
/// Public, because the table is *data the caller may need to present*, not an implementation detail. Anything that
/// shows a UTC clock has to be able to say "the conversion is authoritative through 2017-01-01 and provisional
/// after", and it cannot say that if the table is private.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeapSecond {
    /// Year the offset took effect, UTC.
    pub year: i32,
    /// Month the offset took effect, UTC.
    pub month: u8,
    /// Day of the month the offset took effect, UTC.
    pub day: u8,
    /// `TAI − UTC` in seconds, from this date onwards.
    pub tai_minus_utc: i32,
}

/// The leap-second table, as published by the IERS.
///
/// Each row is "from this UTC date onwards, `TAI − UTC` is this many seconds". The first row is the 1972
/// baseline — 10 s — and the last is 2017-01-01, where it became 37 s. Twenty-seven leap seconds have been
/// inserted since 1972, which is 27 rows after the baseline, and the table has been unchanged since 2017,
/// which is the longest gap in its history.
///
/// **This is data, not physics.** It ships as a constant because it is short and stable, but it expires: a
/// table that ends in 2017 is correct for every date up to 2017-01-01 and *unknown* after the last announced
/// leap second. The module treats "after the table" as a distinct answer from "in the table", never as an
/// extrapolation.
pub const LEAP_SECONDS: [LeapSecond; 28] = [
    LeapSecond { year: 1972, month: 1, day: 1, tai_minus_utc: 10 },
    LeapSecond { year: 1972, month: 7, day: 1, tai_minus_utc: 11 },
    LeapSecond { year: 1973, month: 1, day: 1, tai_minus_utc: 12 },
    LeapSecond { year: 1974, month: 1, day: 1, tai_minus_utc: 13 },
    LeapSecond { year: 1975, month: 1, day: 1, tai_minus_utc: 14 },
    LeapSecond { year: 1976, month: 1, day: 1, tai_minus_utc: 15 },
    LeapSecond { year: 1977, month: 1, day: 1, tai_minus_utc: 16 },
    LeapSecond { year: 1978, month: 1, day: 1, tai_minus_utc: 17 },
    LeapSecond { year: 1979, month: 1, day: 1, tai_minus_utc: 18 },
    LeapSecond { year: 1980, month: 1, day: 1, tai_minus_utc: 19 },
    LeapSecond { year: 1981, month: 7, day: 1, tai_minus_utc: 20 },
    LeapSecond { year: 1982, month: 7, day: 1, tai_minus_utc: 21 },
    LeapSecond { year: 1983, month: 7, day: 1, tai_minus_utc: 22 },
    LeapSecond { year: 1985, month: 7, day: 1, tai_minus_utc: 23 },
    LeapSecond { year: 1988, month: 1, day: 1, tai_minus_utc: 24 },
    LeapSecond { year: 1990, month: 1, day: 1, tai_minus_utc: 25 },
    LeapSecond { year: 1991, month: 1, day: 1, tai_minus_utc: 26 },
    LeapSecond { year: 1992, month: 7, day: 1, tai_minus_utc: 27 },
    LeapSecond { year: 1993, month: 7, day: 1, tai_minus_utc: 28 },
    LeapSecond { year: 1994, month: 7, day: 1, tai_minus_utc: 29 },
    LeapSecond { year: 1996, month: 1, day: 1, tai_minus_utc: 30 },
    LeapSecond { year: 1997, month: 7, day: 1, tai_minus_utc: 31 },
    LeapSecond { year: 1999, month: 1, day: 1, tai_minus_utc: 32 },
    LeapSecond { year: 2006, month: 1, day: 1, tai_minus_utc: 33 },
    LeapSecond { year: 2009, month: 1, day: 1, tai_minus_utc: 34 },
    LeapSecond { year: 2012, month: 7, day: 1, tai_minus_utc: 35 },
    LeapSecond { year: 2015, month: 7, day: 1, tai_minus_utc: 36 },
    LeapSecond { year: 2017, month: 1, day: 1, tai_minus_utc: 37 },
];

/// The last date the shipped table covers: 2017-01-01. After this, `TAI − UTC` is *known to be at least* 37 s
/// and *not known to be more*, which is a different statement from 37.
pub const LEAP_TABLE_ENDS: Civil = Civil {
    year: 2017,
    month: 1,
    day: 1,
    hour: 0,
    minute: 0,
    second: 0,
    fraction: 0.0,
};

/// Number of days from 1970-01-01 to a proleptic Gregorian date. Howard Hinnant's `days_from_civil`.
///
/// Verified by round trip over every day from 1800 to 2200 (146 098 days) with zero failures, and by three
/// known anchors: 1970-01-01 → 0, 2000-01-01 → 10957, 2026-11-22 → 20779.
///
/// Written with truncating division on purpose: `day` is 1-based and the algorithm subtracts one, which is
/// exact in the integers here. There is no `floor` involved, so the negative-year case (before 1970) cannot
/// round the wrong way.
fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let y = if month <= 2 { year as i64 - 1 } else { year as i64 };
    let m = month as i64;
    let d = day as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = if m > 2 { m - 3 } else { m + 9 }; // March = 0
    let doy = (153 * mp + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

/// The inverse: a proleptic Gregorian date from days since 1970-01-01.
fn civil_from_days(z: i64) -> (i32, u8, u8) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (if m <= 2 { y + 1 } else { y } as i32, m as u8, d as u8)
}

impl Civil {
    /// Construct, validating every field. `second` may be 60.
    pub fn new(
        year: i32,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
        fraction: f64,
    ) -> Result<Civil, TimeError> {
        if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return Err(TimeError::BadCivil);
        }
        if hour > 23 || minute > 59 || second > 60 {
            return Err(TimeError::BadCivil);
        }
        if !(0.0..1.0).contains(&fraction) {
            return Err(TimeError::BadCivil);
        }
        // Days in month, with the Gregorian rule spelled out: divisible by 4, except centuries, except
        // multiples of 400. `1900` is not a leap year, `2000` is — the two cases a naive `% 4` gets wrong.
        let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
        let max_day = match month {
            2 if leap => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        };
        if day > max_day {
            return Err(TimeError::BadCivil);
        }
        Ok(Civil {
            year,
            month,
            day,
            hour,
            minute,
            second,
            fraction,
        })
    }

    /// Days since 1970-01-01 for this date. The time of day is not included.
    pub fn days_since_1970(&self) -> i64 {
        days_from_civil(self.year, self.month, self.day)
    }

    /// A `Civil` from days since 1970-01-01 plus a second of day and fraction.
    fn from_days_and_second(days: i64, second_of_day: i64, fraction: f64) -> Civil {
        let (year, month, day) = civil_from_days(days);
        Civil {
            year,
            month,
            day,
            hour: (second_of_day / 3600) as u8,
            minute: ((second_of_day / 60) % 60) as u8,
            second: (second_of_day % 60) as u8,
            fraction,
        }
    }
}

/// `TAI − UTC` in seconds, for a UTC instant expressed as a **naive** scalar (seconds since
/// 1970-01-01T00:00:00, counting each day as 86 400 s and ignoring leaps).
///
/// The naive scalar is the right index into the table and the wrong thing to do arithmetic on. That
/// distinction is the entire content of [`utc_to_tai`].
fn tai_minus_utc_at(days: i64) -> Result<i32, TimeError> {
    // The 1972 baseline, by day number. Anything earlier is refused, not clamped: UTC before 1972 used
    // fractional offsets, and clamping would silently apply a 10 s step to a date where no such step
    // existed.
    let baseline = days_from_civil(1972, 1, 1);
    if days < baseline {
        return Err(TimeError::BeforeLeapSecondEra);
    }
    let mut offset = LEAP_SECONDS[0].tai_minus_utc;
    let mut i = 0;
    while i < LEAP_SECONDS.len() {
        let row = &LEAP_SECONDS[i];
        if days_from_civil(row.year, row.month, row.day) <= days {
            offset = row.tai_minus_utc;
        }
        i += 1;
    }
    Ok(offset)
}

/// The day number of the last date the table covers: 2017-01-01 is `17 167` days after 1970-01-01.
///
/// A literal rather than a call to `days_from_civil`, because the two must be checked against each other by a
/// test rather than by one deriving the other — a constant that derives its own index cannot be wrong in a way
/// anything notices. `tests/time.rs` asserts they agree.
const LEAP_TABLE_END_DAY: i64 = 17_167;

/// True when the date is beyond the shipped table's authority.
///
/// The last date *in* the table is inclusive: at 2017-01-01T00:00:00 the offset that row establishes is in
/// force, and that is a fact the table states. What is unknown is any date after it, because no later leap
/// second has been announced.
fn beyond_table(days: i64) -> bool {
    days > LEAP_TABLE_END_DAY
}

/// The TAI count — seconds since 1970-01-01T00:00:00 TAI — at the start of a UTC day.
///
/// This is the index the inverse searches on. Indexing by the *UTC* day a reading falls in, rather than by a
/// naive scalar, is what makes the leap second representable: `23:59:60` on a leap day and `00:00:00` on the
/// next day are the same point in TAI, and only the day they are filed under tells them apart.
fn tai_start_of_day(day: i64) -> Option<f64> {
    let offset = tai_minus_utc_at(day).ok()?;
    Some(day as f64 * 86_400.0 + offset as f64)
}

/// Seconds by which the day is longer than 24 h: 0, or 1 on a leap day.
fn leap_seconds_on(day: i64) -> u8 {
    let here = tai_minus_utc_at(day).unwrap_or(0);
    let next = tai_minus_utc_at(day + 1).unwrap_or(here);
    if next > here {
        (next - here) as u8
    } else {
        0
    }
}

// ---------------------------------------------------------------------------------------------
// The conversions
// ---------------------------------------------------------------------------------------------

/// The **TT** reading of J2000, in seconds on the naive 1970 scalar: `946 728 000` — exactly.
///
/// J2000 is *defined* as 2000-01-01T12:00:00 **TT**, so its TT reading is the plain count of days and seconds
/// from 1970-01-01T00:00:00: `10 957 · 86 400 + 43 200 = 946 728 000`. There is no offset in this constant and
/// there must not be one, because the definition already chose the timescale.
///
/// **Two ways to get this wrong, both tried.** The first version subtracted `32.184` from the constant *and*
/// added `TT − TAI` in the conversion, which is the same 32.184 s twice. The second removed the subtraction from
/// the constant but left it in the conversion — a different route to the same 32.184 s. Both produced a
/// conversion that is self-consistent to a tenth of a microsecond in both directions, and both put every date in
/// the game 32 seconds off J2000. The measurement that caught it: `utc_to_tdb(2000-01-01T11:58:55.816)` must be
/// ~0 and was 32.1839.
///
/// The correct relation, with every term named:
///
/// ```text
/// N_utc                     the naive scalar of the UTC reading
/// N_utc + (TAI − UTC)       the TAI reading
/// N_utc + (TAI − UTC) + 32.184    the TT reading
/// minus 946 728 000         seconds since J2000, in TT
/// ```
const SECONDS_1970_TO_J2000_TT: f64 = 946_728_000.0;

/// Convert a civil UTC reading to **TDB seconds past J2000**, the scale the kernel's `dt` is in.
///
/// This is the one function a caller needs to turn a wall clock into a time the propagator accepts, and the
/// only place in the crate where leap seconds matter.
///
/// # Errors
///
/// * [`TimeError::BeforeLeapSecondEra`] — before 1972-01-01.
/// * [`TimeError::BeyondLeapSecondTable`] — after 2017-01-01. **Deliberately an error rather than an
///   extrapolation.** Leap seconds are announced about six months ahead; a date past the table is not a
///   computation, it is a guess, and the caller should be able to see that.
/// * [`TimeError::NoLeapSecondOnThisDay`] — a `23:59:60` on a day the table does not record a leap second.
///
/// Presenting a TDB value as a UTC clock reading needs the inverse, [`tdb_to_utc`], and the caller is
/// responsible for showing the provenance of the leap-second table when the date is anywhere near its end.
pub fn utc_to_tdb(civil: &Civil) -> Result<f64, TimeError> {
    let days = civil.days_since_1970();
    if beyond_table(days) {
        return Err(TimeError::BeyondLeapSecondTable {
            table_ends: LEAP_TABLE_ENDS,
        });
    }
    let offset = tai_minus_utc_at(days)?;
    // `23:59:60` only exists on a leap day. Accepting it anywhere else makes two instants share one scalar, which
    // is the whole failure the leap-second table exists to prevent.
    if civil.second == 60 && leap_seconds_on(days) == 0 {
        return Err(TimeError::NoLeapSecondOnThisDay);
    }

    // The calendar scalar: the day number, the offset in force, and the second of day. On a leap day
    // `23:59:60` is exactly second 86 400, which is why it lands where the following midnight would otherwise
    // be — and why the following midnight is at 86 401 there.
    let second_of_day = civil.hour as f64 * 3600.0
        + civil.minute as f64 * 60.0
        + civil.second as f64
        + civil.fraction;
    let tai = days as f64 * 86_400.0 + offset as f64 + second_of_day;

    // TAI -> TT is the definition; TT -> TDB is the periodic term. `S_J` is the *TT* reading of J2000, so this
    // single subtraction is the whole epoch conversion.
    let tt_since_j2000 = tai + 32.184 - SECONDS_1970_TO_J2000_TT;
    Ok(tt_since_j2000 + tdb_minus_tt(tt_since_j2000))
}

/// `TDB − TT` in seconds, to the leading periodic term.
///
/// The dominant term is the Earth's orbital eccentricity through the Sun's potential:
///
/// ```text
/// TDB − TT = 0.001657 sin(g + 0.0167 sin g),  g = 357.53° + 0.9856003° · (JD − 2451545)
/// ```
///
/// Public because a caller who has a TDB time and wants to print a UTC clock needs it, and because hiding it
/// would leave the tests measuring it through two subtractions of 8.5e8-second numbers, which resolves to a
/// microsecond and would hide a factor-of-two error.
///
/// This is the standard engineering form (Fairhead & Bretagnon 1990, truncated). Its amplitude is **1.658 ms**
/// — 12.7 m of along-track LEO motion, and 2.7× the 4.7 m residual the project already documents for the
/// rotation rate. The omitted terms are tens of microseconds, i.e. under a metre at LEO, and they are
/// omitted because a full series is several hundred terms for a correction below the noise of the positions
/// it feeds.
pub fn tdb_minus_tt(seconds_since_j2000_tt: f64) -> f64 {
    let days = seconds_since_j2000_tt / 86_400.0;
    let mean_anomaly_deg = 357.53 + 0.985_600_3 * days;
    let g = mean_anomaly_deg * core::f64::consts::PI / 180.0;
    // `g` grows without bound for dates far from J2000 — 0.9856°/day is 360°/year — so reduce it before the
    // sine. The kernel's own `sin` reduces internally, but the *argument* here is what loses precision: at
    // 100 years out, `g` is 3.6e4 rad and the fractional part of it carries only 12 digits.
    let g = reduce_degrees(mean_anomaly_deg) * core::f64::consts::PI / 180.0;
    0.001_657 * sin(g + 0.016_7 * sin(g))
}

/// Reduce an angle in degrees to `[0, 360)`.
fn reduce_degrees(deg: f64) -> f64 {
    let q = deg / 360.0;
    let n = if q >= 0.0 { q as i64 } else { (q as i64) - 1 };
    let r = deg - (n as f64) * 360.0;
    if r < 0.0 {
        r + 360.0
    } else if r >= 360.0 {
        r - 360.0
    } else {
        r
    }
}

/// Convert **TDB seconds past J2000** back to a UTC civil reading.
///
/// The inverse of [`utc_to_tdb`]. Iterative rather than algebraically inverted, because the leap-second offset
/// is a step function of UTC and inverting a step function in closed form is how a leap second gets skipped
/// twice. The iteration is a fixed point on the `TDB − TT` term, which is a 1.658 ms correction; it runs a
/// fixed three times so the answer cannot depend on a convergence comparison.
pub fn tdb_to_utc(tdb_seconds_since_j2000: f64) -> Result<Civil, TimeError> {
    if !tdb_seconds_since_j2000.is_finite() {
        return Err(TimeError::BadCivil);
    }

    // TDB = TT + periodic(TT), so invert it: TT = TDB − periodic(TT). A fixed three iterations of a 1.658 ms
    // correction converges to the last bit, and a fixed count cannot make the result depend on a rounding
    // comparison.
    let mut tt = tdb_seconds_since_j2000;
    let mut pass = 0;
    while pass < 3 {
        pass += 1;
        tt = tdb_seconds_since_j2000 - tdb_minus_tt(tt);
    }

    // Undo both offsets at once: TT = TAI + 32.184 − S_J.
    let tai = tt + SECONDS_1970_TO_J2000_TT - 32.184;
    let guess = floor_div(tai, 86_400.0);

    // Three candidate days, earliest first. The order is the whole trick: `23:59:60` on a leap day and
    // `00:00:00` on the day after are the same TAI instant, and both are in range for *some* candidate. Asking
    // the earlier day first reports the leap second and never the duplicate — so the inverse always says
    // `23:59:60` rather than dropping the second, which is the behaviour `tests/time.rs` pins.
    for day in [guess - 1, guess, guess + 1] {
        let start = match tai_start_of_day(day) {
            Some(s) => s,
            // Before 1972 there is no offset, so there is no valid answer to fall back to.
            None => continue,
        };
        let second_of_day = tai - start;
        let limit = 86_400.0 + leap_seconds_on(day) as f64;
        if second_of_day >= 0.0 && second_of_day < limit {
            let (year, month, day_of_month) = civil_from_days(day);
            if second_of_day >= 86_400.0 {
                return Ok(Civil {
                    year,
                    month,
                    day: day_of_month,
                    hour: 23,
                    minute: 59,
                    second: 60,
                    fraction: second_of_day - 86_400.0,
                });
            }
            let whole = second_of_day as i64;
            return Ok(Civil::from_days_and_second(
                day,
                whole,
                second_of_day - whole as f64,
            ));
        }
    }
    Err(TimeError::BadCivil)
}

/// `floor(a / b)` for `b > 0`, without `f64::floor`.
///
/// The cast truncates toward zero, which is `floor` for a positive quotient and one too high for a negative
/// one. The core check is therefore "is the truncation below the true value", which is the same as
/// `q != n` — spelling it that way keeps the function correct for exact negative integers too, where
/// `q < 0.0` alone would subtract one from a quotient that was already right.
fn floor_div(a: f64, b: f64) -> i64 {
    let q = a / b;
    let n = q as i64;
    if q < (n as f64) {
        n - 1
    } else {
        n
    }
}

/// Convert a number of seconds on one scale to another, for the pairs that are a constant offset.
///
/// `UTC` is deliberately not accepted here: converting to or from UTC needs the leap-second table and an
/// epoch, which is [`utc_to_tdb`]'s job. Offering it through a "simple" function is how a leap second gets
/// ignored by a caller who thought they were converting timescales.
pub fn convert_constant_offset(seconds: f64, from: Scale, to: Scale) -> Option<f64> {
    let tt_minus_tai = 32.184;
    let base = |s: Scale| match s {
        Scale::Tai => Some(0.0),
        Scale::Tt => Some(tt_minus_tai),
        // TDB is TT plus a periodic term that needs a date, not an offset; refused here on purpose.
        Scale::Tdb => None,
        Scale::Utc => None,
    };
    let a = base(from)?;
    let b = base(to)?;
    Some(seconds + (b - a))
}
