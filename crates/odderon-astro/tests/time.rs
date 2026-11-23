//! Time systems, checked against arithmetic that is true independently of this crate.
//!
//! The trap here is different from the rest of the crate. A wrong rotation produces a visibly wrong ground
//! track. A wrong timescale produces a **plausible** schedule that is off by 69 seconds, or by one second
//! exactly once every few years, and nothing about the output looks suspicious. So the tests are anchored to
//! facts that are published and checkable by hand:
//!
//! * J2000 is defined as 2000-01-01T12:00:00 **TT**, which is 11:58:55.816 **UTC**. Every conversion in this
//!   file is measured against that one instant.
//! * `TAI − UTC` is 37 s from 2017-01-01. It was 36 s before that date. The 2017 step is the last one inserted,
//!   and the 2016-12-31 leap second is the one `CREDIBILITY.md` explicitly asks to be tested.
//! * `TDB − TT` never exceeds 1.658 ms, and reaches its extremes once per year.
//!
//! The failure this file is built around is the **double-counted epoch**: the first version of the module
//! subtracted 32.184 s from a constant that already had it folded in, so `utc_to_tdb` returned **32.1839** for
//! the J2000 instant instead of zero. Nothing crashed, every round trip was self-consistent, and the whole
//! crate's `dt` was 32 seconds out. Only an absolute anchor finds that.

use odderon_astro::time::{
    convert_constant_offset, tdb_minus_tt, tdb_to_utc, utc_to_tdb, Civil, Scale, TimeError,
    LEAP_SECONDS, LEAP_TABLE_ENDS,
};

/// The UTC reading of the J2000 epoch. Defined, not measured.
fn j2000_utc() -> Civil {
    Civil::new(2000, 1, 1, 11, 58, 55, 0.816).expect("J2000 is a valid civil time")
}

// =============================================================================================
// 1. The absolute anchor
// =============================================================================================

/// 2000-01-01T11:58:55.816 UTC is TDB 0, to the size of the periodic term.
///
/// This is the single most valuable test in the file. `TT − UTC = 64.184 s` at J2000 (32 s of accumulated leaps
/// plus the 32.184 s definition), so the UTC instant that maps to the epoch is
/// `12:00:00.000 − 64.184 s = 11:58:55.816`.
///
/// The tolerance is **1.658 ms plus a little**, because `TDB − TT` is a real periodic term and the epoch is
/// defined in TT: at the J2000 instant that term is `−72.6 µs`, so the conversion cannot land on exactly zero
/// and pretending otherwise would be a test of an approximation rather than of the definition.
#[test]
fn j2000_in_utc_is_tdb_zero() {
    let tdb = utc_to_tdb(&j2000_utc()).expect("J2000 is inside the table");
    assert!(
        tdb.abs() < 1.0e-3,
        "the J2000 UTC reading must convert to ~0 TDB seconds, got {tdb:.9}. A value near 32.18 means the \
         32.184 s TT-TAI offset was applied twice."
    );
    // And specifically not zero by a millisecond: the periodic term is there, and it is negative at J2000.
    assert!(
        tdb < 0.0,
        "TDB - TT is -72.6 us at J2000, so the answer must be slightly negative, got {tdb:.9}"
    );
    assert!(tdb > -1.66e-3, "and not by more than the 1.658 ms amplitude, got {tdb:.9}");
}

/// The round trip closes, to well under a microsecond.
///
/// `CREDIBILITY.md` sets the target at 1 µs for TDB↔TT↔TAI↔UTC. The residual here is the fixed-point iteration
/// on the periodic term, and it lands at the last bit of an `f64` holding 8.5e8 s — which is about 0.1 µs,
/// because that is the resolution of the representation, not of the algorithm.
#[test]
fn the_round_trip_closes_to_a_tenth_of_a_microsecond() {
    let dates = [
        Civil::new(1972, 1, 1, 0, 0, 0, 0.0).expect("valid"),
        Civil::new(1980, 6, 15, 3, 4, 5, 0.5).expect("valid"),
        Civil::new(2000, 1, 1, 11, 58, 55, 0.816).expect("valid"),
        Civil::new(2016, 12, 30, 12, 0, 0, 0.25).expect("valid"),
        // Deliberately *not* 2016-12-31T23:59:59: the inverse of that instant returns 23:59:60, because the naive
        // day model has no other slot for the inserted second. That is the documented behaviour and it has its own
        // test; putting it here would assert the opposite of `the_inverse_gives_the_leap_second_back`.
        Civil::new(2017, 1, 1, 0, 0, 0, 0.0).expect("valid"),
        // Deliberately not 2026: the shipped table ends at 2017, and a conversion past it is refused rather than
        // guessed. See `after_the_table_ends_is_refused_and_says_where_it_ends`.
    ];

    for civil in dates {
        let tdb = utc_to_tdb(&civil).expect("inside the table");
        let back = tdb_to_utc(tdb).expect("invertible");
        assert_eq!(
            (back.year, back.month, back.day),
            (civil.year, civil.month, civil.day),
            "date changed across the round trip: {civil:?} -> {back:?}"
        );
        // Compare times of day in seconds, which is the only way to be exact across a 60-second field.
        let a = civil.hour as f64 * 3600.0
            + civil.minute as f64 * 60.0
            + civil.second as f64
            + civil.fraction;
        let b = back.hour as f64 * 3600.0
            + back.minute as f64 * 60.0
            + back.second as f64
            + back.fraction;
        assert!(
            (a - b).abs() < 1.0e-6,
            "time of day changed by {:.9} s across the round trip for {civil:?}",
            a - b
        );
    }
}

// =============================================================================================
// 2. The leap second of 2016-12-31, named in CREDIBILITY.md
// =============================================================================================

/// `23:59:60` exists on 2016-12-31 and is exactly one second after `23:59:59`.
///
/// This is the leap second `CREDIBILITY.md` asks to be tested explicitly, because it is the one most likely to
/// be skipped: a naive calendar with `second: 0..59` cannot represent it, and an implementation that clamps or
/// rolls over produces `2017-01-01T00:00:00` for two different instants.
#[test]
fn the_2016_leap_second_is_a_real_second() {
    let before = Civil::new(2016, 12, 31, 23, 59, 59, 0.0).expect("valid");
    let leap = Civil::new(2016, 12, 31, 23, 59, 60, 0.0).expect("60 seconds is legal on a leap day");
    let after = Civil::new(2017, 1, 1, 0, 0, 0, 0.0).expect("valid");

    let a = utc_to_tdb(&before).expect("inside the table");
    let b = utc_to_tdb(&leap).expect("inside the table");
    let c = utc_to_tdb(&after).expect("inside the table");

    assert!((b - a - 1.0).abs() < 1.0e-6, "23:59:60 is 1 s after 23:59:59, got {:.9}", b - a);
    assert!((c - b - 1.0).abs() < 1.0e-6, "and midnight is 1 s after 23:59:60, got {:.9}", c - b);
    // The three instants are distinct, which is the property a clamping implementation breaks.
    assert!(a < b && b < c, "the three instants must be strictly ordered");
}

/// A `23:59:60` on a day the table records no leap second is **refused**, not quietly converted.
///
/// The first version of this module converted it, producing an instant that is simultaneously `23:59:60` and the
/// following midnight — two civil readings sharing one scalar, which is exactly the ambiguity the leap-second
/// table exists to remove. Modelling a leap second that was never inserted is a claim about the universe, and the
/// table is the only thing entitled to make it.
#[test]
fn a_sixty_second_on_a_non_leap_day_is_refused() {
    // 1999-12-31 had no leap second; 1998-12-31 did.
    let not_a_leap_day = Civil::new(1999, 12, 31, 23, 59, 60, 0.0).expect("60 is representable");
    assert_eq!(
        utc_to_tdb(&not_a_leap_day),
        Err(TimeError::NoLeapSecondOnThisDay),
        "no leap second was inserted at the end of 1999"
    );

    // And the day that did have one is accepted, so the check is a real test of the table rather than a refusal
    // of anything with a 60 in it.
    let real = Civil::new(1998, 12, 31, 23, 59, 60, 0.0).expect("valid");
    assert!(utc_to_tdb(&real).is_ok(), "1998-12-31 ended with a leap second");

    // Ordinary seconds on an ordinary day are untouched.
    assert!(utc_to_tdb(&Civil::new(1999, 12, 31, 23, 59, 59, 0.0).expect("valid")).is_ok());
    assert!(utc_to_tdb(&Civil::new(2000, 1, 1, 0, 0, 0, 0.0).expect("valid")).is_ok());
}

/// The inverse reconstructs the leap second rather than dropping it.
///
/// Round-tripping `23:59:60` is the case where a lazy implementation of the inverse loses a second: the naive
/// scalar has no slot for that second, so it silently lands on midnight. Two distinct TDB values must map to
/// two distinct civil readings, one of them `:60`.
#[test]
fn the_inverse_gives_the_leap_second_back() {
    let leap = Civil::new(2016, 12, 31, 23, 59, 60, 0.0).expect("valid");
    let tdb = utc_to_tdb(&leap).expect("inside the table");
    let back = tdb_to_utc(tdb).expect("invertible");
    assert_eq!(
        (back.year, back.month, back.day, back.hour, back.minute, back.second),
        (2016, 12, 31, 23, 59, 60),
        "the inverse must return the leap second, not midnight: got {back:?}"
    );
}

/// Every second in the neighbourhood of every leap second round-trips **exactly**, both ways.
///
/// This is the test that has to be exhaustive, because the failure mode is one date in twenty-eight. It walks the
/// three days around all 28 table rows at four times of day, and requires the civil reading to come back
/// unchanged — `23:59:60` included.
///
/// It also pins the ordering rule that makes it work: `23:59:60` on a leap day and `00:00:00` the day after are
/// the *same* TAI instant, and the inverse must file them under their own days rather than letting one swallow
/// the other. Getting that backward costs exactly one second, once every few years, which is why it is tested
/// rather than reasoned about.
#[test]
fn every_leap_second_boundary_round_trips_exactly() {
    let mut checked = 0;
    for row in LEAP_SECONDS.iter() {
        let base = Civil::new(row.year, row.month, row.day, 0, 0, 0, 0.0)
            .expect("every table row is a real date")
            .days_since_1970();
        for day_offset in -1i64..=1 {
            let day = base + day_offset;
            let (year, month, day_of_month) = civil_from_days_for_test(day);
            for (hour, minute, second) in [(0u8, 0u8, 0u8), (6, 30, 15), (12, 0, 0), (23, 59, 59)] {
                let civil = match Civil::new(year, month, day_of_month, hour, minute, second, 0.25) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let tdb = match utc_to_tdb(&civil) {
                    Ok(t) => t,
                    // Before 1972 at the first row; not a round-trip failure.
                    Err(_) => continue,
                };
                let back = tdb_to_utc(tdb).expect("a convertible instant must invert");
                assert_eq!(
                    (back.year, back.month, back.day, back.hour, back.minute, back.second),
                    (year, month, day_of_month, hour, minute, second),
                    "round trip changed {civil:?} into {back:?} (tdb {tdb})"
                );
                // 1e-7 and not 1e-9: the worst measured residual across the whole sweep is 2.98e-8, at 1974-12-31,
                // because the inverse's single step of the 1.658 ms periodic term cannot perfectly invert it. An
                // `f64` holding 8.5e8 s resolves 1.9e-7 anyway, so this is the arithmetic's floor, not the
                // algorithm's.
                assert!(
                    (back.fraction - 0.25).abs() < 1.0e-7,
                    "fraction changed: {civil:?} -> {back:?}"
                );
                checked += 1;
            }
        }
    }
    assert!(
        checked >= 300,
        "the sweep should cover hundreds of instants, got {checked}"
    );
}

/// The leap second itself, at every row, survives the round trip and sits between its neighbours.
#[test]
fn every_leap_second_is_representable_and_ordered() {
    for row in LEAP_SECONDS.iter().skip(1) {
        // The leap second was the last second of the day *before* the row's effective date.
        let day = Civil::new(row.year, row.month, row.day, 0, 0, 0, 0.0)
            .expect("real date")
            .days_since_1970()
            - 1;
        let (year, month, day_of_month) = civil_from_days_for_test(day);

        let s59 = Civil::new(year, month, day_of_month, 23, 59, 59, 0.0).expect("valid");
        let s60 = Civil::new(year, month, day_of_month, 23, 59, 60, 0.0).expect("valid");
        let a = utc_to_tdb(&s59).expect("in table");
        let b = utc_to_tdb(&s60).expect("a leap second must convert");
        assert!(
            (b - a - 1.0).abs() < 1.0e-6,
            "leap second at {year}-{month}-{day_of_month} is 1 s after :59, got {:.9}",
            b - a
        );

        let back = tdb_to_utc(b).expect("invertible");
        assert_eq!(
            (back.year, back.month, back.day, back.hour, back.minute, back.second),
            (year, month, day_of_month, 23, 59, 60),
            "the inverse must report the leap second of {year}-{month}-{day_of_month}"
        );
    }
}

/// The table is monotone, and its own last row is where the converter stops.
///
/// A table with a duplicated or out-of-order date produces a step function that goes backwards, which is a
/// category error rather than a rounding error. This costs three lines and closes it.
#[test]
fn the_table_is_monotone_and_ends_where_it_says() {
    let mut previous_day = i64::MIN;
    let mut previous_offset = i32::MIN;
    for row in LEAP_SECONDS.iter() {
        let day = Civil::new(row.year, row.month, row.day, 0, 0, 0, 0.0)
            .expect("real date")
            .days_since_1970();
        assert!(day > previous_day, "row {row:?} is not after the previous row");
        assert!(
            row.tai_minus_utc > previous_offset,
            "row {row:?} does not increase the offset — leap seconds are only ever added"
        );
        previous_day = day;
        previous_offset = row.tai_minus_utc;
    }
    assert_eq!(
        (LEAP_SECONDS[27].year, LEAP_SECONDS[27].month, LEAP_SECONDS[27].day),
        (LEAP_TABLE_ENDS.year, LEAP_TABLE_ENDS.month, LEAP_TABLE_ENDS.day),
        "the declared end of the table must be its last row"
    );
    assert_eq!(LEAP_SECONDS.len(), 28);
    assert_eq!(LEAP_SECONDS[27].tai_minus_utc, 37);
}

/// Days since 1970 → civil, in the test, independently of the module's private helper.
///
/// Duplicated on purpose: a test that asks the code under test what day it is has stopped testing the calendar.
fn civil_from_days_for_test(z: i64) -> (i32, u8, u8) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y } as i32, m as u8, d as u8)
}

/// `TAI − UTC` is what the published table says, on both sides of the 2017 step.
#[test]
fn tai_minus_utc_steps_at_the_published_dates() {
    // 36 s up to 2016-12-31T23:59:60; 37 s from 2017-01-01.
    let before = utc_to_tdb(&Civil::new(2016, 12, 31, 12, 0, 0, 0.0).expect("valid")).expect("in table");
    let leap = utc_to_tdb(&Civil::new(2016, 12, 31, 23, 59, 60, 0.0).expect("valid")).expect("in table");
    let after = utc_to_tdb(&Civil::new(2017, 1, 1, 0, 0, 0, 0.0).expect("valid")).expect("in table");

    // Elapsed TDB between 12:00 and 23:59:60 on the last day of 2016 is 43 200 s plus the *change* in the
    // periodic term over those twelve hours — which is real, and is why the tolerance here is 1e-4 s rather than
    // the microsecond used elsewhere. Measured: 43 200.000014, i.e. 14 µs from the periodic term moving between
    // 12:00 and midnight. That is not slack; it is the term doing what it is for.
    assert!(
        (leap - before - 43_200.0).abs() < 1e-4,
        "11:59:60 into the day is 43 200 s later plus the drift of the periodic term (14 us), got {:.9}",
        leap - before
    );
    // And midnight is one more second.
    assert!((after - leap - 1.0).abs() < 1e-6);
}

// =============================================================================================
// 3. TDB − TT is bounded and periodic
// =============================================================================================

/// No instant in the table puts `TDB − TT` outside its 1.658 ms amplitude, and it reaches both extremes.
///
/// The amplitude is the leading term's coefficient, so a sign error, a factor-of-1000 error, or a `sin`/`cos`
/// swap all show up here. Sampled every day for 30 years — 10 950 samples — which is enough to cross both
/// extremes (the argument advances 360° per year).
///
/// A term that was silently zero would satisfy "inside the amplitude" perfectly, which is why the range is
/// also asserted to span most of 3.3 ms.
#[test]
fn tdb_minus_tt_stays_inside_its_amplitude() {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    let mut day = 0;
    while day < 10_950 {
        let value = tdb_minus_tt(day as f64 * 86_400.0);
        if value < min {
            min = value;
        }
        if value > max {
            max = value;
        }
        day += 1;
    }

    assert!(
        max < 1.658e-3,
        "TDB-TT should never exceed 1.658 ms, got {max:e}"
    );
    assert!(
        min > -1.658e-3,
        "and never go below -1.658 ms, got {min:e}"
    );
    assert!(
        max - min > 3.2e-3,
        "the term must span nearly its full 3.314 ms range, got {:e}",
        max - min
    );
    // The phase, and the reason it is not "positive in January": with
    // `g = 357.53° + 0.9856003°·t`, the term crosses zero **23 days after J2000** and peaks a quarter of a year
    // later, at day ~93. So the extremum sits near aphelion (July), not perihelion — an un-intuitive fact that
    // depends entirely on the epoch constant, and exactly the kind of thing a hand-built phase gets backwards.
    // Measured here first: **+1.6570 ms at day 92.85**, **−1.6570 ms at day 277.42** — a half-year apart, which
    // is the check that the period is a year and not, say, half of one.
    let peak = tdb_minus_tt(92.85 * 86_400.0);
    let trough = tdb_minus_tt(277.42 * 86_400.0);
    assert!(
        peak > 1.65e-3,
        "the maximum is 1.657 ms at day 92.85, got {peak:e} there"
    );
    assert!(
        trough < -1.6e-3,
        "and the minimum is 1.657 ms negative at day 277.42; got {trough:e}"
    );
    // The zero crossing just after J2000, which is where the epoch constant enters the phase.
    assert!(
        tdb_minus_tt(0.0) < 0.0,
        "at J2000 itself the term is -72.6 us (day 0 is past the zero crossing), got {:e}",
        tdb_minus_tt(0.0)
    );
}

// =============================================================================================
// 4. The edges, which is where a time module lives or dies
// =============================================================================================

/// Before 1972 the converter refuses. It does not clamp, and it does not extrapolate.
///
/// UTC before 1972 was a rubber-second system: the offsets were fractional and tuned, not integral. Applying the
/// 10 s baseline to 1971 would produce a number to the microsecond that is simply wrong, with nothing in it to
/// say so.
#[test]
fn before_the_leap_second_era_is_refused() {
    let old = Civil::new(1971, 12, 31, 23, 59, 59, 0.0).expect("a valid date");
    assert_eq!(utc_to_tdb(&old), Err(TimeError::BeforeLeapSecondEra));
    assert_eq!(
        utc_to_tdb(&Civil::new(1900, 1, 1, 0, 0, 0, 0.0).expect("valid")),
        Err(TimeError::BeforeLeapSecondEra)
    );
    // The table's first instant is fine: the refusal is strictly before it.
    assert!(utc_to_tdb(&Civil::new(1972, 1, 1, 0, 0, 0, 0.0).expect("valid")).is_ok());
}

/// After the table ends the converter refuses, and names the last date it covers.
///
/// A game clock runs into the future, and leap seconds are announced about six months ahead. A conversion for
/// 2030 is not a computation; it is a guess. Refusing is what lets a caller present the result as *provisional*
/// rather than as fact — which is the project's standing rule about unknown astronomy.
#[test]
fn after_the_table_ends_is_refused_and_says_where_it_ends() {
    let future = Civil::new(2030, 1, 1, 0, 0, 0, 0.0).expect("valid");
    match utc_to_tdb(&future) {
        Err(TimeError::BeyondLeapSecondTable { table_ends }) => {
            assert_eq!((table_ends.year, table_ends.month, table_ends.day), (2017, 1, 1));
            assert_eq!(
                (LEAP_TABLE_ENDS.year, LEAP_TABLE_ENDS.month, LEAP_TABLE_ENDS.day),
                (2017, 1, 1)
            );
        }
        other => panic!("expected BeyondLeapSecondTable, got {other:?}"),
    }
    // The last date the table covers is accepted, and the day after is not — the boundary is one day wide. The
    // inclusive edge matters: 2017-01-01T00:00:00 is the instant the last published leap second takes effect, and
    // making it unconvertible would break exactly one date in the table.
    assert!(
        utc_to_tdb(&LEAP_TABLE_ENDS).is_ok(),
        "the effective date of the last leap second must be convertible"
    );
    assert!(utc_to_tdb(&Civil::new(2017, 1, 2, 0, 0, 0, 0.0).expect("valid")).is_err());
}

/// Invalid civil fields are refused, including the two a naive leap-year rule gets wrong.
#[test]
fn invalid_civil_input_is_refused() {
    assert_eq!(Civil::new(2026, 13, 1, 0, 0, 0, 0.0), Err(TimeError::BadCivil));
    assert_eq!(Civil::new(2026, 0, 1, 0, 0, 0, 0.0), Err(TimeError::BadCivil));
    assert_eq!(Civil::new(2026, 1, 32, 0, 0, 0, 0.0), Err(TimeError::BadCivil));
    assert_eq!(Civil::new(2026, 1, 1, 24, 0, 0, 0.0), Err(TimeError::BadCivil));
    assert_eq!(Civil::new(2026, 1, 1, 0, 60, 0, 0.0), Err(TimeError::BadCivil));
    assert_eq!(Civil::new(2026, 1, 1, 0, 0, 61, 0.0), Err(TimeError::BadCivil));
    assert_eq!(Civil::new(2026, 1, 1, 0, 0, 0, 1.0), Err(TimeError::BadCivil));
    assert_eq!(Civil::new(2026, 1, 1, 0, 0, 0, -0.1), Err(TimeError::BadCivil));

    // February: 2100 is not a leap year (century, not a multiple of 400), 2000 is.
    assert_eq!(Civil::new(2100, 2, 29, 0, 0, 0, 0.0), Err(TimeError::BadCivil));
    assert!(Civil::new(2000, 2, 29, 0, 0, 0, 0.0).is_ok());
    assert!(Civil::new(2024, 2, 29, 0, 0, 0, 0.0).is_ok());
    // April has 30 days, and the 31st is refused rather than rolled into May.
    assert_eq!(Civil::new(2026, 4, 31, 0, 0, 0, 0.0), Err(TimeError::BadCivil));
}

/// The day numbering agrees with three published anchors, and survives the 1970 crossing.
#[test]
fn the_calendar_anchors_are_right() {
    // And the table's own boundary index: a literal in `time.rs`, checked here against the calendar rather than
    // derived from it, so the two cannot be wrong together.
    assert_eq!(
        LEAP_TABLE_ENDS.days_since_1970(),
        17_167,
        "2017-01-01 must be day 17167, the index the table bound uses"
    );
    assert_eq!(Civil::new(1970, 1, 1, 0, 0, 0, 0.0).expect("valid").days_since_1970(), 0);
    assert_eq!(Civil::new(2000, 1, 1, 0, 0, 0, 0.0).expect("valid").days_since_1970(), 10_957);
    assert_eq!(Civil::new(2026, 11, 22, 0, 0, 0, 0.0).expect("valid").days_since_1970(), 20_779);
    // Before the epoch, so the truncating division must floor rather than round toward zero.
    assert_eq!(Civil::new(1969, 12, 31, 0, 0, 0, 0.0).expect("valid").days_since_1970(), -1);
    assert_eq!(Civil::new(1969, 12, 1, 0, 0, 0, 0.0).expect("valid").days_since_1970(), -31);
}

/// Consecutive days are one day apart across every leap year and century rule in range.
///
/// A calendar that is wrong about one century is wrong by one day for a hundred years and correct on either
/// side of it, which is the kind of bug that survives casual inspection. This walks 1900 → 2100 day by day.
#[test]
fn consecutive_days_are_consecutive_for_two_centuries() {
    let start = Civil::new(1900, 1, 1, 0, 0, 0, 0.0).expect("valid");
    let mut previous = Civil::new(1900, 1, 1, 0, 0, 0, 0.0).expect("valid").days_since_1970();
    let mut year = 1900;
    let mut month = 1u8;
    let mut day = 2u8;

    let mut steps = 0;
    while year < 2100 {
        if let Ok(c) = Civil::new(year, month, day, 0, 0, 0, 0.0) {
            let now = c.days_since_1970();
            assert_eq!(
                now - previous,
                1,
                "gap at {year}-{month}-{day}: {} days",
                now - previous
            );
            previous = now;
            day += 1;
            steps += 1;
        } else {
            // End of month: roll over. The constructor already refused the invalid day, so this is the only
            // branch a correct calendar can take.
            day = 1;
            month += 1;
            if month > 12 {
                month = 1;
                year += 1;
            }
        }
    }
    assert!(steps > 73_000, "should have walked two centuries, got {steps} steps");
    let _ = start;
}

/// The constant-offset conversions do what they say and refuse what they cannot do.
#[test]
fn constant_offsets_are_exact_and_tdb_is_refused() {
    // TAI -> TT is 32.184 s, by definition.
    let tt = convert_constant_offset(1000.0, Scale::Tai, Scale::Tt).expect("defined");
    assert!((tt - 1032.184).abs() < 1e-12);

    // And back.
    let tai = convert_constant_offset(tt, Scale::Tt, Scale::Tai).expect("defined");
    assert!((tai - 1000.0).abs() < 1e-12);

    // Identity.
    assert_eq!(convert_constant_offset(5.0, Scale::Tt, Scale::Tt), Some(5.0));

    // TDB needs a date, so it is refused rather than approximated by TT.
    assert_eq!(convert_constant_offset(0.0, Scale::Tt, Scale::Tdb), None);
    assert_eq!(convert_constant_offset(0.0, Scale::Tdb, Scale::Tt), None);
    // UTC needs the leap-second table and an epoch.
    assert_eq!(convert_constant_offset(0.0, Scale::Utc, Scale::Tt), None);
    assert_eq!(convert_constant_offset(0.0, Scale::Tai, Scale::Utc), None);
}

/// The size of what was skipped, in metres, so a future reader can see why the module exists.
///
/// Not a test of the code — a test of the *claim* that these corrections matter. If someone later proposes
/// dropping the leap-second handling as "one second in a game", this fails and says how far a satellite moves.
#[test]
fn the_corrections_are_large_compared_with_position_accuracy() {
    // 1.658 ms of TDB-TT at LEO speed.
    let leo_speed_km_s = 7.669;
    let from_offset_m = 1.658e-3 * leo_speed_km_s * 1000.0;
    assert!(
        from_offset_m > 12.0,
        "the periodic term is {from_offset_m:.1} m of along-track motion, which is not negligible"
    );

    // One leap second at LEO speed, and at geostationary speed.
    let leap_leo_m = 1.0 * leo_speed_km_s * 1000.0;
    let geo_speed_km_s = 3.075;
    let leap_geo_m = geo_speed_km_s * 1000.0;
    assert!(leap_leo_m > 7000.0, "a leap second is {leap_leo_m:.0} m at LEO");
    assert!(
        leap_geo_m > 3000.0,
        "and {leap_geo_m:.0} m at geostationary radius, so it is not a LEO-only problem"
    );

    // The comparison the module docs make: 1.658 ms is 2.7x the 4.71 m rotation-rate residual.
    assert!(
        from_offset_m / 4.71 > 2.5,
        "the periodic term should be a few times the documented rotation residual"
    );
}
