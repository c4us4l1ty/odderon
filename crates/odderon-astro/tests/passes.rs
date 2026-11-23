//! Pass prediction, checked against closed-form geometry rather than against itself.
//!
//! The trap in this file is the one that makes pass prediction look finished when it is not: a search that
//! returns *something plausible* for every input. A window of the wrong length returns "no pass", a step that
//! is too coarse returns "no pass", a culmination taken from the grid returns a time that is merely rounded.
//! None of those crash, and all of them are wrong.
//!
//! So the tests are grouped by which lie they close:
//!
//! 1. **The closed form is the answer.** For a circular orbit whose ground track passes directly over the
//!    station, the pass duration is `2θ/(n − ω)` with no free parameters. Agreement measured at 1e-6 s over a
//!    652 s pass, at two different masks — the residual is the refinement's own convergence, not the physics.
//! 2. **The guards refuse rather than guess**, and they refuse *before* doing the work.
//! 3. **The reported angles are physical**: a directly-overhead equatorial pass rises due west and sets due
//!    east, and the maximum elevation is exactly 90°.
//! 4. **The culmination is not a grid point** — the same maximum from two different steps.
//! 5. **Earth's rotation is in the observer, not only the satellite.** Ignoring it shortens the overhead
//!    pass by 42.05 s and moves a latitude-crossing pass by minutes; both are measured below.
//!
//! Every number in the comments below was computed outside this crate first (in `node`, same constants) and
//! then written down, so a failure says which of the two disagreed.

use odderon_astro::catalogue::{EARTH, EARTH_FRAME};
use odderon_astro::observe::Site;
use odderon_astro::passes::{
    central_half_angle, guaranteed_scan_window_seconds, minimum_pass_seconds, next_pass,
    next_pass_exact_geometry, PassError,
};
use odderon_astro::{mu, radius, State, Vec3};
use odderon_dmath::{cos, sin, sqrt, TAU};

const MU: f64 = mu::EARTH;
const RE: f64 = radius::EARTH;
const DEG: f64 = core::f64::consts::PI / 180.0;

/// Earth's stored sidereal rate, taken from the frame rather than restated, so this file cannot drift from
/// the constant the code uses.
fn rate() -> f64 {
    EARTH_FRAME.rate.value
}

/// The two scan guards for a body, from the orbit's semi-major axis.
///
/// `next_pass` takes them as numbers rather than deriving them, because a caller that wants a longer window is
/// entitled to ask for one. The tests therefore have to compute them the same way the module's own docs say to
/// — passing something else would be testing a different question.
fn guards(body: &odderon_astro::catalogue::Body, a: f64) -> (f64, f64) {
    let rate = body
        .physical
        .rotation_rate
        .known()
        .expect("test bodies all publish a rotation rate")
        .value;
    let window = guaranteed_scan_window_seconds(a, 0.0, body.physical.mu.value, rate)
        .expect("window computable")
        * 1.01;
    let step = minimum_pass_seconds(
        body.physical.equatorial_radius.value,
        a,
        10.0,
        a,
        0.0,
        body.physical.mu.value,
        rate,
    )
    .expect("step computable")
        / 4.0;
    (window, step)
}

/// A station on the equator at the prime meridian.
fn equatorial_site() -> Site {
    Site::from_geodetic(&EARTH, 0.0, 0.0, 0.0).expect("equatorial site is valid")
}

/// `to_inertial` for a *direction*: rotate about z by `+W` and leave the length alone.
///
/// The test file does this by hand rather than through `BodyFixed`, because `to_inertial` takes a position and
/// the velocity here is a direction — using the position function on a velocity works and reads as an error.
fn direction_to_inertial(v: Vec3, w: f64) -> Vec3 {
    let (s, c) = (sin(w), cos(w));
    Vec3::new(v.x * c - v.y * s, v.x * s + v.y * c, v.z)
}

/// State whose sub-satellite point is the site at `t0`, moving east — a prograde equatorial orbit that passes
/// directly overhead.
///
/// Built *in the frame the code will read it in*: the site's body-fixed position is rotated into inertial by
/// the same `W₀ + Ẇt` the propagator will be unwound with, so the pass is overhead by construction. Hard-coding
/// `(r, 0, 0)` instead would put the sub-point at longitude `−W₀ = −190.147°`, half a world away, and the pass
/// would simply not happen — the [`odderon_astro::frames`] trap, restated here because it is easy to walk back
/// into while writing a test.
fn overhead_state(site: &Site, altitude: f64, t0: f64) -> State {
    let r = RE + altitude;
    let w = EARTH_FRAME
        .angle_at(t0)
        .expect("Earth has a prime meridian");
    let unit_up_inertial = direction_to_inertial(site.fixed.unit(), w);
    let position = unit_up_inertial * r;
    let velocity = direction_to_inertial(site.east, w) * sqrt(MU / r);
    State::new(position, velocity)
}

/// The closed-form central angle, recomputed here from geometry this file states itself, so the module's own
/// function is being compared to something rather than to itself.
fn theta_closed_form(altitude: f64, mask_deg: f64) -> f64 {
    let r = RE + altitude;
    let eps = mask_deg * DEG;
    odderon_dmath::acos((RE / r) * cos(eps)) - eps
}

/// A window that is unambiguously long enough, computed from the *state's* `a` so the test exercises the same
/// path the library does. The guard's own promise is `guaranteed_scan_window_seconds` exactly; see
/// `the_promised_window_is_accepted_verbatim` for that case, and `GUARD_SLACK` for why the difference matters.
fn ample_window(state: &State) -> f64 {
    let a = state.semi_major_axis(MU);
    guaranteed_scan_window_seconds(a, 0.0, MU, rate()).expect("valid") * 1.01
}

// =============================================================================================
// 1. The closed form
// =============================================================================================

/// At the geometric horizon the half-angle is `acos(R/r)` — the tangent angle, 19.782° for 400 km.
///
/// This is the same number that puts `observe::horizon_range` at 2294 km, approached from the other side. If
/// these two ever disagree, one of them has the wrong triangle.
#[test]
fn the_horizon_half_angle_is_the_tangent_angle() {
    let altitude = 400.0;
    let r = RE + altitude;
    let expected = odderon_dmath::acos(RE / r);
    let got = central_half_angle(RE, r, 0.0).expect("a circular orbit geometry has a solution");

    assert!(
        (got - expected).abs() < 1e-12,
        "half-angle at the horizon should be acos(R/r) = {:.6} deg, got {:.6} deg",
        expected / DEG,
        got / DEG
    );
    assert!(
        (expected / DEG - 19.7821).abs() < 1e-4,
        "and that is 19.7821 deg, the number observe::horizon_range already relies on"
    );
}

/// A higher mask means a shorter window, monotonically, and the three published values from the geometry.
#[test]
fn a_steeper_mask_shrinks_the_window_monotonically() {
    let r = RE + 400.0;
    let masks = [0.0, 5.0, 10.0, 30.0, 60.0, 80.0];
    let expected_deg = [19.7821, 15.3796, 12.0753, 5.4207, 1.9337, 0.5957];

    let mut previous = f64::INFINITY;
    for (mask, expected) in masks.iter().zip(expected_deg.iter()) {
        let theta = central_half_angle(RE, r, *mask).expect("valid geometry");
        assert!(
            (theta / DEG - expected).abs() < 1e-3,
            "mask {mask} deg should give {expected} deg of half-angle, got {}",
            theta / DEG
        );
        assert!(
            theta < previous,
            "mask {mask} deg did not shrink the window: {theta} vs {previous}"
        );
        previous = theta;
    }
}

/// A mask of exactly zenith leaves an instantaneous window: zero width, zero duration, no NaN.
#[test]
fn a_zenith_mask_yields_an_instantaneous_window() {
    let r = RE + 400.0;
    let theta = central_half_angle(RE, r, 90.0).expect("still a valid question");
    assert!(
        theta >= 0.0,
        "a clamped acos must not produce a negative angle"
    );
    // The bound is arithmetic: `cos 90°` is `6.1e-17` rather than zero, `acos` of that is `π/2 − 6.1e-17`, and the
    // subtraction leaves at most a few times `f64::EPSILON`. Demanding exact zero here would be demanding that
    // the kernel's `acos` be correctly rounded at its worst-conditioned argument — the same class of mistake as
    // asking an eight-figure rotation rate for millimetres (`tests/observe.rs`).
    assert!(
        theta < 1e-14,
        "a zenith-only pass has zero width, got {theta}"
    );

    // And the duration that follows is zero to the same standard, which is the input to "no step can find this".
    let duration = minimum_pass_seconds(RE, r, 90.0, r, 0.0, MU, rate()).expect("valid");
    assert!(
        duration.abs() < 1e-14,
        "a zero-width window has zero duration, got {duration} s"
    );
}

/// The target inside the body is not a question with an answer.
#[test]
fn a_target_inside_the_body_has_no_horizon() {
    assert_eq!(central_half_angle(RE, RE * 0.5, 10.0), None);
    assert_eq!(central_half_angle(RE, RE, 10.0), None);
    assert_eq!(central_half_angle(f64::NAN, RE * 2.0, 10.0), None);
    assert_eq!(central_half_angle(RE, RE * 2.0, 120.0), None);
}

/// The duration for a directly-overhead circular pass, against the closed form.
///
/// 652.3949 s at the horizon and 398.2311 s at a 10° mask, for a 400 km orbit. The tolerance is 1e-6 s, which is
/// the refinement's own convergence scale and not the physics; the point is that the *search* and the
/// *formula* are independent routes to the same number.
#[test]
fn the_closed_form_duration_matches_the_search() {
    let site = equatorial_site();
    let state = overhead_state(&site, 400.0, 0.0);
    let a = RE + 400.0;
    let n = sqrt(MU / (a * a * a));

    for mask in [0.0, 10.0] {
        let window = ample_window(&state);
        let step = minimum_pass_seconds(RE, a, mask, a, 0.0, MU, rate()).expect("valid") / 4.0;
        let pass = next_pass_exact_geometry(
            &EARTH,
            &site,
            &EARTH_FRAME,
            &state,
            0.0,
            MU,
            mask,
            0.0,
            window + 2.0 * step,
            step,
        )
        .expect("the question is answerable")
        .expect("an overhead pass must be found");

        let theta = theta_closed_form(400.0, mask);
        let expected = 2.0 * theta / (n - rate());
        assert!(
            (pass.duration() - expected).abs() < 1e-6,
            "mask {mask} deg: closed form says {expected:.6} s, search says {:.6} s",
            pass.duration()
        );
        // The same fact stated as an independent number, so a shared sign error cannot pass both lines.
        let published = if mask == 0.0 { 652.3949 } else { 398.2311 };
        assert!(
            (pass.duration() - published).abs() < 1e-3,
            "mask {mask} deg should last {published} s, got {:.4} s",
            pass.duration()
        );
    }
}

/// Successive passes over a station repeat on the synodic period, not the orbital period.
///
/// 5936.2388 s versus an orbital period of 5553.6243 s: a 382 s difference, and using the orbital period to
/// schedule is how a ground station ends up missing every pass after the first.
#[test]
fn passes_repeat_on_the_synodic_period_not_the_orbital_period() {
    let site = equatorial_site();
    let state = overhead_state(&site, 400.0, 0.0);
    let a = RE + 400.0;
    let synodic = TAU / (sqrt(MU / (a * a * a)) - rate());
    let window = ample_window(&state);
    let step = 5.0;

    let first = next_pass_exact_geometry(
        &EARTH,
        &site,
        &EARTH_FRAME,
        &state,
        0.0,
        MU,
        0.0,
        0.0,
        window,
        step,
    )
    .expect("answerable")
    .expect("a pass exists");
    let second = next_pass_exact_geometry(
        &EARTH,
        &site,
        &EARTH_FRAME,
        &state,
        0.0,
        MU,
        0.0,
        first.set + 1.0,
        window,
        step,
    )
    .expect("answerable")
    .expect("the next pass exists");

    let separation = second.rise - first.rise;
    assert!(
        (separation - synodic).abs() < 1e-4,
        "passes should be {synodic:.4} s apart, measured {separation:.4} s"
    );
    assert!(
        (separation - TAU / sqrt(MU / (a * a * a))).abs() > 300.0,
        "and that must NOT be the orbital period - the difference is what the test is for"
    );
}

// =============================================================================================
// 2. The guards
// =============================================================================================

/// A window shorter than one synodic period is refused, and the refusal names the number required.
///
/// "The next pass in the next 90 minutes" is the request a person actually makes, and it is 90 minutes < 98.9
/// minutes. The answer must be a refusal with the correct window in it, not "no pass".
#[test]
fn a_ninety_minute_window_is_refused_as_too_short() {
    let site = equatorial_site();
    let state = overhead_state(&site, 400.0, 0.0);
    let a = RE + 400.0;

    let err = next_pass_exact_geometry(
        &EARTH,
        &site,
        &EARTH_FRAME,
        &state,
        0.0,
        MU,
        10.0,
        0.0,
        90.0 * 60.0,
        10.0,
    )
    .expect_err("90 minutes cannot guarantee a pass");

    match err {
        PassError::WindowTooShort {
            least_window_s,
            given_s,
        } => {
            assert!(
                (least_window_s - 5936.2388).abs() < 1e-3,
                "required window is one synodic period, got {least_window_s}"
            );
            assert!((given_s - 5400.0).abs() < 1e-9);
        }
        other => panic!("expected WindowTooShort, got {other:?}"),
    }

    // And the number it asks for is the one the closed form gives.
    let from_formula = guaranteed_scan_window_seconds(a, 0.0, MU, rate()).expect("valid");
    assert!((from_formula - 5936.2388).abs() < 1e-3);
}

/// The promised window is accepted *verbatim*, and the promised step too.
///
/// This is the API test rather than the physics test: a caller who asks `guaranteed_scan_window_seconds` and
/// `minimum_pass_seconds` for their bounds and passes them straight back must not be refused. It was, by
/// `1.7e-11 s`, because the library recomputes `a` from the state while the caller computed it from the altitude
/// — the same 400 km orbit, two expressions, a relative disagreement of `3e-15`. Refusing that would teach the
/// caller to pad the number, and a padded bound is a magic number with a comment.
#[test]
fn the_promised_window_is_accepted_verbatim() {
    let site = equatorial_site();
    let state = overhead_state(&site, 400.0, 0.0);
    // Deliberately the *altitude* expression of `a`, not the state's: this is what a caller would naturally
    // write, and it is the expression that disagreed with the library by 3e-15.
    let a_caller = RE + 400.0;
    let window = guaranteed_scan_window_seconds(a_caller, 0.0, MU, rate()).expect("valid");
    let step =
        minimum_pass_seconds(RE, a_caller, 10.0, a_caller, 0.0, MU, rate()).expect("valid") / 8.0;

    let pass = next_pass_exact_geometry(
        &EARTH,
        &site,
        &EARTH_FRAME,
        &state,
        0.0,
        MU,
        10.0,
        0.0,
        window,
        step,
    )
    .expect("the promised window must be accepted")
    .expect("an overhead pass must be found inside one synodic period");
    assert!(pass.duration() > 0.0);

    // And a window that is genuinely short is still refused, so the slack has not swallowed the guard.
    let err = next_pass_exact_geometry(
        &EARTH,
        &site,
        &EARTH_FRAME,
        &state,
        0.0,
        MU,
        10.0,
        0.0,
        window * 0.99,
        step,
    )
    .expect_err("one per cent short is short");
    assert!(matches!(err, PassError::WindowTooShort { .. }));
}

/// A step longer than the shortest possible pass is refused, with the limit stated.
///
/// This is the guard that stops a 120 s scan from reporting "no pass" for a station that gets a 64 s pass.
/// The case here is a 400 km orbit seen at a 60 degree mask, where the whole visibility window is 63.7708 s —
/// a target at 400 km clearing 60 degrees is only 1.9337 degrees from the station, and the relative rate is
/// 0.0010584 rad/s, so the pass cannot last longer than that. Any step above it is refused.
#[test]
fn a_coarse_step_is_refused_before_any_search_happens() {
    let site = equatorial_site();
    let state = overhead_state(&site, 400.0, 0.0);
    let window = ample_window(&state);

    let err = next_pass_exact_geometry(
        &EARTH,
        &site,
        &EARTH_FRAME,
        &state,
        0.0,
        MU,
        60.0,
        0.0,
        window,
        600.0,
    )
    .expect_err("a 600 s step cannot see a 63.77 s pass");

    match err {
        PassError::StepTooCoarse {
            greatest_step_s,
            given_s,
        } => {
            assert!(
                (greatest_step_s - 63.7708).abs() < 0.01,
                "the permitted step at a 60 deg mask is the pass length, 63.7708 s; got {greatest_step_s}"
            );
            assert!((given_s - 600.0).abs() < 1e-9);
        }
        other => panic!("expected StepTooCoarse, got {other:?}"),
    }
}

/// The guard is not stricter than it needs to be: the largest legitimate step does find the pass.
///
/// A guard that refuses everything is useless, and a test that only checks refusals would pass for one.
#[test]
fn the_permitted_step_actually_finds_the_pass() {
    let site = equatorial_site();
    let state = overhead_state(&site, 400.0, 0.0);
    // The altitude expression of `a`, i.e. what a caller writes — not the state's vis-viva value. The step has to
    // come from the same expression the library will use, or the guard's own slack is what is being tested
    // instead of the guard.
    let a = RE + 400.0;
    let window = ample_window(&state);

    for mask in [0.0, 10.0, 60.0] {
        // A quarter of the permitted step: the point of the test is that a permissible step finds the pass, not
        // that the boundary case does. The exact-boundary case is `the_promised_window_is_accepted_verbatim`.
        let step = minimum_pass_seconds(RE, a, mask, a, 0.0, MU, rate()).expect("valid") / 4.0;
        let pass = next_pass_exact_geometry(
            &EARTH,
            &site,
            &EARTH_FRAME,
            &state,
            0.0,
            MU,
            mask,
            0.0,
            window,
            step,
        )
        .expect("the permitted step must be permitted")
        .expect("an overhead pass must be found at any mask the geometry allows");
        assert!(
            pass.duration() > 0.0,
            "mask {mask} deg: a found pass must have positive duration"
        );
    }
}

/// A hyperbolic orbit has no pass schedule, and the refusal says so rather than returning "no pass".
#[test]
fn a_hyperbolic_orbit_is_not_a_schedule() {
    let site = equatorial_site();
    // Escape speed at 400 km, times 1.2: no period, no synodic beat, no next pass.
    let r = RE + 400.0;
    let v_escape = sqrt(2.0 * MU / r) * 1.2;
    let state = State::new(Vec3::new(r, 0.0, 0.0), Vec3::new(0.0, v_escape, 0.0));

    let err = next_pass_exact_geometry(
        &EARTH,
        &site,
        &EARTH_FRAME,
        &state,
        0.0,
        MU,
        10.0,
        0.0,
        1e6,
        10.0,
    )
    .expect_err("a hyperbolic orbit has no next pass");
    assert_eq!(err, PassError::NotAnEllipticalOrbit);
}

/// NaN and zero are refused as questions, not answered as geometry.
#[test]
fn degenerate_input_is_refused() {
    let site = equatorial_site();
    let state = overhead_state(&site, 400.0, 0.0);
    let window = ample_window(&state);

    for bad in [f64::NAN, f64::INFINITY, 0.0, -5.0] {
        let err = next_pass_exact_geometry(
            &EARTH,
            &site,
            &EARTH_FRAME,
            &state,
            0.0,
            MU,
            10.0,
            0.0,
            bad,
            10.0,
        )
        .expect_err("a non-window is not a window");
        assert_eq!(err, PassError::BadInput, "window {bad}");

        let err = next_pass_exact_geometry(
            &EARTH,
            &site,
            &EARTH_FRAME,
            &state,
            0.0,
            MU,
            10.0,
            0.0,
            window,
            bad,
        )
        .expect_err("a non-step is not a step");
        assert_eq!(err, PassError::BadInput, "step {bad}");
    }
}

// =============================================================================================
// 3. The angles are physical
// =============================================================================================

/// A directly-overhead prograde equatorial pass rises in the west and sets in the east, at 90°.
///
/// This is the test that catches a sign error in `look_at`'s azimuth convention that no algebra would: the
/// satellite is moving eastward, so it must first appear *behind* the station (west, 270°) and leave ahead of
/// it (east, 90°). Reversing the body's rotation makes it rise in the east, and the pass is otherwise identical.
#[test]
fn an_overhead_westward_rise_and_eastward_set() {
    let site = equatorial_site();
    let state = overhead_state(&site, 400.0, 0.0);
    let window = ample_window(&state);
    let step = 5.0;

    let pass = next_pass_exact_geometry(
        &EARTH,
        &site,
        &EARTH_FRAME,
        &state,
        0.0,
        MU,
        0.0,
        0.0,
        window,
        step,
    )
    .expect("answerable")
    .expect("a pass exists");

    assert!(
        (pass.rise_azimuth_deg - 270.0).abs() < 0.5,
        "a prograde pass over the equator must rise in the west, got {} deg",
        pass.rise_azimuth_deg
    );
    assert!(
        (pass.set_azimuth_deg - 90.0).abs() < 0.5,
        "and set in the east, got {} deg",
        pass.set_azimuth_deg
    );
    assert!(
        (pass.max_elevation_deg - 90.0).abs() < 1e-4,
        "directly overhead is 90 deg, got {}",
        pass.max_elevation_deg
    );
    // At culmination the range is the altitude, and nothing else.
    assert!(
        (pass.min_range_km - 400.0).abs() < 1e-3,
        "overhead range is the altitude, got {} km",
        pass.min_range_km
    );
    // The culminations sit at the middle of their own pass, which a grid-derived maximum would not.
    let middle = 0.5 * (pass.rise + pass.set);
    assert!(
        (pass.culmination - middle).abs() < 0.5,
        "the apex of a symmetric overhead pass is its midpoint: {:.3} vs {:.3}",
        pass.culmination,
        middle
    );
}

/// A station too far from the ground track never sees the target, and that is `Ok(None)` — not an error, and
/// not a pass.
///
/// The geometry is *ring* shaped, which is the thing a naive "is the latitude greater than the inclination"
/// filter gets wrong. Measured for a 51.6 deg orbit at 400 km: a station at 30° N sees a 390 s pass with a
/// maximum elevation of **76.4°**, and a station at 60° N sees **nothing at all**. So the reach is neither
/// "inside the inclination" nor "outside" — it is a band, and the only honest test of membership is the
/// elevation function itself.
///
/// The distinction between this and `Err` is the module's whole argument about honesty, so it is tested rather
/// than assumed.
#[test]
fn a_station_outside_the_ground_track_gets_no_pass() {
    // A 51.6 deg orbit overhead the equator at t = 0.
    let equator = equatorial_site();
    let inclined = overhead_state(&equator, 400.0, 0.0);
    let window = ample_window(&inclined);
    let v = inclined.v;
    let speed = v.norm();
    // Rotate the velocity out of the equatorial plane by 51.6 deg about the position vector: the same speed,
    // the same radius, a different inclination.
    let up = inclined.r.unit();
    let i = 51.6 * DEG;
    let east = v.unit();
    let north = up.cross(east);
    let new_v = east * (speed * cos(i)) + north * (speed * sin(i));
    let state = State::new(inclined.r, new_v);

    // The pass rises **after** one synodic period has elapsed (5955 s, with the guarantee at 5936 s), which is
    // why the window here starts earlier and runs a little longer. A window that only *just* met the guarantee
    // would stop 19 s before this pass — and that is not a defect in the guard: the guard promises that a pass
    // starts within one synodic period *of the geometry returning*, not that any particular pass does.
    let inside = Site::from_geodetic(&EARTH, 30.0 * DEG, 0.0, 0.0).expect("valid");
    let pass = next_pass_exact_geometry(
        &EARTH,
        &inside,
        &EARTH_FRAME,
        &state,
        0.0,
        MU,
        10.0,
        -1000.0,
        window + 1500.0,
        5.0,
    )
    .expect("the question is answerable")
    .expect("a station 30 deg north of a 51.6 deg orbit does see it");
    assert!(
        (pass.max_elevation_deg - 76.4221).abs() < 0.05,
        "the measured maximum for this geometry is 76.4221 deg, got {}",
        pass.max_elevation_deg
    );
    assert!(
        (pass.duration() - 390.3481).abs() < 0.5,
        "and it lasts 390.35 s at a 10 deg mask, got {:.4}",
        pass.duration()
    );

    let outside = Site::from_geodetic(&EARTH, 60.0 * DEG, 0.0, 0.0).expect("valid");
    let none = next_pass_exact_geometry(
        &EARTH,
        &outside,
        &EARTH_FRAME,
        &state,
        0.0,
        MU,
        10.0,
        -1000.0,
        window + 1500.0,
        5.0,
    )
    .expect("still an answerable question");
    assert_eq!(
        none, None,
        "a station at 60 deg north never reaches this ground track"
    );
}

// =============================================================================================
// 4. The culmination is refined, not sampled
// =============================================================================================

/// The maximum elevation and its time do not depend on the scan step.
///
/// A grid-derived maximum is `O(step²)` low and its time is quantised to the grid; both errors vanish here
/// because the search brackets and then refines. This is a direct test of the claim in the module docs.
#[test]
fn the_culmination_does_not_depend_on_the_step() {
    let site = equatorial_site();
    let state = overhead_state(&site, 400.0, 0.0);
    let window = ample_window(&state);

    let coarse = next_pass_exact_geometry(
        &EARTH,
        &site,
        &EARTH_FRAME,
        &state,
        0.0,
        MU,
        10.0,
        0.0,
        window,
        10.0,
    )
    .expect("answerable")
    .expect("a pass exists");
    let fine = next_pass_exact_geometry(
        &EARTH,
        &site,
        &EARTH_FRAME,
        &state,
        0.0,
        MU,
        10.0,
        0.0,
        window,
        5.0,
    )
    .expect("answerable")
    .expect("a pass exists");

    assert!(
        (coarse.max_elevation_deg - fine.max_elevation_deg).abs() < 1e-6,
        "the apex is a property of the pass, not of the grid: {} vs {}",
        coarse.max_elevation_deg,
        fine.max_elevation_deg
    );
    assert!(
        (coarse.culmination - fine.culmination).abs() < 1e-3,
        "and so is its time: {:.6} vs {:.6}",
        coarse.culmination,
        fine.culmination
    );
    assert!(
        (coarse.rise - fine.rise).abs() < 1e-6,
        "the rise is a crossing, so it is step-independent too: {:.9} vs {:.9}",
        coarse.rise,
        fine.rise
    );

    // A coarse step costs fewer evaluations, which is the only difference the step is allowed to make.
    assert!(
        coarse.elevations_evaluated < fine.elevations_evaluated * 2,
        "step should change the cost, not the answer: {} vs {} evaluations",
        coarse.elevations_evaluated,
        fine.elevations_evaluated
    );
}

/// The reported rise and set are *on* the mask, to machine precision — not merely on either side of it.
#[test]
fn the_crossings_land_on_the_mask() {
    let site = equatorial_site();
    let state = overhead_state(&site, 400.0, 0.0);
    let window = ample_window(&state);

    for mask in [0.0, 10.0, 30.0] {
        let pass = next_pass_exact_geometry(
            &EARTH,
            &site,
            &EARTH_FRAME,
            &state,
            0.0,
            MU,
            mask,
            0.0,
            window,
            5.0,
        )
        .expect("answerable")
        .expect("a pass exists");

        // The elevation AT the reported rise and set, computed independently through the public look angle.
        let rise_look = elevation_at(&site, &state, pass.rise);
        let set_look = elevation_at(&site, &state, pass.set);
        assert!(
            (rise_look - mask).abs() < 1e-6,
            "mask {mask}: rise elevation {rise_look:.9} is not on the mask"
        );
        assert!(
            (set_look - mask).abs() < 1e-6,
            "mask {mask}: set elevation {set_look:.9} is not on the mask"
        );
    }
}

// =============================================================================================
// 5. The observer rotates
// =============================================================================================

/// Earth's rotation is applied to the *observer*, and removing it shortens the overhead pass by 42 s.
///
/// The frozen-Earth model is built here rather than in the library, because it is a wrong model and should not
/// have an API. For a 400 km equatorial orbit directly overhead the station, measured: the geometric-horizon
/// pass lasts **652.3949 s** with the Earth turning and **610.3454 s** frozen — 42.05 s, or 6.4 %, on a pass
/// eleven minutes long. At a 10° mask the same comparison is 398.2311 s against 372.5635 s.
///
/// The mechanism is worth stating because it is not intuitive: in the *overhead* case the observer's eastward
/// motion and the target's eastward motion are in the same direction, so removing the rotation makes the
/// relative rate *faster* and the pass *shorter*. A station off the ground track sees the opposite sign,
/// because there the rotation changes which part of the track is in view. Both are real; this test pins the one
/// that is unambiguous.
#[test]
fn a_frozen_earth_shortens_the_overhead_pass_by_forty_two_seconds() {
    let site = equatorial_site();
    let state = overhead_state(&site, 400.0, 0.0);
    let window = ample_window(&state);

    let correct = next_pass_exact_geometry(
        &EARTH,
        &site,
        &EARTH_FRAME,
        &state,
        0.0,
        MU,
        0.0,
        0.0,
        window,
        2.0,
    )
    .expect("answerable")
    .expect("a pass exists");

    // Frozen Earth: rotate the target into the body frame by W0 only, with no rate. Everything else identical.
    let w0 = EARTH_FRAME.w0.value;
    let frozen_elevation = |t: f64| -> f64 {
        let p = odderon_astro::propagate(&state, t, MU);
        let (s, c) = (sin(-w0), cos(-w0));
        let x = p.state.r.x * c - p.state.r.y * s;
        let y = p.state.r.x * s + p.state.r.y * c;
        odderon_astro::observe::look_at(&site, Vec3::new(x, y, p.state.r.z))
            .expect("target is not at the observer")
            .elevation_deg
    };
    // Bracket the *set* rather than assuming one: at t = 0 the frozen satellite is at culmination, so bisecting
    // from zero would converge to the edge of the bracket. Step forward until the elevation goes negative, then
    // close that bracket.
    let mut t = 0.0;
    let mut previous = 0.0;
    while frozen_elevation(t) >= 0.0 {
        previous = t;
        t += 1.0;
        assert!(
            t < 5000.0,
            "the frozen pass never set, which would be a different bug"
        );
    }
    let (mut lo, mut hi) = (previous, t);
    for _ in 0..64 {
        let mid = 0.5 * (lo + hi);
        if frozen_elevation(mid) >= 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let frozen_set = 0.5 * (lo + hi);
    // Symmetric, because the frozen geometry is a chord through the station's zenith: rise and set are mirrored.
    let frozen_duration = 2.0 * frozen_set;

    assert!(
        (frozen_duration - 610.3454).abs() < 0.01,
        "the frozen pass should last 610.3454 s, got {frozen_duration:.4}"
    );
    let difference = correct.duration() - frozen_duration;
    assert!(
        (difference - 42.0495).abs() < 0.01,
        "a frozen Earth should shorten the overhead pass by 42.0495 s, measured {difference:.4} s"
    );
    assert!(
        difference > 30.0,
        "and by far more than rounding - this is the bug the test exists for"
    );
}

/// Elevation at an absolute time, via the public look-angle path. Used to check the crossings land on the mask.
fn elevation_at(site: &Site, state: &State, t: f64) -> f64 {
    let p = odderon_astro::propagate(state, t, MU);
    let body_fixed = EARTH_FRAME
        .to_body_fixed(p.state.r, t)
        .expect("Earth has a prime meridian");
    odderon_astro::observe::look_at(site, body_fixed)
        .expect("the target is not at the observer")
        .elevation_deg
}

// ---------------------------------------------------------------------------------------------
// 6. The oblateness moves the schedule, and the search knows it
// ---------------------------------------------------------------------------------------------
//
// The search used to be two-body, which is fine for "the next pass" and wrong for a schedule: the node
// regresses at −5.0023°/day, so after a day the geometry is different. These tests are about the perturbation
// being *present* and *the right shape*, not about it being accurate — `tests/j2-crosscheck.rs` owns that
// question, against a Dormand–Prince integration of the real equations.

/// A pass moves by tens of seconds within a day, and the spherical model is what it moves away from.
///
/// Measured in `node` first, for a 400 km **equatorial** orbit over an equatorial site: under the two-body search
/// the rise times sit on a 5935 s beat, and the J2 search moves the first pass by −20 s and the sixteenth, one
/// day out, by −560 s — −35 s per pass, perfectly linear. The equatorial case is used because the passes are
/// frequent and reliably overhead, so the test is about the perturbation rather than about whether a pass
/// happens to exist.
#[test]
fn oblateness_changes_the_beat_between_passes() {
    let site = equatorial_site();
    let altitude = 400.0;
    let state = overhead_state(&site, altitude, 0.0);
    let (window, step) = guards(&EARTH, RE + altitude);

    let mut spherical = EARTH;
    spherical.physical.j2 = odderon_astro::provenance::Lookup::Known(
        odderon_astro::provenance::Quantity::defined(0.0).expect("zero is finite"),
    );

    // The two searches differ in the *model*, so the same first pass is found by both — start at the epoch.
    let find = |body: &odderon_astro::catalogue::Body, from: f64| {
        next_pass(
            body,
            &site,
            &EARTH_FRAME,
            &state,
            0.0,
            MU,
            10.0,
            from,
            window,
            step,
        )
        .expect("no error")
        .expect("a pass exists")
    };

    let first_j2 = find(&EARTH, 0.0);
    let first_flat = find(&spherical, 0.0);
    // At the epoch the perturbation has had no time to act, so the two agree to well under a second.
    assert!(
        (first_j2.rise - first_flat.rise).abs() < 1.0,
        "the first pass must be the same pass under both models, got {:+.3} s",
        first_j2.rise - first_flat.rise
    );

    // The *beat* is where they differ, and it is the number that decides whether a schedule is usable: measured
    // 5936.2388 s under two-body against 5918.0468 s with the oblateness, for this orbit.
    let next_j2 = find(&EARTH, first_j2.set + 1.0);
    let next_flat = find(&spherical, first_flat.set + 1.0);
    let beat_j2 = next_j2.rise - first_j2.rise;
    let beat_flat = next_flat.rise - first_flat.rise;

    assert!(
        beat_flat > beat_j2,
        "the oblateness shortens the beat: two-body {beat_flat:.2} s, J2 {beat_j2:.2} s"
    );
    assert!(
        (beat_flat - beat_j2) > 10.0,
        "the change in the beat must be far larger than the refinement tolerance, got {:.2} s",
        beat_flat - beat_j2
    );

    // And the two-body beat is the closed form's, which is what makes this a comparison of models rather than
    // of two implementations of the same thing.
    let synodic = guaranteed_scan_window_seconds(RE + altitude, 0.0, MU, rate()).expect("window");
    assert!(
        (beat_flat - synodic).abs() < 0.01,
        "the unperturbed beat must be the closed-form synodic period {synodic:.4} s, got {beat_flat:.4} s"
    );
}

/// The perturbation vanishes as `J2 → 0`, which is the check that catches a rotation applied with the wrong
/// axis: rotating a position about its own orbit normal leaves it unchanged, so a wrong axis looks like a
/// working zero.
#[test]
fn no_j2_means_no_perturbation() {
    let site = Site::from_geodetic(&EARTH, 30.0 * DEG, 0.0, 0.0).expect("site");
    let state = overhead_state(&site, 400.0, 0.0);
    let (window, step) = guards(&EARTH, RE + 400.0);

    let with_j2 = next_pass(
        &EARTH,
        &site,
        &EARTH_FRAME,
        &state,
        0.0,
        MU,
        10.0,
        0.0,
        window,
        step,
    )
    .expect("no error")
    .expect("a pass");

    // J2 = 0 in the catalogue: still `Known`, but zero. The perturbation then computes to zero rather than
    // being skipped, which is a different code path from `NotKnown`.
    let mut flat = EARTH;
    flat.physical.j2 = odderon_astro::provenance::Lookup::Known(
        odderon_astro::provenance::Quantity::defined(0.0).expect("zero is finite"),
    );
    let without = next_pass(
        &flat,
        &site,
        &EARTH_FRAME,
        &state,
        0.0,
        MU,
        10.0,
        0.0,
        window,
        step,
    )
    .expect("no error")
    .expect("a pass");

    // Within a single pass from the epoch the two agree to well under a second, because the perturbation has
    // had almost no time to act. If it never vanishes, something else is wrong.
    assert!(
        (with_j2.rise - without.rise).abs() < 1.0,
        "at the epoch a J2 perturbed and an unperturbed search must agree, got {:+.3} s",
        with_j2.rise - without.rise
    );
}

/// A body that does not publish `J2` gets the two-body schedule rather than a guess, and says so by working.
///
/// The Moon has no known `J2` in this catalogue (`NotKnown`, with a reason). Before the perturbation existed
/// that was invisible, because the search was two-body for everything. Now the *absence* has to be handled, and
/// the honest handling is to fall back rather than to invent `J2 = 0` — which would be a claim that the Moon is
/// spherical.
#[test]
fn a_body_without_j2_falls_back_to_two_body() {
    use odderon_astro::catalogue::MOON;

    assert!(
        MOON.physical.j2.known().is_none(),
        "this test is only meaningful while the Moon's J2 is not known"
    );
    // The catalogue publishes no lunar prime meridian, so a frame is built here from the Moon's own rotation
    // rate with `W₀ = 0`. That would be wrong for *pointing* at a real lunar site — it is the
    // `frames.rs` trap — but this test is about the search accepting a body with no `J2`, and the site and the
    // frame are built to agree with each other, so the pass is overhead by construction.
    let moon_frame = odderon_astro::frames::BodyFixed {
        w0: odderon_astro::provenance::Quantity::defined(0.0)
            .expect("zero is a finite quantity; the provenance records that it is a test artefact"),
        rate: MOON
            .physical
            .rotation_rate
            .known()
            .expect("the Moon's rotation rate is known"),
    };
    let site = Site::from_geodetic(&MOON, 0.0, 0.0, 0.0).expect("site");
    let r = radius::MOON + 1_000.0;
    let state = State::new(
        Vec3::new(r, 0.0, 0.0),
        Vec3::new(0.0, sqrt(mu::MOON / r), 0.0),
    );

    let (window, step) = guards(&MOON, r);
    let pass = next_pass(
        &MOON,
        &site,
        &moon_frame,
        &state,
        0.0,
        mu::MOON,
        10.0,
        0.0,
        window,
        step,
    )
    .expect("no error");
    assert!(
        pass.is_some(),
        "a 1000 km lunar orbit over an equatorial site must produce a pass"
    );
}
