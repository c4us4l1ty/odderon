//! Apsis targeting must search the whole window `(after_s, end_s)`, including
//! the spline segment that straddles `after_s`. Skipping that segment silently
//! loses a periapsis that falls just after a burn ends, which places the next
//! kick a full revolution late.
use odd_astro::{
    force_model::{Environment, FlightLimits, ForceModel},
    integrator::{IntegrationError, Options},
    spline::Keyframe,
};
use odd_sim::targeting::{next_radial_turn, TargetError};

const MU: f64 = 3.986_004_418e14;
const R: f64 = 6_378_137.0;

fn model() -> ForceModel {
    ForceModel {
        mu: MU,
        zonal: None,
        drag: None,
        radiation: None,
        third_body: false,
        burn: None,
    }
}
fn limits() -> FlightLimits {
    FlightLimits {
        pole: [0.0, 0.0, 1.0],
        equatorial_radius_m: R,
        polar_radius_m: 6_356_752.314,
        entry_altitude_m: Some(120_000.0),
    }
}
fn options() -> Options {
    Options {
        relative_tolerance: 1e-11,
        absolute_tolerance: 1e-11,
        initial_step: 5.0,
        max_step: 60.0,
        max_steps: 1_000_000,
    }
}
fn unavailable(_: f64) -> Result<Environment, IntegrationError> {
    Err(IntegrationError::ForceFailure)
}
fn radial(s: Keyframe) -> f64 {
    s.position.iter().zip(s.velocity).map(|(r, v)| r * v).sum()
}

/// Start at apoapsis of a 6,600 x 7,000 km ellipse so the first periapsis is
/// exactly half a period away and the radial velocity is strictly negative
/// throughout the approach.
fn ellipse() -> (Keyframe, f64, f64) {
    let apo = 7_000_000.0;
    let peri = 6_600_000.0;
    let a = 0.5 * (apo + peri);
    let speed = libm::sqrt(MU * (2.0 / apo - 1.0 / a));
    let period = 2.0 * std::f64::consts::PI * libm::sqrt(a * a * a / MU);
    let initial = Keyframe {
        time: 0.0,
        position: [apo, 0.0, 0.0],
        velocity: [0.0, speed, 0.0],
        mass: 1000.0,
    };
    (initial, 0.5 * period, period)
}

#[test]
fn finds_periapsis_just_after_the_window_start() {
    let (initial, peri_time, period) = ellipse();
    // One second before periapsis: strictly inside the cached segment that
    // contains the turn, because max_step is 60 s.
    let after = peri_time - 1.0;
    let found = next_radial_turn(
        initial,
        after,
        after + 200_000.0,
        true,
        &model(),
        &[],
        limits(),
        options(),
        unavailable,
    )
    .expect("the upcoming periapsis is inside the window");
    assert!(
        (found.time - peri_time).abs() < 1e-3,
        "expected periapsis at {peri_time}, got {} (off by {} s, period {period})",
        found.time,
        found.time - peri_time
    );
    assert!(
        radial(found).abs() < 1.0,
        "not an apsis: r·v={}",
        radial(found)
    );
}

#[test]
fn window_start_exactly_on_a_turn_returns_the_following_turn() {
    let (initial, peri_time, period) = ellipse();
    let found = next_radial_turn(
        initial,
        peri_time,
        peri_time + 2.0 * period,
        true,
        &model(),
        &[],
        limits(),
        options(),
        unavailable,
    )
    .expect("the next periapsis exists");
    assert!(
        (found.time - (peri_time + period)).abs() < 1e-2,
        "expected the following periapsis at {}, got {}",
        peri_time + period,
        found.time
    );
}

#[test]
fn apoapsis_search_also_honours_a_straddling_window_start() {
    let (initial, peri_time, period) = ellipse();
    let apo_time = peri_time + 0.5 * period;
    let after = apo_time - 1.0;
    let found = next_radial_turn(
        initial,
        after,
        after + 200_000.0,
        false,
        &model(),
        &[],
        limits(),
        options(),
        unavailable,
    )
    .expect("the upcoming apoapsis is inside the window");
    assert!(
        (found.time - apo_time).abs() < 1e-3,
        "expected apoapsis at {apo_time}, got {}",
        found.time
    );
}

/// `split_tug` searches from `burns.last().end`. When that end falls just
/// before a periapsis, the turn belongs to the cached segment straddling the
/// window start and must still be found, or the next kick lands a revolution
/// late.
#[test]
fn finds_periapsis_immediately_after_a_burn_end() {
    let (initial, peri_time, period) = ellipse();
    // Near periapsis the spacecraft travels toward -y, so -y thrust is
    // prograde and raises the far side instead of driving an impact.
    let burn = odd_astro::perturb::burn::Burn {
        start: peri_time - 102.0,
        end: peri_time - 2.0,
        force_newtons: [0.0, -2000.0, 0.0],
        specific_impulse_seconds: 450.0,
    };
    let search = |after: f64| {
        next_radial_turn(
            initial,
            after,
            after + 200_000.0,
            true,
            &model(),
            &[burn],
            limits(),
            options(),
            unavailable,
        )
    };
    // The burn moves the apsis, so locate the periapsis that actually follows
    // the burn instead of assuming the unperturbed half-period.
    let reference = search(burn.end).expect("a periapsis follows the burn");
    assert!(reference.time > burn.end);
    assert!(radial(reference).abs() < 10.0);

    // Re-querying from one second before that same turn must return it, not
    // the following revolution. This is the straddling-segment case.
    let again = search(reference.time - 1.0).expect("the same periapsis is still in the window");
    assert!(
        (again.time - reference.time).abs() < 1e-3,
        "expected the same periapsis at {}, got {} (off by {} s, period {period})",
        reference.time,
        again.time,
        again.time - reference.time
    );
}

#[test]
fn rejects_degenerate_and_backwards_windows() {
    let (initial, peri_time, _) = ellipse();
    for (after, end) in [
        (peri_time, peri_time),
        (peri_time, peri_time - 1.0),
        (f64::NAN, peri_time),
        (0.0, f64::INFINITY),
        (-1.0, peri_time),
    ] {
        assert_eq!(
            next_radial_turn(
                initial,
                after,
                end,
                true,
                &model(),
                &[],
                limits(),
                options(),
                unavailable,
            ),
            Err(TargetError::InvalidInput),
            "window ({after}, {end}) should be rejected"
        );
    }
}
