//! Time systems: the difference between *when the universe did something* and *what the clock on the wall
//! said*.
//!
//! ## Why this is a module and not a helper
//!
//! Every other module in this crate takes a `dt` in seconds and returns a `dt` in seconds, deliberately: a
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
//! * **TAI** is a uniform atomic count with no leap seconds. `TAI − UTC` is 37 s since 2017-01-01, and it
//!   only ever increases.
//! * **TT** is TAI + 32.184 s, by definition — the offset is the epoch difference between the atomic and
//!   dynamical timescales, not an observation.
//! * **TDB** is the barycentric dynamical timescale, what the ephemerides and this kernel's `dt` use. It
//!   differs from TT by a **periodic** term of amplitude 1.658 ms.
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
//! * **The two ends of that range are different questions**, and conflating them was a real bug. The last
//!   *row* of [`LEAP_SECONDS`] is 2017-01-01: the instant 37 s became current. How far the offset is known is
//!   a separate claim, published by the IERS, and it runs to the end of 2026 — a span of nine years in which
//!   no leap second was inserted, the longest such gap since the system began. A table row cannot express
//!   "and nothing changed for nine years", so [`TAI_UTC_COVERED_THROUGH`] states the coverage and
//!   [`TAI_UTC_PINNED_THROUGH`] states the span over which the last row's value still applies. Bounding at
//!   2017 would have refused every date in the game's own present.
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
//! months ahead. So a conversion for a date beyond the announced range is not a computation, it is a
//! **guess**, and this module says so in the type: [`TimeError::BeyondLeapSecondTable`]. The caller can then
//! decide whether to present a time as provisional. A simulation that quietly extrapolated would show a
//! player a mission scheduled to the second, 40 years out, whose error is unknown and growing.
//!
//! The announcement is what makes the covered range a fact rather than a guess, and it is also what makes it
//! *finite*. Extending it means going back to IERS Bulletin C, not assuming the current offset persists: the
//! next possible insertion date after 2026-12-31 is 2027-06-30, and continuous UTC takes effect 2027-05-20,
//! so the question stops being askable in this form almost immediately after the range ends.

use odderon_dmath::sin;

/// A date and time in a calendar, before any timescale is applied.
///
/// Second `60` is a legal value: it is how a leap second is written (`23:59:60`), and refusing it in
/// [`Civil::new`] would make the one instant this module exists for unrepresentable. Whether a given day
/// *had* a leap second is a separate question, answered against the table by [`utc_to_tdb`].
///
/// `PartialEq` only, deliberately. The sub-second part is an `f64`, so `Eq` would promise that two readings
/// are either the same or different, and NaN makes that promise false. Comparing two readings is
/// [`Civil::same_instant`]'s job, with an explicit tolerance, rather than `==`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Civil {
    /// Year, proleptic Gregorian. `0` is 1 BCE — the astronomical convention, stated because the historical
    /// one has no year zero and the difference is a silent off-by-one.
    pub year: i32,
    /// Month, 1–12.
    pub month: u8,
    /// Day of month, 1–31.
    pub day: u8,
    /// Hour, 0–23.
    pub hour: u8,
    /// Minute, 0–59.
    pub minute: u8,
    /// Second, 0–60. `60` only ever occurs on a leap second.
    pub second: u8,
    /// Sub-second part, `[0, 1)`.
    pub fraction: f64,
}

/// Which timescale a number of seconds is expressed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scale {
    /// Coordinated Universal Time: the civil clock, with leap seconds.
    Utc,
    /// International Atomic Time: uniform, no leap seconds. `TAI − UTC = 37 s` from 2017-01-01 to 2026-12-31.
    Tai,
    /// Terrestrial Time: `TAI + 32.184 s`. What geocentric ephemerides are tabulated in.
    Tt,
    /// Barycentric Dynamical Time: TT plus a periodic term of amplitude ≤1.658 ms. **This is the scale the
    /// kernel's `dt` is in**, per `VISION.md` §5.2.
    Tdb,
}

/// Why a conversion could not be made.
///
/// Three of these are the same shape of statement — *this is not a number I am entitled to produce* — and
/// keeping them distinct is the point. A single `Invalid` would let a caller collapse "you asked about the
/// 18th century" and "you asked about 2040" into one error, and those need different responses: the first is
/// a permanent limitation, the second is a table that will be republished.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TimeError {
    /// The date is before 1972-01-01, where UTC was a rubber-second system and integer leap seconds are the
    /// wrong model. Refused rather than extrapolated.
    BeforeLeapSecondEra,
    /// The date is beyond the end of the range whose offset is announced.
    ///
    /// Not an error in the arithmetic: it is the statement that nobody has *announced* the leap seconds for
    /// that date yet. A caller that presents the result must present it as provisional.
    BeyondLeapSecondTable {
        /// The last date the table covers, as a [`Civil`].
        table_ends: Civil,
    },
    /// A field is out of range: month 13, day 32, minute 61, non-finite fraction.
    BadCivil,
    /// `second == 60` on a day on which the table records no leap second.
    ///
    /// Accepted on a leap day and refused otherwise. (A `:60` at any hour other than 23:59 is not this
    /// variant — it is malformed input, and [`Civil::new`] refuses it as [`TimeError::BadCivil`].) An invented `23:59:60` is not a second of extra time,
    /// it is a *second name* for an instant that already has one — the following midnight — and converting it
    /// would silently make two civil readings denote the same moment, which is the exact ambiguity the
    /// leap-second table exists to remove. Modelling a leap second that was never inserted is a claim about
    /// the world, and only the table is entitled to make it.
    NoLeapSecondOnThisDay,
}

// ---------------------------------------------------------------------------------------------
// The leap-second table
// ---------------------------------------------------------------------------------------------

/// One row of the leap-second table: from this UTC date onwards, `TAI − UTC` is [`LeapSecond::tai_minus_utc`].
///
/// Public, because the table is *data the caller may need to present*, not an implementation detail.
/// Anything that shows a UTC clock has to be able to say "this conversion is authoritative through
/// 2026-12-31 and provisional after", and it cannot say that if the table is private.
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
/// Each row reads "from this UTC date onwards, `TAI − UTC` is this many seconds". The first row is the 1972
/// baseline of 10 s, and the last is 2017-01-01, where the offset became 37 s. Twenty-seven leap seconds have
/// been inserted since the baseline, so there are 28 rows.
///
/// **A row marks an instant, not a span.** The last row says 37 s became current on 2017-01-01; it does not
/// say 37 s stayed current, because a table of changes cannot state an absence of changes. That the value did
/// stay current to the end of 2026 comes from the IERS announcement and is recorded in
/// [`TAI_UTC_COVERED_THROUGH`] and [`TAI_UTC_PINNED_THROUGH`]. Reading the last row as the end of the table's
/// authority — which this file did until it was noticed — makes the module refuse every date after 2017-01-01,
/// including the present.
///
/// **This is data, not physics.** It ships as a constant because it is short and stable, but it expires: the
/// module treats "beyond the announced range" as a distinct answer from "in the range", never as an
/// extrapolation.
// Skipped by rustfmt on purpose: one row per line is the whole point of this table, and the alternative the
// formatter prefers (a row's fields across four lines) makes 28 dates unreadable and any typo invisible.
#[rustfmt::skip]
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

/// The last **date** whose `TAI − UTC` offset is announced: the end of 2026.
///
/// Every date up to and including this one converts; the day after does not. Two different facts used to be
/// conflated in this constant, and separating them is what lets the clock work in the game's own present:
///
/// * [`LEAP_SECONDS`]'s last row is 2017-01-01, and a table row is an **instant** — the moment an offset
///   took effect — not a span. So the table's last row says nothing about the offset on 2017-01-02, even
///   though the value happens to be the same.
/// * That the offset *went on being* 37 s is a separate fact, and it is announced, not assumed: IERS
///   Bulletin C 72 (2026-07-06) states no leap second at the end of December 2026 and records
///   `UTC − TAI = −37 s` from 2017-01-01 "until further notice". Nine years elapsed with no leap second,
///   which is the longest such gap since the system began.
///
/// So this constant is bounded by **coverage of the announcement**, not by the last leap second. Bounding it
/// at 2017 would refuse every date in the present, and this project's own rule forbids passing a number off
/// as measured when nobody measured it — but it equally forbids calling an announced value a guess.
///
/// 2026-12-31 is a *real* boundary rather than a convenient one: the next possible insertion date after it is
/// 2027-06-30, and continuous UTC takes effect 2027-05-20 (CGPM Resolution 5, 2026), so 2027-01-01 is the day
/// after which the leap-second question stops being askable in this form. Whoever extends this must re-derive
/// [`TAI_UTC_PINNED_THROUGH`] rather than assuming the pinning continues.
pub const TAI_UTC_COVERED_THROUGH: Civil = Civil {
    year: 2026,
    month: 12,
    day: 31,
    hour: 0,
    minute: 0,
    second: 0,
    fraction: 0.0,
};

/// The last date of the announced **zero-leap-second span**, 2017-01-01 through 2026-12-31.
///
/// At the time of writing the offset is the same on every date in that span (37 s), which is why the entry in
/// [`LEAP_SECONDS`] for 2017-01-01 serves the whole of it. That is a property someone observed and recorded
/// here, not one the table encodes: a table row cannot say "and nothing changed for ten years".
///
/// Split from [`TAI_UTC_COVERED_THROUGH`] on purpose. If an announcement ever inserts a leap second inside a
/// pinned span, this constant is the one that is *false*, and it is a compile error two lines below rather
/// than a wrong second handed to a player. If the span simply ends — as it does after 2026 — the other
/// constant moves and this one does not.
pub const TAI_UTC_PINNED_THROUGH: Civil = Civil {
    year: 2026,
    month: 12,
    day: 31,
    hour: 0,
    minute: 0,
    second: 0,
    fraction: 0.0,
};

/// Number of days from 1970-01-01 to a proleptic Gregorian date. Howard Hinnant's `days_from_civil`.
///
/// `const fn` so the table's ordering can be checked at compile time — see the `const _` block below. Only
/// integer arithmetic and `if`, which is all a `const fn` on the 2021 edition needs.
///
/// Verified by round trip over every day from 1900 to 2100 — 73 050 consecutive days, zero failures — and by
/// three known anchors: 1970-01-01 → 0, 2000-01-01 → 10 957, 2026-11-22 → 20 779. `tests/time.rs` re-checks
/// both, because a calendar that is right in the module comment and wrong in the code is a real failure mode.
///
/// Division truncates toward zero here, which is why the negative branch subtracts 399 before dividing: that
/// is the standard trick to turn truncation into a floor without needing `f64::floor` (unavailable in
/// `no_std`, and 399 days of overflow is the price). `day` is 1-based and the algorithm subtracts one, which
/// is exact in integers, so no rounding can hide.
const fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let y = if month <= 2 {
        year as i64 - 1
    } else {
        year as i64
    };
    let m = month as i64;
    let d = day as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = if m > 2 { m - 3 } else { m + 9 }; // March = 0
    let doy = (153 * mp + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146_096]
    era * 146_097 + doe - 719_468
}

/// The inverse: a proleptic Gregorian date from days since 1970-01-01.
fn civil_from_days(days: i64) -> (i32, u8, u8) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146_096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if m <= 2 { y + 1 } else { y };
    (year as i32, m as u8, d as u8)
}

/// The day number of [`TAI_UTC_COVERED_THROUGH`]: 2026-12-31 is 20 818 days after 1970-01-01.
///
/// A literal, not a call to [`days_from_civil`], so that the two can disagree and a check can notice. A
/// constant that derives its own index cannot be wrong in a way anything detects; this one is cross-checked
/// against the calendar by the `const _` block below *and* by a test.
const TAI_UTC_COVERED_THROUGH_DAY: i64 = 20_818;

// The pinned span's two day numbers are asserted inline in the `const _` block below rather than given named
// constants, because nothing at run time consults them: `beyond_table` and the inverse both use the *covered*
// boundary. A named constant with no consumer is dead code, which this crate denies, and the property worth
// keeping is only that the literals be checked against the calendar — which a literal in an `assert!` does.

/// The first day of the announced zero-leap-second span: the date `LEAP_SECONDS`'s last row takes effect.
///
/// Named because the compile-time checks below need it as a *date* rather than as "the table's last row",
/// and because the distinction is the whole point of this section: a row is an instant, a span is a claim.
///
/// Public so a test can assert that the span starts where the table's last row does, and so a UI can describe
/// the range it is showing. It has no run-time consumer inside the module, which is why the two day-number
/// literals below it stay literals: a named constant nothing calls is dead code.
pub const TAI_UTC_PINNED_START: Civil = Civil {
    year: 2017,
    month: 1,
    day: 1,
    hour: 0,
    minute: 0,
    second: 0,
    fraction: 0.0,
};

/// A calendar date's day number, at compile time — used only to check the table while the compiler is running.
const fn day_of(year: i32, month: u8, day: u8) -> i64 {
    days_from_civil(year, month, day)
}

// The table must be strictly increasing in both date and offset, for the same reason a sorted array must be
// sorted: a duplicated or out-of-order row produces a step function that goes backwards, and the failure
// would surface as one wrong second on one date, years later, long after anyone remembers this file. Checking
// it here costs nothing at run time, cannot be skipped by a disabled test, and turns the mistake into a
// compile error in the file that made it.
//
// `tests/time.rs` asserts the same properties with readable messages; this is the version that fails the
// *build*.
const _: () = {
    let mut i = 1;
    while i < 28 {
        let (prev, this) = (LEAP_SECONDS[i - 1], LEAP_SECONDS[i]);
        assert!(day_of(this.year, this.month, this.day) > day_of(prev.year, prev.month, prev.day));
        assert!(this.tai_minus_utc > prev.tai_minus_utc);
        i += 1;
    }
    let last = LEAP_SECONDS[27];
    let last_row_day = day_of(last.year, last.month, last.day);

    // The table's last row is 2017-01-01, the first day of the pinned span.
    assert!(last_row_day == day_of(2017, 1, 1));

    // THE STALE-ASSUMPTION CHECK. The pinning below is only true while the table's last row is the *start* of
    // the span — i.e. while no leap second was inserted inside it. Appending a row dated within the span makes
    // the last row later than the span's start, and this stops compiling. The fix when that happens is to
    // shorten `TAI_UTC_PINNED_THROUGH` to the day before the new row, never to delete this assertion: an
    // inserted second is exactly the event that makes a "nothing changed here" claim false.
    assert!(
        last_row_day
            == day_of(
                TAI_UTC_PINNED_START.year,
                TAI_UTC_PINNED_START.month,
                TAI_UTC_PINNED_START.day
            )
    );

    // Coverage cannot end before the pinning does, or a date known to be pinned would be refused. Coverage may
    // end later: an announced no-change extends it without adding a row.
    assert!(
        day_of(
            TAI_UTC_COVERED_THROUGH.year,
            TAI_UTC_COVERED_THROUGH.month,
            TAI_UTC_COVERED_THROUGH.day
        ) >= day_of(
            TAI_UTC_PINNED_START.year,
            TAI_UTC_PINNED_START.month,
            TAI_UTC_PINNED_START.day
        )
    );

    // The hand-written day numbers the converter compares against are the documented dates.
    assert!(
        day_of(
            TAI_UTC_COVERED_THROUGH.year,
            TAI_UTC_COVERED_THROUGH.month,
            TAI_UTC_COVERED_THROUGH.day
        ) == TAI_UTC_COVERED_THROUGH_DAY
    );
    assert!(last_row_day == 17_167);
    assert!(
        day_of(
            TAI_UTC_PINNED_THROUGH.year,
            TAI_UTC_PINNED_THROUGH.month,
            TAI_UTC_PINNED_THROUGH.day
        ) == 20_818
    );
};

impl Civil {
    /// Construct a reading, validating every field. `second` may be `60`.
    ///
    /// This does not consult the leap-second table: whether a `60` is legitimate depends on the day, which is
    /// [`utc_to_tdb`]'s question. Keeping validation and table lookup apart is what lets a caller build a
    /// date for display without owning a timescale.
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
        // A `:60` is only ever the last second of the last minute of a day: that is where a leap second is
        // inserted. Refused here rather than later, because `utc_to_tdb` would answer `NoLeapSecondOnThisDay`
        // for `13:00:60` — an error about the *day* for a fault in the *time of day*, which sends whoever reads
        // it looking in the wrong place. Whether this day actually had a leap second is still the table's
        // question, asked at conversion.
        if second == 60 && (hour != 23 || minute != 59) {
            return Err(TimeError::BadCivil);
        }
        if !(0.0..1.0).contains(&fraction) {
            return Err(TimeError::BadCivil);
        }
        // Days in month, with the Gregorian rule spelled out: divisible by 4, except centuries, except
        // multiples of 400. 1900 is not a leap year and 2000 is — the two cases a naive `% 4` gets wrong, and
        // both are inside the era this module serves.
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

    /// Whether two readings name the same clock time, to within `tolerance_seconds`.
    ///
    /// This exists because `==` cannot answer the question: `fraction` is an `f64`, so [`Civil`] is only
    /// `PartialEq`, and a round-trip test wants "did I get the same instant back" without hand-rolling seven
    /// field comparisons. The tolerance is **required**, never defaulted: a caller who has not decided how
    /// much error to expect has not thought about the question, and a silent `1e-9` would fail conversions
    /// whose drift is real and understood.
    pub fn same_instant(&self, other: &Civil, tolerance_seconds: f64) -> bool {
        (
            self.year,
            self.month,
            self.day,
            self.hour,
            self.minute,
            self.second,
        ) == (
            other.year,
            other.month,
            other.day,
            other.hour,
            other.minute,
            other.second,
        ) && odderon_dmath::fabs(self.fraction - other.fraction) <= tolerance_seconds
    }

    /// Days since 1970-01-01 for this date. The time of day is not included.
    pub fn days_since_1970(&self) -> i64 {
        days_from_civil(self.year, self.month, self.day)
    }

    /// A [`Civil`] from a day number, a second of day and a fraction.
    ///
    /// Private: the only caller is [`tdb_to_utc`], and the "second of day" here excludes the leap second,
    /// which is precisely the detail a public constructor would invite callers to get wrong.
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

/// `TAI − UTC` in seconds, for a UTC **day number**.
///
/// The table is a step function of the calendar date, so the date is a complete index into it and the time of
/// day never enters. That is not a shortcut, it is why the conversion can be exact: an offset change happens
/// at midnight, so a day is *entirely* before or *entirely* after it. Indexing instead by a naive scalar of
/// elapsed seconds is the mistake that puts `23:59:60` and the following midnight on the same integer.
fn tai_minus_utc_at(days: i64) -> Result<i32, TimeError> {
    // The 1972 baseline, by day number. Anything earlier is refused, not clamped: UTC before 1972 used
    // fractional offsets, and clamping would silently apply a 10 s step to a date where no such step existed.
    if days < days_from_civil(1972, 1, 1) {
        return Err(TimeError::BeforeLeapSecondEra);
    }
    let mut offset = LEAP_SECONDS[0].tai_minus_utc;
    // Iterating by value: `LeapSecond` is `Copy`, so this needs no index and cannot go out of bounds.
    for row in LEAP_SECONDS {
        if day_of(row.year, row.month, row.day) <= days {
            offset = row.tai_minus_utc;
        }
    }
    Ok(offset)
}

/// True when the date is beyond the range whose offset is announced.
///
/// The last covered date is inclusive: at 2026-12-31T00:00:00 the announced offset is in force, and that is a
/// fact the IERS states. What is unknown is any date after it, because no later leap second has been
/// announced. The comparison is therefore `>` and not `>=`; `>=` would make the announced
/// last date unconvertible, which is the sort of off-by-one that reads as "the table is stale" rather than as
/// a bug.
fn beyond_table(days: i64) -> bool {
    days > TAI_UTC_COVERED_THROUGH_DAY
}

/// The TAI count — seconds since 1970-01-01T00:00:00 TAI — at the start of a UTC day.
///
/// This is the index the inverse searches on, and it is what makes the leap second representable:
/// `23:59:60` on a leap day and `00:00:00` on the following day are the *same* TAI instant, so the day they
/// are filed under is the only thing that distinguishes them. `None` before 1972, where there is no answer to
/// give.
fn tai_start_of_day(day: i64) -> Option<f64> {
    let offset = tai_minus_utc_at(day).ok()?;
    Some(day as f64 * SECONDS_PER_DAY + offset as f64)
}

/// How much longer than 24 hours the day is: `0`, or `1` on a day that ends in a leap second.
///
/// Asked as a difference rather than looked up, because "did this day have a leap second" is exactly "did the
/// offset grow at the next midnight", and deriving it keeps the two facts from disagreeing.
fn leap_seconds_on(day: i64) -> u8 {
    let here = tai_minus_utc_at(day).unwrap_or(0);
    let next = tai_minus_utc_at(day + 1).unwrap_or(here);
    if next > here {
        (next - here) as u8
    } else {
        0
    }
}

/// Seconds from 1970-01-01T00:00:00 to J2000 in **TT**.
///
/// This is the whole epoch conversion, and it is a single subtraction because TT is the scale on which the
/// offset is a definition rather than an observation. `946 728 000` s is 2000-01-01T12:00:00 — which is
/// 12:00:00 **TT**, since J2000 is defined in TT.
///
/// **Do not "simplify" this constant.** Two plausible-looking variants are both wrong, and both were shipped
/// before being caught:
///
/// * `946_728_000.0 - 32.184` — subtracts TT − TAI here *and* applies it in the conversion, so every date is
///   32.184 s off J2000.
/// * `946_728_000.0` with `TT = TAI + 32.184 + (TAI − UTC)` — adds the leap-second offset twice, because
///   `TAI − UTC` is already inside the TAI count.
///
/// Both variants leave the round trip agreeing with itself to 0.1 µs, because a conversion and its inverse
/// that share a wrong constant cancel. **Only an absolute anchor catches either**: the test
/// `j2000_in_utc_is_tdb_zero` asserts that `utc_to_tdb(2000-01-01T11:58:55.816)` is within a millisecond of
/// zero, and it is the only reason these were found.
const SECONDS_1970_TO_J2000_TT: f64 = 946_728_000.0;

/// `TT − TAI` in seconds, by definition.
const TT_MINUS_TAI: f64 = 32.184;

/// Seconds in a nominal day.
///
/// "Nominal" is doing work here: a UTC day is 86 400 or 86 401 seconds, a TAI day is always 86 400, and this
/// constant is used only where the distinction is already resolved — as the length of a TAI day, or as the
/// normaliser that turns seconds since J2000 into days for the orbital phase. Naming it keeps those uses
/// auditable, and stops `86_400.0` from spreading into places where a leap second makes it wrong.
const SECONDS_PER_DAY: f64 = 86_400.0;

/// `TDB − TT` in seconds, to the leading periodic term.
///
/// Amplitude **1.6570 ms**, which is 12.7 m of along-track motion at low Earth orbit — 2.7× the 4.7 m
/// constant-precision residual in `tests/observe.rs`, which is the whole argument for this function existing.
///
/// The argument is **seconds since J2000 in TT**. Passing TDB or UTC instead moves the result by well under
/// the term's own size, but the distinction is the reason the parameter is named rather than anonymous.
///
/// The phase is the part worth stating, because it is the opposite of the obvious guess. The term is
/// proportional to the Earth's *radial velocity*, not to its distance from the barycentre, so its extremes
/// fall **near the equinoxes** — a quarter of an orbit away from perihelion and aphelion — and it is close to
/// zero at both apsides. Measured against this implementation: maximum `+1.6570 ms` at day 92.85 (early
/// April), minimum `−1.6570 ms` at day 277.42 (early October), and `−0.0726 ms` at day 0, i.e. almost
/// nothing on January 1, which is within a day of perihelion.
///
/// This is the standard engineering form (Fairhead & Bretagnon 1990, truncated). The omitted terms are tens of
/// microseconds — under a metre at LEO — and they are omitted because the full series is several hundred terms
/// for a correction below the noise of the positions it feeds.
pub fn tdb_minus_tt(seconds_since_j2000_tt: f64) -> f64 {
    let days = seconds_since_j2000_tt / SECONDS_PER_DAY;
    let mean_anomaly_deg = 357.53 + 0.985_600_3 * days;
    // `mean_anomaly_deg` grows without bound — 0.9856°/day is a full turn per year — so reduce it before
    // taking the sine. The kernel's `sin` reduces internally, but the *argument* is what loses precision first:
    // a century out, the angle is 3.6e4 rad and its fractional part carries only twelve digits.
    let g = reduce_degrees(mean_anomaly_deg) * core::f64::consts::PI / 180.0;
    // 0.0167 is the Earth's orbital eccentricity: the `e·sin g` term is the equation of the centre, the
    // first-order correction that makes this an ellipse rather than a circle.
    0.001_657 * sin(g + 0.016_7 * sin(g))
}

/// Reduce an angle in degrees to `[0, 360)`.
///
/// An exact identity for angles already in range, which is the common case (every date within a year of
/// J2000) and keeps the periodic term bit-for-bit predictable there.
fn reduce_degrees(degrees: f64) -> f64 {
    if (0.0..360.0).contains(&degrees) {
        return degrees;
    }
    let reduced = degrees - floor_div(degrees, 360.0) * 360.0;
    // `floor_div` guarantees `[0, 360)` up to rounding, so these are not expected to fire; they are here so
    // that a rounding edge cannot return an angle outside the range the caller was promised.
    if reduced >= 360.0 {
        reduced - 360.0
    } else if reduced < 0.0 {
        reduced + 360.0
    } else {
        reduced
    }
}

/// `floor(a / b)` for finite `b != 0`, without `f64::floor` — which is a `std` method and unavailable to a
/// `no_std` crate.
///
/// The sign test is `truncated > quotient`, not `quotient < 0.0`. Those look equivalent and are not: the
/// second is wrong for any quotient in `(-1, 0)`, where truncation gives `0` and the true floor is `-1`. That
/// bug is invisible for positive dates, which is every date this module was first tested against, and then
/// silently moves the day boundary by one second for dates before 1970.
fn floor_div(numerator: f64, denominator: f64) -> f64 {
    let quotient = numerator / denominator;
    let truncated = quotient as i64 as f64;
    if truncated > quotient {
        truncated - 1.0
    } else {
        truncated
    }
}

/// Convert a UTC reading to **TDB seconds since J2000**.
///
/// This is the entry point a schedule uses: a game clock, a mission planner, a pass banner. The result is in
/// the same units as the kernel's `dt`, so it can be handed straight to [`crate::propagate`].
///
/// ## Errors, each a different statement
///
/// * [`TimeError::BeforeLeapSecondEra`] — before 1972-01-01. UTC then was a rubber-second system with
///   fractional offsets, and the integer table is the wrong model. Refused rather than extrapolated
///   backwards, because a plausible number is worse than no number.
/// * [`TimeError::BeyondLeapSecondTable`] — after 2026-12-31, the end of the announced range. **Deliberately
///   an error rather than an extrapolation.** Leap seconds are announced about six months ahead; a date past
///   the last announcement is not a computation, it is a guess, and a caller has to be able to see that in
///   order to present it as provisional. The variant carries the range's end so the message can say *when* it
///   stops.
/// * [`TimeError::NoLeapSecondOnThisDay`] — a `23:59:60` on a day the table records no leap second.
///
/// ## Why the arithmetic is in this order
///
/// The second-of-day is formed in the **UTC** calendar first, where `23:59:60` is exactly second 86 400 —
/// the next day's midnight is second 86 401 on that day, which is what makes the leap second a second rather
/// than a relabelling. Only then is the day's offset added, giving a TAI count that is monotone across the
/// boundary. Doing it the other way round (adding the offset per-second) is how implementations end up with
/// two instants sharing one scalar.
pub fn utc_to_tdb(civil: &Civil) -> Result<f64, TimeError> {
    let days = civil.days_since_1970();
    if beyond_table(days) {
        return Err(TimeError::BeyondLeapSecondTable {
            table_ends: TAI_UTC_COVERED_THROUGH,
        });
    }
    let offset = tai_minus_utc_at(days)?;
    if civil.second == 60 && leap_seconds_on(days) == 0 {
        return Err(TimeError::NoLeapSecondOnThisDay);
    }

    let second_of_day = civil.hour as f64 * 3600.0
        + civil.minute as f64 * 60.0
        + civil.second as f64
        + civil.fraction;
    let tai = days as f64 * SECONDS_PER_DAY + offset as f64 + second_of_day;

    // TAI -> TT is the definition; TT -> TDB is the periodic term. `S_J` is the *TT* reading of J2000, so one
    // subtraction is the entire epoch conversion — see the warning on the constant.
    let tt_since_j2000 = tai + TT_MINUS_TAI - SECONDS_1970_TO_J2000_TT;
    Ok(tt_since_j2000 + tdb_minus_tt(tt_since_j2000))
}

/// Convert **TDB seconds since J2000** back to a UTC reading.
///
/// The inverse of [`utc_to_tdb`], and exact wherever the forward conversion is defined: every leap-second
/// boundary round-trips, `23:59:60` included, which `tests/time.rs` checks for all 28 table rows rather than
/// for the one date someone thought of.
///
/// ## The part that is easy to get wrong
///
/// `23:59:60` on a leap day and `00:00:00` the following day are the **same TAI instant**. A naive inverse
/// therefore has two civil readings for one scalar, and whichever it picks, the other becomes unreachable —
/// the first version of this function oscillated between them and produced `hour: 24`. The fix is to search
/// candidate days **earliest first** and to accept a second-of-day up to 86 401 on a leap day: asking the
/// earlier day first reports the leap second, and the duplicate midnight is then simply never returned. The
/// function is a bijection on the instants it can produce, which is the property the tests pin.
///
/// The refusals are the same three the forward conversion makes, and for the same reasons: `BeforeLeapSecondEra`
/// outside 1972–2026, `BeyondLeapSecondTable` past the covered range (carrying its end date so a UI can show it), and
/// `BadCivil` for a non-finite input or an instant that cannot be represented. The directions are symmetric on
/// purpose — an inverse that could produce a civil reading the forward conversion refuses would let a caller
/// manufacture a date the module claims not to support.
pub fn tdb_to_utc(tdb_seconds_since_j2000: f64) -> Result<Civil, TimeError> {
    if !tdb_seconds_since_j2000.is_finite() {
        return Err(TimeError::BadCivil);
    }

    // TDB = TT + periodic(TT), so invert: TT = TDB − periodic(TT). A fixed three passes of a 1.658 ms
    // correction converges past the last bit that matters, and a fixed count cannot make the answer depend on
    // a rounding comparison — which is how inverse functions acquire a one-ulp discontinuity nobody can debug.
    let mut tt = tdb_seconds_since_j2000;
    for _ in 0..3 {
        tt = tdb_seconds_since_j2000 - tdb_minus_tt(tt);
    }

    // Undo both offsets at once, since TT = TAI + 32.184 − S_J.
    let tai = tt + SECONDS_1970_TO_J2000_TT - TT_MINUS_TAI;
    let guess = floor_div(tai, SECONDS_PER_DAY) as i64;

    for day in [guess - 1, guess, guess + 1] {
        // A day beyond the announced range must be skipped, not converted. `tai_minus_utc_at` would happily
        // answer for a date past it — the last published offset is the best guess available — and the inverse
        // would then produce a civil reading for exactly the dates the forward conversion refuses, which is how
        // a module ends up with a gate that only works in one direction.
        if beyond_table(day) {
            continue;
        }
        let start = match tai_start_of_day(day) {
            Some(s) => s,
            // Before 1972 there is no offset, so there is no valid answer to fall back to.
            None => continue,
        };
        let second_of_day = tai - start;
        let limit = SECONDS_PER_DAY + leap_seconds_on(day) as f64;
        if (0.0..limit).contains(&second_of_day) {
            let (year, month, day_of_month) = civil_from_days(day);
            if second_of_day >= SECONDS_PER_DAY {
                // The leap second: `:60` of the last minute of the day, with its own fraction. This is the
                // branch that makes the identity hold at the boundary.
                return Ok(Civil {
                    year,
                    month,
                    day: day_of_month,
                    hour: 23,
                    minute: 59,
                    second: 60,
                    fraction: second_of_day - SECONDS_PER_DAY,
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

    // No candidate day held this instant, so it is outside the representable range. Classify it against the two
    // ends of the table rather than by remembering which candidate failed: the search visits neighbours, so a
    // flag set during it depends on which of three days happened to be in range, and an instant one second past
    // the end would then be reported as malformed instead of as provisional.
    let first = tai_start_of_day(days_from_civil(1972, 1, 1)).unwrap_or(0.0);
    // The end of the *covered* span, not of the pinned one. `leap_seconds_on` answers 0 everywhere inside the
    // pinning, which is correct here: no leap second was inserted inside it. Using the covered boundary keeps
    // this consistent with `beyond_table`, so the inverse cannot answer for a date the forward refuses.
    let last = tai_start_of_day(TAI_UTC_COVERED_THROUGH_DAY).unwrap_or(first)
        + SECONDS_PER_DAY
        + leap_seconds_on(TAI_UTC_COVERED_THROUGH_DAY) as f64;
    if tai < first {
        Err(TimeError::BeforeLeapSecondEra)
    } else if tai >= last {
        Err(TimeError::BeyondLeapSecondTable {
            table_ends: TAI_UTC_COVERED_THROUGH,
        })
    } else {
        Err(TimeError::BadCivil)
    }
}

/// Convert a number of seconds between two scales whose offset is a constant, returning `None` where it is
/// not.
///
/// TAI and TT differ by 32.184 s by definition, so converting between them needs no date and no table. UTC
/// needs leap seconds and TDB needs the periodic term, both of which require a date — so both are refused
/// here rather than silently treated as TAI. This is the function a caller reaches for when they have a
/// duration rather than an instant; for an instant, use [`utc_to_tdb`].
pub fn convert_constant_offset(seconds: f64, from: Scale, to: Scale) -> Option<f64> {
    let offset_of = |scale: Scale| match scale {
        Scale::Tai => Some(0.0),
        Scale::Tt => Some(TT_MINUS_TAI),
        // Neither is a constant offset from an atomic count: UTC steps by whole seconds at unpredictable
        // dates, and TDB varies periodically with the date. Returning `seconds` here — the tempting
        // convenience — would be wrong by up to 69 s and 1.658 ms respectively, and wrong *silently*.
        Scale::Utc | Scale::Tdb => None,
    };
    let from_offset = offset_of(from)?;
    let to_offset = offset_of(to)?;
    Some(seconds + (to_offset - from_offset))
}
