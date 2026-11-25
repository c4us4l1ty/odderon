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

// ---------------------------------------------------------------------------------------------
// THE CLOCK: what the room is doing a given number of seconds after the curtain came down.
// ---------------------------------------------------------------------------------------------
//
// The four functions above are STEADY-STATE numbers: flip the curtain and they snap to their new values.
// That is arithmetically right and it is not what happens to a room. `tests/thermal.rs` measures the
// other half -- **5.9 K in the first ten minutes** from a free-floating start, against a 44.4 min time
// constant -- and until now that trajectory was unreachable from the page: the room could be a
// calculator, not a simulation.
//
// ## Stateless on purpose
//
// `room_state_at(t)` RE-SIMULATES from the settled starting point every time it is called. It would be
// cheaper to keep the zone state in a `static mut` and step it, and it would be wrong for three reasons
// this project has already paid for:
//
//   1. **Determinism.** A mutable global makes the answer depend on call history, so the same `t` gives
//      different temperatures depending on how the host got there. Every other export here is a pure
//      function of its arguments; this one stays one.
//   2. **Replay.** A mission replay has to be able to ask "what was the room doing at t = 300 s" without
//      having simulated the 299 seconds before it.
//   3. **Seekability.** The page's clock can be stopped, restarted, or dragged, and nothing accumulates.
//
// The cost is `t / 30` Euler steps for a query at time `t` -- 20 for a ten-minute query, 240 for the
// longest the page can ask for (the two-hour cap). On a budget of one call per drawn frame, that is
// measured against the 1.5 ms frame budget, on the shipped module, at 20 000 calls per timing:
//
//     t =    60 s      2 steps     0.11 us
//     t =   600 s     20 steps     0.16 us
//     t =  1800 s     60 steps     0.64 us
//     t =  3600 s    120 steps     1.35 us
//     t =  7200 s    240 steps     2.57 us   (~11 ns per step)
//
// The worst case is **0.17 % of the budget**, and a whole 4800 s clock run draws roughly 1200 frames,
// so the trajectory costs about 3 ms in total across a run that takes 20 s of wall time. It is a real
// O(t/30), but the constant is small enough that the page does not pay for it, which is why the
// stateless design above is kept: determinism, replay and seekability are worth more than the 2.5 us
// the worst-case call costs. This is the measurement that closes the "should this be stepped instead"
// question -- it has been measured, and it stays a pure function.
//
// ## The step size is the model's, not a chosen one
//
// 30 s because `tests/thermal.rs` uses 30 s and asserts it is inside `max_stable_step()`. Taking the
// bound and dividing by two here instead would be a second, silently different discretisation of the
// same physics, which is the "two constants for one quantity" defect this file has already been
// corrected for once.

/// The step the trajectory is integrated at, s. The test suite's own.
const ROOM_STEP_S: f64 = 30.0;

/// The longest trajectory the page may ask for, s (two hours).
///
/// A host that passes a nonsense `t` gets a clamped answer rather than an unbounded loop: this is a
/// `no_std` cdylib whose only failure mode is a trap, and a trap in the renderer is a blank page.
const ROOM_MAX_T_S: f64 = 7200.0;

/// Sanity clamps on a caller-supplied starting temperature, kelvin (0.01 K to 2000 K).
///
/// The host sends a number it got from this module, so these should never bite. They exist because a
/// `no_std` cdylib's only failure mode is a trap: a host that passes `-1e9` or a stale pointer-decoded
/// value gets a clamped room rather than a NaN that spreads into the shader.
const ROOM_MIN_K: f64 = 0.01;
const ROOM_MAX_K: f64 = 2000.0;

/// Zone temperature `elapsed_s` seconds into the current curtain state, kelvin.
///
/// ## The room has a memory, and assuming otherwise was a visible lie
///
/// The first version of this function had no `from_k`: it started every trajectory at the SETTLED
/// temperature of the previous curtain state, on the argument that a settled room is where a real room is
/// when someone gets up and pulls the curtain. That is true the first time, and false every time after.
/// Measured on the page: close the curtain, let the room cool to 329.48 K, then reopen it -- the room
/// jumped to 346.82 K **in one frame**, a step of **17.34 K**, where a 120 kJ/K zone with UA 45 W/K takes
/// about forty minutes to get back. The model in `thermal.rs` has heat capacity; the presentation had
/// none. A room that cannot remember being cold is not a room with a thermal mass, it is a calculator
/// that reads the curtain position.
///
/// Worse, it was not merely a jump at the moment of the toggle: while the curtain was OPEN the page read
/// `crate::room_settled_k(0)` directly, so the room sat at its open resting temperature forever, mid-heat-up or
/// not. Every reopening discarded the cooling the user had just watched.
///
/// So the trajectory now takes its starting point as an ARGUMENT. `from_k <= 0` keeps the old behaviour
/// (start from the previous state's resting temperature) because that is the honest answer to "this room
/// has been sitting like this since forever", and it is what the first close should use.
/// `to_closed` is the state the room is moving INTO. The default starting point (when `from_k` is not
/// given) is the settled temperature of the state it is coming FROM, which is the other one.
///
/// The name is `to_closed` and not `was_closed` because the first version of this used `was_closed` and
/// built `reference_zone(!was_closed)` from it -- so `crate::room_temperature_from_k(1, ...)`, documented as "the
/// curtain is closing", actually simulated the OPEN room. Measured on the page before the fix: closing the
/// curtain made the temperature read 346.82 K (the settled OPEN room) and reopening it read 317.48 K (the
/// settled CLOSED one). Every direction was exactly backwards, and both values were plausible, which is
/// why the panel printed them without complaint.
fn room_temperature_after(to_closed: bool, from_k: f64, elapsed_s: f64) -> f64 {
    let outdoors = reference_outdoors();
    let internal = reference_internal();
    let start = if from_k.is_finite() && from_k > 0.0 {
        from_k
    } else {
        let before = reference_zone(!to_closed); // ... the state it is coming from
        match before.free_float_temperature(&outdoors, &internal) {
            Some(t) => t,
            None => return 0.0, // no conductance => no resting temperature => nothing to report
        }
    };
    let after = reference_zone(to_closed);

    // `Infinity` is a real reading and it means "fully settled", so it must clamp UP to the cap like any
    // other large finite time -- the old branch sent every non-finite value to 0.0, which inverted the
    // meaning: `crate::room_temperature_from_k(1, 300.0, f64::INFINITY)` returned the START temperature (300.0)
    // instead of the settled one (317.48 K), and it looked like a plausible number on the panel. Only
    // NaN has no reading; NaN.clamp is NaN, so it is the one case that needs the explicit floor.
    let t = if elapsed_s.is_nan() {
        0.0
    } else {
        elapsed_s.clamp(0.0, ROOM_MAX_T_S)
    };

    let mut state = odderon_astro::thermal::ZoneState {
        temperature_k: start.clamp(ROOM_MIN_K, ROOM_MAX_K),
    };
    let mut elapsed = 0.0;
    // `while elapsed + step <= t` rather than `while elapsed < t`: the last partial step would be a
    // different dt from every other one, and an explicit Euler step's error depends on dt. Better to
    // stop on the last full step than to mix step sizes in one trajectory.
    while elapsed + ROOM_STEP_S <= t {
        state = after.step(&state, &outdoors, &internal, 0.0, ROOM_STEP_S);
        elapsed += ROOM_STEP_S;
    }
    state.temperature_k
}

/// Zone temperature, kelvin, `elapsed_s` seconds after the curtain CLOSES from a settled open room.
///
/// `elapsed_s` is floored at 0 and capped at two hours (`ROOM_MAX_T_S`); see `room_temperature_after`.
/// Kept as the exact first-close trajectory, because it is the one `tests/thermal.rs` measures and the
/// one the 5.9 K figure belongs to. A room that has already moved should use `room_temperature_from_k`.
#[unsafe(no_mangle)]
pub extern "C" fn room_temperature_k(elapsed_s: f64) -> f64 {
    room_temperature_after(true, 0.0, elapsed_s)
}

/// Zone temperature `elapsed_s` seconds after the curtain moves, STARTING FROM `from_k`, kelvin.
///
/// This is the general trajectory and the one the page uses. `curtain` is the state the room is moving
/// INTO: 0 for open, non-zero for closed. `from_k <= 0` means "start from the previous state's resting
/// temperature", which is what a room that has been left alone looks like -- so a host that has no better
/// number is not forced to invent one.
///
/// Replaces the old `room_temperature_k` + read-`room_settled_k`-while-open arrangement, which was a
/// different law for each curtain position: a curve one way, a constant the other. Both halves are now
/// the same first-order response of the same zone, which is the only thing that makes reopening the
/// curtain the reverse of closing it.
#[unsafe(no_mangle)]
pub extern "C" fn room_temperature_from_k(curtain: u32, from_k: f64, elapsed_s: f64) -> f64 {
    // `curtain != 0` is "the room is moving into the closed state". Written with the C convention spelled
    // out, because the first version had this as `curtain == 0` and every trajectory ran backwards: a
    // closing curtain simulated the room WARMING to 323.42 K and an opening one simulated it COOLING.
    room_temperature_after(curtain != 0, from_k, elapsed_s)
}

/// The room's SETTLED temperature, kelvin, with the curtain in the given state.
///
/// This is the free-floating temperature the zone holds with that much sun coming in -- the room's
/// answer to "what does it sit at", as opposed to what it is doing on the way there.
#[unsafe(no_mangle)]
pub extern "C" fn room_settled_k(curtain: u32) -> f64 {
    let zone = reference_zone(curtain != 0);
    zone.free_float_temperature(&reference_outdoors(), &reference_internal())
        .unwrap_or(0.0)
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

#[cfg(test)]
mod tests {
    // The ABI is a contract a host can violate with a single bad number, and two of the values it can
    // send are not "large" but *non-finite*, which is a different failure: it is not an out-of-range
    // reading that a clamp should absorb, it is a reading with no meaning at all. These tests pin the
    // mapping so a future refactor cannot silently invert it.
    //
    // Written without float methods (`.abs()`, `.min()`, ...) because `ci/kernel-policy.sh` R1 scans this
    // whole file for host transcendentals and would reject the test module along with the kernel.

    /// Within a tolerance, either way. `a - b` and `b - a` rather than a method call.
    fn near(a: f64, b: f64) -> bool {
        (a - b) < 1e-9 && (b - a) < 1e-9
    }

    #[test]
    fn infinite_elapsed_means_fully_settled_not_the_start() {
        // The defect this test exists for: the old code sent every non-finite `elapsed_s` to 0.0, so
        // "infinitely long ago" and "just now" returned the SAME answer -- the start temperature, for a
        // call whose whole meaning is how far along the trajectory the room is.
        //
        // The trajectory is asymptotic, so the two-hour cap is close to but not exactly the settled
        // temperature; the property that matters is that Infinity reads like a very long time, not like
        // the start. Compare it against the same call at 1e9 s (finite, clamps to the cap) and against
        // the start temperature it must no longer equal.
        let at_infinity = crate::room_temperature_from_k(1, 300.0, f64::INFINITY);
        let at_huge_finite = crate::room_temperature_from_k(1, 300.0, 1e9);
        let at_zero = crate::room_temperature_from_k(1, 300.0, 0.0);
        assert!(
            near(at_infinity, at_huge_finite),
            "Infinity ({at_infinity}) must clamp to the cap like a finite time ({at_huge_finite})"
        );
        assert!(
            at_infinity - at_zero > 10.0,
            "Infinity returned the start ({at_infinity}); the clamp inverted the meaning"
        );
    }

    #[test]
    fn nan_elapsed_means_no_time_elapsed() {
        // NaN has no reading, so it floors to zero rather than to the cap. The point is that NaN and
        // Infinity must NOT collapse onto the same answer: that collapse is exactly what the bug did.
        let at_nan = crate::room_temperature_from_k(1, 300.0, f64::NAN);
        let at_zero = crate::room_temperature_from_k(1, 300.0, 0.0);
        assert!(
            near(at_nan, at_zero),
            "NaN elapsed should read as zero time"
        );
        assert!(
            at_nan - 300.0 < 1e-6 && 300.0 - at_nan < 1e-6,
            "NaN elapsed returned {at_nan}, expected the start"
        );
    }

    #[test]
    fn large_finite_and_negative_elapsed_clamp() {
        // 1e9 is finite, so it clamps to the two-hour cap; the cap is short of the asymptote, so this
        // asserts the cap is reached, not that the asymptote is.
        let at_cap = crate::room_temperature_from_k(1, 300.0, 7200.0);
        let big = crate::room_temperature_from_k(1, 300.0, 1e9);
        assert!(near(big, at_cap), "1e9 s should clamp to the two-hour cap");
        // Negative time is clamped to zero, not inverted.
        let neg = crate::room_temperature_from_k(1, 300.0, -1.0);
        let zero = crate::room_temperature_from_k(1, 300.0, 0.0);
        assert!(near(neg, zero), "negative elapsed should read as zero time");
    }

    #[test]
    fn non_finite_start_falls_back_to_the_previous_resting_temperature() {
        // A host that has no better number than NaN or infinity must not be forced to invent a
        // temperature: it starts from the state the room was already in.
        let from_nan = crate::room_temperature_from_k(1, f64::NAN, 600.0);
        let from_inf = crate::room_temperature_from_k(1, f64::INFINITY, 600.0);
        assert!(
            from_nan.is_finite() && from_nan > 0.0,
            "NaN from_k produced {from_nan}"
        );
        assert!(
            near(from_nan, from_inf),
            "NaN and Inf from_k must agree on the fallback"
        );
    }
}
