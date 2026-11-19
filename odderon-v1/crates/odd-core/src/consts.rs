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

/// Newtonian constant of gravitation, m³ kg⁻¹ s⁻² (CODATA 2018 recommended value).
pub const GRAVITATIONAL_CONSTANT: f64 = 6.674_30e-11;
/// Standard gravity, m s⁻² (CGPM 1901, conventional exact value).
pub const STANDARD_GRAVITY_M_S2: f64 = 9.806_65;
/// Stefan–Boltzmann constant, W m⁻² K⁻⁴ (CODATA 2018, rounded).
pub const STEFAN_BOLTZMANN_W_M2_K4: f64 = 5.670_374_419e-8;
/// Molar gas constant, J mol⁻¹ K⁻¹ (CODATA 2018, rounded).
pub const MOLAR_GAS_CONSTANT_J_MOL_K: f64 = 8.314_462_618_153_24;
/// Earth's sidereal rotation rate, rad/s (IAU 2009, nominal).
pub const EARTH_ROTATION_RATE_RAD_S: f64 = 7.292_115e-5;
