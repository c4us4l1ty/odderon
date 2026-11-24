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
