//! Pass prediction: *when* will the observer see it, and *for how long?*
//!
//! ## Why this is the rung after look angles
//!
//! [`crate::observe::look_at`] answers "is it visible **now**". A pass prediction is a **forecast**, and a
//! forecast is the first thing in this project that can be wrong in a way the player notices days later. A
//! ground station told that a pass starts at 14:31:20 that hears nothing at 14:31:20 does not wonder about
//! the ellipsoid; it concludes the tool is broken. So this module is built around the ways a search
//! *returns a confident wrong answer* — the failure mode the previous attempt in this project died of
//! (`BUILD-PLAN.md` §0, Fact 2).
//!
//! ## The four ways a pass search lies
//!
//! **1. The scan steps over the pass.** A LEO pass can be 44 s long; a scan with a 120 s step is the
//! obvious thing to write and it reports *no pass* for a site that gets one. Measured: an equatorial
//! observer at 19.75° latitude under a 400 km equatorial orbit has a **64.5 s** pass, and a 120 s scan
//! returns "no pass in window" — no error, no warning, nothing to investigate. So [`next_pass`] *refuses* a
//! step it cannot justify ([`PassError::StepTooCoarse`]) instead of trusting the caller, who cannot know
//! the pass duration because that is the thing they are asking for.
//!
//! **2. The window is too short to contain a pass.** "Next pass in the next 90 minutes" sounds safe — LEO
//! periods *are* 90 minutes. It is not: the pass repeats on the **synodic** period, the beat between the
//! orbit's angular rate `n` and the body's rotation rate `ω`, and at 400 km that is `2π/(n − ω) = 5936 s`,
//! **98.9 minutes**. A 90-minute window can contain no pass at all, as a matter of geometry.
//!
//! **3. The culmination is taken from the sample grid.** Elevation is a smooth hump whose maximum is almost
//! never on a sample. Taking `max` over samples reports a maximum that is *low*, and reports its time as the
//! time of a sample — tens of seconds off. [`PassWindow::culmination`] is refined by golden section inside
//! the bracketed interval and does not depend on the step. Tested: same maximum to 1e-9 at a 10 s step and at
//! a 5 s step.
//!
//! **4. Earth's rotation applied to the satellite but not the observer.** The classic beginner error, and it
//! produces a plausible schedule minutes wrong: for a 400 km equatorial orbit, ignoring rotation puts the
//! first 10° pass at **2748 s** when it is at **2936 s** — 188 s out, on a pass only 398 s long.
//!
//! ## The bounds, and why they are derived rather than tuned
//!
//! Two numbers decide whether a search is honest, and both are geometry:
//!
//! * **Minimum pass duration.** For a circular orbit of radius `r` seen from a body of radius `R` above mask
//!   elevation `ε`, the target is up while the central angle between observer and sub-point is within
//!   `θ(ε)`, and the relative angular rate is `n − ω`. So `T_min = 2θ/(n_max − ω)`, with
//!   ```text
//!   θ = acos( (R/r) cos ε ) − ε
//!   ```
//!   Verified against a bisection on the exact elevation function to **1e-9**: 652.3949 s at the geometric
//!   horizon, 398.2311 s at a 10° mask, for 400 km. At the horizon this is the tangent angle `acos(R/r)` —
//!   the same 19.782° that puts [`crate::observe::horizon_range`] at 2294 km. One fact, two views.
//! * **Longest wait you ever have to budget for.** The relative geometry repeats on `2π/(n − ω)`, because the
//!   ground track closes after exactly one synodic period. One such period *plus* one minimum pass always
//!   contains a rise if the geometry allows one at all.
//!
//! Eccentricity is in both bounds rather than ignored: the true-anomaly rate runs from
//! `n(1−e)²/(1−e²)^{3/2}` at apoapsis to `n(1+e)²/(1−e²)^{3/2}` at periapsis, so the shortest pass uses the
//! fast end (pessimistic) and the longest wait uses the slow end (also pessimistic). Both errors point the
//! same way, which is the only reason a single pair of bounds is defensible.
//!
//! ## What this module does not do
//!
//! * **No terrain, refraction or atmosphere.** The mask elevation is the caller's, because a radio horizon
//!   is a few degrees below the geometric one and a mountain is above it. Both are inputs, not guesses.
//! * **No J2 in the search.** A two-body prediction is good for a pass this hour, not for a week. Measured
//!   for 400 km / 51.6° over a 45° site: the pass moves by **8.4 s on day 0**, **80 s on day 1**, and by day
//!   2 the schedule has drifted far enough that "the next pass" is a *different pass*. `docs/CREDIBILITY.md`
//!   carries the row; the fix is the J2-aware element propagation that already exists in [`crate::j2`], and
//!   the reason it is not here yet is that a secular model is the wrong tool for a 60-second window.
//! * **No eclipse or illumination.** Whether the satellite is sunlit is a lighting question, not a geometry
//!   one, and it belongs with the ephemeris.

use crate::catalogue::Body;
use crate::frames::BodyFixed;
use crate::observe::{look_at, Look, Site};
use crate::{is_positive_finite, propagate, State};
use odderon_dmath::{acos, cos, fabs, fmax, sqrt, TAU};

/// Relative slack in the two guards, dimensionless.
///
/// The guards exist to catch a step that can **step over** a pass, or a window that cannot contain one — errors
/// of factors, not of ulps. Comparing exactly would make the API hostile in a very particular way: a caller who
/// does the correct thing, computing the promised window from their own `a` and `e` and passing it back, is
/// refused because the library recomputes `a` from the state and gets a different last bit. Measured: the same
/// 400 km circular orbit yields **5936.238838696649 s** from `a = R + 400` and **5936.238838696666 s** from the
/// state's vis-viva `a`, a relative disagreement of `3e-15`. Under an exact comparison that is a refusal, and the
/// caller's only repair is to add a fudge factor — which is how a derived bound quietly becomes a magic number.
///
/// `1e-12` is a thousand ulps in relative terms, i.e. `6e-9 s` on a 5936 s window: far below anything a
/// propagation can resolve, and far above the rounding of two equivalent expressions for the same quantity.
const GUARD_SLACK: f64 = 1e-12;

/// A predicted pass above the observer's mask elevation.
///
/// Times are **seconds past J2000**, the same epoch as the state the prediction started from, so a caller can
/// subtract the current time and get "in *N* seconds" without a conversion step that goes wrong.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PassWindow {
    /// Mask crossing, upward. Elevation equals the mask to within the solver's tolerance.
    pub rise: f64,
    /// Mask crossing, downward.
    pub set: f64,
    /// Time of maximum elevation, refined inside `[rise, set]`. Never a sample time.
    pub culmination: f64,
    /// Maximum elevation, degrees.
    pub max_elevation_deg: f64,
    /// Azimuth at rise, degrees clockwise from north.
    pub rise_azimuth_deg: f64,
    /// Azimuth at set, degrees clockwise from north.
    pub set_azimuth_deg: f64,
    /// Closest approach during the pass, km.
    pub min_range_km: f64,
    /// Elevations evaluated to produce this. Reported because a prediction costing ten thousand propagations
    /// has a different place in a frame budget than one costing sixty, and the caller should not have to
    /// guess which they got.
    pub elevations_evaluated: u32,
}

impl PassWindow {
    /// Duration above the mask, seconds.
    #[inline]
    pub fn duration(&self) -> f64 {
        self.set - self.rise
    }
}

/// Why a search could not be carried out at all.
///
/// The distinction between "I cannot answer honestly" and "there is no pass" is the whole point of this type.
/// [`PassError`] is the first; `Ok(None)` is the second. Collapsing them is how a tool ends up telling a
/// controller there is no pass when it simply did not look properly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PassError {
    /// The window is shorter than the guaranteed repeat period, so "no pass" would not be a finding.
    WindowTooShort {
        /// The window the geometry requires, seconds.
        least_window_s: f64,
        /// What the caller asked for.
        given_s: f64,
    },
    /// The step is longer than the shortest pass this geometry allows, so the scan can step over one.
    StepTooCoarse {
        /// Largest step that keeps the scan honest, seconds.
        greatest_step_s: f64,
        /// What the caller asked for.
        given_s: f64,
    },
    /// The target's orbit is not elliptical, or the state is degenerate, so no period exists.
    NotAnEllipticalOrbit,
    /// The relative geometry never repeats, so no finite window can be *guaranteed* to contain a pass.
    ///
    /// This is the geostationary case: `n − ω ≈ 0`, so the ground track is a point and the target either
    /// stands still above the station or stands still somewhere else. Distinct from
    /// [`PassError::NotAnEllipticalOrbit`] because the orbit here is perfectly good — what is missing is a
    /// *change* in the geometry, and the caller's repair is different (ask for a look angle, not a pass).
    GeometryNeverRepeats,
    /// The target stayed above the mask for longer than any pass this geometry can produce.
    ///
    /// A pass is bounded by `2π/(n_min − ω)`; something that has not set by then is not passing over the
    /// station, it is parked there. Reporting a "pass" with an invented end time would be worse than
    /// reporting that the question does not apply.
    PassNeverCloses,
    /// The rotation model could not produce an angle.
    RotationUnavailable,
    /// A parameter was NaN, infinite, or otherwise unusable.
    BadInput,
}

// ---------------------------------------------------------------------------------------------
// The two bounds
// ---------------------------------------------------------------------------------------------

/// The central angle between observer and sub-target at which the target appears at elevation `mask`.
///
/// ```text
/// θ = acos( (R/r) cos ε ) − ε
/// ```
///
/// At `ε = 0` this reduces to `acos(R/r)`, the tangent angle. At a higher mask it shrinks, because a target
/// must stand higher in the sky to clear a steeper elevation: 12.075° at a 10° mask, 1.934° at 60°, 0.596° at
/// 80° — for 400 km.
///
/// Returns `None` when the geometry has no solution: a target inside the body has no horizon to clear.
pub fn central_half_angle(
    body_radius: f64,
    target_radius: f64,
    mask_elevation_deg: f64,
) -> Option<f64> {
    if !is_positive_finite(body_radius) || !is_positive_finite(target_radius) {
        return None;
    }
    if !(-90.0..=90.0).contains(&mask_elevation_deg) {
        return None;
    }
    if target_radius <= body_radius {
        return None;
    }
    let eps = mask_elevation_deg * core::f64::consts::PI / 180.0;
    // `acos` clamps its own domain, so a mask above the highest elevation this radius can reach yields
    // exactly 0 here rather than NaN. That is the right answer — the pass is instantaneous — and it also
    // makes every step "too coarse" downstream, which tells the caller something true instead of handing
    // them a zero-length window a scan would skip.
    let theta = acos((body_radius / target_radius) * cos(eps)) - eps;
    Some(fmax(theta, 0.0))
}

/// The target's angular rate at apoapsis and periapsis, rad/s: `n(1 ∓ e)²/(1−e²)^{3/2}`.
///
/// The two ends of the range the true-anomaly rate sweeps through each orbit. For a circular orbit both equal
/// `n`.
fn rate_bounds(a: f64, e: f64, mu: f64) -> Option<(f64, f64)> {
    if !is_positive_finite(a) || !is_positive_finite(mu) || !(0.0..1.0).contains(&e) {
        return None;
    }
    let n = sqrt(mu / (a * a * a));
    let one_minus = 1.0 - e * e;
    let denom = one_minus * sqrt(one_minus);
    let slow = n * (1.0 - e) * (1.0 - e) / denom;
    let fast = n * (1.0 + e) * (1.0 + e) / denom;
    if !is_positive_finite(slow) || !is_positive_finite(fast) {
        return None;
    }
    Some((slow, fast))
}

/// Shortest pass this geometry can produce, seconds.
///
/// `2θ / (n_max − ω)`: the angular width of the visibility window divided by the fastest relative rate the
/// orbit ever has. Deliberately the *shortest*, because a generous bound tells the caller a step is fine when
/// it is not — the exact failure this module exists to prevent.
///
/// Returns `None` when the relative rate is zero (geostationary: the geometry never changes, so no pass has a
/// duration) or when any input is unusable.
pub fn minimum_pass_seconds(
    body_radius: f64,
    target_radius: f64,
    mask_elevation_deg: f64,
    a: f64,
    e: f64,
    mu: f64,
    body_rotation_rate: f64,
) -> Option<f64> {
    let theta = central_half_angle(body_radius, target_radius, mask_elevation_deg)?;
    let (_, fast) = rate_bounds(a, e, mu)?;
    if !body_rotation_rate.is_finite() {
        return None;
    }
    let relative = fast - body_rotation_rate;
    if !is_positive_finite(relative) {
        return None;
    }
    Some(2.0 * theta / relative)
}

/// The longest a caller ever has to wait for the relative geometry to return, seconds: one synodic period.
///
/// Uses the **slowest** angular rate the orbit has, so the window is an upper bound rather than an optimistic
/// guess. Returns `None` for a geostationary target, where the relative geometry never changes and no finite
/// window can be guaranteed — a genuinely different answer from "very long", and one the caller must handle.
pub fn guaranteed_scan_window_seconds(
    a: f64,
    e: f64,
    mu: f64,
    body_rotation_rate: f64,
) -> Option<f64> {
    let (slow, _) = rate_bounds(a, e, mu)?;
    if !body_rotation_rate.is_finite() {
        return None;
    }
    let relative = fabs(slow - body_rotation_rate);
    if !is_positive_finite(relative) {
        return None;
    }
    Some(TAU / relative)
}

// ---------------------------------------------------------------------------------------------
// The search
// ---------------------------------------------------------------------------------------------

/// Elevation as a function of absolute time. Holds everything the search needs so the inner loop stays
/// allocation-free.
struct Elevation<'a> {
    site: &'a Site,
    frame: &'a BodyFixed,
    state: &'a State,
    state_epoch: f64,
    mu: f64,
    evaluations: u32,
}

impl Elevation<'_> {
    /// Elevation in degrees at absolute time `t`, or `None` if the propagator failed.
    fn at(&mut self, t: f64) -> Option<f64> {
        self.evaluations = self.evaluations.saturating_add(1);
        let p = propagate(self.state, t - self.state_epoch, self.mu);
        if p.unconverged || p.overflowed {
            return None;
        }
        let body_fixed = self.frame.to_body_fixed(p.state.r, t)?;
        let look = look_at(self.site, body_fixed)?;
        Some(look.elevation_deg)
    }

    /// The full look at an instant, so a reported azimuth and a live look angle can never disagree.
    fn look(&mut self, t: f64) -> Option<Look> {
        let p = propagate(self.state, t - self.state_epoch, self.mu);
        if p.unconverged || p.overflowed {
            return None;
        }
        let body_fixed = self.frame.to_body_fixed(p.state.r, t)?;
        look_at(self.site, body_fixed)
    }
}

/// Find the first pass above `mask_elevation_deg` that starts in `[search_from, search_from + window_s)`.
///
/// `state` is the target's inertial position and velocity at absolute time `state_epoch`; `search_from` and
/// the returned window are both absolute times (seconds past J2000), so the caller does no epoch arithmetic.
///
/// # Arguments
///
/// * `body` — the body being orbited, which supplies the radius the horizon is measured from.
/// * `site` — the observer, from `Site::from_geodetic`.
/// * `frame` — the body's rotation model, which is what makes the observer move.
/// * `mask_elevation_deg` — the minimum elevation worth calling a pass. **This number decides whether the
///   answer is useful**: for 400 km a pass lasts 652 s at the geometric horizon, 398 s at 10°, and 64 s at
///   60°. A station with a 60° mask sees a fraction of the passes that a station with a 0° mask does, which
///   is why "raise the mask" is a modelling decision and not a filter.
/// * `search_from` — where to start looking, absolute seconds past J2000.
/// * `window_s` — how far to look. Must be at least `guaranteed_scan_window_seconds`.
/// * `step_s` — the scan step. Must be at most `minimum_pass_seconds`.
///
/// Both limits are compared with a relative slack of [`GUARD_SLACK`], so passing back exactly the number one of
/// those functions returned is always accepted. That is deliberate: a caller who computes the promised window and
/// hands it in is doing precisely what the API asks, and a refusal by one part in `3e-15` would teach them to pad
/// the number instead — turning a derived bound into folklore.
///
/// # Returns
///
/// * `Ok(Some(pass))` — a pass was found.
/// * `Ok(None)` — no pass **starts** in the window, and the window was long enough for that to mean something.
/// * `Err(_)` — the question as asked cannot be answered honestly, and **no scan was performed**. That
///   ordering is deliberate: a refusal is cheap and a wrong answer is not.
///
/// The window bounds *rises*, not passes. Distinguishing "no pass in the next 90 minutes" from "no pass
/// *starting* in the next 90 minutes" is the difference between a scheduler and a guess.
#[allow(clippy::too_many_arguments)]
pub fn next_pass(
    body: &Body,
    site: &Site,
    frame: &BodyFixed,
    state: &State,
    state_epoch: f64,
    mu: f64,
    mask_elevation_deg: f64,
    search_from: f64,
    window_s: f64,
    step_s: f64,
) -> Result<Option<PassWindow>, PassError> {
    if !state_epoch.is_finite() || !search_from.is_finite() {
        return Err(PassError::BadInput);
    }
    if !is_positive_finite(window_s) || !is_positive_finite(step_s) || !is_positive_finite(mu) {
        return Err(PassError::BadInput);
    }
    if !mask_elevation_deg.is_finite() {
        return Err(PassError::BadInput);
    }

    // The orbit, from the state the caller gave. Elements rather than a stored `a`, so a state produced by a
    // manoeuvre works without the caller having to reason about it.
    let a = state.semi_major_axis(mu);
    let el = state.to_elements(mu);
    if !is_positive_finite(a) || !(0.0..1.0).contains(&el.e) {
        return Err(PassError::NotAnEllipticalOrbit);
    }
    let body_rate = frame.rate.value;
    if !body_rate.is_finite() {
        return Err(PassError::RotationUnavailable);
    }
    if !is_positive_finite(body.physical.equatorial_radius.value) {
        return Err(PassError::BadInput);
    }
    let body_radius = body.physical.equatorial_radius.value;

    // Both guards are evaluated before a single propagation happens. If either bound cannot be computed the
    // question is ill-posed and the caller is told, rather than being handed a scan that might have worked.
    let least_window = guaranteed_scan_window_seconds(a, el.e, mu, body_rate)
        .ok_or(PassError::GeometryNeverRepeats)?;
    // `GUARD_SLACK` rather than an exact comparison; see the constant for the measurement that forced it.
    if window_s < least_window * (1.0 - GUARD_SLACK) {
        return Err(PassError::WindowTooShort {
            least_window_s: least_window,
            given_s: window_s,
        });
    }

    // Circular-equivalent radius makes the bound slightly pessimistic for an eccentric orbit — the safe
    // direction, since it shortens the permitted step.
    let greatest_step =
        minimum_pass_seconds(body_radius, a, mask_elevation_deg, a, el.e, mu, body_rate)
            .ok_or(PassError::BadInput)?;
    if step_s > greatest_step * (1.0 + GUARD_SLACK) {
        return Err(PassError::StepTooCoarse {
            greatest_step_s: greatest_step,
            given_s: step_s,
        });
    }

    let mut f = Elevation {
        site,
        frame,
        state,
        state_epoch,
        mu,
        evaluations: 0,
    };

    // Sample until the first grid point at or above the mask.
    let mut t_prev = search_from;
    let mut e_prev = f.at(t_prev).ok_or(PassError::RotationUnavailable)?;
    let mut bracket = None;
    let mut t = search_from;
    while t < search_from + window_s {
        t += step_s;
        let e = f.at(t).ok_or(PassError::RotationUnavailable)?;
        if e_prev < mask_elevation_deg && e >= mask_elevation_deg {
            bracket = Some((t_prev, t));
            break;
        }
        t_prev = t;
        e_prev = e;
    }

    let (mut lo, mut hi) = match bracket {
        Some(b) => b,
        None => return Ok(None),
    };

    // Rise: close the bracket on the crossing, rising through the mask.
    //
    // 64 halvings of a bracket no longer than `step_s` (which the guard has already bounded by the shortest
    // possible pass) puts the answer far below the resolution of the arithmetic — 2^-64 of 64 s is 3e-18 s. The
    // count is fixed rather than convergence-tested on purpose: a deterministic iteration count cannot make a
    // result depend on the sign of a rounding decision, which is the rule the whole kernel is built on.
    for _ in 0..64 {
        let mid = 0.5 * (lo + hi);
        if f.at(mid).ok_or(PassError::RotationUnavailable)? >= mask_elevation_deg {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let rise = 0.5 * (lo + hi);

    // Set: step forward from the last grid point known to be above the mask. Safe for the same reason the
    // whole scan is — the pass is at least as long as the permitted step.
    //
    // The upper limit is exact rather than a fudge: the visibility window is at most half the sky, so the
    // central angle is `θ ≤ π`, so the duration is at most `2π/(n_min − ω)`, which is precisely
    // `least_window`. Anything still up after that has not been passed over; it has been parked above.
    let mut t_above = hi;
    // `loop` with a `break` value rather than a pre-initialised `Option`: the initialiser would be a value that
    // is written and never read, which `unused_assignments` correctly refuses, and the `Option` dance would then
    // need an unreachable arm that a reader has to verify by hand. Here the loop assigns exactly once.
    let (mut slo, mut shi) = loop {
        let next = t_above + step_s;
        if next > rise + least_window {
            return Err(PassError::PassNeverCloses);
        }
        let e = f.at(next).ok_or(PassError::RotationUnavailable)?;
        if e < mask_elevation_deg {
            break (t_above, next);
        }
        t_above = next;
    };
    for _ in 0..64 {
        let mid = 0.5 * (slo + shi);
        if f.at(mid).ok_or(PassError::RotationUnavailable)? >= mask_elevation_deg {
            slo = mid;
        } else {
            shi = mid;
        }
    }
    let set = 0.5 * (slo + shi);

    // Culmination by golden-section maximisation — deliberately *not* the best sample, because the maximum is
    // almost never on the grid and reporting a sample's time as the culmination is a wrong answer that looks
    // like precision.
    let inv_phi = 0.618_033_988_749_895;
    let mut a_lo = rise;
    let mut a_hi = set;
    let mut c = a_hi - inv_phi * (a_hi - a_lo);
    let mut d = a_lo + inv_phi * (a_hi - a_lo);
    let mut fc = f.at(c).ok_or(PassError::RotationUnavailable)?;
    let mut fd = f.at(d).ok_or(PassError::RotationUnavailable)?;
    for _ in 0..64 {
        if fc > fd {
            a_hi = d;
            d = c;
            fd = fc;
            c = a_hi - inv_phi * (a_hi - a_lo);
            fc = f.at(c).ok_or(PassError::RotationUnavailable)?;
        } else {
            a_lo = c;
            c = d;
            fc = fd;
            d = a_lo + inv_phi * (a_hi - a_lo);
            fd = f.at(d).ok_or(PassError::RotationUnavailable)?;
        }
    }
    let culmination = 0.5 * (a_lo + a_hi);

    // The reported angles come from `look_at` itself, so a pass window and a live look angle can never
    // disagree about the same instant.
    let at_rise = f.look(rise).ok_or(PassError::RotationUnavailable)?;
    let at_set = f.look(set).ok_or(PassError::RotationUnavailable)?;
    let at_culm = f.look(culmination).ok_or(PassError::RotationUnavailable)?;

    Ok(Some(PassWindow {
        rise,
        set,
        culmination,
        max_elevation_deg: fmax(
            at_culm.elevation_deg,
            fmax(at_rise.elevation_deg, at_set.elevation_deg),
        ),
        rise_azimuth_deg: at_rise.azimuth_deg,
        set_azimuth_deg: at_set.azimuth_deg,
        min_range_km: at_culm.range_km,
        elevations_evaluated: f.evaluations,
    }))
}
