//! A sounding rocket trajectory: the suborbital flight from a surface launch.
//!
//! ## Why this is in the astrodynamics crate
//!
//! Same reason [`cryo`] and [`thermal`] are here: `odderon-astro` is the only `no_std`,
//! `forbid(unsafe_code)` crate in the workspace, gated by the same kernel rules. A sounding rocket
//! is Kepler mechanics — the same [`propagate`] that flies the orbital body — plus the
//! surface-relative readings that turn an orbit into a *flight*: apogee altitude, time to apogee,
//! impact time, ground range, and a verdict.
//!
//! ## The claim this exists to make testable
//!
//! `GAME-DESIGN-BIBLE.md` §17 Block 4 and the P3 row of `docs/BUILD-PLAN.md` §5: the player
//! authors a small vehicle — launch parameters, not a purchased design — and flies it. The gate
//! to leave P3 is *bit-identical replay in a second browser*, which is a falsifiable statement
//! about the trajectory being a **pure function of the launch parameters and `t`**. This module
//! is the smallest thing that can falsify it: no clock, no global state, no allocator, so the
//! same parameters produce the same flight on every platform, every run.
//!
//! ## The trajectory
//!
//! A sounding rocket is a **suborbital Kepler ellipse** whose perigee lies below the surface:
//! the existing [`propagate`] solves it exactly, so this module adds only the launch-state
//! construction from the authored parameters and the surface-relative readings that make the
//! orbit a *flight*. The launch parameters are the authored design (`GAME-DESIGN-BIBLE.md` §1
//! P3 — authorship over ownership): latitude, azimuth, elevation, and burnout velocity. There
//! is no working default — the inherited machine's parameters produce a flight the player must
//! diagnose, which is the authorship mechanic.
//!
//! ## What is modelled, and what is not
//!
//! * **Two-body Kepler propagation only.** No atmosphere, no drag, no gravity losses during the
//!   powered phase. The burnout state is the start of the coast; everything after is [`propagate`].
//!   A real sounding rocket's trajectory is dominated by aerodynamic drag in the first 10 km, and
//!   this module does not model it. The trajectory is a *coast*, which is the honest statement of
//!   what is being claimed: the physics that is modelled is exact, and what is not is named.
//! * **Earth is a point mass.** The `mu` and `radius` constants come from [`mu::EARTH`] and
//!   [`radius::EARTH`]. J2 is not applied to the trajectory — the existing [`j2`] module is
//!   available for orbital missions, but a suborbital arc is too short for secular J2 to matter,
//!   and applying it would be a claim this module does not support.
//! * **The impact is a surface crossing.** The rocket "lands" when its trajectory intersects the
//!   surface radius. There is no landing burn, no parachute, no recovery sequence — the impact
//!   time and range are the point at which the trajectory returns to `r = R`, and nothing after.

use crate::{mu, propagate, radius, true_anomaly, Elements, State, Vec3};
use odderon_dmath::{acos, atan2, clamp_unit, cos, is_finite, sin, sqrt};

/// The Kármán line: 100 km above the surface, the conventional boundary of space.
///
/// The verdict's "reaches space" test. A named constant rather than a bare `100.0`, because the
/// Kármán line is a *convention* — a definition adopted by the Fédération Aéronautique
/// Internationale — and not a physical threshold. Naming it makes that clear.
pub const KARMAN_LINE_KM: f64 = 100.0;

/// Tolerance for the suborbital test: a perigee within this distance of the surface is treated
/// as grazing (orbital) rather than suborbital.
///
/// The surface radius is ~6378 km, so `1e-3` km is 1 m — far below any real trajectory's
/// precision and far above floating-point noise in the element computation.
const SURFACE_TOLERANCE_KM: f64 = 1e-3;

/// The authored launch parameters: the vehicle design the player authors before flight.
///
/// These are the *inputs* to the trajectory, not outputs. The kernel is a pure function of them:
/// the same parameters produce the same flight on every platform, every run, which is what makes
/// the replay gate testable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaunchParams {
    /// Launch site latitude, radians. North positive. The launch site is at longitude 0.
    pub latitude_rad: f64,
    /// Launch azimuth, radians, measured clockwise from north. 0 = north, π/2 = east.
    pub azimuth_rad: f64,
    /// Launch elevation, radians, above the local horizontal. 0 = horizontal, π/2 = vertical.
    pub elevation_rad: f64,
    /// Burnout velocity, km/s. The speed at the end of the powered phase, when the coast begins.
    pub burnout_velocity_km_s: f64,
}

impl LaunchParams {
    /// The inherited sounding rocket's default launch: a suborbital flight that does **not**
    /// reach the Kármán line.
    ///
    /// This is the "almost-working inherited machine" (`GAME-DESIGN-BIBLE.md` §1 P3): a
    /// physically complete launch that produces a flight the player must diagnose. The apogee
    /// falls short of space, and the player authors the parameters — elevation, burnout
    /// velocity — to reach it. There is no working default; the default is deliberately
    /// *almost* right.
    pub const fn inherited() -> Self {
        LaunchParams {
            latitude_rad: 0.0,
            azimuth_rad: 0.0,
            elevation_rad: odderon_dmath::PI / 4.0,
            burnout_velocity_km_s: 1.5,
        }
    }
}

/// The verdict on a rocket's trajectory: what kind of flight the authored parameters produce.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RocketVerdict {
    /// The perigee lies below the surface: the trajectory returns to Earth. Apogee, impact
    /// time, and range are all defined.
    Suborbital,
    /// The perigee lies at or above the surface: the trajectory stays in orbit. Apogee is
    /// defined; impact time and range are not.
    Orbital,
    /// The trajectory is hyperbolic: the rocket escapes. Apogee, impact time, and range are
    /// all undefined.
    Escape,
    /// The input is degenerate — non-finite parameters, zero velocity, or a state that produces
    /// no orbit. No reading is meaningful.
    Refused,
}

/// The surface-relative readings of a rocket's flight: the numbers that turn an orbit into a
/// *flight* the player can read.
///
/// Fields that do not apply to the verdict are `NaN`, never zero or a made-up value. A `NaN`
/// reading is a refusal, not an answer — the same rule that governs every other module in this
/// crate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RocketFlight {
    /// Apogee altitude above the surface, km. Defined for suborbital and orbital trajectories;
    /// `NaN` for escape.
    pub apogee_km: f64,
    /// Seconds from launch to apogee. Defined for suborbital and orbital; `NaN` for escape.
    pub time_to_apogee_s: f64,
    /// Seconds from launch to surface impact. Defined for suborbital only; `NaN` for orbital
    /// and escape.
    pub impact_time_s: f64,
    /// Ground range from launch to impact, km. Defined for suborbital only; `NaN` for orbital
    /// and escape.
    pub range_km: f64,
    /// What kind of flight the authored parameters produce.
    pub verdict: RocketVerdict,
}

/// Build the initial state vector from launch parameters, at the moment of burnout.
///
/// The launch site is at latitude `latitude_rad`, longitude 0, on the surface. The local frame
/// is East-North-Up (ENU) at that point:
///
/// * **East** = `(0, 1, 0)`
/// * **North** = `(-sin(lat), 0, cos(lat))`
/// * **Up** = `(cos(lat), 0, sin(lat))`
///
/// Azimuth is measured clockwise from north, so the horizontal launch direction is
/// `sin(az)·East + cos(az)·North`. Elevation is above the horizontal, so the velocity direction
/// is `cos(el)·horizontal + sin(el)·Up`. The burnout velocity scales that direction.
///
/// The result is the state at the start of the coast — the point from which [`propagate`] takes
/// over. The position is exactly on the surface; the velocity is the authored burnout velocity
/// in the authored direction.
pub fn launch_state(params: &LaunchParams) -> State {
    let r_earth = radius::EARTH;
    let lat = params.latitude_rad;
    let az = params.azimuth_rad;
    let el = params.elevation_rad;
    let v = params.burnout_velocity_km_s;

    // Position on the surface at longitude 0.
    let r = Vec3::new(r_earth * cos(lat), 0.0, r_earth * sin(lat));

    // Local ENU basis at the launch site.
    let east = Vec3::new(0.0, 1.0, 0.0);
    let north = Vec3::new(-sin(lat), 0.0, cos(lat));
    let up = Vec3::new(cos(lat), 0.0, sin(lat));

    // Horizontal launch direction: azimuth clockwise from north.
    let horiz = Vec3::new(
        sin(az) * east.x + cos(az) * north.x,
        sin(az) * east.y + cos(az) * north.y,
        sin(az) * east.z + cos(az) * north.z,
    );

    // Velocity direction: elevation above the horizontal.
    let dir = Vec3::new(
        cos(el) * horiz.x + sin(el) * up.x,
        cos(el) * horiz.y + sin(el) * up.y,
        cos(el) * horiz.z + sin(el) * up.z,
    );

    let vel = Vec3::new(dir.x * v, dir.y * v, dir.z * v);
    State::new(r, vel)
}

/// Compute the full flight readings from launch parameters.
///
/// This is the core of the module: a pure function of [`LaunchParams`] that produces every
/// surface-relative number the player reads. It builds the launch state, computes the orbital
/// elements, and derives the readings from them:
///
/// * **Apogee** — the apoapsis radius minus the surface radius, from [`Elements::apoapsis`].
/// * **Time to apogee** — from Kepler's equation: the mean anomaly at the launch true anomaly
///   to the mean anomaly at apoapsis (π), divided by the mean motion.
/// * **Impact time** — twice the time to apogee, by the symmetry of the suborbital ellipse: the
///   launch and impact points are both at `r = R`, symmetric about the apoapsis.
/// * **Range** — the central angle between launch and impact positions, times the surface radius.
///
/// The verdict determines which readings are defined. A [`RocketVerdict::Refused`] verdict
/// produces `NaN` for every reading — a refusal, not an answer.
pub fn rocket_flight(params: &LaunchParams) -> RocketFlight {
    let state = launch_state(params);
    let mu_earth = mu::EARTH;
    let r_earth = radius::EARTH;

    // Degenerate input: non-finite parameters, zero or negative burnout velocity, or a state
    // that produces no orbit.
    if !is_finite(params.latitude_rad)
        || !is_finite(params.azimuth_rad)
        || !is_finite(params.elevation_rad)
        || !is_finite(params.burnout_velocity_km_s)
        || params.burnout_velocity_km_s <= 0.0
        || !is_finite(state.r.x)
        || !is_finite(state.r.y)
        || !is_finite(state.r.z)
        || !is_finite(state.v.x)
        || !is_finite(state.v.y)
        || !is_finite(state.v.z)
    {
        return RocketFlight {
            apogee_km: f64::NAN,
            time_to_apogee_s: f64::NAN,
            impact_time_s: f64::NAN,
            range_km: f64::NAN,
            verdict: RocketVerdict::Refused,
        };
    }

    let energy = state.specific_energy(mu_earth);
    let h_norm = state.angular_momentum().norm();

    // Escape test uses specific energy, not eccentricity. A radial (perfectly vertical)
    // launch has zero angular momentum, so its elements degenerate to e -> 1 — yet it is
    // bound (energy < 0) and returns to the surface. Eccentricity alone would misclassify
    // every vertical launch as escape; the energy is the honest discriminator.
    if energy >= 0.0 || !is_finite(energy) {
        return RocketFlight {
            apogee_km: f64::NAN,
            time_to_apogee_s: f64::NAN,
            impact_time_s: f64::NAN,
            range_km: f64::NAN,
            verdict: RocketVerdict::Escape,
        };
    }

    // Bound (energy < 0): the trajectory is an ellipse. A radial launch — angular momentum
    // below tolerance — is the degenerate limit: a straight line up and down. Its elements
    // collapse (e -> 1, a -> r_apo/2), so the orbital-elements path below cannot serve it;
    // the radial case is handled by the one-dimensional energy formulas instead.
    if h_norm < RADIAL_ANGULAR_MOMENTUM_TOLERANCE {
        return radial_flight(&state, energy, mu_earth, r_earth);
    }

    let elements: Elements = state.to_elements(mu_earth);
    let e = elements.e;

    // Elliptic. Apogee is always defined: the apoapsis radius minus the surface.
    let apogee_km = elements.apoapsis() - r_earth;

    // Time to apogee via Kepler's equation.
    // E = 2·atan2(√(1-e)·sin(ν/2), √(1+e)·cos(ν/2)), M = E - e·sin(E), t = (π - M) / n
    let nu_launch = true_anomaly(&state, mu_earth);
    let half_nu = nu_launch * 0.5;
    let e_anom = 2.0 * atan2(sqrt(1.0 - e) * sin(half_nu), sqrt(1.0 + e) * cos(half_nu));
    let mean_anom_launch = e_anom - e * sin(e_anom);

    let a = elements.semi_major_axis();
    let mean_motion = sqrt(mu_earth / (a * a * a));
    let time_to_apogee_s = (odderon_dmath::PI - mean_anom_launch) / mean_motion;

    // Suborbital test: does the perigee dip below the surface?
    let perigee_km = elements.periapsis();
    if perigee_km < r_earth - SURFACE_TOLERANCE_KM {
        // Suborbital: the trajectory returns to the surface.
        // By symmetry, impact time = 2 × time to apogee (launch and impact are both at r = R,
        // symmetric about the apoapsis).
        let impact_time_s = 2.0 * time_to_apogee_s;

        // Impact position: propagate to the impact time, then compute the ground range as the
        // central angle between launch and impact positions, times the surface radius.
        let impact_prop = propagate(&state, impact_time_s, mu_earth);
        let r_launch = state.r;
        let r_impact = impact_prop.state.r;
        let cos_angle = r_launch.dot(r_impact) / (r_launch.norm() * r_impact.norm());
        let central_angle = acos(clamp_unit(cos_angle));
        let range_km = r_earth * central_angle;

        RocketFlight {
            apogee_km,
            time_to_apogee_s,
            impact_time_s,
            range_km,
            verdict: RocketVerdict::Suborbital,
        }
    } else {
        // Orbital: perigee at or above the surface. Apogee is defined; no impact.
        RocketFlight {
            apogee_km,
            time_to_apogee_s,
            impact_time_s: f64::NAN,
            range_km: f64::NAN,
            verdict: RocketVerdict::Orbital,
        }
    }
}

/// Tolerance for the radial-trajectory test: a launch whose angular momentum is below this
/// magnitude (km²/s) is treated as a straight-line radial flight.
///
/// The tolerance is 1e-3 km²/s — small relative to any non-vertical launch's angular momentum
/// (a 45° launch at 1.5 km/s gives ~4500 km²/s), but large enough to absorb the floating-point
/// residue of a computed "vertical" velocity (the ENU decomposition of a true-vertical burn
/// leaves ~1e-16 residuals, giving h ~1e-12).
const RADIAL_ANGULAR_MOMENTUM_TOLERANCE: f64 = 1e-3;

/// The surface-relative readings of a radial (perfectly vertical) flight.
///
/// A radial trajectory is the degenerate limit of a Kepler ellipse: a straight line from the
/// surface up to apogee and back down, with zero angular momentum. Its orbital elements
/// collapse, so the apogee, time-to-apogee, and impact are derived directly from the specific
/// energy instead:
///
/// * **Apogee** — where the radial velocity reaches zero: `r_apo = -mu / energy`.
/// * **Time to apogee** — the degenerate-ellipse form of Kepler's equation: with `a = r_apo / 2`
///   (perigee at the centre, apogee at `r_apo`), the launch eccentric anomaly is
///   `E = acos(1 - 2r_launch / r_apo)` and the time is `(π - E + sin(E)) / n`.
/// * **Impact time** — twice the time to apogee, by the symmetry of the up-and-down line.
/// * **Range** — zero: a vertical launch lands on its own pad.
fn radial_flight(state: &State, energy: f64, mu_earth: f64, r_earth: f64) -> RocketFlight {
    // Apogee radius: where the radial kinetic energy is spent against the potential.
    let r_apo = -mu_earth / energy;
    let apogee_km = r_apo - r_earth;

    // Degenerate ellipse: perigee at the centre (r = 0), apogee at r_apo, so a = r_apo / 2.
    let a = r_apo * 0.5;
    let mean_motion = sqrt(mu_earth / (a * a * a));

    // Launch eccentric anomaly from r = a(1 - cos E) = a(1 - cos E); with perigee at r=0,
    // r_launch = a(1 - cos E_launch)  =>  cos E_launch = 1 - 2·r_launch / r_apo.
    let r_launch = state.r.norm();
    let cos_e_launch = 1.0 - 2.0 * r_launch / r_apo;
    let e_launch = acos(clamp_unit(cos_e_launch));

    // M = E - sin(E) for the degenerate ellipse (e = 1); time = (M_apo - M_launch) / n,
    // with M_apo = π - sin(π) = π.
    let time_to_apogee_s = (odderon_dmath::PI - e_launch + sin(e_launch)) / mean_motion;
    let impact_time_s = 2.0 * time_to_apogee_s;

    RocketFlight {
        apogee_km,
        time_to_apogee_s,
        impact_time_s,
        range_km: 0.0,
        verdict: RocketVerdict::Suborbital,
    }
}

/// The state at time `t` seconds after launch, from launch parameters.
///
/// A thin wrapper over [`launch_state`] and [`propagate`], so the trajectory the page renders
/// and the readings the page reports come from the same propagator. The same parameters and
/// time produce the same state on every platform, which is the replay claim.
pub fn rocket_state_at(params: &LaunchParams, t: f64) -> State {
    let state = launch_state(params);
    propagate(&state, t, mu::EARTH).state
}

/// The deterministic design hash of the rocket module's constants.
///
/// FNV-1a, 64-bit, over the constants that define what a rocket *is* in this kernel: the
/// Kármán line, Earth's radius, and Earth's gravitational parameter. Folding them in means an
/// Akashi entry or a replay bundle can be pinned to this kernel version — if any of these
/// constants change, the hash changes, and a stale replay is detected rather than silently
/// accepted.
pub fn rocket_design_hash() -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |v: f64| {
        let bits = v.to_bits();
        hash ^= bits;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    mix(KARMAN_LINE_KM);
    mix(radius::EARTH);
    mix(mu::EARTH);
    hash
}
