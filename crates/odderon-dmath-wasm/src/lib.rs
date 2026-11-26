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

// =============================================================================================
// The pad: a cryogenic fluid network and its reproducible failure
// =============================================================================================
//
// `odderon_astro::cryo` is a pure function of the input log and `t`, so these exports are stateless
// accessors over it -- the same shape as the room exports above. A refusal (a non-finite plan, fill rate
// or elapsed time) crosses the ABI as `NaN`, which the page reads as "no reading" rather than as a number,
// the same convention the room's trajectory uses for a missing kernel.
//
// The one export that carries the P1 gate is [`pad_failure_time`]: the simulated time at which the pump
// first cavitates, or -1.0 if the load runs clean. Same inputs, same answer, on every platform -- which is
// what makes the phase's "reproducible failure from the input log" checkable from outside Rust.

/// Vapour (saturation) pressure of LOX at `t_k`, kPa. `NaN` for a non-finite or non-positive temperature.
#[unsafe(no_mangle)]
pub extern "C" fn pad_vapour_pressure_kpa(t_k: f64) -> f64 {
    odderon_astro::cryo::vapour_pressure_kpa(t_k).unwrap_or(f64::NAN)
}

/// Residual heat per kilogram left in the line after a chilldown, J/kg. `NaN` for a non-finite plan.
#[unsafe(no_mangle)]
pub extern "C" fn pad_chilldown_residual_heat(chill_s: f64, bleeds: u32) -> f64 {
    let plan = odderon_astro::cryo::ChillPlan { chill_s, bleeds };
    odderon_astro::cryo::chilldown_residual_heat(&plan).unwrap_or(f64::NAN)
}

/// The mass fraction of incoming cryogen that flashes to vapour, 0..1. `NaN` for a non-finite input.
#[unsafe(no_mangle)]
pub extern "C" fn pad_flash_fraction(q_res: f64, margin: f64) -> f64 {
    odderon_astro::cryo::flash_fraction(q_res, margin).unwrap_or(f64::NAN)
}

/// The simulated time at which the pump first cavitates, or **-1.0** if the load runs clean to the end of
/// the window, or `NaN` if the run is refused (a non-finite plan, fill rate or elapsed time).
///
/// This is the P1 gate export. The bible's productive failure -- an under-specified chilldown froths, the
/// pump cavitates, the test article dies -- is a function of the input log, and this returns the instant it
/// happens. The same `(chill_s, bleeds, fill_rate, elapsed_s)` returns the same value on every platform.
#[unsafe(no_mangle)]
pub extern "C" fn pad_failure_time(
    chill_s: f64,
    bleeds: u32,
    fill_rate: f64,
    elapsed_s: f64,
) -> f64 {
    let net = odderon_astro::cryo::CryoNetwork::reference();
    let plan = odderon_astro::cryo::ChillPlan { chill_s, bleeds };
    match net.simulate(&plan, fill_rate, elapsed_s) {
        Some(out) => match out.failure {
            Some((_, t)) => t,
            None => -1.0,
        },
        None => f64::NAN,
    }
}

/// The pump inlet vapour fraction at the end of the run, 0..1. `NaN` if the run is refused.
///
/// Carried because a run that *did not* fail still shows how close it came, and "how close" is the quantity
/// the cavitation predicate reads.
#[unsafe(no_mangle)]
pub extern "C" fn pad_inlet_vapour_fraction(
    chill_s: f64,
    bleeds: u32,
    fill_rate: f64,
    elapsed_s: f64,
) -> f64 {
    let net = odderon_astro::cryo::CryoNetwork::reference();
    let plan = odderon_astro::cryo::ChillPlan { chill_s, bleeds };
    net.simulate(&plan, fill_rate, elapsed_s)
        .map(|o| o.pump_inlet_vapour_fraction)
        .unwrap_or(f64::NAN)
}

/// Liquid cryogen in the tank at the end of the run, kg. `NaN` if the run is refused.
#[unsafe(no_mangle)]
pub extern "C" fn pad_liquid_kg(chill_s: f64, bleeds: u32, fill_rate: f64, elapsed_s: f64) -> f64 {
    let net = odderon_astro::cryo::CryoNetwork::reference();
    let plan = odderon_astro::cryo::ChillPlan { chill_s, bleeds };
    net.simulate(&plan, fill_rate, elapsed_s)
        .map(|o| o.liquid_kg)
        .unwrap_or(f64::NAN)
}

/// Tank pressure at the end of the run, kPa. `NaN` if the run is refused.
#[unsafe(no_mangle)]
pub extern "C" fn pad_pressure_kpa(
    chill_s: f64,
    bleeds: u32,
    fill_rate: f64,
    elapsed_s: f64,
) -> f64 {
    let net = odderon_astro::cryo::CryoNetwork::reference();
    let plan = odderon_astro::cryo::ChillPlan { chill_s, bleeds };
    net.simulate(&plan, fill_rate, elapsed_s)
        .map(|o| o.pressure_kpa)
        .unwrap_or(f64::NAN)
}

/// Tank temperature at the end of the run, K. `NaN` if the run is refused.
#[unsafe(no_mangle)]
pub extern "C" fn pad_temperature_k(
    chill_s: f64,
    bleeds: u32,
    fill_rate: f64,
    elapsed_s: f64,
) -> f64 {
    let net = odderon_astro::cryo::CryoNetwork::reference();
    let plan = odderon_astro::cryo::ChillPlan { chill_s, bleeds };
    net.simulate(&plan, fill_rate, elapsed_s)
        .map(|o| o.temperature_k)
        .unwrap_or(f64::NAN)
}

/// Non-zero if this input log produces a **wreck** -- a failed cryo load, kept as a record on the stand.
///
/// The wreck is the bible's "the wreckage is always examinable": a failed run leaves a record (the input
/// log and the state it failed into) rather than being cut away. This is the predicate the page uses to
/// decide whether a fill leaves something to examine. `u32` rather than `bool` because `bool` is not a
/// C-ABI type; a refused run (a non-finite input) is NOT a wreck, so it returns 0 rather than recording a
/// failure that was never reached.
#[unsafe(no_mangle)]
pub extern "C" fn pad_is_wreck(chill_s: f64, bleeds: u32, fill_rate: f64, window_s: f64) -> u32 {
    let net = odderon_astro::cryo::CryoNetwork::reference();
    let plan = odderon_astro::cryo::ChillPlan { chill_s, bleeds };
    u32::from(net.is_wreck(&plan, fill_rate, window_s))
}

// --- P2: The Mark -------------------------------------------------------------------------------
//
// The grease-pencil mark, the ghost twin, the one-arrow attribution, and the Akashi design hash.
// These extend the P1 pad into the Loop of Consequence (`GAME-DESIGN-BIBLE.md` §6.1, §6.2, §4.4):
// the player commits an expectation (the mark), the kernel runs the real model, and the gap between
// the mark and the actual is the reward prediction error δ.
//
// The wasm C ABI only supports scalar returns, so each field of the ghost twin and the attribution
// is a separate export -- the same shape as the P1 pad accessors above. A refused computation (a
// non-finite expected value or a non-finite plan) crosses as `NaN` in every float field and `0` in
// every integer field, the same convention the P1 exports use.

/// The kernel's actual inlet vapour fraction for a grease-pencil mark's plan.
///
/// `expected` is the value the player marked; the actual is computed from the same plan, so the
/// divergence is a function of the mark and the plan, not of when the run happens. `NaN` if the
/// expected value is non-finite or the plan is refused.
#[unsafe(no_mangle)]
pub extern "C" fn pad_ghost_twin_actual(
    chill_s: f64,
    bleeds: u32,
    expected: f64,
    fill_rate: f64,
    window_s: f64,
) -> f64 {
    let net = odderon_astro::cryo::CryoNetwork::reference();
    let plan = odderon_astro::cryo::ChillPlan { chill_s, bleeds };
    let mark = odderon_astro::cryo::Mark::inlet(plan, expected);
    odderon_astro::cryo::ghost_twin(&mark, &net, fill_rate, window_s)
        .map(|g| g.actual)
        .unwrap_or(f64::NAN)
}

/// The ghost twin's divergence δ = actual - expected, for a grease-pencil mark.
///
/// `NaN` if the ghost twin could not be computed (a non-finite expected value or a refused plan).
#[unsafe(no_mangle)]
pub extern "C" fn pad_ghost_twin_divergence(
    chill_s: f64,
    bleeds: u32,
    expected: f64,
    fill_rate: f64,
    window_s: f64,
) -> f64 {
    let net = odderon_astro::cryo::CryoNetwork::reference();
    let plan = odderon_astro::cryo::ChillPlan { chill_s, bleeds };
    let mark = odderon_astro::cryo::Mark::inlet(plan, expected);
    odderon_astro::cryo::ghost_twin(&mark, &net, fill_rate, window_s)
        .map(|g| g.divergence)
        .unwrap_or(f64::NAN)
}

/// The one-arrow attribution verdict for a grease-pencil mark.
///
/// 0 = none (no divergence, or the flash is clamped at a rail), 1 = chill time, 2 = bleeds. The
/// arrow names the plan input that most contributes to the divergence between the mark and the
/// actual reading.
#[unsafe(no_mangle)]
pub extern "C" fn pad_attribution_which(
    chill_s: f64,
    bleeds: u32,
    expected: f64,
    fill_rate: f64,
    window_s: f64,
) -> u32 {
    let net = odderon_astro::cryo::CryoNetwork::reference();
    let plan = odderon_astro::cryo::ChillPlan { chill_s, bleeds };
    let mark = odderon_astro::cryo::Mark::inlet(plan, expected);
    match odderon_astro::cryo::attribution(&mark, &net, fill_rate, window_s).which {
        odderon_astro::cryo::Attribution::None => 0,
        odderon_astro::cryo::Attribution::ChillTime => 1,
        odderon_astro::cryo::Attribution::Bleeds => 2,
    }
}

/// |∂actual/∂chill_s|, the sensitivity of the actual reading to chill time, for a grease-pencil mark.
///
/// `NaN` if the attribution could not be computed.
#[unsafe(no_mangle)]
pub extern "C" fn pad_attribution_sensitivity_chill(
    chill_s: f64,
    bleeds: u32,
    expected: f64,
    fill_rate: f64,
    window_s: f64,
) -> f64 {
    let net = odderon_astro::cryo::CryoNetwork::reference();
    let plan = odderon_astro::cryo::ChillPlan { chill_s, bleeds };
    let mark = odderon_astro::cryo::Mark::inlet(plan, expected);
    odderon_astro::cryo::attribution(&mark, &net, fill_rate, window_s).sensitivity_chill
}

/// |∂actual/∂bleeds|, the sensitivity of the actual reading to purge cycles, for a grease-pencil mark.
///
/// `NaN` if the attribution could not be computed.
#[unsafe(no_mangle)]
pub extern "C" fn pad_attribution_sensitivity_bleeds(
    chill_s: f64,
    bleeds: u32,
    expected: f64,
    fill_rate: f64,
    window_s: f64,
) -> f64 {
    let net = odderon_astro::cryo::CryoNetwork::reference();
    let plan = odderon_astro::cryo::ChillPlan { chill_s, bleeds };
    let mark = odderon_astro::cryo::Mark::inlet(plan, expected);
    odderon_astro::cryo::attribution(&mark, &net, fill_rate, window_s).sensitivity_bleeds
}

/// The deterministic design hash of the pad kernel's constants.
///
/// An Akashi entry is stamped with this so a reader can tell which kernel version produced it. It is
/// a pure function: the same kernel produces the same hash on every platform, every run.
#[unsafe(no_mangle)]
pub extern "C" fn pad_design_hash() -> u64 {
    odderon_astro::cryo::design_hash()
}

// =============================================================================================
// The sounding rocket: a suborbital flight from a surface launch
// =============================================================================================
//
// `odderon_astro::rocket` is a pure function of the authored launch parameters and `t`, so these exports
// are stateless accessors over it -- the same shape as the room and pad exports above. The four launch
// parameters cross the ABI as scalars (the C ABI cannot return a struct), and a refusal (a non-finite
// or non-positive launch) crosses as `NaN` in every float field and `0` in the verdict, the same
// convention the pad uses.
//
// The trajectory itself is the gate to leave P3: bit-identical replay in a second browser. The position
// at time `t` is the same on every platform, every run, for the same parameters -- which is what makes
// that claim checkable from outside Rust. The readings (apogee, impact time, range, verdict) are the
// numbers the player reads to judge the flight the parameters produce.

/// Build a [`LaunchParams`] from the four scalar launch inputs the page authors.
///
/// The single place the four ABI arguments become a kernel struct, so the eight accessors below cannot
/// describe four different launches.
fn launch_params(lat: f64, az: f64, el: f64, v: f64) -> odderon_astro::rocket::LaunchParams {
    odderon_astro::rocket::LaunchParams {
        latitude_rad: lat,
        azimuth_rad: az,
        elevation_rad: el,
        burnout_velocity_km_s: v,
    }
}

/// `x` of the rocket position at time `t` seconds after launch, km.
///
/// The rocket's position at time `t`, for the authored launch. `NaN` if the launch is refused (a
/// non-finite or non-positive parameter) or the trajectory has not been modelled at that time.
#[unsafe(no_mangle)]
pub extern "C" fn rocket_state_x(lat: f64, az: f64, el: f64, v: f64, t: f64) -> f64 {
    odderon_astro::rocket::rocket_state_at(&launch_params(lat, az, el, v), t)
        .r
        .x
}

/// `y` of the rocket position, km. See [`rocket_state_x`].
#[unsafe(no_mangle)]
pub extern "C" fn rocket_state_y(lat: f64, az: f64, el: f64, v: f64, t: f64) -> f64 {
    odderon_astro::rocket::rocket_state_at(&launch_params(lat, az, el, v), t)
        .r
        .y
}

/// `z` of the rocket position, km. See [`rocket_state_x`].
#[unsafe(no_mangle)]
pub extern "C" fn rocket_state_z(lat: f64, az: f64, el: f64, v: f64, t: f64) -> f64 {
    odderon_astro::rocket::rocket_state_at(&launch_params(lat, az, el, v), t)
        .r
        .z
}

/// Apogee altitude above the surface, km, for the authored launch.
///
/// `NaN` if the launch is refused or the trajectory escapes (no apogee).
#[unsafe(no_mangle)]
pub extern "C" fn rocket_apogee_km(lat: f64, az: f64, el: f64, v: f64) -> f64 {
    odderon_astro::rocket::rocket_flight(&launch_params(lat, az, el, v)).apogee_km
}

/// Seconds from launch to apogee, for the authored launch. `NaN` if refused or escaping.
#[unsafe(no_mangle)]
pub extern "C" fn rocket_time_to_apogee_s(lat: f64, az: f64, el: f64, v: f64) -> f64 {
    odderon_astro::rocket::rocket_flight(&launch_params(lat, az, el, v)).time_to_apogee_s
}

/// Seconds from launch to surface impact, for the authored launch.
///
/// `NaN` if refused, escaping, or orbital (no impact).
#[unsafe(no_mangle)]
pub extern "C" fn rocket_impact_time_s(lat: f64, az: f64, el: f64, v: f64) -> f64 {
    odderon_astro::rocket::rocket_flight(&launch_params(lat, az, el, v)).impact_time_s
}

/// Ground range from launch to impact, km, for the authored launch.
///
/// `NaN` if refused, escaping, or orbital (no impact). A vertical launch has zero range.
#[unsafe(no_mangle)]
pub extern "C" fn rocket_range_km(lat: f64, az: f64, el: f64, v: f64) -> f64 {
    odderon_astro::rocket::rocket_flight(&launch_params(lat, az, el, v)).range_km
}

/// The flight verdict for the authored launch, as a `u32`.
///
/// 0 = Suborbital, 1 = Orbital, 2 = Escape, 3 = Refused. `u32` rather than `bool` because `bool` is
/// not a C-ABI type; a refused launch (a non-finite or non-positive parameter) returns 3, so a host
/// can distinguish "no flight" from "a flight of some kind".
#[unsafe(no_mangle)]
pub extern "C" fn rocket_verdict(lat: f64, az: f64, el: f64, v: f64) -> u32 {
    match odderon_astro::rocket::rocket_flight(&launch_params(lat, az, el, v)).verdict {
        odderon_astro::rocket::RocketVerdict::Suborbital => 0,
        odderon_astro::rocket::RocketVerdict::Orbital => 1,
        odderon_astro::rocket::RocketVerdict::Escape => 2,
        odderon_astro::rocket::RocketVerdict::Refused => 3,
    }
}

/// The deterministic design hash of the rocket kernel's constants.
///
/// Pins the Kármán line, Earth's radius, and Earth's gravitational parameter, so a replay bundle or
/// Akashi entry can detect a stale kernel. A pure function: the same kernel produces the same hash on
/// every platform, every run.
#[unsafe(no_mangle)]
pub extern "C" fn rocket_design_hash() -> u64 {
    odderon_astro::rocket::rocket_design_hash()
}

// =============================================================================================
// The orbit mission: ground-station pass prediction
// =============================================================================================
//
// `odderon_astro::observe` (ground stations) and `odderon_astro::passes` (pass prediction) are the
// deepest, most-validated part of the astro kernel -- 18 and 17 tests respectively, each keyed to a
// measured number -- and until now **no pixel ever read them**. The page had an orbiting body and no
// way to answer the question every orbital mission starts with: *when will it be over my station?*
// These exports are the seam.
//
// The mission is fixed (the reference orbit) and the player authors the ground station: geodetic
// latitude and longitude. The kernel predicts the next pass of the orbiting body over that station --
// rise, culmination, set, maximum elevation, minimum range, rise/set azimuth -- searching from
// `from` (the orbit clock, seconds past the reference epoch) onward. The returned times are absolute
// seconds past the same epoch, so the page's orbit clock and the pass window share one timeline.
//
// Every reading is a pure function of `(lat, lon, from)`: the same station and time give the same pass
// on every platform, every run. That is the P3 replay claim, and it is what makes the prediction
// checkable from outside Rust.
//
// A pass that does not exist is reported as `exists = 0` with `NaN` readings; a question that cannot
// be answered honestly -- a site the ellipsoid refuses, or a search window too short for the orbit --
// is `exists = 2`. Both are refusals, not answers, the same rule the rocket's verdict follows.

/// Minimum elevation, degrees, for a period to count as a pass. The standard link-feasibility mask:
/// a 10° mask avoids low-elevation passes whose geometry is poor. For 400 km this is the difference
/// between a 398 s pass and a 652 s one at the horizon.
const PASS_MASK_DEG: f64 = 10.0;

/// How far ahead to search for the next pass, seconds. Six hours covers roughly two reference-orbit
/// periods -- comfortably past the guaranteed scan window, so `Ok(None)` means "no pass in six hours"
/// and not "the search was too short to look properly."
const PASS_WINDOW_S: f64 = 21_600.0;

/// The scan step, seconds. Must be at most the orbit's minimum pass duration; 30 s steps clean over
/// no pass for this orbit while keeping the search cheap enough to run on a station edit.
const PASS_STEP_S: f64 = 30.0;

/// `0` = no pass, `1` = a pass was found, `2` = the question could not be answered honestly.
fn pass_status(lat: f64, lon: f64, from: f64) -> u32 {
    match next_pass_window(lat, lon, from) {
        Ok(Some(_)) => 1,
        Ok(None) => 0,
        Err(_) => 2,
    }
}

/// The next pass window for the authored station, searched from `from`.
///
/// The site is built through the kernel's own `Site::from_geodetic` (which solves the ellipsoid
/// properly, with `up` as the geodetic normal) and the search through `passes::next_pass` (which
/// includes the oblateness). Composing the two here rather than re-deriving the geometry keeps the
/// prediction identical to the one the astro tests validated.
fn next_pass_window(
    lat: f64,
    lon: f64,
    from: f64,
) -> Result<Option<odderon_astro::passes::PassWindow>, u32> {
    let site = odderon_astro::observe::Site::from_geodetic(
        &odderon_astro::catalogue::EARTH,
        lat,
        lon,
        0.0,
    )
    .map_err(|_| 2u32)?;
    odderon_astro::passes::next_pass(
        &odderon_astro::catalogue::EARTH,
        &site,
        &odderon_astro::catalogue::EARTH_FRAME,
        &reference_state(),
        0.0,
        odderon_astro::mu::EARTH,
        PASS_MASK_DEG,
        from,
        PASS_WINDOW_S,
        PASS_STEP_S,
    )
    .map_err(|_| 2u32)
}

/// `1` if the orbiting body passes over the authored station after `from`, else `0` (no pass) or `2`
/// (the question could not be answered). The gate the page reads before believing any field.
#[unsafe(no_mangle)]
pub extern "C" fn pass_next_exists(lat: f64, lon: f64, from: f64) -> u32 {
    pass_status(lat, lon, from)
}

/// Seconds past the reference epoch when the pass rises above the mask, or `NaN` if there is no pass.
#[unsafe(no_mangle)]
pub extern "C" fn pass_next_rise_s(lat: f64, lon: f64, from: f64) -> f64 {
    match next_pass_window(lat, lon, from) {
        Ok(Some(w)) => w.rise,
        _ => f64::NAN,
    }
}

/// Seconds past the reference epoch of maximum elevation, refined inside the pass. Never a sample time.
#[unsafe(no_mangle)]
pub extern "C" fn pass_next_culmination_s(lat: f64, lon: f64, from: f64) -> f64 {
    match next_pass_window(lat, lon, from) {
        Ok(Some(w)) => w.culmination,
        _ => f64::NAN,
    }
}

/// Seconds past the reference epoch when the pass sets below the mask, or `NaN` if there is no pass.
#[unsafe(no_mangle)]
pub extern "C" fn pass_next_set_s(lat: f64, lon: f64, from: f64) -> f64 {
    match next_pass_window(lat, lon, from) {
        Ok(Some(w)) => w.set,
        _ => f64::NAN,
    }
}

/// Maximum elevation during the pass, degrees. Always above the mask by construction.
#[unsafe(no_mangle)]
pub extern "C" fn pass_next_max_elevation_deg(lat: f64, lon: f64, from: f64) -> f64 {
    match next_pass_window(lat, lon, from) {
        Ok(Some(w)) => w.max_elevation_deg,
        _ => f64::NAN,
    }
}

/// Closest approach during the pass, km. Always below the orbit's altitude by construction.
#[unsafe(no_mangle)]
pub extern "C" fn pass_next_min_range_km(lat: f64, lon: f64, from: f64) -> f64 {
    match next_pass_window(lat, lon, from) {
        Ok(Some(w)) => w.min_range_km,
        _ => f64::NAN,
    }
}

/// Azimuth at rise, degrees clockwise from north.
#[unsafe(no_mangle)]
pub extern "C" fn pass_next_rise_az_deg(lat: f64, lon: f64, from: f64) -> f64 {
    match next_pass_window(lat, lon, from) {
        Ok(Some(w)) => w.rise_azimuth_deg,
        _ => f64::NAN,
    }
}

/// Azimuth at set, degrees clockwise from north.
#[unsafe(no_mangle)]
pub extern "C" fn pass_next_set_az_deg(lat: f64, lon: f64, from: f64) -> f64 {
    match next_pass_window(lat, lon, from) {
        Ok(Some(w)) => w.set_azimuth_deg,
        _ => f64::NAN,
    }
}

// =============================================================================================
// The orbit mission: light-time comms
// =============================================================================================
//
// The pass exports above answer *when* the body is over the station. Light-time answers the other
// half of the causal-mail claim (`GAME-DESIGN-BIBLE.md` §11.2): a message does not arrive when it is
// sent, it arrives after it crosses the distance at the speed of light. For an orbital body that
// distance is hundreds of kilometres, so the delay is real and measurable -- and it is the quantity
// a controller actually feels, which is why round-trip is exported alongside one-way.
//
// The composition is the same shape as the pass exports: the player authors the ground station
// (geodetic lat/lon), the kernel builds the site through `Site::from_geodetic`, rotates it into the
// inertial frame at the orbit clock `from`, takes the orbiting body's position at the same instant
// from the same reference orbit, and divides the distance by the one defined speed of light. Both
// positions are in the Earth-centered inertial frame, so the distance is a straight line and the
// delay is a pure function of `(lat, lon, from)` -- the P3 replay claim.
//
// A link that cannot be computed (a site the ellipsoid refuses, a non-finite position, a zero
// distance) is reported as `NaN` readings with `comms_link_status = 0` or `2`, never as a number the
// page would print as a delay.

/// One-way light time between two positions in the same inertial frame, seconds.
///
/// The primitive the whole comms link rests on: Euclidean distance divided by the exact, defined speed
/// of light. Returns `NaN` when the distance is not strictly positive and finite (NaN/infinite input,
/// or two coincident positions) -- a refusal, not a delay.
#[unsafe(no_mangle)]
pub extern "C" fn light_time_s(fx: f64, fy: f64, fz: f64, tx: f64, ty: f64, tz: f64) -> f64 {
    let from = odderon_astro::Vec3::new(fx, fy, fz);
    let to = odderon_astro::Vec3::new(tx, ty, tz);
    match odderon_astro::lighttime::light_time_s(from, to) {
        Some(t) => t,
        None => f64::NAN,
    }
}

/// `0` = no link (the light time could not be computed), `1` = the link is computable,
/// `2` = the station could not be built. The gate the page reads before believing any comms field.
fn comms_status(lat: f64, lon: f64, from: f64) -> u32 {
    match station_body_light_time_s(lat, lon, from) {
        Some(_) => 1,
        None => {
            // Distinguish "the site was refused" from "the distance was refused": only the first is a
            // station the player can fix by moving it.
            if site_is_refused(lat, lon) {
                2
            } else {
                0
            }
        }
    }
}

/// Whether the geodetic site itself could not be built (the ellipsoid refused it).
fn site_is_refused(lat: f64, lon: f64) -> bool {
    odderon_astro::observe::Site::from_geodetic(&odderon_astro::catalogue::EARTH, lat, lon, 0.0)
        .is_err()
}

/// The one-way light time from the authored ground station to the orbiting body, seconds, at orbit
/// clock `from`. Both positions are taken at the same instant in the inertial frame. `None` when the
/// link cannot be computed.
fn station_body_light_time_s(lat: f64, lon: f64, from: f64) -> Option<f64> {
    let site = odderon_astro::observe::Site::from_geodetic(
        &odderon_astro::catalogue::EARTH,
        lat,
        lon,
        0.0,
    )
    .ok()?;
    let station = site.inertial_position(&odderon_astro::catalogue::EARTH_FRAME, from)?;
    let body = state_at(from).r;
    odderon_astro::lighttime::light_time_s(station, body)
}

/// `1` if the orbiting body is reachable from the authored station at `from` (a finite light time),
/// else `0` (no link) or `2` (the station could not be built).
#[unsafe(no_mangle)]
pub extern "C" fn comms_link_status(lat: f64, lon: f64, from: f64) -> u32 {
    comms_status(lat, lon, from)
}

/// One-way light time from the authored station to the orbiting body at `from`, seconds, or `NaN`.
#[unsafe(no_mangle)]
pub extern "C" fn station_light_time_s(lat: f64, lon: f64, from: f64) -> f64 {
    station_body_light_time_s(lat, lon, from).unwrap_or(f64::NAN)
}

/// Round-trip light time: the signal to the body and the reply back, seconds, or `NaN`. The quantity
/// a real comms link is sized against -- the latency a controller feels, not the one-way figure.
#[unsafe(no_mangle)]
pub extern "C" fn comms_round_trip_s(lat: f64, lon: f64, from: f64) -> f64 {
    match station_body_light_time_s(lat, lon, from) {
        Some(t) => 2.0 * t,
        None => f64::NAN,
    }
}

/// The arrival time on the orbit clock of a message sent at `from` from the authored station to the
/// orbiting body, seconds, or `NaN`. The arrival is `from + light_time`, the pure function the
/// light-cone ledger is built from.
#[unsafe(no_mangle)]
pub extern "C" fn comms_arrival_s(lat: f64, lon: f64, from: f64) -> f64 {
    match station_body_light_time_s(lat, lon, from) {
        Some(t) => from + t,
        None => f64::NAN,
    }
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
