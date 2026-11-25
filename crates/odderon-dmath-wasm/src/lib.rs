//! A C-ABI shell around [`odderon_dmath`], so that a wasm host can *read* the determinism hash.
//!
//! This crate exists for exactly one job: making the cross-platform determinism claim testable from
//! outside Rust. `tools/determinism.mjs` loads the compiled `.wasm`, calls [`golden`], and compares the
//! result against the same `tests/golden.txt` the native tests check.
//!
//! Keeping it separate from `odderon-dmath` means the kernel crate stays a clean `no_std` library with
//! no ABI surface, and the kernel/presenter boundary lint stays meaningful.
//!
//! `cdylib` with `no_mangle` exports and no imports is deliberate: a truly standalone module needs no
//! bindings generator, so Node can instantiate it with `{}` as the import object and the check has no
//! build-tool dependencies.

#![no_std]

// `no_mangle` is an unsafe attribute: it hands a symbol to the linker. This is the *only* crate in the
// workspace that is allowed to do so, it does so at the ABI boundary and nowhere else, and every export
// below is a thin wrapper over a safe kernel function. `forbid(unsafe_code)` is therefore lifted here
// deliberately, and only for this reason — the kernel crates keep it.

// A `no_std` cdylib must provide its own panic handler. In wasm a panic is unrecoverable anyway, and
// this one is deliberately tiny: it aborts, which wasm lowers to `unreachable` and the host traps on.
// Size matters more than message formatting here — the whole engine is a 2.5 MB download target.
#[cfg(target_arch = "wasm32")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}

/// The determinism hash computed inside wasm.
///
/// `u64` crosses the C ABI as a single wasm `i64`, which JS sees as a `BigInt`. That is the one
/// awkwardness of comparing 64-bit values across the boundary, and it is handled once in the checker.
#[unsafe(no_mangle)]
pub extern "C" fn golden() -> u64 {
    odderon_dmath::golden_hash()
}

/// Number of probes folded into [`golden`]. Lets the checker confirm it is looking at the probe set it
/// expects rather than a stale build's.
#[unsafe(no_mangle)]
pub extern "C" fn probe_count() -> u32 {
    odderon_dmath::probe_count()
}

/// `sin` re-exported, so the browser build can be smoke-tested before the full kernel exists.
#[unsafe(no_mangle)]
pub extern "C" fn sin(x: f64) -> f64 {
    odderon_dmath::sin(x)
}

/// `cos` re-exported, for the same reason.
#[unsafe(no_mangle)]
pub extern "C" fn cos(x: f64) -> f64 {
    odderon_dmath::cos(x)
}

/// `ln` re-exported, for the same reason.
#[unsafe(no_mangle)]
pub extern "C" fn ln(x: f64) -> f64 {
    odderon_dmath::ln(x)
}

/// `atan2` re-exported, for the same reason.
#[unsafe(no_mangle)]
pub extern "C" fn atan2(y: f64, x: f64) -> f64 {
    odderon_dmath::atan2(y, x)
}

/// `sqrt` re-exported. Owned rather than imported: see the note on [`odderon_dmath::sqrt`].
#[unsafe(no_mangle)]
pub extern "C" fn sqrt(x: f64) -> f64 {
    odderon_dmath::sqrt(x)
}

// =============================================================================================
// The propagation boundary
// =============================================================================================
//
// `web/index.html` used to draw its satellite by solving Kepler's equation in JavaScript and returning
// a position. That stand-in was labelled as one -- in three separate comments -- and it survived every
// check in the repository, because every check asked whether the page *rendered*, and the stand-in
// rendered beautifully. What it could not do is be a *state*: it returned `[x, y, z]` and no velocity,
// so the page had nothing it could advance across an idle gap. A render-on-demand renderer that cannot
// answer "where is it now" without re-deriving from elements is a renderer whose orbit is a guess about
// the last time it looked.
//
// These exports are the real kernel. `tools/check-orbit.mjs` fails the build if the page grows a local
// solver again, and runs the exports below against the same physical invariants the native test in
// `crates/odderon-astro/tests/reference-orbit.rs` checks.

/// The reference orbit, in elements: `a = 9800 km`, `e = 0.30`, inclined 0.55 rad, starting at
/// periapsis.
///
/// The page drew exactly this orbit before, so wiring the boundary changes where the numbers come from
/// without changing what is on screen. Values are plain constants rather than a derived `Elements` so
/// that [`semi_latus_rectum`] is the single place the `a(1-e²)` relation is written.
const A_KM: f64 = 9800.0;
const ECC: f64 = 0.30;
const INC_RAD: f64 = 0.55;

/// `p = a(1 - e²)` — the semi-latus rectum, which is the form `Elements` stores.
///
/// Written once, here, because the kernel's `Elements` takes `p` and not `a`: two places computing
/// `a(1-e²)` is two places that can disagree, and a disagreement there is a silently different orbit.
fn semi_latus_rectum() -> f64 {
    A_KM * (1.0 - ECC * ECC)
}

/// The reference orbit as a state vector at `dt = 0`.
///
/// Built through the kernel's own `Elements::to_state`, not from a hand-written `(r, v)` pair. A
/// hand-written pair can be internally inconsistent -- a position off the conic, or a velocity that does
/// not satisfy vis-viva -- and the inconsistency would then be propagated faithfully and blamed on the
/// propagator. The native test `reference-orbit.rs` asserts the resulting speed against
/// `sqrt(mu(2/r - 1/a))` and the position against the periapsis radius, so "consistent" is checked rather
/// than intended.
fn reference_state() -> odderon_astro::State {
    let elements = odderon_astro::Elements {
        p: semi_latus_rectum(),
        e: ECC,
        i: INC_RAD,
        raan: 0.0,
        argp: 0.0,
        nu: 0.0, // start at periapsis
        degeneracy: odderon_astro::Degeneracy::default(),
    };
    elements.to_state(odderon_astro::mu::EARTH)
}

/// Propagate the reference orbit by `dt` seconds.
///
/// The one place `propagate` is called, so the eight accessors below cannot describe different orbits.
fn state_at(dt: f64) -> odderon_astro::State {
    odderon_astro::propagate(&reference_state(), dt, odderon_astro::mu::EARTH).state
}

/// `x` of the satellite position at `dt` seconds past the reference epoch, in km.
///
/// **Why six scalar exports instead of one function filling a buffer.** This is a `no_std` cdylib with
/// no allocator, so it cannot return a slice. The alternative -- `orbit_state(out_ptr: *mut f64, dt)` --
/// needs `unsafe` pointer writes in the ABI crate, and this crate's `unsafe` allowance exists for
/// `#[unsafe(no_mangle)]` alone: handing a symbol to the linker, not dereferencing a caller's memory.
/// Six calls across the boundary cost tens of nanoseconds, which is nothing next to the shader this
/// feeds, and it leaves the module a pure function of `dt` with no hidden state.
#[unsafe(no_mangle)]
pub extern "C" fn orbit_state_x(dt: f64) -> f64 {
    state_at(dt).r.x
}

/// `y` of the satellite position, km. See [`orbit_state_x`].
#[unsafe(no_mangle)]
pub extern "C" fn orbit_state_y(dt: f64) -> f64 {
    state_at(dt).r.y
}

/// `z` of the satellite position, km. See [`orbit_state_x`].
#[unsafe(no_mangle)]
pub extern "C" fn orbit_state_z(dt: f64) -> f64 {
    state_at(dt).r.z
}

/// `x` of the satellite velocity, km/s. See [`orbit_state_x`].
///
/// The velocity is the export whose absence defined the old stub's defect, and the one a resumed frame
/// needs: position alone cannot be advanced without re-deriving from elements and re-solving.
#[unsafe(no_mangle)]
pub extern "C" fn orbit_velocity_x(dt: f64) -> f64 {
    state_at(dt).v.x
}

/// `y` of the satellite velocity, km/s. See [`orbit_state_x`].
#[unsafe(no_mangle)]
pub extern "C" fn orbit_velocity_y(dt: f64) -> f64 {
    state_at(dt).v.y
}

/// `z` of the satellite velocity, km/s. See [`orbit_state_x`].
#[unsafe(no_mangle)]
pub extern "C" fn orbit_velocity_z(dt: f64) -> f64 {
    state_at(dt).v.z
}

// ---------------------------------------------------------------------------------------------------
// The room, from the thermal network
// ---------------------------------------------------------------------------------------------------
//
// Claim 2 is the reason this room is the first build: "the room is a spacecraft -- closing the curtain
// drops solar gain by the amount §6.7 says, and the AC's electrical draw visibly falls." That claim has
// been measured in `odderon_astro::thermal` for some time and **no pixel ever read it**. These four
// exports are the seam.
//
// The unit that crosses the boundary is a FRACTION OF CLEAR SKY, not a colour and not a brightness. The
// shader multiplies it by its own sun radiance, so the picture is a consequence of the network's
// arithmetic rather than of a constant someone liked the look of. `1.0` means "the window is passing
// what an unshaded window passes"; `0.0` means nothing is getting through.
//
// Everything here is `Option`-free on purpose: these are `extern "C"` and a `f64` cannot express "no
// answer". The inputs are the network's own constructor values, so the only failure mode is a
// non-physical irradiance, which is floored at zero rather than NaN-ing the frame. The page refuses to
// draw a warm room from a network it could not load; it does not have to also refuse to multiply.

/// The reference room the game ships: 25 m² floor, 3 m² glazing, SHGC 0.60 open.
fn reference_zone(curtain: bool) -> odderon_astro::thermal::Zone {
    let z = odderon_astro::thermal::Zone::small_apartment_room();
    if curtain {
        z.curtain_closed()
    } else {
        z
    }
}

/// Outdoor conditions the room is judged against: §6.7's peak summer day.
///
/// These are **the test suite's fixture, not a new set of assumptions** -- `tests/thermal.rs`'s
/// `summer()` and `occupant()`. That matters: the first version of this file invented 32 C and no
/// occupant, and the AC draw came out at **582 W open / 161.5 W closed, a 72.25 % drop**, against the
/// module's documented **713 / 304 W, a 57.4 % drop**. Both are internally consistent; only one is the
/// number `BUILD-PLAN.md` ships. Re-deriving a fixture beside a tested one is how two constants for one
/// quantity get into a codebase, so this reads the same values and pins them against the test's own.
fn reference_outdoors() -> odderon_astro::thermal::Outdoors {
    odderon_astro::thermal::Outdoors {
        temperature_k: 35.0 + 273.15, // §6.7 peak summer, as `summer()` has it
        irradiance_w_per_m2: 800.0,   // plane of the glazing
    }
}

/// The light load of a person and a laptop, as `occupant()` has it. 300 W is 17 % of the open-window
/// solar gain, so leaving it out is not a rounding difference -- it moves the draw drop by 15 points.
fn reference_internal() -> odderon_astro::thermal::InternalGain {
    odderon_astro::thermal::InternalGain { watts: 300.0 }
}

/// Solar gain through the window, W.
///
/// Open: 800 × 3 × 0.60 = **1440 W**. Closed: 800 × 3 × 0.05 = **120 W**. Those two numbers are the
/// bible's §6.7 arithmetic and the entire curtain experiment.
#[unsafe(no_mangle)]
pub extern "C" fn room_solar_w(curtain: u32) -> f64 {
    let zone = reference_zone(curtain != 0);
    let outdoors = reference_outdoors();
    let g = zone.solar_gain(&outdoors);
    if g.is_finite() && g > 0.0 {
        g
    } else {
        0.0
    }
}

/// Solar gain as a fraction of the fully-open window, 0..1.
///
/// This is what the shader multiplies its sun radiance by, so the room's brightness IS the network's
/// ratio of two gains rather than a second constant. Closing the curtain gives 0.05/0.60 = **0.0833**,
/// which is the SHGC ratio and nothing else.
#[unsafe(no_mangle)]
pub extern "C" fn room_light_fraction(curtain: u32) -> f64 {
    let open = room_solar_w(0);
    if open <= 0.0 {
        return 0.0;
    }
    let here = room_solar_w(if curtain != 0 { 1 } else { 0 });
    (here / open).clamp(0.0, 1.0)
}

/// Cooling the AC must supply to hold the setpoint, W -- the quantity whose fall claim 2 is about.
#[unsafe(no_mangle)]
pub extern "C" fn room_cooling_w(curtain: u32) -> f64 {
    let zone = reference_zone(curtain != 0);
    let outdoors = reference_outdoors();
    let unit = odderon_astro::thermal::VapourCompression::window_unit();
    let w = unit.cooling_to_hold(&zone, &outdoors, &reference_internal());
    if w.is_finite() && w > 0.0 {
        w
    } else {
        0.0
    }
}

/// Electrical draw of the unit doing that cooling, W.
///
/// This is the number on a meter, and the one the bible says "visibly falls": **713 W to 304 W**, a 57%
/// drop, and *not* the 59% load ratio, because ASHRAE's cyclic degradation coefficient makes a cycling
/// unit worse at part load. If this ever returns a ratio equal to the load ratio, the degradation term
/// has been lost.
#[unsafe(no_mangle)]
pub extern "C" fn room_draw_w(curtain: u32) -> f64 {
    let zone = reference_zone(curtain != 0);
    let outdoors = reference_outdoors();
    let unit = odderon_astro::thermal::VapourCompression::window_unit();
    let cooling = unit.cooling_to_hold(&zone, &outdoors, &reference_internal());
    match unit.draw_for(cooling) {
        Some(w) if w.is_finite() && w > 0.0 => w,
        _ => 0.0,
    }
}

/// Newton iterations the propagator needed at `dt`.
///
/// A pure function of `dt` like the accessors above, so a caller gets the iteration count for the
/// propagation it just asked about. That matters because the universal-variable solver is iterative:
/// a device that reports convergence is reporting something a device that assumes it cannot.
///
/// `u32` rather than `usize` so the width does not depend on the target.
#[unsafe(no_mangle)]
pub extern "C" fn orbit_iterations(dt: f64) -> u32 {
    odderon_astro::propagate(&reference_state(), dt, odderon_astro::mu::EARTH).iterations
}

/// Non-zero if the propagation at `dt` failed to converge or overflowed.
///
/// `u32` rather than `bool` because `bool` is not a C-ABI type: its wasm representation is not
/// guaranteed to round-trip as 0/1, and a caller testing the wrong byte silently sees "converged".
#[unsafe(no_mangle)]
pub extern "C" fn orbit_unconverged(dt: f64) -> u32 {
    let p = odderon_astro::propagate(&reference_state(), dt, odderon_astro::mu::EARTH);
    u32::from(p.unconverged || p.overflowed)
}

/// Gravitational parameter of Earth, km³/s², as the kernel defines it.
///
/// The page reads `mu` from here rather than carrying its own copy: a page-local constant is a second
/// definition of Earth that can drift from the one every test is calibrated against.
#[unsafe(no_mangle)]
pub extern "C" fn orbit_mu() -> f64 {
    odderon_astro::mu::EARTH
}

/// Semi-major axis of the reference orbit, km.
#[unsafe(no_mangle)]
pub extern "C" fn orbit_semi_major_axis() -> f64 {
    A_KM
}

/// Eccentricity of the reference orbit.
#[unsafe(no_mangle)]
pub extern "C" fn orbit_eccentricity() -> f64 {
    ECC
}
