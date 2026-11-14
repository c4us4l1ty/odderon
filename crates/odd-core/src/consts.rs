//! Defined astronomical constants used at data boundaries.

/// Speed of light in vacuum, m/s (SI definition, BIPM SI Brochure, 9th ed.).
pub const SPEED_OF_LIGHT_M_S: f64 = 299_792_458.0;
/// Julian year in seconds (365.25 days of 86,400 SI seconds; IAU units guidance).
pub const JULIAN_YEAR_S: f64 = 31_557_600.0;
/// Astronomical unit in meters (IAU 2012 Resolution B2, exact).
pub const ASTRONOMICAL_UNIT_M: f64 = 149_597_870_700.0;
/// Parsec in meters (IAU 2015 Resolution B2: 648,000 / pi astronomical units).
pub const PARSEC_M: f64 = ASTRONOMICAL_UNIT_M * (648_000.0 / std::f64::consts::PI);
/// Julian Date of J2000.0 in TDB (NAIF SPICE time conventions).
pub const J2000_JULIAN_DATE_TDB: f64 = 2_451_545.0;
