//! The Sounding Rocket, as one measurable claim: a suborbital flight that is a pure function of
//! the authored launch parameters.
//!
//! `docs/BUILD-PLAN.md` §5 names Block 4's job: *author a small vehicle, fly it, recover it* —
//! the first phase that is pure authorship, and the one whose gate is *bit-identical replay in a
//! second browser* (`GAME-DESIGN-BIBLE.md` §17, P3). The gate is a falsifiable statement about
//! the trajectory: the same launch parameters and time must produce the same position, on every
//! platform, every run. `rocket.rs` is the smallest thing that can falsify it.
//!
//! The tests below are grouped by which part of the claim they can falsify:
//!
//! 1. **The inherited machine does not reach space.** The default launch is a complete,
//!    recoverable flight whose apogee falls short of the Kármán line — the "almost-working
//!    inherited machine" the bible describes. If the default reached space, the authorship
//!    mechanic has nothing to diagnose.
//! 2. **The flight is reproducible.** The same parameters, simulated twice, return every reading
//!    bit-for-bit. This is the gate itself: a trajectory that depends on when you ran it is a
//!    story, not a flight.
//! 3. **The verdict boundary is honest.** Escape is decided by energy, not eccentricity, so a
//!    perfectly vertical launch — the degenerate radial limit — is correctly bound and recoverable
//!    rather than misclassified as escape. The four verdicts each mean what they say.
//! 4. **The Kepler invariants hold.** The apogee is where the velocity goes to zero; the impact
//!    time returns the rocket to the surface radius; the launch state is on the surface at the
//!    authored speed and direction. The analytic readings agree with the propagator they wrap.
//! 5. **The launch-state construction is correct.** The position is on the surface at the site
//!    latitude and longitude 0; the velocity magnitude is the burnout velocity; the direction is
//!    the authored azimuth and elevation.
//! 6. **The design hash is stable and distinct.** It pins the kernel constants, so an Akashi entry
//!    or replay bundle can detect a stale kernel.
//! 7. **The refusals.** A non-finite launch is refused rather than answered with a fabricated
//!    trajectory.
//!
//! Every number quoted in a comment was computed against the same constants before being written
//! down, so a failure says which of the two disagreed.

use odderon_astro::rocket::{
    launch_state, rocket_design_hash, rocket_flight, rocket_state_at, LaunchParams, RocketVerdict,
    KARMAN_LINE_KM,
};
use odderon_astro::{mu, propagate, radius};

// ---------------------------------------------------------------------------
// 1. The inherited machine does not reach space
// ---------------------------------------------------------------------------

#[test]
fn inherited_flight_is_suborbital_and_falls_short_of_space() {
    // The inherited launch: 45° elevation, 1.5 km/s burnout, north. Apogee ~59 km, well below
    // the 100 km Kármán line. A complete, recoverable flight — but one that does not reach space,
    // which is the authorship puzzle the player must solve.
    let params = LaunchParams::inherited();
    let flight = rocket_flight(&params);

    assert_eq!(flight.verdict, RocketVerdict::Suborbital);
    // Apogee ~59 km, below the Kármán line.
    assert!(
        flight.apogee_km < KARMAN_LINE_KM,
        "inherited apogee {} should fall short of the Kármán line",
        flight.apogee_km
    );
    // It is a real, recoverable flight: it has an apogee time and an impact time.
    assert!(flight.time_to_apogee_s > 0.0);
    assert!(flight.impact_time_s > flight.time_to_apogee_s);
    assert!(flight.range_km > 0.0);
    // Verified value: apogee ≈ 58.996 km, impact ≈ 223.17 s, range ≈ 233.82 km.
    assert!((flight.apogee_km - 58.99616489732307).abs() < 1e-6);
    assert!((flight.impact_time_s - 223.16693056629583).abs() < 1e-6);
    assert!((flight.range_km - 233.81531963963184).abs() < 1e-6);
}

// ---------------------------------------------------------------------------
// 2. The flight is reproducible (the gate itself)
// ---------------------------------------------------------------------------

#[test]
fn flight_is_bit_identical_across_runs() {
    // The replay gate: the same parameters, simulated twice, return every reading bit-for-bit.
    let params = LaunchParams {
        latitude_rad: 0.4,
        azimuth_rad: 1.1,
        elevation_rad: 0.6,
        burnout_velocity_km_s: 2.8,
    };
    let a = rocket_flight(&params);
    let b = rocket_flight(&params);
    assert_eq!(a, b);
    assert_eq!(a.apogee_km.to_bits(), b.apogee_km.to_bits());
    assert_eq!(a.impact_time_s.to_bits(), b.impact_time_s.to_bits());
    assert_eq!(a.range_km.to_bits(), b.range_km.to_bits());
}

#[test]
fn trajectory_state_is_bit_identical_across_runs() {
    // The state the page renders must be the same on every run too — not just the summary.
    let params = LaunchParams::inherited();
    let t = 100.0;
    let a = rocket_state_at(&params, t);
    let b = rocket_state_at(&params, t);
    assert_eq!(a.r.x.to_bits(), b.r.x.to_bits());
    assert_eq!(a.r.y.to_bits(), b.r.y.to_bits());
    assert_eq!(a.r.z.to_bits(), b.r.z.to_bits());
    assert_eq!(a.v.x.to_bits(), b.v.x.to_bits());
    assert_eq!(a.v.y.to_bits(), b.v.y.to_bits());
    assert_eq!(a.v.z.to_bits(), b.v.z.to_bits());
}

// ---------------------------------------------------------------------------
// 3. The verdict boundary is honest
// ---------------------------------------------------------------------------

#[test]
fn radial_vertical_launch_is_bound_not_escape() {
    // A perfectly vertical launch has zero angular momentum: its elements degenerate to e -> 1,
    // but it is bound (energy < 0) and returns to the surface. Eccentricity alone would call this
    // escape; the energy discriminator must not. This is the honest test of the boundary.
    let params = LaunchParams {
        latitude_rad: 0.0,
        azimuth_rad: 0.0,
        elevation_rad: std::f64::consts::PI / 2.0,
        burnout_velocity_km_s: 3.0,
    };
    let flight = rocket_flight(&params);
    assert_eq!(flight.verdict, RocketVerdict::Suborbital);
    // A vertical launch lands on its own pad: zero range.
    assert_eq!(flight.range_km, 0.0);
    // Verified: apogee ≈ 494.9 km, impact ≈ 676.68 s.
    assert!((flight.apogee_km - 494.8997661995554).abs() < 1e-6);
    assert!((flight.impact_time_s - 676.6769808399731).abs() < 1e-6);
}

#[test]
fn high_velocity_vertical_launch_still_bound() {
    // Even a fast vertical launch below escape velocity is bound and recoverable.
    let params = LaunchParams {
        latitude_rad: 0.0,
        azimuth_rad: 0.0,
        elevation_rad: std::f64::consts::PI / 2.0,
        burnout_velocity_km_s: 5.0,
    };
    let flight = rocket_flight(&params);
    assert_eq!(flight.verdict, RocketVerdict::Suborbital);
    assert!(flight.range_km == 0.0);
    // Verified: apogee ≈ 1594.7 km.
    assert!((flight.apogee_km - 1594.6998707008634).abs() < 1e-6);
}

#[test]
fn escape_is_decided_by_energy() {
    // A launch above escape velocity is genuinely hyperbolic: the rocket never returns.
    let params = LaunchParams {
        latitude_rad: 0.0,
        azimuth_rad: 0.0,
        elevation_rad: std::f64::consts::PI / 2.0,
        burnout_velocity_km_s: 12.0,
    };
    let flight = rocket_flight(&params);
    assert_eq!(flight.verdict, RocketVerdict::Escape);
    // No apogee, no impact, no range: the readings do not apply.
    assert!(flight.apogee_km.is_nan());
    assert!(flight.impact_time_s.is_nan());
    assert!(flight.range_km.is_nan());
}

#[test]
fn orbital_launch_has_no_impact() {
    // A horizontal launch above circular velocity at the surface is orbital: perigee at or above
    // the surface, so there is no impact. Apogee is still defined.
    let params = LaunchParams {
        latitude_rad: 0.0,
        azimuth_rad: std::f64::consts::PI / 2.0,
        elevation_rad: 0.0,
        burnout_velocity_km_s: 8.0,
    };
    let flight = rocket_flight(&params);
    assert_eq!(flight.verdict, RocketVerdict::Orbital);
    assert!(flight.impact_time_s.is_nan());
    assert!(flight.range_km.is_nan());
    // Verified: apogee ≈ 314.82 km.
    assert!((flight.apogee_km - 314.81839358244633).abs() < 1e-6);
}

#[test]
fn every_verdict_is_reachable() {
    // The four verdicts are not a dead code path: each is produced by a launch that genuinely
    // has that character. A gate is a gate only when the branches are exercised.
    let suborbital = rocket_flight(&LaunchParams::inherited()).verdict;
    let orbital = rocket_flight(&LaunchParams {
        latitude_rad: 0.0,
        azimuth_rad: std::f64::consts::PI / 2.0,
        elevation_rad: 0.0,
        burnout_velocity_km_s: 8.0,
    })
    .verdict;
    let escape = rocket_flight(&LaunchParams {
        latitude_rad: 0.0,
        azimuth_rad: 0.0,
        elevation_rad: std::f64::consts::PI / 2.0,
        burnout_velocity_km_s: 12.0,
    })
    .verdict;
    let refused = rocket_flight(&LaunchParams {
        latitude_rad: f64::NAN,
        azimuth_rad: 0.0,
        elevation_rad: 0.5,
        burnout_velocity_km_s: 2.0,
    })
    .verdict;

    assert_eq!(suborbital, RocketVerdict::Suborbital);
    assert_eq!(orbital, RocketVerdict::Orbital);
    assert_eq!(escape, RocketVerdict::Escape);
    assert_eq!(refused, RocketVerdict::Refused);
}

// ---------------------------------------------------------------------------
// 4. The Kepler invariants hold
// ---------------------------------------------------------------------------

#[test]
fn apogee_is_where_velocity_goes_to_zero() {
    // The apogee reading must agree with the propagator: at time-to-apogee, the rocket's radial
    // speed has fallen to zero and its distance is the surface radius plus the apogee altitude.
    let params = LaunchParams::inherited();
    let flight = rocket_flight(&params);
    let at_apogee = propagate(&launch_state(&params), flight.time_to_apogee_s, mu::EARTH);

    let r_expected = radius::EARTH + flight.apogee_km;
    assert!(
        (at_apogee.state.r.norm() - r_expected).abs() < 1e-3,
        "apogee radius {} vs expected {}",
        at_apogee.state.r.norm(),
        r_expected
    );
    // At apogee the radial component of velocity is zero (the rocket is momentarily moving
    // purely tangentially). The tangential speed is non-zero for a non-radial arc, so the test
    // checks the radial component, not the full speed.
    let r_hat = at_apogee.state.r.unit();
    let radial_v = at_apogee.state.v.dot(r_hat);
    assert!(
        radial_v.abs() < 1e-3,
        "radial velocity at apogee {} should be ~0",
        radial_v
    );
}

#[test]
fn impact_time_returns_to_surface() {
    // At the impact time, the rocket is back on the surface — the reading and the propagator
    // must agree that the trajectory closes.
    let params = LaunchParams::inherited();
    let flight = rocket_flight(&params);
    let at_impact = propagate(&launch_state(&params), flight.impact_time_s, mu::EARTH);
    assert!(
        (at_impact.state.r.norm() - radius::EARTH).abs() < 1e-3,
        "impact radius {} vs surface {}",
        at_impact.state.r.norm(),
        radius::EARTH
    );
}

#[test]
fn impact_time_is_twice_time_to_apogee() {
    // The suborbital ellipse is symmetric: launch and impact are both at r = R, mirrored about
    // the apoapsis. So impact time is exactly twice the time to apogee — a pure invariant.
    let params = LaunchParams::inherited();
    let flight = rocket_flight(&params);
    assert!(
        (flight.impact_time_s - 2.0 * flight.time_to_apogee_s).abs() < 1e-6,
        "impact {} should be twice apogee time {}",
        flight.impact_time_s,
        flight.time_to_apogee_s
    );
}

#[test]
fn range_is_the_central_angle_times_radius() {
    // The ground range is the central angle between launch and impact positions, times the
    // surface radius. This must agree with propagating to impact and measuring directly.
    let params = LaunchParams::inherited();
    let flight = rocket_flight(&params);
    let at_impact = propagate(&launch_state(&params), flight.impact_time_s, mu::EARTH);
    let r_launch = launch_state(&params).r;
    let r_impact = at_impact.state.r;
    let central_angle_true = central_angle_of(&r_launch, &r_impact);
    let range_from_propagator = radius::EARTH * central_angle_true;
    assert!(
        (flight.range_km - range_from_propagator).abs() < 1e-3,
        "range {} vs propagator {}",
        flight.range_km,
        range_from_propagator
    );
}

/// Central angle between two position vectors, in radians.
fn central_angle_of(a: &odderon_astro::Vec3, b: &odderon_astro::Vec3) -> f64 {
    let cos_a = a.dot(*b) / (a.norm() * b.norm());
    f64::acos(cos_a.clamp(-1.0, 1.0))
}

// ---------------------------------------------------------------------------
// 5. The launch-state construction is correct
// ---------------------------------------------------------------------------

#[test]
fn launch_state_is_on_surface_at_site_latitude() {
    // The launch position is on the surface, at the site latitude, longitude 0.
    let params = LaunchParams {
        latitude_rad: 0.5,
        azimuth_rad: 0.0,
        elevation_rad: 0.5,
        burnout_velocity_km_s: 2.0,
    };
    let state = launch_state(&params);
    let r = state.r;
    assert!((r.norm() - radius::EARTH).abs() < 1e-6);
    // latitude = 0.5 rad: z/r = sin(lat), x/r = cos(lat), y = 0.
    assert!((r.z / r.x - f64::tan(0.5)).abs() < 1e-9);
    assert!(r.y.abs() < 1e-9);
}

#[test]
fn launch_state_velocity_magnitude_is_burnout_velocity() {
    // The velocity magnitude is exactly the authored burnout velocity.
    let params = LaunchParams {
        latitude_rad: 0.3,
        azimuth_rad: 1.0,
        elevation_rad: 0.7,
        burnout_velocity_km_s: 2.4,
    };
    let state = launch_state(&params);
    assert!((state.v.norm() - 2.4).abs() < 1e-9);
}

#[test]
fn launch_state_velocity_direction_matches_azimuth_and_elevation() {
    // At the equator (lat = 0), north is +z, east is +y, up is +x. A 45°/45° launch points
    // northeast-up: the velocity direction should have equal components in +z (north), +y
    // (east), and +x (up), scaled by cos/sin of elevation.
    let params = LaunchParams {
        latitude_rad: 0.0,
        azimuth_rad: std::f64::consts::PI / 4.0, // northeast
        elevation_rad: std::f64::consts::PI / 4.0,
        burnout_velocity_km_s: 1.0,
    };
    let v = launch_state(&params).v;
    // unit direction components: east = sin(az)*cos(el), north = cos(az)*cos(el), up = sin(el).
    let east = f64::sin(std::f64::consts::PI / 4.0) * f64::cos(std::f64::consts::PI / 4.0);
    let north = f64::cos(std::f64::consts::PI / 4.0) * f64::cos(std::f64::consts::PI / 4.0);
    let up = f64::sin(std::f64::consts::PI / 4.0);
    assert!(
        (v.y - east).abs() < 1e-9,
        "east component {} vs {}",
        v.y,
        east
    );
    assert!(
        (v.z - north).abs() < 1e-9,
        "north component {} vs {}",
        v.z,
        north
    );
    assert!((v.x - up).abs() < 1e-9, "up component {} vs {}", v.x, up);
}

// ---------------------------------------------------------------------------
// 6. The design hash
// ---------------------------------------------------------------------------

#[test]
fn design_hash_is_stable_and_distinct() {
    // The hash must be a pure function of the kernel constants: stable across calls, and a
    // distinct value that an Akashi entry or replay bundle can pin.
    let a = rocket_design_hash();
    let b = rocket_design_hash();
    assert_eq!(a, b);
    assert_ne!(a, 0);
}

// ---------------------------------------------------------------------------
// 7. The refusals
// ---------------------------------------------------------------------------

#[test]
fn non_finite_launch_is_refused() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let params = LaunchParams {
            latitude_rad: bad,
            azimuth_rad: 0.0,
            elevation_rad: 0.5,
            burnout_velocity_km_s: 2.0,
        };
        let flight = rocket_flight(&params);
        assert_eq!(flight.verdict, RocketVerdict::Refused);
        assert!(flight.apogee_km.is_nan());
        assert!(flight.impact_time_s.is_nan());
        assert!(flight.range_km.is_nan());
    }
}

#[test]
fn zero_velocity_is_refused() {
    // Zero burnout velocity produces no orbit: the rocket simply sits on the pad. Refused, not
    // answered with a fake trajectory.
    let params = LaunchParams {
        latitude_rad: 0.0,
        azimuth_rad: 0.0,
        elevation_rad: 0.5,
        burnout_velocity_km_s: 0.0,
    };
    let flight = rocket_flight(&params);
    assert_eq!(flight.verdict, RocketVerdict::Refused);
}

// ---------------------------------------------------------------------------
// Cross-checks against the existing orbital core
// ---------------------------------------------------------------------------

#[test]
fn rocket_state_at_matches_propagate() {
    // The convenience wrapper must produce exactly what the propagator produces, so the page's
    // rendered trajectory and the reported readings come from the same source.
    let params = LaunchParams::inherited();
    let t = 80.0;
    let from_wrapper = rocket_state_at(&params, t);
    let from_propagator = propagate(&launch_state(&params), t, mu::EARTH).state;
    assert_eq!(from_wrapper.r.x.to_bits(), from_propagator.r.x.to_bits());
    assert_eq!(from_wrapper.r.y.to_bits(), from_propagator.r.y.to_bits());
    assert_eq!(from_wrapper.r.z.to_bits(), from_propagator.r.z.to_bits());
    assert_eq!(from_wrapper.v.x.to_bits(), from_propagator.v.x.to_bits());
    assert_eq!(from_wrapper.v.y.to_bits(), from_propagator.v.y.to_bits());
    assert_eq!(from_wrapper.v.z.to_bits(), from_propagator.v.z.to_bits());
}

#[test]
fn higher_burnout_velocity_reaches_higher_apogee() {
    // Monotonicity: a faster burnout, same direction, reaches a higher apogee. A trajectory that
    // does not improve with more energy is not a trajectory.
    let low = rocket_flight(&LaunchParams {
        latitude_rad: 0.0,
        azimuth_rad: 0.0,
        elevation_rad: std::f64::consts::PI / 4.0,
        burnout_velocity_km_s: 2.0,
    });
    let high = rocket_flight(&LaunchParams {
        latitude_rad: 0.0,
        azimuth_rad: 0.0,
        elevation_rad: std::f64::consts::PI / 4.0,
        burnout_velocity_km_s: 3.0,
    });
    assert!(high.apogee_km > low.apogee_km);
}
