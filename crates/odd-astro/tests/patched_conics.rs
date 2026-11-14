use odd_astro::patched_conics::{
    bi_elliptic_transfer, patch_to_primary, patch_to_secondary, sphere_of_influence,
};
use odd_astro::{hohmann_transfer, vis_viva_speed, OrbitError};
use odd_core::frames::State;
const MU: f64 = 3.986_004_418e14;
fn near(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() <= t, "{a} != {b} (tolerance {t})");
}

#[test]
fn published_bi_elliptic_example_and_reverse() {
    // https://en.wikipedia.org/wiki/Bi-elliptic_transfer example, rounded table.
    let p = bi_elliptic_transfer(MU, 6_700_000.0, 93_800_000.0, 507_688_000.0).unwrap();
    near(p.departure_delta_v_m_s, 3123.62, 0.01);
    near(p.intermediate_delta_v_m_s, 351.836, 0.002);
    near(p.arrival_delta_v_m_s, -616.926, 0.002);
    near(p.total_delta_v_m_s, 4092.38, 0.01);
    assert!(
        p.total_delta_v_m_s
            < hohmann_transfer(MU, 6_700_000.0, 93_800_000.0)
                .unwrap()
                .total_delta_v_m_s
    );
    let reverse = bi_elliptic_transfer(MU, 93_800_000.0, 6_700_000.0, 507_688_000.0).unwrap();
    near(reverse.departure_delta_v_m_s, -p.arrival_delta_v_m_s, 1e-10);
    near(
        reverse.intermediate_delta_v_m_s,
        -p.intermediate_delta_v_m_s,
        1e-10,
    );
    near(reverse.arrival_delta_v_m_s, -p.departure_delta_v_m_s, 1e-10);
    near(reverse.coast_time_s, p.coast_time_s, 1e-7);
    assert_eq!(
        bi_elliptic_transfer(MU, 7e6, 8e6, 7.5e6),
        Err(OrbitError::InvalidInput)
    );
}
#[test]
fn soi_and_patch_are_instantaneous_approximations() {
    // Earth/Sun example: direct independent evaluation from masses and 1 AU.
    near(
        sphere_of_influence(149_597_870_700.0, 5.9722e24, 1.98847e30).unwrap(),
        924_637_562.051_003,
        1e-4,
    );
    assert_eq!(
        sphere_of_influence(1.0, 2.0, 1.0),
        Err(OrbitError::InvalidInput)
    );
    let craft = State {
        position_m: [10., 20., 30.],
        velocity_m_s: [1., 2., 3.],
    };
    let body = State {
        position_m: [5., 7., 11.],
        velocity_m_s: [0.5, 1., 1.5],
    };
    assert_eq!(
        patch_to_primary(patch_to_secondary(craft, body).unwrap(), body).unwrap(),
        craft
    );
    assert_eq!(
        patch_to_secondary(craft, body).unwrap(),
        State {
            position_m: [5., 13., 19.],
            velocity_m_s: [0.5, 1., 1.5]
        }
    );
    assert_eq!(
        patch_to_secondary(
            craft,
            State {
                position_m: [f64::NAN, 0., 0.],
                ..body
            }
        ),
        Err(OrbitError::InvalidInput)
    );
}
#[test]
fn vis_viva_for_all_conics() {
    let r = 7e6;
    near(
        vis_viva_speed(MU, r, Some(-1e7)).unwrap(),
        (MU * (2.0 / r + 1.0 / 1e7)).sqrt(),
        1e-9,
    );
    near(
        vis_viva_speed(MU, r, None).unwrap(),
        (2.0 * MU / r).sqrt(),
        1e-9,
    );
    near(
        vis_viva_speed(MU, r, Some(r)).unwrap(),
        (MU / r).sqrt(),
        1e-9,
    );
    assert_eq!(
        vis_viva_speed(MU, r, Some(0.0)),
        Err(OrbitError::InvalidInput)
    );
    assert_eq!(
        vis_viva_speed(MU, r, Some(r / 3.0)),
        Err(OrbitError::RadiusOutsideEllipse)
    );
}

#[test]
fn hyperbolic_vis_viva_avoids_reciprocal_overflow() {
    // The reciprocal of |a| overflows, but mu/|a| and the speed are finite.
    let speed = odd_astro::vis_viva_speed(1e-300, 1.0, Some(-1e-310)).unwrap();
    assert!((speed / 100_000.0 - 1.0).abs() < 1e-13);
}

#[test]
fn bi_elliptic_close_radii_retain_small_burns() {
    // mpmath at 80 digits using the exact binary64 inputs.
    let result =
        bi_elliptic_transfer(MU, 7_000_000.0, 7_000_000.000_001, 7_000_000.000_002).unwrap();
    let reverse =
        bi_elliptic_transfer(MU, 7_000_000.000_001, 7_000_000.0, 7_000_000.000_002).unwrap();
    near(
        reverse.departure_delta_v_m_s,
        -result.arrival_delta_v_m_s,
        1e-24,
    );
    near(
        reverse.intermediate_delta_v_m_s,
        -result.intermediate_delta_v_m_s,
        1e-24,
    );
    near(
        reverse.arrival_delta_v_m_s,
        -result.departure_delta_v_m_s,
        1e-24,
    );
    let same = bi_elliptic_transfer(MU, 7_000_000.0, 7_000_000.0, 7_000_000.0).unwrap();
    assert_eq!(same.total_delta_v_m_s, 0.0);
    assert!(same.coast_time_s > 0.0); // Two explicitly requested half-circles.
    for (got, expected) in [
        (result.departure_delta_v_m_s, 5.388_824_140_738_829e-10),
        (result.intermediate_delta_v_m_s, 2.695_667_036_401_307e-10),
        (result.arrival_delta_v_m_s, -2.693_157_104_337_233_5e-10),
    ] {
        assert!((got / expected - 1.0).abs() < 1e-13, "{result:?}");
    }
}
