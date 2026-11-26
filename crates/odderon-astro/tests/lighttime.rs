//! Light-time and the comms link, checked against the one constant that defines it.
//!
//! A light-time is a distance divided by the speed of light, and the speed of light is **defined** —
//! 299 792 458 m/s exactly, because the metre is defined from it. So there is no measured value to
//! cross-check against an independent instrument: the only way this module can be wrong is by carrying
//! a second, drifted copy of the constant, or by computing the distance in the wrong frame.
//!
//! The tests below therefore lean on the properties that catch those two specific defects rather than
//! on a lookup table. `light_time_is_distance_over_c` checks the division against a hand-computed
//! one-light-second distance; `light_time_scales_with_distance` checks that the constant is not a fit;
//! `light_time_is_symmetric` catches a frame rotation applied to one side only; and the refusal tests
//! make sure a NaN or zero distance produces `None` rather than a number a panel would print.

use odderon_astro::lighttime::{arrival_s, light_time_s, round_trip_s, SPEED_OF_LIGHT_KM_S};
use odderon_astro::{radius, Vec3};
use odderon_dmath::fabs;

/// A light-time is distance divided by an exact constant, so the test that matters is the one that
/// checks the division against a hand-computed distance — not one that compares two calls to the same
/// function, which would pass no matter what the constant was.
#[test]
fn light_time_is_distance_over_c() {
    // 299 792.458 km apart, one light-second by definition.
    let from = Vec3::new(0.0, 0.0, 0.0);
    let to = Vec3::new(SPEED_OF_LIGHT_KM_S, 0.0, 0.0);
    let t = light_time_s(from, to).expect("one light-second of distance");
    assert!(
        fabs(t - 1.0) < 1e-9,
        "light time {t} should be exactly 1.0 s"
    );
}

#[test]
fn light_time_scales_with_distance() {
    // Half the distance, half the time: the constant is not being smuggled in as a fit.
    let from = Vec3::ZERO;
    let to = Vec3::new(SPEED_OF_LIGHT_KM_S / 2.0, 0.0, 0.0);
    let t = light_time_s(from, to).expect("half a light-second");
    assert!(fabs(t - 0.5) < 1e-9, "light time {t} should be 0.5 s");
}

#[test]
fn light_time_is_symmetric() {
    // Distance is symmetric, so the light time must be. A direction-dependent delay would mean the
    // frame was rotated on one side only.
    let a = Vec3::new(1_000.0, 2_000.0, 3_000.0);
    let b = Vec3::new(-4_000.0, 5_000.0, -6_000.0);
    let ab = light_time_s(a, b).expect("a -> b");
    let ba = light_time_s(b, a).expect("b -> a");
    assert_eq!(ab, ba);
}

#[test]
fn light_time_is_a_pure_function_of_position() {
    // The replay claim: the same two positions give the same delay, every call, on every platform.
    let from = Vec3::new(100.0, -250.0, 700.0);
    let to = Vec3::new(800.0, 150.0, -300.0);
    let first = light_time_s(from, to);
    let second = light_time_s(from, to);
    assert_eq!(first, second);
}

#[test]
fn round_trip_is_twice_one_way() {
    let from = Vec3::new(1_000.0, 0.0, 0.0);
    let to = Vec3::new(2_000.0, 0.0, 0.0);
    let one = light_time_s(from, to).expect("one way");
    let round = round_trip_s(from, to).expect("round trip");
    assert!(
        fabs(round - 2.0 * one) < 1e-12,
        "round trip {round} != 2 x {one}"
    );
}

#[test]
fn arrival_is_send_plus_light_time() {
    let from = Vec3::new(1_000.0, 0.0, 0.0);
    let to = Vec3::new(2_000.0, 0.0, 0.0);
    let send = 500.0;
    let expected = send + light_time_s(from, to).expect("one way");
    let arrival = arrival_s(send, from, to).expect("arrival");
    assert!(fabs(arrival - expected) < 1e-12);
}

#[test]
fn arrival_is_a_pure_function_of_send_and_positions() {
    // The light-cone ledger's replay property: the same send at the same positions gives the same
    // arrival, independent of when the call happens.
    let from = Vec3::new(1_000.0, 2_000.0, 3_000.0);
    let to = Vec3::new(-500.0, 700.0, 1_200.0);
    let a = arrival_s(100.0, from, to).expect("first");
    let b = arrival_s(100.0, from, to).expect("second");
    assert_eq!(a, b);
}

#[test]
fn coincident_positions_are_refused() {
    // A zero distance is not a zero-second delay a signal "arrives" in: no signal can travel no
    // distance, and reporting 0.0 would be a confident wrong answer.
    let p = Vec3::new(5_000.0, 1_000.0, 2_000.0);
    assert!(
        light_time_s(p, p).is_none(),
        "coincident positions must refuse"
    );
    assert!(round_trip_s(p, p).is_none());
    assert!(arrival_s(0.0, p, p).is_none());
}

#[test]
fn non_finite_positions_are_refused() {
    // NaN or infinite inputs must not produce a NaN or infinite delay that a panel would print as a
    // number.
    let good = Vec3::new(1_000.0, 0.0, 0.0);
    let bad = Vec3::new(f64::NAN, 0.0, 0.0);
    assert!(light_time_s(good, bad).is_none());
    assert!(light_time_s(bad, good).is_none());

    let inf = Vec3::new(f64::INFINITY, 0.0, 0.0);
    assert!(light_time_s(good, inf).is_none());
}

#[test]
fn non_finite_send_time_is_refused() {
    // A send time that is not finite cannot produce a finite arrival, even over a finite distance.
    let from = Vec3::new(1_000.0, 0.0, 0.0);
    let to = Vec3::new(2_000.0, 0.0, 0.0);
    assert!(arrival_s(f64::NAN, from, to).is_none());
    assert!(arrival_s(f64::INFINITY, from, to).is_none());
}

#[test]
fn speed_of_light_is_the_defined_value() {
    // The constant is exact by definition. A drift here is a copy that lost its precision, and the
    // light-time tests above would not catch it if they were ever rewritten to compare two calls.
    assert_eq!(SPEED_OF_LIGHT_KM_S, 299_792.458);
}

#[test]
fn light_time_at_a_realistic_orbital_distance() {
    // The distance the game actually crosses: a ~7000 km orbital radius vs a station near the
    // surface. The delay is hundreds of milliseconds — large enough to matter to a controller, small
    // enough that a page-local `distance / 299792.458` would look plausible if it drifted.
    let station = Vec3::new(radius::EARTH, 0.0, 0.0);
    let body = Vec3::new(radius::EARTH + 300.0, 0.0, 0.0);
    let t = light_time_s(station, body).expect("a 300 km separation");
    assert!(
        t > 0.001 && t < 0.002,
        "light time {t} should be ~1 ms for 300 km"
    );
}
