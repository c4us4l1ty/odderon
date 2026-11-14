use odd_core::utc::{UtcDateTime, UtcError};

fn utc(y: i32, m: u8, d: u8, h: u8, min: u8, s: u8) -> UtcDateTime {
    UtcDateTime {
        year: y,
        month: m,
        day: d,
        hour: h,
        minute: min,
        second: s,
        fraction: 0.0,
    }
}

#[test]
fn naif_cassini_reference() {
    // NAIF WebGeocalc reference: 140254384.184620 TDB seconds past J2000.
    // https://naif.jpl.nasa.gov/naif/lessons_wgc_r2.html
    let et = utc(2004, 6, 11, 19, 32, 0).to_tdb().unwrap();
    assert!((et.seconds() as f64 + et.fraction() - 140_254_384.184_620).abs() < 0.000_001);
}

#[test]
fn leap_second_is_distinct_and_continuous() {
    let before = utc(2016, 12, 31, 23, 59, 59).to_tdb().unwrap();
    let leap = utc(2016, 12, 31, 23, 59, 60).to_tdb().unwrap();
    let after = utc(2017, 1, 1, 0, 0, 0).to_tdb().unwrap();
    assert!((leap.seconds_since(before) - 1.0).abs() < 1e-8);
    assert!((after.seconds_since(leap) - 1.0).abs() < 1e-8);
    assert_eq!(
        utc(2016, 12, 30, 23, 59, 60).to_tdb(),
        Err(UtcError::InvalidLeapSecond)
    );
    assert_eq!(
        utc(2017, 1, 1, 0, 0, 60).to_tdb(),
        Err(UtcError::InvalidLeapSecond)
    );
}

#[test]
fn rejects_unknown_and_invalid_dates() {
    assert_eq!(
        utc(1971, 12, 31, 23, 59, 59).to_tdb(),
        Err(UtcError::OutsideTableCoverage)
    );
    assert_eq!(
        utc(2027, 1, 1, 0, 0, 0).to_tdb(),
        Err(UtcError::OutsideTableCoverage)
    );
    assert!(utc(2026, 12, 31, 23, 59, 59).to_tdb().is_ok());
    assert_eq!(
        utc(2025, 2, 29, 0, 0, 0).to_tdb(),
        Err(UtcError::InvalidCalendar)
    );
    assert!(utc(2024, 2, 29, 0, 0, 0).to_tdb().is_ok());
    assert_eq!(
        UtcDateTime {
            fraction: f64::NAN,
            ..utc(2024, 1, 1, 0, 0, 0)
        }
        .to_tdb(),
        Err(UtcError::InvalidCalendar)
    );
}
