//! Time systems, checked against absolute anchors rather than against their own inverse.
//!
//! The trap in this file is specific and it bit this module twice, so it leads: **a conversion and its inverse
//! agree with each other even when both are wrong.** Subtract 32.184 s from the J2000 epoch constant *and*
//! apply the TT − TAI offset in the conversion, and every round trip still closes to 0.1 µs while every date
//! sits 32 s off J2000. The same trap in the other direction fixes the constant and then adds the leap-second
//! offset twice. Neither shows up in a symmetric test; both show up immediately against a value known
//! independently.
//!
//! So the tests are grouped by which lie they close:
//!
//! 1. **An absolute anchor.** J2000 in UTC is 2000-01-01T11:58:55.816, and its TDB value is zero plus the
//!    periodic term — negative, small, and not free to be anything else. This is the test that catches an epoch
//!    error; nothing else in the file does.
//! 2. **The leap second is a real second**, measured as three consecutive deltas of exactly 1.000000000 s.
//! 3. **Every boundary round-trips**, at all 28 table rows, both directions — not the one date someone thought
//!    of. 328 instants.
//! 4. **The refusals are distinct**, because "before 1972" and "after 2026" need different responses.
//! 5. **The corrections are big enough to matter**, expressed in metres of along-track motion, so the module's
//!    justification is a number in a test rather than a claim in a comment.
//!
//! Every number in the comments was computed outside this crate first — in `node`, against the same constants —
//! and then written here, so a failure says which of the two disagreed.

use odderon_astro::time::{
    convert_constant_offset, tdb_minus_tt, tdb_to_utc, utc_to_tdb, Civil, Scale, TimeError,
    LEAP_SECONDS, TAI_UTC_COVERED_THROUGH, TAI_UTC_PINNED_START, TAI_UTC_PINNED_THROUGH,
};

/// `2000-01-01T11:58:55.816` UTC is J2000: 12:00:00 TT, minus 32.184 s of TT − TAI, minus 32 s of TAI − UTC.
fn j2000_utc() -> Civil {
    Civil::new(2000, 1, 1, 11, 58, 55, 0.816).expect("a valid date")
}

// -------------------------------------------------------------------------------------------
// 1. The absolute anchor
// -------------------------------------------------------------------------------------------

/// J2000 converts to zero in TDB — plus the periodic term, which is negative there and about 72.6 µs.
///
/// **This is the test the module needed and did not have.** A symmetric round trip cannot see an epoch error,
/// because the conversion and its inverse share the constant and cancel. An absolute anchor cannot cancel: it
/// says what TDB at that instant *is*, so a 32 s or a 69 s error goes straight to the top of the assertion.
///
/// The value is `−72.60 µs` rather than exactly 0 because TDB ≠ TT: at day 0 the periodic term is small but not
/// zero, since J2000 falls about a day after perihelion and the term is near zero at the apsides — near, not at.
/// Both bounds matter. `> 0` would mean the sign of the term is inverted; `< −1.658 ms` would mean the epoch is
/// wrong by more than the effect being modelled, which is how the 32.184 s bug announced itself.
#[test]
fn j2000_in_utc_is_tdb_zero() {
    let tdb = utc_to_tdb(&j2000_utc()).expect("J2000 is inside the table");
    assert!(
        tdb > -1.0e-3 && tdb < 0.0,
        "TDB at J2000 UTC must be the periodic term alone, negative and under 1 ms; got {tdb:e}"
    );
    assert!(
        (tdb - (-7.2602e-5_f64)).abs() < 1.0e-7,
        "measured -72.602 us at J2000; got {tdb:e}"
    );
}

/// The epoch constant is right in *both* directions: the inverse of zero is J2000 UTC.
#[test]
fn tdb_zero_inverts_to_j2000_utc() {
    let back = tdb_to_utc(0.0).expect("zero is inside the table");
    assert_eq!(
        (
            back.year,
            back.month,
            back.day,
            back.hour,
            back.minute,
            back.second
        ),
        (2000, 1, 1, 11, 58, 55),
        "TDB 0 is J2000 in TT, which is 11:58:55.816 UTC; got {back:?}"
    );
    // The leftover fraction is 0.816 s plus the 72.6 µs the periodic term took out on the way in.
    assert!(
        (back.fraction - 0.816).abs() < 1.0e-4,
        "expected 11:58:55.816 UTC plus the periodic term; got fraction {}",
        back.fraction
    );
}

// -------------------------------------------------------------------------------------------
// 2. The leap second is a real second
// -------------------------------------------------------------------------------------------

/// The 2016-12-31 leap second is three consecutive instants one second apart.
///
/// This is the property a naive implementation loses: with the offset applied per-second rather than per-day,
/// `23:59:60` and the following `00:00:00` land on the same number and the middle delta becomes 0.
#[test]
fn the_2016_leap_second_is_a_real_second() {
    let s59 = Civil::new(2016, 12, 31, 23, 59, 59, 0.0).expect("valid");
    let s60 = Civil::new(2016, 12, 31, 23, 59, 60, 0.0).expect("60 is representable");
    let next = Civil::new(2017, 1, 1, 0, 0, 0, 0.0).expect("valid");

    let a = utc_to_tdb(&s59).expect("in table");
    let b = utc_to_tdb(&s60).expect("in table");
    let c = utc_to_tdb(&next).expect("in table");

    assert!(
        (b - a - 1.0).abs() < 1.0e-6,
        "23:59:59 -> 23:59:60 must be 1 s, got {:e}",
        b - a
    );
    assert!(
        (c - b - 1.0).abs() < 1.0e-6,
        "23:59:60 -> 00:00:00 must be 1 s, got {:e}",
        c - b
    );
}

/// The inverse gives the leap second back, at its own name, rather than collapsing it into the next midnight.
///
/// This is the property that took two attempts. `23:59:60` and `00:00:00` share a TAI instant, so the inverse
/// has two civil readings for one scalar and must pick one deliberately. It picks the leap second, and this
/// asserts that choice, because the alternative — reporting `00:00:00` twice and making `23:59:60` unreachable
/// — is silent: every round trip of a non-leap instant still passes.
#[test]
fn the_inverse_gives_the_leap_second_back() {
    let s59 = Civil::new(2016, 12, 31, 23, 59, 59, 0.0).expect("valid");
    let s60 = Civil::new(2016, 12, 31, 23, 59, 60, 0.0).expect("valid");

    // One second after 23:59:59 TDB *is* the leap second, so the inverse of that value must say so.
    let one_second_on = utc_to_tdb(&s59).expect("in table") + 1.0;
    let back = tdb_to_utc(one_second_on).expect("invertible");
    assert_eq!(
        (
            back.year,
            back.month,
            back.day,
            back.hour,
            back.minute,
            back.second
        ),
        (2016, 12, 31, 23, 59, 60),
        "the instant one second after 23:59:59 is the leap second; got {back:?}"
    );
    assert!(
        back.same_instant(&s60, 1.0e-5),
        "and it must be the leap second to within the periodic term's drift: {} vs {}",
        back.fraction,
        s60.fraction
    );
}

/// A `23:59:60` on a day the table records no leap second is **refused**, not quietly converted.
///
/// The first draft converted it, producing an instant that is simultaneously `23:59:60` and the following
/// midnight. Two civil readings for one scalar is exactly the ambiguity the table exists to remove, so the
/// honest answer is a refusal. Inventing a leap second that was never inserted is a claim about the world, and
/// only the table is entitled to make it.
#[test]
fn a_sixty_second_on_a_non_leap_day_is_refused() {
    // No leap second was inserted at the end of 1999; one was at the end of 1998.
    let not_a_leap_day = Civil::new(1999, 12, 31, 23, 59, 60, 0.0).expect("60 is representable");
    assert_eq!(
        utc_to_tdb(&not_a_leap_day),
        Err(TimeError::NoLeapSecondOnThisDay),
        "no leap second was inserted at the end of 1999"
    );

    // The day that did have one is accepted, so this is a test of the table and not a refusal of anything
    // containing a 60.
    let real = Civil::new(1998, 12, 31, 23, 59, 60, 0.0).expect("valid");
    assert!(
        utc_to_tdb(&real).is_ok(),
        "1998-12-31 ended with a leap second"
    );

    // Ordinary seconds on ordinary days are untouched.
    assert!(utc_to_tdb(&Civil::new(1999, 12, 31, 23, 59, 59, 0.0).expect("valid")).is_ok());
    assert!(utc_to_tdb(&Civil::new(2000, 1, 1, 0, 0, 0, 0.0).expect("valid")).is_ok());
}

/// A `:60` anywhere other than the last second of the last minute is not a time at all.
///
/// It used to be accepted at construction and refused later as `NoLeapSecondOnThisDay` — an error about the
/// *day* for a fault in the *time of day*, which points a reader at the wrong thing. The distinction matters
/// because the two faults have different fixes: a `13:00:60` is a malformed input, while a `23:59:60` on a
/// day with no leap second is a well-formed input the table happens to disagree with.
#[test]
fn a_sixty_second_must_be_the_last_second_of_the_day() {
    for (hour, minute) in [(13u8, 0u8), (0, 59), (23, 58), (0, 0)] {
        assert_eq!(
            Civil::new(2016, 12, 31, hour, minute, 60, 0.0).err(),
            Some(TimeError::BadCivil),
            "{hour:02}:{minute:02}:60 is not a time of day"
        );
    }
    // And the one position where it *is* meaningful is accepted at construction, whatever the table later says
    // about that day — 1999 is used precisely because it had no leap second, so this asserts construction does
    // not consult the table.
    assert!(
        Civil::new(1999, 12, 31, 23, 59, 60, 0.0).is_ok(),
        "the last second of the day is a legal Civil value regardless of what happened that day"
    );
}

// -------------------------------------------------------------------------------------------
// 3. Every boundary, both directions
// -------------------------------------------------------------------------------------------

/// Every second in the neighbourhood of every leap second round-trips exactly.
///
/// Exhaustive on purpose: the failure mode is one date in twenty-eight, and a test written against the 2016
/// leap second — the one everyone remembers — would pass while 1997 was wrong. This walks the three days around
/// all 28 rows at four times of day.
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
                let civil = match Civil::new(year, month, day_of_month, hour, minute, second, 0.25)
                {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let tdb = match utc_to_tdb(&civil) {
                    Ok(t) => t,
                    // Before 1972 at the first row, where the conversion is refused by design.
                    Err(_) => continue,
                };
                let back = tdb_to_utc(tdb).expect("a convertible instant must invert");
                assert_eq!(
                    (
                        back.year,
                        back.month,
                        back.day,
                        back.hour,
                        back.minute,
                        back.second
                    ),
                    (year, month, day_of_month, hour, minute, second),
                    "round trip changed {civil:?} into {back:?} (tdb {tdb})"
                );
                // 1e-7, not 1e-9: the worst residual measured across this sweep is 2.98e-8 s (1974-12-31),
                // because a single step of the 1.658 ms periodic term cannot invert perfectly. An `f64` holding
                // 8.5e8 s resolves 1.9e-7 anyway, so this is the arithmetic's floor rather than the algorithm's.
                assert!(
                    back.same_instant(&civil, 1.0e-7),
                    "the fraction changed beyond the arithmetic's resolution: {civil:?} -> {back:?}"
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

/// The leap second itself, at every row, sits exactly one second after `:59` and inverts to `:60`.
#[test]
fn every_leap_second_is_representable_and_ordered() {
    for row in LEAP_SECONDS.iter().skip(1) {
        // A leap second is the last second of the day *before* the row's effective date.
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
            "leap second at {year}-{month}-{day_of_month} is 1 s after :59, got {:e}",
            b - a
        );

        let back = tdb_to_utc(b).expect("invertible");
        assert_eq!(
            (
                back.year,
                back.month,
                back.day,
                back.hour,
                back.minute,
                back.second
            ),
            (year, month, day_of_month, 23, 59, 60),
            "the inverse must report the leap second of {year}-{month}-{day_of_month}"
        );
    }
}

/// The table is monotone, the offsets only grow, and its last row is both the declared end and the hand-written
/// day number the converter uses.
///
/// The module also asserts this at compile time; the duplication is deliberate. The `const _` block is the one
/// that fails the build, and this is the one that says which row was wrong.
#[test]
fn the_table_is_monotone_and_ends_where_it_says() {
    let mut previous_day = i64::MIN;
    let mut previous_offset = i32::MIN;
    for row in LEAP_SECONDS.iter() {
        let day = Civil::new(row.year, row.month, row.day, 0, 0, 0, 0.0)
            .expect("real date")
            .days_since_1970();
        assert!(
            day > previous_day,
            "row {row:?} is not after the previous row"
        );
        assert!(
            row.tai_minus_utc > previous_offset,
            "row {row:?} does not increase the offset; leap seconds are only ever added"
        );
        previous_day = day;
        previous_offset = row.tai_minus_utc;
    }
    assert_eq!(LEAP_SECONDS.len(), 28);
    let last = LEAP_SECONDS[27];
    assert_eq!(
        (last.year, last.month, last.day),
        (
            TAI_UTC_PINNED_START.year,
            TAI_UTC_PINNED_START.month,
            TAI_UTC_PINNED_START.day
        ),
        "the zero-leap-second span must start at the table's last row"
    );
    assert_eq!(
        last.tai_minus_utc, 37,
        "TAI - UTC has been 37 s since 2017-01-01"
    );
    // The literals the converter compares against, cross-checked against the calendar through the public API.
    assert_eq!(TAI_UTC_PINNED_START.days_since_1970(), 17_167);
    assert_eq!(TAI_UTC_COVERED_THROUGH.days_since_1970(), 20_818);
    assert_eq!(TAI_UTC_PINNED_THROUGH.days_since_1970(), 20_818);
}

// -------------------------------------------------------------------------------------------
// 4. TAI - UTC steps where it was published to step
// -------------------------------------------------------------------------------------------

/// `TAI − UTC` steps at the published dates, and only there.
///
/// Checked by differencing the converted value across each step rather than by reading the table back: the
/// table is data, the step is the behaviour, and only the second is what a caller experiences.
#[test]
fn tai_minus_utc_steps_at_the_published_dates() {
    // The 1971->1972 boundary is deliberately absent: the date before it is refused, so there is nothing to
    // difference against. That refusal has its own test rather than a row here that would always `continue`.
    for (before, after, expected_step) in [
        ((1972, 6, 30), (1972, 7, 1), 1),
        ((2016, 12, 31), (2017, 1, 1), 1),
        ((1998, 12, 31), (1999, 1, 1), 1),
        ((1983, 7, 1), (1983, 7, 2), 0), // no leap second: an ordinary midnight
    ] {
        let (y0, m0, d0) = before;
        let (y1, m1, d1) = after;
        let last_second_before = Civil::new(y0, m0, d0, 23, 59, 59, 0.0).expect("valid");
        let first_second_after = Civil::new(y1, m1, d1, 0, 0, 0, 0.0).expect("valid");

        let a =
            utc_to_tdb(&last_second_before).expect("both ends of every row are inside the table");
        let b = utc_to_tdb(&first_second_after).expect("in table");
        // Ordinary seconds, plus any leap second, plus the drift of the periodic term over 1 s (< 1e-8).
        let elapsed = b - a;
        let expected = 1.0 + expected_step as f64;
        assert!(
            (elapsed - expected).abs() < 1.0e-6,
            "{y0}-{m0}-{d0} 23:59:59 -> {y1}-{m1}-{d1} 00:00:00 should be {expected} s, got {elapsed:.9}"
        );
    }
}

// -------------------------------------------------------------------------------------------
// 5. The refusals
// -------------------------------------------------------------------------------------------

/// Before 1972 the table does not apply and the conversion says so.
///
/// Refused rather than extrapolated: UTC before 1972 was a rubber-second system with fractional offsets, so a
/// backwards-extended integer table would produce a number that looks authoritative and is wrong by a fraction
/// of a second that grows as you go back.
#[test]
fn before_the_leap_second_era_is_refused() {
    let old = Civil::new(1971, 12, 31, 23, 59, 59, 0.0).expect("valid");
    assert_eq!(utc_to_tdb(&old), Err(TimeError::BeforeLeapSecondEra));

    // The first convertible instant is the baseline row itself.
    let first = Civil::new(1972, 1, 1, 0, 0, 0, 0.0).expect("valid");
    assert!(
        utc_to_tdb(&first).is_ok(),
        "1972-01-01T00:00:00 is in the table"
    );

    // And the inverse refuses symmetrically rather than inventing a date the forward conversion rejects.
    assert_eq!(tdb_to_utc(-1.0e12), Err(TimeError::BeforeLeapSecondEra));
}

/// Beyond the announced range the conversion says *where* it ends, in the error itself.
///
/// The payload is not decoration: a UI that shows a provisional clock needs the date to display. A bare
/// `Err(())` would push the caller to hard-code 2026 in a second place, which is how two constants become one
/// that is wrong.
#[test]
fn after_the_covered_range_ends_is_refused_and_says_where_it_ends() {
    let beyond = Civil::new(2027, 1, 1, 0, 0, 0, 0.0).expect("valid");
    match utc_to_tdb(&beyond) {
        Err(TimeError::BeyondLeapSecondTable { table_ends }) => {
            assert_eq!(
                (table_ends.year, table_ends.month, table_ends.day),
                (2026, 12, 31),
                "the error must name the last announced date"
            );
        }
        other => panic!("expected BeyondLeapSecondTable, got {other:?}"),
    }

    // The last covered date converts: the bound is inclusive, and an exclusive one would make the module's
    // stated range a lie by one day.
    let last_day = Civil::new(2026, 12, 31, 0, 0, 0, 0.0).expect("valid");
    assert!(
        utc_to_tdb(&last_day).is_ok(),
        "the last announced date must convert"
    );

    // The inverse refuses the same region. Without this the module would have a one-way gate: a TDB value in
    // 2027 would invert to a confident civil reading for a date `utc_to_tdb` says it cannot convert, and the
    // "provisional beyond 2026" promise would be false in one direction.
    //
    // Derived from the module rather than written as a magic number, because a round figure chosen by hand
    // lands wherever it likes relative to the boundary. Two days past the last covered day is unambiguously
    // outside.
    let last_instant = utc_to_tdb(&last_day).expect("the last announced date converts");
    assert!(matches!(
        tdb_to_utc(last_instant + 2.0 * 86_400.0),
        Err(TimeError::BeyondLeapSecondTable { .. })
    ));
}

/// The whole zero-leap-second span converts, and to *one* offset.
///
/// This is the test the old boundary needed and could not have: it pins the property that made the extension
/// safe — the offset announced on 2017-01-01 is still the offset announced for every date through 2026-12-31 —
/// rather than merely asserting that some date in 2026 happens to convert. A leap second inserted anywhere in
/// the span would break it, which is the event the compile-time check also guards.
#[test]
fn the_zero_leap_second_span_is_one_offset() {
    for (year, month, day) in [
        (2017, 1, 1),
        (2018, 7, 1),
        (2020, 2, 29), // a leap day inside the span, which is about the calendar and not the second
        (2023, 12, 31),
        (2026, 6, 30), // an insertion date on which nothing was inserted
        (2026, 12, 31),
    ] {
        let civil = Civil::new(year, month, day, 12, 0, 0, 0.0).expect("valid date");
        let converted = utc_to_tdb(&civil)
            .unwrap_or_else(|e| panic!("{year}-{month}-{day} must convert inside the span: {e:?}"));

        // 12:00:00 UTC on day N is a whole number of days plus the offset away from 12:00:00 UTC on 2017-01-01,
        // and the periodic term differs by far less than a millisecond across a decade (it is a sinusoid with a
        // 1.658 ms amplitude). A one-second slip in the offset anywhere in the span would show up here.
        let anchor = Civil::new(2017, 1, 1, 12, 0, 0, 0.0).expect("valid");
        let expected = utc_to_tdb(&anchor).expect("the anchor converts")
            + (civil.days_since_1970() - anchor.days_since_1970()) as f64 * 86_400.0;
        assert!(
            (converted - expected).abs() < 5.0e-3,
            "offset changed inside the span at {year}-{month}-{day}: off by {:e} s",
            converted - expected
        );
    }
}

/// The game's own present converts, which is what the old boundary got wrong.
///
/// A clock module that refuses every date after 2017 cannot be used by a simulation set in 2026 — and this one
/// is. Written as an explicit test because the failure was not a subtle numerical one: the module was
/// answering `BeyondLeapSecondTable` for today, and no test compared its range against the date the project is
/// actually being worked on.
#[test]
fn the_present_converts() {
    let today = Civil::new(2026, 11, 22, 12, 0, 0, 0.0).expect("valid");
    assert!(
        utc_to_tdb(&today).is_ok(),
        "the date this project is being written on must convert; the module is set in the present"
    );
}

/// Out-of-range fields are refused at construction, so no later stage has to re-check them.
#[test]
fn invalid_civil_input_is_refused() {
    assert_eq!(
        Civil::new(2026, 13, 1, 0, 0, 0, 0.0),
        Err(TimeError::BadCivil)
    );
    assert_eq!(
        Civil::new(2026, 0, 1, 0, 0, 0, 0.0),
        Err(TimeError::BadCivil)
    );
    assert_eq!(
        Civil::new(2026, 1, 32, 0, 0, 0, 0.0),
        Err(TimeError::BadCivil)
    );
    assert_eq!(
        Civil::new(2026, 1, 1, 24, 0, 0, 0.0),
        Err(TimeError::BadCivil)
    );
    assert_eq!(
        Civil::new(2026, 1, 1, 0, 60, 0, 0.0),
        Err(TimeError::BadCivil)
    );
    assert_eq!(
        Civil::new(2026, 1, 1, 0, 0, 61, 0.0),
        Err(TimeError::BadCivil)
    );
    assert_eq!(
        Civil::new(2026, 1, 1, 0, 0, 0, 1.0),
        Err(TimeError::BadCivil)
    );
    assert_eq!(
        Civil::new(2026, 1, 1, 0, 0, 0, -0.1),
        Err(TimeError::BadCivil)
    );
    // February, the two cases a naive `% 4` gets wrong, plus the one it gets wrong the other way.
    assert_eq!(
        Civil::new(1900, 2, 29, 0, 0, 0, 0.0),
        Err(TimeError::BadCivil)
    );
    assert!(
        Civil::new(2000, 2, 29, 0, 0, 0, 0.0).is_ok(),
        "2000 is a leap year"
    );
    assert!(
        Civil::new(2024, 2, 29, 0, 0, 0, 0.0).is_ok(),
        "2024 is a leap year"
    );
    assert_eq!(
        Civil::new(2026, 2, 29, 0, 0, 0, 0.0),
        Err(TimeError::BadCivil)
    );
    assert_eq!(
        Civil::new(2026, 4, 31, 0, 0, 0, 0.0),
        Err(TimeError::BadCivil)
    );
}

// -------------------------------------------------------------------------------------------
// 6. The calendar underneath
// -------------------------------------------------------------------------------------------

/// The three known day numbers, and the table end, are what they should be.
#[test]
fn the_calendar_anchors_are_right() {
    assert_eq!(
        Civil::new(1970, 1, 1, 0, 0, 0, 0.0)
            .expect("valid")
            .days_since_1970(),
        0
    );
    assert_eq!(
        Civil::new(2000, 1, 1, 0, 0, 0, 0.0)
            .expect("valid")
            .days_since_1970(),
        10_957
    );
    assert_eq!(
        Civil::new(2026, 11, 22, 0, 0, 0, 0.0)
            .expect("valid")
            .days_since_1970(),
        20_779
    );
    // Days before 1970 are negative, and the floor direction matters: 1969-12-31 is -1, not 0.
    assert_eq!(
        Civil::new(1969, 12, 31, 0, 0, 0, 0.0)
            .expect("valid")
            .days_since_1970(),
        -1
    );
    assert_eq!(
        Civil::new(1969, 1, 1, 0, 0, 0, 0.0)
            .expect("valid")
            .days_since_1970(),
        -365
    );
    // The day numbers the converter compares against, checked against the calendar through the public API. Both
    // span ends are here because they are separate constants that could drift apart independently.
    assert_eq!(TAI_UTC_PINNED_START.days_since_1970(), 17_167);
    assert_eq!(TAI_UTC_PINNED_THROUGH.days_since_1970(), 20_818);
    assert_eq!(TAI_UTC_COVERED_THROUGH.days_since_1970(), 20_818);
}

/// Consecutive days are consecutive for two centuries, in both directions from the epoch.
///
/// A calendar bug that shifts a day boundary is invisible on the dates anyone tests — it shows up as a wrong
/// leap-second offset on exactly the days before a step, i.e. at the least convenient moment. 73 050 days is
/// cheap and removes the category.
#[test]
fn consecutive_days_are_consecutive_for_two_centuries() {
    // Starts one before the range so the first iteration has a real predecessor. `i64::MIN` here would make the
    // `previous + 1` check assert against a sentinel that is not a date, and fail on the very first day with a
    // message that reads like a calendar bug.
    let mut previous = -25_568i64;
    // 1900-01-01 is day -25 567 and 2100-01-01 is day 47 482: 73 050 consecutive days. The bound is written
    // as the literal, not as `..=47_483`, because an inclusive range that overruns by one asserts yesterday's
    // day number against today's and fails with a message that looks like a calendar bug and is not.
    for day in -25_567i64..=47_482i64 {
        let offset_into_civil = day;
        let (year, month, day_of_month) = civil_from_days_for_test(offset_into_civil);
        let recomputed = days_since_1970_for_test(year, month, day_of_month);
        assert_eq!(
            recomputed, day,
            "{year}-{month}-{day_of_month} should be day {day}, recomputed {recomputed}"
        );
        assert_eq!(day, previous + 1, "day numbers must step by one");
        previous = day;
    }
}

// -------------------------------------------------------------------------------------------
// 7. The corrections are big enough to matter, in metres
// -------------------------------------------------------------------------------------------

/// `TDB − TT` stays inside its 1.657 ms amplitude, is nearly zero at the apsides, and gets there once a year.
///
/// Asserted in milliseconds because that is the only way the size is visible: the bound looks like rounding
/// error until it is multiplied by a velocity. Superseded by nothing; this is the term the module exists for.
#[test]
fn tdb_minus_tt_stays_inside_its_amplitude() {
    let mut max = f64::MIN;
    let mut min = f64::MAX;
    let mut max_day = 0.0_f64;
    let mut min_day = 0.0_f64;
    // Ten years, hourly: 87 660 samples, enough to find the extrema to a day.
    for step in 0..87_660u32 {
        let day = step as f64 / 24.0;
        let value = tdb_minus_tt(day * 86_400.0);
        if value > max {
            max = value;
            max_day = day;
        }
        if value < min {
            min = value;
            min_day = day;
        }
    }
    // `>` against 1.6580e-3, not `>=` against 1.6570e-3: the sine can return exactly 1.0 at a sampled peak, so
    // the observed maximum *equals* the amplitude constant. A bound written as `<= 1.6570e-3` passes in node and
    // fails in CI on the one sample that lands on the peak — which is what a `-D warnings` build then reports as
    // a real failure, hours later.
    assert!(
        max <= 1.6580e-3,
        "amplitude must not exceed 1.657 ms plus rounding, got {max:e}"
    );
    assert!(
        min >= -1.6580e-3,
        "amplitude must not exceed 1.657 ms plus rounding, got {min:e}"
    );
    assert!(
        max > 1.6500e-3,
        "the maximum should nearly reach the amplitude, got {max:e}"
    );
    assert!(
        min < -1.6500e-3,
        "the minimum should nearly reach the amplitude, got {min:e}"
    );

    // Extrema near the equinoxes, a quarter-orbit from the apsides: days 92.85 and 277.42 in the first year.
    let year = 365.2422;
    let max_phase = (max_day % year + year) % year;
    let min_phase = (min_day % year + year) % year;
    assert!(
        (max_phase - 92.85).abs() < 1.0 || (max_phase - 92.85 - year).abs() < 1.0,
        "the maximum should fall near day 92.85, found day {max_phase:.2}"
    );
    assert!(
        (min_phase - 277.42).abs() < 1.0 || (min_phase - 277.42 - year).abs() < 1.0,
        "the minimum should fall near day 277.42, found day {min_phase:.2}"
    );

    // And it is almost zero at J2000, which is a day from perihelion — the sign of the term is negative there.
    let at_j2000 = tdb_minus_tt(0.0);
    assert!(
        at_j2000 < 0.0 && at_j2000 > -1.0e-4,
        "at J2000 the term is -72.6 us, got {at_j2000:e}"
    );
}

/// The three corrections, expressed in metres at low Earth orbit, so the module's justification is arithmetic.
///
/// A 7.7 km/s satellite covers 7.7 m per millisecond. The numbers here are the reason the periodic term is not
/// skipped: it is larger than the residual the rest of the crate documents.
#[test]
fn the_corrections_are_large_compared_with_position_accuracy() {
    const LEO_SPEED_KM_S: f64 = 7.7;
    const DOCUMENTED_RESIDUAL_M: f64 = 4.7;

    let periodic_m = 1.6570e-3 * LEO_SPEED_KM_S * 1000.0;
    assert!(
        (periodic_m - 12.76).abs() < 0.01,
        "1.657 ms at 7.7 km/s is 12.76 m, got {periodic_m}"
    );
    assert!(
        periodic_m > 2.5 * DOCUMENTED_RESIDUAL_M,
        "the periodic term must dominate the documented 4.7 m residual ({periodic_m} m)"
    );

    let leap_second_m = LEO_SPEED_KM_S * 1000.0;
    assert!(
        (leap_second_m - 7700.0).abs() < 1.0,
        "a leap second is 7.7 km of along-track motion, got {leap_second_m}"
    );
}

// -------------------------------------------------------------------------------------------
// 8. Constant offsets, and the scales that do not have one
// -------------------------------------------------------------------------------------------

/// TAI ↔ TT is an exact 32.184 s, and UTC and TDB are refused rather than treated as TAI.
#[test]
fn constant_offsets_are_exact_and_the_datable_scales_are_refused() {
    assert_eq!(
        convert_constant_offset(0.0, Scale::Tai, Scale::Tt),
        Some(32.184)
    );
    assert_eq!(
        convert_constant_offset(0.0, Scale::Tt, Scale::Tai),
        Some(-32.184)
    );
    assert_eq!(
        convert_constant_offset(1000.0, Scale::Tai, Scale::Tai),
        Some(1000.0)
    );
    assert_eq!(
        convert_constant_offset(1000.0, Scale::Tt, Scale::Tt),
        Some(1000.0)
    );

    // UTC's offset changes by whole seconds on announced dates, and TDB's varies continuously: neither is a
    // constant, and returning the input unchanged would be wrong by 69 s and 1.66 ms respectively.
    assert_eq!(convert_constant_offset(0.0, Scale::Utc, Scale::Tai), None);
    assert_eq!(convert_constant_offset(0.0, Scale::Tai, Scale::Utc), None);
    assert_eq!(convert_constant_offset(0.0, Scale::Tt, Scale::Tdb), None);
    assert_eq!(convert_constant_offset(0.0, Scale::Utc, Scale::Tdb), None);
}

// -------------------------------------------------------------------------------------------
// Helpers, reimplemented here on purpose
// -------------------------------------------------------------------------------------------

/// Days since 1970-01-01 → civil, independently of the module's private helper.
///
/// Duplicated deliberately: a test that asks the code under test what day it is has stopped testing the
/// calendar and started testing that the code agrees with itself.
fn civil_from_days_for_test(days: i64) -> (i32, u8, u8) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year as i32, m as u8, d as u8)
}

/// Civil → days since 1970-01-01, likewise independent.
fn days_since_1970_for_test(year: i32, month: u8, day: u8) -> i64 {
    let y = if month <= 2 {
        year as i64 - 1
    } else {
        year as i64
    };
    let m = month as i64;
    let d = day as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}
