use odd_astro::{circular_speed, elliptical_speed, hohmann_transfer, OrbitError};

// Earth GM as used by Vallado's example in ARCHITECTURE_LAWS.md, SI units.
const EARTH_MU: f64 = 3.986_004_418e14;

fn near(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() < tolerance,
        "{actual} != {expected}"
    );
}

#[test]
fn published_circular_and_transfer_speeds() {
    // Independent worked example: https://ai-solutions.com/_freeflyeruniversityguide/hohmann_transfer.htm
    // 7,000 to 20,000 km; published numbers rounded to 0.001 km/s.
    let plan = hohmann_transfer(EARTH_MU, 7_000_000.0, 20_000_000.0).unwrap();
    near(circular_speed(EARTH_MU, 7_000_000.0).unwrap(), 7546.0, 0.5);
    near(
        elliptical_speed(EARTH_MU, 7_000_000.0, 13_500_000.0).unwrap(),
        9185.0,
        0.5,
    );
    near(plan.departure_delta_v_m_s, 1639.0, 1.0);
    near(plan.arrival_delta_v_m_s, 1249.0, 1.0);
    assert_eq!(plan.transfer_semi_major_axis_m, 13_500_000.0);
}

#[test]
fn leo_to_geo_sanity_and_reverse_transfer() {
    // LEO-to-GEO sanity check: published rounded example at
    // https://academickids.com/encyclopedia/index.php/Hohmann_transfer_orbit
    let up = hohmann_transfer(EARTH_MU, 7_000_000.0, 42_164_000.0).unwrap();
    near(up.departure_delta_v_m_s, 2336.8, 0.2);
    near(up.arrival_delta_v_m_s, 1433.9, 0.2);
    near(up.coast_time_s, 19_178.2, 1.0);
    let down = hohmann_transfer(EARTH_MU, 42_164_000.0, 7_000_000.0).unwrap();
    near(down.departure_delta_v_m_s, -up.arrival_delta_v_m_s, 1e-9);
    near(down.arrival_delta_v_m_s, -up.departure_delta_v_m_s, 1e-9);
    near(down.total_delta_v_m_s, up.total_delta_v_m_s, 1e-9);
    near(down.coast_time_s, up.coast_time_s, 1e-9);
}

#[test]
fn invalid_values_and_equal_orbits() {
    assert_eq!(circular_speed(EARTH_MU, 0.0), Err(OrbitError::InvalidInput));
    assert_eq!(circular_speed(f64::NAN, 1.0), Err(OrbitError::InvalidInput));
    assert_eq!(
        elliptical_speed(EARTH_MU, 3.0, 1.0),
        Err(OrbitError::RadiusOutsideEllipse)
    );
    assert_eq!(
        hohmann_transfer(EARTH_MU, 1.0, f64::INFINITY),
        Err(OrbitError::InvalidInput)
    );
    assert_eq!(
        circular_speed(f64::MAX, f64::MIN_POSITIVE),
        Err(OrbitError::NumericalRange)
    );
    let same = hohmann_transfer(EARTH_MU, 7_000_000.0, 7_000_000.0).unwrap();
    assert_eq!(same.total_delta_v_m_s, 0.0);
    assert_eq!(same.coast_time_s, 0.0);
}

#[test]
fn vis_viva_near_degenerate_ellipse_apocenter() {
    // 80-digit mpmath result from the exact binary64 inputs. At r just below
    // 2a, subtracting two rounded reciprocals previously produced zero.
    let tiny = f64::from_bits(1);
    assert_eq!(elliptical_speed(tiny, tiny, tiny).unwrap(), 1.0);
    let a = 1e18_f64;
    let r = f64::from_bits((2.0 * a).to_bits() - 1);
    let expected = 2.258_779_682_713_655e-10;
    let result = elliptical_speed(EARTH_MU, r, a).unwrap();
    assert!(
        (result / expected - 1.0).abs() < 1e-14,
        "{result} != {expected}"
    );
    assert_eq!(
        elliptical_speed(EARTH_MU, 2.0 * a, a),
        Err(OrbitError::RadiusOutsideEllipse)
    );
}

#[test]
fn nearly_equal_hohmann_radii_retain_small_burns() {
    // mpmath 80-digit calculation with exact f64 radius inputs.
    let up = hohmann_transfer(EARTH_MU, 7_000_000.0, 7_000_000.000_001).unwrap();
    let expected_departure = 2.695_667_036_401_499e-10;
    let expected_arrival = 2.695_667_036_401_403e-10;
    assert!(
        (up.departure_delta_v_m_s / expected_departure - 1.0).abs() < 1e-13,
        "{up:?}"
    );
    assert!(
        (up.arrival_delta_v_m_s / expected_arrival - 1.0).abs() < 1e-13,
        "{up:?}"
    );
    let down = hohmann_transfer(EARTH_MU, 7_000_000.000_001, 7_000_000.0).unwrap();
    near(down.departure_delta_v_m_s, -up.arrival_delta_v_m_s, 1e-24);
    near(down.arrival_delta_v_m_s, -up.departure_delta_v_m_s, 1e-24);
}
