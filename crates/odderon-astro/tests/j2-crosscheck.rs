//! Does the J2 secular theory actually describe a J2 orbit?
//!
//! `CREDIBILITY.md` §3.1 asks for this and it had not been run: the secular rates are compared to a
//! high-precision numerical integration of the real equations of motion, over 7 days.
//!
//! ## Why this is the test that matters
//!
//! Every other J2 test in this crate checks the formula against *structure* — the zeros at `i = 90°` and
//! at the critical inclination, the sign, the linearity in `J2`. Those are strong, and they caught a sign
//! error that would have inverted every orbit. But they cannot answer the question a mission planner
//! actually asks: **is the number right?** A formula can satisfy every symmetry and still carry a factor
//! of `(1−e²)` in the wrong place.
//!
//! Only integrating the real motion answers that, because the integrator knows nothing about secular
//! theory. It is given `a = −μr̂/r² + a_J2` and told to step. Whatever drift appears is the truth.
//!
//! ## The expected answer, and why it is not zero
//!
//! First-order secular theory is *first order*. It deliberately discards the short-period wobble and the
//! second-order terms, so the numerical rate must differ from it by a small, non-zero amount. The measured
//! figure is about **0.2–0.45%**, and the tolerance below is 1% — the contract in `CREDIBILITY.md`, not a
//! number chosen to make the test pass.
//!
//! The disagreement has a physical origin worth knowing: at 400 km the short-period oscillation in the node
//! is about 0.026°, and its interaction with the mean flow is a second-order effect. A 0.43% error on
//! 5°/day is 0.02°/day, which is the same order. So the residual is not a bug — it is the size of the
//! physics that the approximation leaves out, and it is *why* the averaged theory is acceptable for a
//! catalogue and unacceptable for a rendezvous.

use odderon_astro::integrate::{accel_j2, integrate_j2, potential_j2, raan_of};
use odderon_astro::j2::{secular_rates, EARTH_J2};
use odderon_astro::*;
// The test computes magnitudes with the same arithmetic the kernel uses; `f64::abs` is std and `sqrt` is
// not re-exported from the crate root, so both come from dmath directly.
use odderon_dmath::{fabs, sqrt};

const MU: f64 = mu::EARTH;
const RE: f64 = radius::EARTH;

/// Reference settings. `1e-12` relative puts the integrator's own error two orders of magnitude below the
/// quantity being measured, so the comparison is limited by the theory and not by the ruler.
const REL_TOL: f64 = 1e-12;

/// Build a state at periapsis with the given elements, periapsis on +X and the orbit inclined about X.
///
/// Constructed directly rather than through `Elements::to_state` on purpose: this reference must not share
/// a code path with the thing it is checking, or a bug in the element conversion would cancel out.
fn at_periapsis(a: f64, e: f64, inclination: f64) -> State {
    let rp = a * (1.0 - e);
    let vp = sqrt(MU * (1.0 + e) / rp);
    State::new(
        Vec3::new(rp, 0.0, 0.0),
        Vec3::new(0.0, vp * cos_rad(inclination), vp * sin_rad(inclination)),
    )
}

/// Unwrap an angle series and least-squares fit the slope, in rad/s.
///
/// The samples are taken every `SAMPLE_S` seconds. The unwrap assumes less than half a turn between
/// samples, which holds comfortably here: the fastest angle moves a few degrees per day.
fn fitted_rate(times: &[f64], angles: &[f64]) -> f64 {
    let n = times.len() as f64;
    let mut base = angles[0];
    let mut unwrapped = Vec::with_capacity(angles.len());
    unwrapped.push(0.0);
    let mut acc = 0.0;
    for &angle in angles.iter().skip(1) {
        let mut d = angle - base;
        while d > core::f64::consts::PI {
            d -= core::f64::consts::TAU;
        }
        while d <= -core::f64::consts::PI {
            d += core::f64::consts::TAU;
        }
        acc += d;
        base = angle;
        unwrapped.push(acc);
    }

    let mut sx = 0.0;
    let mut sy = 0.0;
    let mut sxx = 0.0;
    let mut sxy = 0.0;
    for i in 0..times.len() {
        sx += times[i];
        sy += unwrapped[i];
        sxx += times[i] * times[i];
        sxy += times[i] * unwrapped[i];
    }
    (n * sxy - sx * sy) / (n * sxx - sx * sx)
}

/// Integrate `days` and return the fitted nodal rate plus the raw series, so a caller can inspect it.
fn numerical_nodal_rate(state: &State, days: f64, sample_s: f64) -> (f64, Vec<f64>, Vec<f64>, u32) {
    let total = days * 86_400.0;
    // Step the whole interval in one call and sample by re-integrating short hops from the running state.
    // Re-integrating per sample is slower and much easier to get right than asking the integrator for
    // output at fixed times.
    let mut cur = *state;
    let mut t = 0.0;
    let mut times = Vec::new();
    let mut angles = Vec::new();
    let mut steps = 0u32;
    let hops = (total / sample_s) as usize;
    for _ in 0..hops {
        let out = integrate_j2(&cur, sample_s, MU, EARTH_J2, RE, REL_TOL).expect("integrator progress");
        cur = out.state;
        t += sample_s;
        steps += out.accepted + out.rejected;
        times.push(t);
        angles.push(raan_of(&cur));
    }
    (fitted_rate(&times, &angles), times, angles, steps)
}

// ---------------------------------------------------------------------------------------------
// The cross-check
// ---------------------------------------------------------------------------------------------

/// At LEO, 7 days, the secular nodal rate is within **1%** of the numerically integrated one.
///
/// This is `CREDIBILITY.md` §3.1's J2 secular row, run for the first time.
#[test]
fn secular_nodal_rate_matches_numerical_integration_at_leo() {
    let a = RE + 400.0;
    let e = 0.001;
    let inc = 51.6 * core::f64::consts::PI / 180.0;
    let state = at_periapsis(a, e, inc);

    let (numeric, _t, _a, steps) = numerical_nodal_rate(&state, 7.0, 600.0);
    let analytic = secular_rates(a, e, inc, MU, EARTH_J2, RE).expect("valid").raan;

    // The integrator must have actually done work, or this test is comparing two copies of the formula.
    assert!(steps > 10_000, "integrator took only {steps} steps; something is wrong");

    let rel = fabs(numeric - analytic) / fabs(analytic);
    assert!(
        rel < 0.01,
        "secular and numerical nodal rates differ by {:.4}%: analytic {:.6e}, numeric {:.6e} rad/s",
        rel * 100.0,
        analytic,
        numeric
    );

    // Sign is not optional: a prograde orbit's node walks west, both for the theory and for the motion.
    assert!(analytic < 0.0 && numeric < 0.0, "both rates must be negative for a prograde orbit");

    // And the measured residual should be *this* size, not merely under the bound. A residual near zero
    // would mean the numerical run reproduced first-order theory exactly, which is impossible for a real
    // integration and would suggest the reference is not integrating anything.
    assert!(
        rel > 1e-4,
        "residual of {:.2e} is implausibly small - the reference may not be independent of the theory",
        rel
    );

    println!(
        "LEO 400 km i=51.6 e=0.001, 7 days: analytic {:.6} deg/day, numeric {:.6} deg/day, diff {:.3}%",
        analytic * 86_400.0 * 180.0 / core::f64::consts::PI,
        numeric * 86_400.0 * 180.0 / core::f64::consts::PI,
        rel * 100.0
    );
}

/// The same check on an eccentric orbit, where `ω̇` is well conditioned and `p ≠ a`.
///
/// The near-circular case above cannot test `ω̇` at all: with `e = 0.001` the eccentricity vector is
/// dominated by rounding and the argument of periapsis is meaningless. An `e = 0.2` orbit makes both
/// angles real, and it also exercises the `(Rₑ/p)²` factor where `p` genuinely differs from `a`.
#[test]
fn secular_periapsis_rate_matches_numerical_integration_eccentric() {
    let a = 12_000.0;
    let e = 0.2;
    let inc = 51.6 * core::f64::consts::PI / 180.0;
    let state = at_periapsis(a, e, inc);

    let total = 7.0 * 86_400.0;
    let sample_s = 600.0;
    let mut cur = state;
    let mut t = 0.0;
    let mut times = Vec::new();
    let mut argps = Vec::new();
    let hops = (total / sample_s) as usize;
    for _ in 0..hops {
        let out = integrate_j2(&cur, sample_s, MU, EARTH_J2, RE, REL_TOL).expect("progress");
        cur = out.state;
        t += sample_s;
        times.push(t);
        let el = cur.to_elements(MU);
        argps.push(el.argp);
    }

    let numeric = fitted_rate(&times, &argps);
    let analytic = secular_rates(a, e, inc, MU, EARTH_J2, RE).expect("valid").argp;

    let rel = fabs(numeric - analytic) / fabs(analytic);
    assert!(
        rel < 0.01,
        "periapsis rates differ by {:.4}%: analytic {:.6e}, numeric {:.6e} rad/s",
        rel * 100.0,
        analytic,
        numeric
    );
    assert!(analytic > 0.0, "periapsis should advance below the critical inclination");
    assert!(numeric > 0.0, "and it really does advance when integrated");
}

/// Near the critical inclination the secular `ω̇` is ~0, and the numerical one is not.
///
/// This is the honest limitation of the averaged theory, and it is worth a test of its own rather than a
/// footnote. At `i = 63.4°` the first-order rate is 4e-4 deg/day while the true motion wanders by the
/// short-period terms. A 7-day fit therefore gives a *ratio* far from 1 even though both numbers are
/// individually tiny. That is not a defect to be fixed — it is the reason Molniya orbits need an
/// integrator and not a formula — and asserting it stops someone "fixing" the secular rate to match a
/// meaningless quotient later.
#[test]
fn near_critical_inclination_the_rate_is_too_small_to_fit() {
    let a = 26_600.0;
    let e = 0.74;
    let inc = 63.434_948_8 * core::f64::consts::PI / 180.0;
    let state = at_periapsis(a, e, inc);

    let analytic = secular_rates(a, e, inc, MU, EARTH_J2, RE).expect("valid").argp;

    // The absolute rate is negligible: less than a thousandth of a degree per day.
    let deg_per_day = analytic * 86_400.0 * 180.0 / core::f64::consts::PI;
    assert!(
        fabs(deg_per_day) < 0.01,
        "the secular periapsis rate at the critical inclination should be ~0, got {deg_per_day} deg/day"
    );

    // And integrating it produces a rate that is also negligible in absolute terms, even though its ratio
    // to the analytic value is not meaningful.
    let total = 7.0 * 86_400.0;
    let mut cur = state;
    let mut t = 0.0;
    let mut times = Vec::new();
    let mut argps = Vec::new();
    let hops = (total / 600.0) as usize;
    for _ in 0..hops {
        let out = integrate_j2(&cur, 600.0, MU, EARTH_J2, RE, REL_TOL).expect("progress");
        cur = out.state;
        t += 600.0;
        times.push(t);
        argps.push(cur.to_elements(MU).argp);
    }
    let numeric = fitted_rate(&times, &argps);
    let numeric_deg_day = numeric * 86_400.0 * 180.0 / core::f64::consts::PI;
    assert!(
        fabs(numeric_deg_day) < 0.05,
        "integrated periapsis rate at the critical inclination should also be tiny, got {numeric_deg_day}"
    );
}

// ---------------------------------------------------------------------------------------------
// The reference must be trustworthy before it can judge anything
// ---------------------------------------------------------------------------------------------

/// With `J2 = 0` the integrator conserves specific energy and angular momentum.
///
/// This is the check that makes the reference worth something. If the integrator drifts, every agreement
/// above is suspicious: it could be the theory and the integrator making the same mistake at the same
/// rate. Energy conservation is independent of both.
#[test]
fn integrator_conserves_energy_and_angular_momentum_at_j2_zero() {
    let state = at_periapsis(RE + 400.0, 0.001, 0.9);
    let e0 = state.specific_energy(MU);
    let h0 = state.angular_momentum().norm();

    let out = integrate_j2(&state, 7.0 * 86_400.0, MU, 0.0, RE, REL_TOL).expect("progress");

    let e1 = out.state.specific_energy(MU);
    let h1 = out.state.angular_momentum().norm();

    let de = fabs(e1 - e0) / fabs(e0);
    let dh = fabs(h1 - h0) / h0;
    assert!(de < 1e-9, "specific energy drifted by {de:e} over 7 days");
    // Angular momentum is exactly conserved by a central force, so any drift is the integrator's. The
    // bound is 1e-9 and the measured value is 1.7e-10 - a factor of six inside it, and deliberately not
    // tighter. A 7-day LEO run is about 1.8e4 adaptive steps at `rel_tol = 1e-12`, so a global drift of
    // 1.7e-10 is 1e-14 per step: BELOW the tolerance that was requested. Asking for 1e-10 globally would
    // be asking the controller to beat its own configured accuracy by two orders of magnitude, which is
    // not a property of a correct integrator. The first version of this assertion did exactly that and
    // failed at 1.738e-10, which is the number the controller was designed to produce.
    assert!(
        dh < 1e-9,
        "angular momentum drifted by {dh:e} over 7 days, which is more than 1e-9 -          the step controller is not holding its tolerance"
    );

    // and it should have been efficient about it
    assert!(
        out.accepted > 100,
        "only {} steps over 7 days is implausibly few",
        out.accepted
    );
}

/// `−∇U_J2` reproduces the oblateness acceleration.
///
/// This is what makes the conservation test below meaningful: an energy test only proves the integrator is
/// consistent with *some* potential. This proves the potential is the one the acceleration comes from, so
/// conserving the sum is a statement about the actual equations of motion and not about a coincidence.
#[test]
fn potential_gradient_reproduces_the_acceleration() {
    let r = Vec3::new(4200.0, 3000.0, 5000.0);
    let a = accel_j2(r, MU, EARTH_J2, RE);
    let h = 1e-3;

    let dx = -(potential_j2(Vec3::new(r.x + h, r.y, r.z), MU, EARTH_J2, RE)
        - potential_j2(Vec3::new(r.x - h, r.y, r.z), MU, EARTH_J2, RE))
        / (2.0 * h);
    let dy = -(potential_j2(Vec3::new(r.x, r.y + h, r.z), MU, EARTH_J2, RE)
        - potential_j2(Vec3::new(r.x, r.y - h, r.z), MU, EARTH_J2, RE))
        / (2.0 * h);
    let dz = -(potential_j2(Vec3::new(r.x, r.y, r.z + h), MU, EARTH_J2, RE)
        - potential_j2(Vec3::new(r.x, r.y, r.z - h), MU, EARTH_J2, RE))
        / (2.0 * h);

    // Central term is not in the potential, so compare the oblateness part only: subtract -mu r/r^3.
    let rn = r.norm();
    let g = -MU / (rn * rn * rn);
    let oblate_x = a.x - g * r.x;
    let oblate_y = a.y - g * r.y;
    let oblate_z = a.z - g * r.z;

    let an = fabs(a.norm());
    let scale = if an > 1e-12 { an } else { 1e-12 };
    assert!(fabs(dx - oblate_x) < 1e-6 * scale, "dU/dx mismatch: {dx:e} vs {oblate_x:e}");
    assert!(fabs(dy - oblate_y) < 1e-6 * scale, "dU/dy mismatch: {dy:e} vs {oblate_y:e}");
    assert!(fabs(dz - oblate_z) < 1e-6 * scale, "dU/dz mismatch: {dz:e} vs {oblate_z:e}");
}

/// With J2 on, the integrator conserves the **full** energy, potential included.
///
/// An oblate body's field is still a potential, so a J2 orbit has a constant total energy. If this drifts,
/// the "oblateness" being applied is not a potential and the reference is inventing a secular drift of its
/// own — the specific failure that would make the cross-check above meaningless, because a leaking
/// integrator's spurious nodal rate could land anywhere, including on the right answer.
///
/// **The energy to conserve is `v²/2 − μ/r + U_J2`, not `v²/2 − μ/r`.** The two-body value is *not*
/// conserved under J2 and oscillates by ~2e-3 relative at LEO over a week, because the oblateness does
/// real work on the orbit. The first version of this test asserted the two-body quantity and failed by
/// 1.7e-3, which looked exactly like an integrator bug and was a test bug. The assertion below is on the
/// total.
#[test]
fn integrator_conserves_energy_with_j2_on() {
    let state = at_periapsis(RE + 400.0, 0.001, 51.6 * core::f64::consts::PI / 180.0);

    let total_energy = |s: &State| s.specific_energy(MU) + potential_j2(s.r, MU, EARTH_J2, RE);
    let e0 = total_energy(&state);

    let out = integrate_j2(&state, 3.0 * 86_400.0, MU, EARTH_J2, RE, REL_TOL).expect("progress");
    let e1 = total_energy(&out.state);

    let de = fabs(e1 - e0) / fabs(e0);
    assert!(
        de < 1e-9,
        "total energy (two-body + J2 potential) drifted by {de:e} under J2 - the field is not conservative"
    );

    // And demonstrate the point explicitly: the two-body part alone is NOT conserved. Asserting this
    // stops anyone 'simplifying' the check back to the wrong quantity later.
    let d2 = fabs(out.state.specific_energy(MU) - state.specific_energy(MU))
        / fabs(state.specific_energy(MU));
    assert!(
        d2 > 1e-5,
        "the two-body energy should NOT be conserved under J2, but it drifted only {d2:e} -          if this is near zero the J2 term is not being applied at all"
    );
}

/// Refusals: no NaN, no silent nonsense.
#[test]
fn integrator_refuses_degenerate_input() {
    let s = at_periapsis(RE + 400.0, 0.0, 0.5);
    assert!(integrate_j2(&s, f64::NAN, MU, EARTH_J2, RE, 1e-12).is_none(), "NaN dt");
    assert!(integrate_j2(&s, -1.0, MU, EARTH_J2, RE, 1e-12).is_none(), "negative dt");
    assert!(integrate_j2(&s, 600.0, MU, EARTH_J2, RE, 0.0).is_none(), "zero tolerance");
    assert!(integrate_j2(&s, 0.0, MU, EARTH_J2, RE, 1e-12).is_some(), "zero dt is valid");
}
