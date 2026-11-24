//! The reference orbit the wasm boundary exports, checked natively.
//!
//! `crates/odderon-dmath-wasm` exports `orbit_state_*` so `web/index.html` can stop solving Kepler in
//! JavaScript. That crate cannot be built for the host — it is a `cdylib` with a wasm-only panic
//! handler — so **nothing in it can be unit-tested**. The arithmetic it exports is therefore tested
//! here, natively, on the same elements it builds, and a CI gate (`tools/check-orbit.mjs`) asserts that
//! the page calls the exports rather than re-deriving an orbit of its own.
//!
//! The numbers below are not the kernel's outputs copied down. They are what the orbit *must* be, from
//! the definition of an ellipse, so a change in the propagator has to argue with physics rather than
//! with a previous run:
//!
//!   a = 9800 km, e = 0.30, mu = 398600.4418  =>  p = a(1-e²) = 8918 km
//!   periapsis r = p/(1+e)   = 6860 km
//!   apoapsis  r = p/(1-e)   = 12740 km     (= 2a - periapsis)
//!   v at periapsis = sqrt(mu(2/r - 1/a)) = 8.6912 km/s
//!   period = 2*pi*sqrt(a³/mu) = 160.916 min

use odderon_astro::{mu, propagate, Degeneracy, Elements, State};

const A: f64 = 9800.0;
const E: f64 = 0.30;
const I: f64 = 0.55;
const PERIAPSIS: f64 = 6860.0;
const APOAPSIS: f64 = 12740.0;
const V_PERIAPSIS: f64 = 8.691_175;
const PERIOD_S: f64 = 160.915_857 * 60.0;

/// Exactly the elements `odderon-dmath-wasm::reference_state` builds.
fn reference() -> Elements {
    Elements {
        p: A * (1.0 - E * E),
        e: E,
        i: I,
        raan: 0.0,
        argp: 0.0,
        nu: 0.0,
        degeneracy: Degeneracy::default(),
    }
}

fn norm(v: &odderon_astro::Vec3) -> f64 {
    (v.x * v.x + v.y * v.y + v.z * v.z).sqrt()
}

/// The exported reference orbit starts at periapsis with the speed the vis-viva equation demands.
///
/// This is the check that matters most for the *page*: the page used to state
/// `v = sqrt(mu / r)` — a circular-orbit speed — on an orbit with `e = 0.30`. At periapsis that is wrong
/// by 15%, and a wrong speed on a real state vector is a different orbit, not a rounding error.
#[test]
fn reference_state_starts_at_periapsis_with_vis_viva_speed() {
    let s: State = reference().to_state(mu::EARTH);
    let r = norm(&s.r);
    let v = norm(&s.v);

    assert!(
        (r - PERIAPSIS).abs() < 1e-9,
        "periapsis radius {r} != {PERIAPSIS}"
    );
    assert!(
        (v - V_PERIAPSIS).abs() < 1e-6,
        "periapsis speed {v} != {V_PERIAPSIS} (a circular-orbit sqrt(mu/r) would be {})",
        (mu::EARTH / PERIAPSIS).sqrt()
    );

    // Vis-viva, stated as the thing being asserted rather than left implicit.
    let expected = (mu::EARTH * (2.0 / PERIAPSIS - 1.0 / A)).sqrt();
    assert!((v - expected).abs() < 1e-9, "vis-viva: {v} != {expected}");
}

/// The orbit's specific energy is conserved by propagation: that is what makes it an orbit.
///
/// Specific orbital energy is `v²/2 - mu/r`, and for an ellipse it must equal `-mu/2a` **at every point**.
/// A propagator can be wrong in ways that preserve position on the conic but not velocity (a bad
/// derivative), and those show up here and nowhere else.
#[test]
fn specific_energy_is_conserved_through_a_full_period() {
    let s0 = reference().to_state(mu::EARTH);
    let e0 = norm(&s0.v).powi(2) / 2.0 - mu::EARTH / norm(&s0.r);
    assert!(
        (e0 - -mu::EARTH / (2.0 * A)).abs() < 1e-9,
        "specific energy {e0} != -mu/2a = {}",
        -mu::EARTH / (2.0 * A)
    );

    // Twenty steps is enough that a drift would show, and few enough to name the step if it fails.
    for k in 1..=20 {
        let dt = PERIOD_S * (k as f64) / 20.0;
        let p = propagate(&s0, dt, mu::EARTH);
        assert!(!p.unconverged, "no convergence at dt={dt}");
        assert!(!p.overflowed, "overflow at dt={dt}");
        let e = norm(&p.state.v).powi(2) / 2.0 - mu::EARTH / norm(&p.state.r);
        assert!(
            (e - e0).abs() < 1e-9,
            "energy drifted to {e} at dt={dt} (was {e0})"
        );
    }
}

/// A full period returns the satellite to where it started.
///
/// The closed-orbit property is the one a renderer depends on: ask for the position at `t` and at
/// `t + period` and the two must agree, or the orbit visibly precesses for no physical reason.
#[test]
fn one_period_returns_to_the_start() {
    let s0 = reference().to_state(mu::EARTH);
    let p = propagate(&s0, PERIOD_S, mu::EARTH);
    assert!(!p.unconverged);

    let dr = [
        p.state.r.x - s0.r.x,
        p.state.r.y - s0.r.y,
        p.state.r.z - s0.r.z,
    ];
    let miss = (dr[0] * dr[0] + dr[1] * dr[1] + dr[2] * dr[2]).sqrt();
    // 6860 km orbit, so 1 metre is 1.5e-7 of relative error. Generous against iteration tolerance,
    // strict against a real bug.
    assert!(
        miss < 1e-3,
        "after one period the position is {miss} km off"
    );
}

/// It passes through apoapsis at half a period, and never leaves the conic.
///
/// This is the check that catches a propagator returning a *plausible* radius at every time. Apoapsis
/// is at `t = period/2` for a start at periapsis, and it is the maximum: one metre past it and the
/// radius must already be falling.
#[test]
fn apoapsis_falls_at_half_a_period_and_bounds_every_radius() {
    let s0 = reference().to_state(mu::EARTH);

    let mid = propagate(&s0, PERIOD_S / 2.0, mu::EARTH);
    let r_mid = norm(&mid.state.r);
    assert!(
        (r_mid - APOAPSIS).abs() < 1e-6,
        "half-period radius {r_mid} != apoapsis {APOAPSIS}"
    );

    // Sample the whole period: nothing may exceed apoapsis or fall below periapsis.
    for k in 0..=64 {
        let dt = PERIOD_S * (k as f64) / 64.0;
        let r = norm(&propagate(&s0, dt, mu::EARTH).state.r);
        assert!(
            (PERIAPSIS - 1e-6..=APOAPSIS + 1e-6).contains(&r),
            "radius {r} outside [{PERIAPSIS}, {APOAPSIS}] at dt={dt}"
        );
    }
}

/// Angular momentum is conserved, which is the statement that the orbit stays in one plane.
///
/// `h = r x v` must be constant. If it rotates, the plane is drifting, and the page's camera would show
/// an orbit that slowly tilts — a defect that is easy to miss frame-to-frame and impossible to justify.
#[test]
fn angular_momentum_vector_is_constant() {
    let s0 = reference().to_state(mu::EARTH);
    let h0 = cross(&s0.r, &s0.v);

    for k in 1..=12 {
        let dt = PERIOD_S * (k as f64) / 12.0;
        let st = propagate(&s0, dt, mu::EARTH).state;
        let h = cross(&st.r, &st.v);
        for (name, was, now) in [("x", h0.0, h.0), ("y", h0.1, h.1), ("z", h0.2, h.2)] {
            assert!(
                (now - was).abs() < 1e-6,
                "h.{name} drifted from {was} to {now} at dt={dt}"
            );
        }
    }

    // And its magnitude is sqrt(mu * p), independently.
    let mag = (h0.0 * h0.0 + h0.1 * h0.1 + h0.2 * h0.2).sqrt();
    let expect = (mu::EARTH * A * (1.0 - E * E)).sqrt();
    assert!(
        (mag - expect).abs() < 1e-6,
        "|h| {mag} != sqrt(mu p) {expect}"
    );
}

/// Backwards propagation is the exact inverse of forwards propagation.
///
/// A renderer asks for the state at a *past* time whenever it resumes after idle — "where was it 40 s
/// ago" and "where is it now" must be consistent, or a resumed frame jumps. `propagate(dt)` followed by
/// `propagate(-dt)` has to return to the start.
#[test]
fn backward_propagation_inverts_forward() {
    let s0 = reference().to_state(mu::EARTH);
    for dt in [1.0, 37.0, 900.0, PERIOD_S / 3.0] {
        let fwd = propagate(&s0, dt, mu::EARTH).state;
        let back = propagate(&fwd, -dt, mu::EARTH).state;
        let dr = norm(&odderon_astro::Vec3::new(
            back.r.x - s0.r.x,
            back.r.y - s0.r.y,
            back.r.z - s0.r.z,
        ));
        assert!(dr < 1e-6, "round trip over {dt}s is {dr} km off");
    }
}

fn cross(a: &odderon_astro::Vec3, b: &odderon_astro::Vec3) -> (f64, f64, f64) {
    (
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}
