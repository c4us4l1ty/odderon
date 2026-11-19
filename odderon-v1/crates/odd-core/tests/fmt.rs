use odd_core::consts::{JULIAN_YEAR_S, SPEED_OF_LIGHT_M_S};
use odd_core::fmt;
use odd_core::units::{Meters, Radians, RadiansPerSecond, Seconds};

#[test]
fn display_units_are_separate_from_si_storage() {
    assert_eq!(fmt::kilometers(Meters(1234.0)), 1.234);
    assert_eq!(fmt::days(Seconds(172_800.0)), 2.0);
    assert_eq!(
        fmt::light_years(Meters(SPEED_OF_LIGHT_M_S * JULIAN_YEAR_S)),
        1.0
    );
    assert!((fmt::degrees(Radians(std::f64::consts::PI)) - 180.0).abs() < 1e-12);
    assert!(
        (fmt::revolutions_per_minute(RadiansPerSecond(std::f64::consts::TAU / 60.0)) - 1.0).abs()
            < 1e-12
    );
}
