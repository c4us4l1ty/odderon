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

// ---------------------------------------------------------------------------------------------------
// Causal mail — other ships in the sky, messages at the speed of light (bible §11.2)
// ---------------------------------------------------------------------------------------------------
//
// "Other players' ships appear in your sky as objects with real orbital elements, and their messages
// arrive with real light-time delay, because the kernel is deterministic and the universe is a
// function of time. No netcode, no lobbies, no tick reconciliation."
//
// The peer ships are not the reference orbit: each carries its own osculating elements, and its
// position at any instant is the kernel propagating those elements from the reference epoch. A
// message a peer sends at orbit-clock `send_s` reaches the station at `send_s + light_time`, where
// the light-time is the *same primitive* the comms link uses -- Euclidean distance over the exact
// speed of light -- between the peer's propagated position and the station's inertial position. The
// page computes no light-time and holds no speed-of-light constant; the arrival is a pure function
// of (peer elements, station, send_s), so the same send lands the same way on every platform.
//
// The position exports are exposed alongside the arrival so the mail light-time is *verifiable*
// against the primitive [`light_time_s`] rather than trusted as one opaque number: a verifier can
// re-derive it from the peer's and the station's positions and compare. That is the P4 gate.

/// Build a peer ship's state from real orbital elements and propagate it by `dt` seconds past the
/// reference epoch. `None` when the elements do not describe a finite conic -- the kernel refuses
/// rather than guessing a position. Shared by the position exports and the causal-mail arrival.
fn peer_state(
    p: f64,
    e: f64,
    i: f64,
    raan: f64,
    argp: f64,
    nu: f64,
    dt: f64,
) -> Option<odderon_astro::State> {
    let elements = odderon_astro::Elements {
        p,
        e,
        i,
        raan,
        argp,
        nu,
        degeneracy: odderon_astro::Degeneracy::default(),
    };
    let state = elements.to_state(odderon_astro::mu::EARTH);
    // `to_state` returns a NaN state when the conic is invalid (a non-positive `p`). Propagating
    // that would carry NaN through the solver and return a position with no meaning. The check is
    // written as `x != x` -- true only for NaN -- rather than a host method, so it stays inside the
    // kernel policy. Refuse here, at the boundary, so the position exports return NaN.
    if state.r.x != state.r.x {
        return None;
    }
    Some(odderon_astro::propagate(&state, dt, odderon_astro::mu::EARTH).state)
}

/// The authored ground station's inertial position at orbit clock `from`, km. `None` when the
/// geodetic site cannot be built. Exposed so the causal-mail light-time is verifiable against the
/// primitive [`light_time_s`] rather than trusted as a single opaque number.
fn station_position(lat: f64, lon: f64, from: f64) -> Option<odderon_astro::Vec3> {
    let site = odderon_astro::observe::Site::from_geodetic(
        &odderon_astro::catalogue::EARTH,
        lat,
        lon,
        0.0,
    )
    .ok()?;
    site.inertial_position(&odderon_astro::catalogue::EARTH_FRAME, from)
}

/// `x` of a peer ship's position, propagated from its orbital elements by `dt` seconds past the
/// reference epoch, km. `NaN` when the elements do not describe a finite conic. See [`peer_state`].
#[unsafe(no_mangle)]
pub extern "C" fn peer_pos_x(
    p: f64,
    e: f64,
    i: f64,
    raan: f64,
    argp: f64,
    nu: f64,
    dt: f64,
) -> f64 {
    peer_state(p, e, i, raan, argp, nu, dt)
        .map(|s| s.r.x)
        .unwrap_or(f64::NAN)
}

/// `y` of a peer ship's position, km. See [`peer_pos_x`].
#[unsafe(no_mangle)]
pub extern "C" fn peer_pos_y(
    p: f64,
    e: f64,
    i: f64,
    raan: f64,
    argp: f64,
    nu: f64,
    dt: f64,
) -> f64 {
    peer_state(p, e, i, raan, argp, nu, dt)
        .map(|s| s.r.y)
        .unwrap_or(f64::NAN)
}

/// `z` of a peer ship's position, km. See [`peer_pos_x`].
#[unsafe(no_mangle)]
pub extern "C" fn peer_pos_z(
    p: f64,
    e: f64,
    i: f64,
    raan: f64,
    argp: f64,
    nu: f64,
    dt: f64,
) -> f64 {
    peer_state(p, e, i, raan, argp, nu, dt)
        .map(|s| s.r.z)
        .unwrap_or(f64::NAN)
}

/// `x` of a peer ship's velocity, km/s. `NaN` when the state cannot be built. See [`peer_pos_x`].
#[unsafe(no_mangle)]
pub extern "C" fn peer_vel_x(
    p: f64,
    e: f64,
    i: f64,
    raan: f64,
    argp: f64,
    nu: f64,
    dt: f64,
) -> f64 {
    peer_state(p, e, i, raan, argp, nu, dt)
        .map(|s| s.v.x)
        .unwrap_or(f64::NAN)
}

/// `y` of a peer ship's velocity, km/s. `NaN` when the state cannot be built. See [`peer_pos_x`].
#[unsafe(no_mangle)]
pub extern "C" fn peer_vel_y(
    p: f64,
    e: f64,
    i: f64,
    raan: f64,
    argp: f64,
    nu: f64,
    dt: f64,
) -> f64 {
    peer_state(p, e, i, raan, argp, nu, dt)
        .map(|s| s.v.y)
        .unwrap_or(f64::NAN)
}

/// `z` of a peer ship's velocity, km/s. `NaN` when the state cannot be built. See [`peer_pos_x`].
#[unsafe(no_mangle)]
pub extern "C" fn peer_vel_z(
    p: f64,
    e: f64,
    i: f64,
    raan: f64,
    argp: f64,
    nu: f64,
    dt: f64,
) -> f64 {
    peer_state(p, e, i, raan, argp, nu, dt)
        .map(|s| s.v.z)
        .unwrap_or(f64::NAN)
}

/// `x` of the authored ground station's inertial position at `from`, km. `NaN` when the site cannot
/// be built. See [`station_position`].
#[unsafe(no_mangle)]
pub extern "C" fn station_pos_x(lat: f64, lon: f64, from: f64) -> f64 {
    station_position(lat, lon, from)
        .map(|v| v.x)
        .unwrap_or(f64::NAN)
}

/// `y` of the station's inertial position, km. See [`station_pos_x`].
#[unsafe(no_mangle)]
pub extern "C" fn station_pos_y(lat: f64, lon: f64, from: f64) -> f64 {
    station_position(lat, lon, from)
        .map(|v| v.y)
        .unwrap_or(f64::NAN)
}

/// `z` of the station's inertial position, km. See [`station_pos_x`].
#[unsafe(no_mangle)]
pub extern "C" fn station_pos_z(lat: f64, lon: f64, from: f64) -> f64 {
    station_position(lat, lon, from)
        .map(|v| v.z)
        .unwrap_or(f64::NAN)
}

/// The one-way light-time from a peer ship to the authored ground station, seconds. The peer's
/// position is propagated from its orbital elements at `send_s`; the station's position is taken at
/// the same instant; the delay is the primitive [`light_time_s`]. `NaN` when either position is
/// unavailable or the distance is not strictly positive and finite.
fn mail_light_time_opt(
    p: f64,
    e: f64,
    i: f64,
    raan: f64,
    argp: f64,
    nu: f64,
    lat: f64,
    lon: f64,
    send_s: f64,
) -> Option<f64> {
    let peer = peer_state(p, e, i, raan, argp, nu, send_s)?;
    let station = station_position(lat, lon, send_s)?;
    odderon_astro::lighttime::light_time_s(peer.r, station)
}

/// The one-way light-time from a peer ship to the station, seconds, or `NaN`. See
/// [`mail_light_time_opt`].
#[unsafe(no_mangle)]
pub extern "C" fn mail_light_time_s(
    p: f64,
    e: f64,
    i: f64,
    raan: f64,
    argp: f64,
    nu: f64,
    lat: f64,
    lon: f64,
    send_s: f64,
) -> f64 {
    mail_light_time_opt(p, e, i, raan, argp, nu, lat, lon, send_s).unwrap_or(f64::NAN)
}

/// The arrival time on the orbit clock of a message a peer sent at `send_s`: `send_s` plus the
/// kernel's light-time. The pure function the causal-mail ledger is built from -- a message can
/// never arrive before it was sent, because the light-time is non-negative by construction. `NaN`
/// when the link cannot be computed.
#[unsafe(no_mangle)]
pub extern "C" fn mail_arrival_s(
    p: f64,
    e: f64,
    i: f64,
    raan: f64,
    argp: f64,
    nu: f64,
    lat: f64,
    lon: f64,
    send_s: f64,
) -> f64 {
    match mail_light_time_opt(p, e, i, raan, argp, nu, lat, lon, send_s) {
        Some(t) => send_s + t,
        None => f64::NAN,
    }
}

// =============================================================================================
// Deeper light-cone comms (Block N4: the causal structure beyond light-time)
// =============================================================================================

/// The radius of the light cone after `dt_s` seconds, in km: `c * dt_s`.
///
/// The region a signal can reach from a point after time `dt_s`. `NaN` for a non-finite or
/// non-positive time.
#[unsafe(no_mangle)]
pub extern "C" fn light_cone_radius_km(dt_s: f64) -> f64 {
    odderon_astro::lightcone::light_cone_radius_km(dt_s).unwrap_or(f64::NAN)
}

/// Whether a position at distance `r_km` is within the light cone after `dt_s` seconds.
///
/// `1` if reachable (a signal sent now can arrive), `0` if not. `0` for non-finite or
/// non-positive inputs.
#[unsafe(no_mangle)]
pub extern "C" fn causal_reachable(r_km: f64, dt_s: f64) -> u32 {
    if odderon_astro::lightcone::causal_reachable(r_km, dt_s) {
        1
    } else {
        0
    }
}

/// The Euclidean magnitude of a displacement vector given its Cartesian components, in km.
/// `NaN` for non-finite components or a non-finite magnitude.
#[unsafe(no_mangle)]
pub extern "C" fn signal_distance_km(dx: f64, dy: f64, dz: f64) -> f64 {
    odderon_astro::lightcone::signal_distance_km(dx, dy, dz).unwrap_or(f64::NAN)
}

/// The relativistic Doppler factor: `f_received / f_emitted` for a source moving at
/// `v_km_s` relative to the observer, with the signal at angle `theta_rad` to the velocity
/// vector. `NaN` for non-finite inputs or a velocity at or above the speed of light.
#[unsafe(no_mangle)]
pub extern "C" fn doppler_factor(v_km_s: f64, theta_rad: f64) -> f64 {
    odderon_astro::lightcone::doppler_factor(v_km_s, theta_rad).unwrap_or(f64::NAN)
}

/// The relativistic Doppler factor computed directly from a velocity vector and a line-of-sight
/// direction vector, so the caller supplies raw geometry and the kernel owns every trigonometric
/// and relativistic step. `NaN` for non-finite components, zero magnitudes, or a speed at or above
/// the speed of light. See `doppler_factor` for the formula.
#[unsafe(no_mangle)]
pub extern "C" fn doppler_factor_from_vecs(
    vx: f64,
    vy: f64,
    vz: f64,
    lx: f64,
    ly: f64,
    lz: f64,
) -> f64 {
    odderon_astro::lightcone::doppler_factor_from_vecs(vx, vy, vz, lx, ly, lz).unwrap_or(f64::NAN)
}

/// The inverse-square attenuation: the fraction of transmitted signal power that survives at
/// distance `d_km`, relative to the reference distance `d_ref_km`. `NaN` for non-finite or
/// non-positive distances.
#[unsafe(no_mangle)]
pub extern "C" fn signal_power_ratio(d_km: f64, d_ref_km: f64) -> f64 {
    odderon_astro::lightcone::signal_power_ratio(d_km, d_ref_km).unwrap_or(f64::NAN)
}

/// The deterministic design hash of the light-cone module's constants.
///
/// Folds in the speed of light (reused from `lighttime`, not duplicated) and the inverse-square
/// exponent, so a replay bundle or an Akashi entry can detect a stale light-cone kernel.
#[unsafe(no_mangle)]
pub extern "C" fn lightcone_design_hash() -> u64 {
    odderon_astro::lightcone::lightcone_design_hash()
}

// =============================================================================================
// The mission economy and Esse (P5: the career core, bible §3.2 / §4.3 / §12.1)
// =============================================================================================
//
// `odderon_astro::mission` is the career core of the kernel: the economy (§12.1 — "money is
// physics, not a difficulty knob") and the Esse calibration score (§4.3 — proper scoring rules,
// never accuracy %). These exports are the seam the page's mission loop reads through.
//
// The economy: a contract's price is a pure function of the work it asks for — the Δv budget, the
// mass, the risk, the window — computed here from pinned constants. The page holds no price formula
// and no price constant; a verifier, given the contract's parameters, calls `contract_price` and
// gets the same number the panel shows. That is what makes "money is physics" testable.
//
// Esse: the two proper scoring rules the mission loop's PREDICT beat commits to. `log_score` for a
// categorical call (a probability the mission meets its Δv budget), `crps` for an interval
// prediction (a predicted Δv range). Both are proper — the expected score is minimised by the true
// distribution — so hedging and over-claiming are both punished, which is the property accuracy %
// lacks and the reason the bible forbids it.
//
// `achieved_dv` reads the Δv the mission actually spent through the rocket kernel, so the score is
// a function of the same physics the flight is. A refused launch yields `NaN`: a mission that
// never flew has no achieved Δv, and the score of a prediction against it is a refusal.
//
// Every function is a pure function of its arguments: the same inputs give the same output on every
// platform, every run. That determinism is the P3 replay claim, and it is what makes an Esse score
// recomputable from a verified replay (§4.3 rule 4).

/// The contract price for a mission, in mission credits (MCU).
///
/// A pure function of the work the contract asks for (§12.1): the Δv budget, the payload mass, the
/// risk, and the window. `NaN` if any input is non-finite or if the Δv, mass, or window is
/// non-positive or the risk is negative -- a refusal, not an answer.
#[unsafe(no_mangle)]
pub extern "C" fn contract_price(dv_km_s: f64, mass_kg: f64, risk: f64, window_days: f64) -> f64 {
    odderon_astro::mission::contract_price(dv_km_s, mass_kg, risk, window_days)
}

/// The Δv a mission actually achieved, km/s, for an authored launch.
///
/// The burnout velocity the rocket imparts, read through the rocket kernel so a refused launch yields
/// `NaN` rather than a number. `NaN` if the launch is refused.
#[unsafe(no_mangle)]
pub extern "C" fn achieved_dv(lat: f64, az: f64, el: f64, v: f64) -> f64 {
    odderon_astro::mission::achieved_dv_km_s(lat, az, el, v)
}

/// The log score of a categorical prediction (§4.3).
///
/// `predicted_p` is the probability the player committed, in `[0,1]`; `occurred` is `1` if the
/// outcome occurred, `0` if not. **Lower is better**: `0` is a perfect, certain, correct call.
/// `NaN` if `p` is non-finite or outside `[0,1]`; `+∞` if the player claimed certainty and was
/// wrong (the honest score for that, not a refusal).
#[unsafe(no_mangle)]
pub extern "C" fn log_score(predicted_p: f64, occurred: u32) -> f64 {
    odderon_astro::mission::log_score(predicted_p, occurred)
}

/// The continuous ranked probability score of an interval prediction (§4.3).
///
/// `predicted_lo` and `predicted_hi` are the committed interval; `actual` is the achieved value.
/// **Lower is better**: `0` is a degenerate point prediction exactly at the outcome. `NaN` if any
/// input is non-finite or if the interval has non-positive width -- a refusal, not a score.
#[unsafe(no_mangle)]
pub extern "C" fn crps(predicted_lo: f64, predicted_hi: f64, actual: f64) -> f64 {
    odderon_astro::mission::crps(predicted_lo, predicted_hi, actual)
}

/// The deterministic design hash of the mission economy's constants.
///
/// Pins the price model's reference point, exponents, and base price, so a replay bundle or an
/// Akashi entry can detect a stale economy kernel. A pure function: the same kernel produces the
/// same hash on every platform, every run.
#[unsafe(no_mangle)]
pub extern "C" fn mission_design_hash() -> u64 {
    odderon_astro::mission::mission_design_hash()
}

// =============================================================================================
// Orbital rendezvous (Block N1: the Hohmann transfer, the phase angle, the window)
// =============================================================================================

/// The total Hohmann transfer Δv between two circular orbits, km/s.
///
/// `r_from` and `r_to` are the orbital radii in km; `mu` is the central body's gravitational
/// parameter in km³/s². The total is the sum of the departure and arrival burn magnitudes. `NaN`
/// if any input is non-positive or non-finite. `0.0` if the orbits are the same (no transfer
/// needed).
#[unsafe(no_mangle)]
pub extern "C" fn rendezvous_transfer_dv(r_from: f64, r_to: f64, mu: f64) -> f64 {
    odderon_astro::rendezvous::rendezvous_transfer_dv(r_from, r_to, mu)
}

/// The half-period of the Hohmann transfer ellipse, seconds.
///
/// The time for the transfer burn to carry the vehicle from one circular orbit to the other.
/// `NaN` if any input is invalid or if the orbits are the same (no transfer to time).
#[unsafe(no_mangle)]
pub extern "C" fn rendezvous_transfer_time_s(r_from: f64, r_to: f64, mu: f64) -> f64 {
    odderon_astro::rendezvous::rendezvous_transfer_time_s(r_from, r_to, mu)
}

/// The lead angle the target must have at the moment of the transfer burn, radians.
///
/// A pure function of the radius ratio (the central-body parameter cancels). Positive for an
/// outward transfer (target must be ahead), negative for an inward transfer (target must be
/// behind). `NaN` if any input is invalid or if the orbits are the same.
#[unsafe(no_mangle)]
pub extern "C" fn rendezvous_phase_angle(r_from: f64, r_to: f64, mu: f64) -> f64 {
    odderon_astro::rendezvous::rendezvous_phase_angle(r_from, r_to, mu)
}

/// The time in seconds until the next rendezvous window, given the current lead angle.
///
/// `theta` is the current angular lead of the target over the chaser, in `[0, 2π)` radians.
/// Returns `0.0` if the window is open now, `NaN` if the inputs are invalid or if the orbits are
/// the same (no relative motion).
#[unsafe(no_mangle)]
pub extern "C" fn rendezvous_window_s(r_from: f64, r_to: f64, theta: f64, mu: f64) -> f64 {
    odderon_astro::rendezvous::rendezvous_window_s(r_from, r_to, theta, mu)
}

/// The deterministic design hash of the rendezvous module's constants.
///
/// Folds in the bi-elliptic threshold and Earth's gravitational parameter, so a replay bundle or
/// an Akashi entry can detect a stale rendezvous kernel.
#[unsafe(no_mangle)]
pub extern "C" fn rendezvous_design_hash() -> u64 {
    odderon_astro::rendezvous::rendezvous_design_hash()
}

// =============================================================================================
// The protégé's teaching arc (Block N2: learning by teaching, bible §5.6, §6)
// =============================================================================================

/// The protégé's tier from the reputation count (correct briefings answered).
///
/// 0 = novice (0–1), 1 = junior (2–4), 2 = analyst (5–9), 3 = peer (10+). The thresholds are
/// pinned in the kernel, so a replay bundle can detect a stale arc model.
#[unsafe(no_mangle)]
pub extern "C" fn protege_tier(reputation: u32) -> u32 {
    odderon_astro::protege::protege_tier(reputation)
}

/// The depth of the protégé's next question, from the tier and the kernel's readings for the error
/// entry.
///
/// `verdict` is the `RocketVerdict` code (0 = Suborbital, 1 = Orbital, 2 = Escape, 3 = Refused);
/// `apogee_km` is the apogee in km (`NaN` if the launch was refused). The depth is the tier, capped
/// at 1 for a refused launch or a non-finite apogee (the margin and counterfactual questions need
/// an apogee).
#[unsafe(no_mangle)]
pub extern "C" fn protege_question_depth(verdict: u32, apogee_km: f64, tier: u32) -> u32 {
    odderon_astro::protege::protege_question_depth(verdict, apogee_km, tier)
}

/// What the protégé can notice at a given tier.
///
/// 0 = nothing, 1 = a mismatched record, 2 = a stale record, 3 = a tampered record. Never exceeds
/// the tier.
#[unsafe(no_mangle)]
pub extern "C" fn protege_notice_level(tier: u32) -> u32 {
    odderon_astro::protege::protege_notice_level(tier)
}

/// The deterministic design hash of the protégé arc's constants.
///
/// Folds in the three tier thresholds, so a replay bundle or an Akashi entry can detect a stale
/// arc model.
#[unsafe(no_mangle)]
pub extern "C" fn protege_design_hash() -> u64 {
    odderon_astro::protege::protege_design_hash()
}

// =============================================================================================
// The Esse career ledger (P5: the career core, bible §4.3)
// =============================================================================================
//
// `odderon_astro::essence` aggregates a career's scored predictions into the `E_cal` rating §4.3
// defines: a reliability curve (predicted probability binned against observed frequency), N, a 95%
// interval, and the §2.2(a) bias. [`mission`] scores one prediction; these exports aggregate the
// sequence.
//
// **Why these exports fill buffers, unlike the orbit exports.** The orbit case is a function of a
// *scalar* (`dt`) with scalar outputs, so scalar exports were natural and the `unsafe` allowance
// stayed on `#[no_mangle]` alone. The Esse ledger is a function of an *array* — a career's prediction
// log — and an array cannot cross the ABI as a return value in a `no_std` cdylib with no allocator.
// So the host writes the predictions into linear memory and these exports read them, compute, and
// write the result back into caller-provided memory. The `unsafe` pointer reads and writes are
// confined to this ABI boundary; the kernel module (`odderon_astro::essence`) is `forbid(unsafe_code)`
// and stays a pure function of a slice. There is no hidden state: each call is a pure function of the
// caller's buffer, which is what keeps the rating recomputable from a verified replay (§4.3 rule 4).
//
// The layouts, so a host and the gate agree:
//   * `essence_summary` writes 7 f64: `[n, mean_log_score, brier, bias, ci95_lo, ci95_hi, provisional]`
//     (provisional as 0.0/1.0). Returns 7, or 0 if the read or write was out of bounds.
//   * `essence_reliability` writes `BIN_COUNT * 4` f64, per bin in order: `[center, predicted,
//     observed, count]`. Returns BIN_COUNT, or 0 on failure.
// A return of 0 means the call refused — an over-large `n` or an out-of-bounds pointer — and the
// host must treat it as a refusal, not read a partial result.

/// The maximum number of predictions a single ledger call accepts. A career log is bounded in
/// practice; beyond this the call refuses (returns 0) rather than reading past a sane bound.
const ESSENCE_MAX_N: usize = 256;

/// The number of f64 the summary export writes: n, mean, brier, bias, ci_lo, ci_hi, provisional.
const ESSENCE_SUMMARY_FLOATS: usize = 7;

/// Read `n` `(predicted_p, occurred)` pairs from linear memory at `p_off` / `occ_off` into a
/// fixed-size stack array. `None` if `n` exceeds the bound or the pointers would read out of bounds.
///
/// # Safety
///
/// The caller must guarantee `p_off` points to `n` `f64` and `occ_off` to `n` `u32` in linear memory.
fn read_predictions(p_off: u32, occ_off: u32, n: u32) -> Option<[(f64, u32); ESSENCE_MAX_N]> {
    let n = n as usize;
    if n > ESSENCE_MAX_N {
        return None;
    }
    // SAFETY: bounded by the caller's guarantee, checked against ESSENCE_MAX_N above.
    let p = unsafe { core::slice::from_raw_parts(p_off as *const f64, n) };
    // SAFETY: as above, for the u32 outcomes.
    let occ = unsafe { core::slice::from_raw_parts(occ_off as *const u32, n) };
    let mut arr = [(0.0f64, 0u32); ESSENCE_MAX_N];
    for i in 0..n {
        arr[i] = (p[i], occ[i]);
    }
    Some(arr)
}

/// The `E_cal` scalar summary of a career's predictions (§4.3).
///
/// Reads `n` `(predicted_p, occurred)` pairs from linear memory at `p_off` / `occ_off`, aggregates
/// them, and writes 7 f64 to `out_off`: `[n, mean_log_score, brier, bias, ci95_lo, ci95_hi,
/// provisional]`. Returns 7 on success, 0 if `n` exceeds the bound or a pointer was out of bounds.
///
/// A refusal (empty ledger or a poisoned prediction) is expressed as `NaN` in the computed fields,
/// not as a 0 — the same rule every other export follows.
#[unsafe(no_mangle)]
pub extern "C" fn essence_summary(p_off: u32, occ_off: u32, n: u32, out_off: u32) -> u32 {
    let preds = match read_predictions(p_off, occ_off, n) {
        Some(p) => p,
        None => return 0,
    };
    let cal = odderon_astro::essence::essence_cal(&preds[..n as usize]);
    // SAFETY: the caller guarantees `out_off` points to ESSENCE_SUMMARY_FLOATS f64 in linear memory.
    let out =
        unsafe { core::slice::from_raw_parts_mut(out_off as *mut f64, ESSENCE_SUMMARY_FLOATS) };
    out[0] = cal.n as f64;
    out[1] = cal.mean_log_score;
    out[2] = cal.brier;
    out[3] = cal.bias;
    out[4] = cal.ci95_lo;
    out[5] = cal.ci95_hi;
    out[6] = if cal.provisional { 1.0 } else { 0.0 };
    ESSENCE_SUMMARY_FLOATS as u32
}

/// The reliability curve of a career's predictions (§4.3 — "aggregated into a reliability curve, not
/// an average").
///
/// Reads the same `n` pairs as [`essence_summary`] and writes `BIN_COUNT * 4` f64 to `out_off`, one
/// bin at a time in order from the lowest predicted probability: `[center, predicted, observed,
/// count]`. Returns `BIN_COUNT` on success, 0 on failure. An empty bin has `NaN` predicted/observed
/// and a 0 count, so a hole is visible rather than a zero that reads as a measurement.
#[unsafe(no_mangle)]
pub extern "C" fn essence_reliability(p_off: u32, occ_off: u32, n: u32, out_off: u32) -> u32 {
    let preds = match read_predictions(p_off, occ_off, n) {
        Some(p) => p,
        None => return 0,
    };
    let cal = odderon_astro::essence::essence_cal(&preds[..n as usize]);
    let bins = odderon_astro::essence::BIN_COUNT;
    // SAFETY: the caller guarantees `out_off` points to `bins * 4` f64 in linear memory.
    let out = unsafe { core::slice::from_raw_parts_mut(out_off as *mut f64, bins * 4) };
    for (i, b) in cal.bins.iter().enumerate() {
        out[i * 4] = b.center;
        out[i * 4 + 1] = b.predicted;
        out[i * 4 + 2] = b.observed;
        out[i * 4 + 3] = b.count as f64;
    }
    bins as u32
}

/// The deterministic design hash of the Esse aggregation's constants.
///
/// Pins the bin count, the provisional threshold, and the interval's critical value, so a replay
/// bundle or an Akashi entry can detect a stale aggregation kernel. A pure function: the same kernel
/// produces the same hash on every platform, every run.
#[unsafe(no_mangle)]
pub extern "C" fn essence_design_hash() -> u64 {
    odderon_astro::essence::essence_design_hash()
}

// =============================================================================================
// The Anomaly Drawer and the logic rack (P5 Block 5: the failure-design and session layer)
// =============================================================================================
//
// `odderon_astro::anomaly` derives the E_an rating from a mission's divergence, caps the drawer at
// five, and compiles logic-rack rules. These exports are the ABI the page calls:
//
// * `anomaly_from_mission` derives an anomaly from a mission's raw facts (the committed prediction,
//   the outcome, the risk, the window) and writes the `Anomaly` struct to `out_off` as 5 f64:
//   `[source_id, depth, telemetry, elo, resolved]`. Returns 5 on success, 0 if the mission did not
//   diverge enough to open an anomaly or an input was invalid.
// * `drawer_can_open` is the cap predicate: 1 if the drawer can take another anomaly, 0 if full.
// * `elo_expected` and `elo_update` are the duel-style Elo: `elo_update` returns the new `E_an`
//   rating, or NaN if the inputs are invalid.
// * `rack_compile` validates and compiles a rule `IF sensor <op> threshold THEN action`, writing
//   the `RackRule` to `out_off` as 4 f64: `[sensor, op, threshold, action]`. Returns 4 on success,
//   0 if the op is invalid or the threshold is non-finite.
// * `rack_fires` reads a compiled rule back from `rule_off` and evaluates it against `reading`:
//   1 if the rule fires, 0 if not (or if the reading is NaN).
// * `anomaly_design_hash` pins the drawer/Elo/rack constants.
//
// Like the essence exports, the anomaly and the rack rule cross the ABI as caller-provided buffers
// because they are array/struct values; the kernel module stays `forbid(unsafe_code)` and pure, and
// all `unsafe` is confined to this ABI boundary.

/// The number of f64 the `Anomaly` struct occupies in linear memory.
const ANOMALY_FLOATS: usize = 5;
/// The number of f64 the `RackRule` struct occupies in linear memory.
const RACK_RULE_FLOATS: usize = 4;

/// Derive an anomaly from a mission's divergence (§7.3, §4).
///
/// Reads the mission's raw facts and writes the `Anomaly` to `out_off` as 5 f64:
/// `[source_id, depth, telemetry, elo, resolved]`. Returns 5 on success, 0 if the mission did not
/// diverge enough to open an anomaly or an input was invalid (a refusal — the page must treat 0 as
/// "no anomaly opened," not read a partial result).
#[unsafe(no_mangle)]
pub extern "C" fn anomaly_from_mission(
    source_id: u32,
    predicted_p: f64,
    occurred: u32,
    risk: f64,
    window_days: f64,
    out_off: u32,
) -> u32 {
    let a = match odderon_astro::anomaly::anomaly_from_mission(
        source_id,
        predicted_p,
        occurred,
        risk,
        window_days,
    ) {
        Some(a) => a,
        None => return 0,
    };
    // SAFETY: the caller guarantees `out_off` points to ANOMALY_FLOATS f64 in linear memory.
    let out = unsafe { core::slice::from_raw_parts_mut(out_off as *mut f64, ANOMALY_FLOATS) };
    out[0] = a.source_id as f64;
    out[1] = a.depth;
    out[2] = a.telemetry;
    out[3] = a.elo;
    out[4] = if a.resolved { 1.0 } else { 0.0 };
    ANOMALY_FLOATS as u32
}

/// Whether the drawer can open another anomaly (§7.3: at most five). 1 if yes, 0 if full.
#[unsafe(no_mangle)]
pub extern "C" fn drawer_can_open(open_count: u32) -> u32 {
    if odderon_astro::anomaly::drawer_can_open(open_count as usize) {
        1
    } else {
        0
    }
}

/// The Elo expected score of `rating` against `opponent` (§4's duel-style Elo). NaN on refusal.
#[unsafe(no_mangle)]
pub extern "C" fn elo_expected(rating: f64, opponent: f64) -> f64 {
    odderon_astro::anomaly::elo_expected(rating, opponent)
}

/// The weighted `E_an` update from resolving an anomaly (§4). Returns the new rating, or NaN if the
/// inputs are invalid (a refusal, never a fabricated rating).
#[unsafe(no_mangle)]
pub extern "C" fn elo_update(
    rating: f64,
    opponent: f64,
    outcome: u32,
    depth: f64,
    telemetry: f64,
) -> f64 {
    odderon_astro::anomaly::elo_update(rating, opponent, outcome, depth, telemetry)
}

/// Compile a logic-rack rule `IF sensor <op> threshold THEN action` (§5.5).
///
/// Writes the `RackRule` to `out_off` as 4 f64: `[sensor, op, threshold, action]`. Returns 4 on
/// success, 0 if the op is invalid or the threshold is non-finite (a refusal — the page must treat
/// 0 as "the rule did not compile," not read a partial result).
#[unsafe(no_mangle)]
pub extern "C" fn rack_compile(
    sensor: u32,
    op: u32,
    threshold: f64,
    action: u32,
    out_off: u32,
) -> u32 {
    let rule = match odderon_astro::anomaly::rack_compile(sensor, op as u8, threshold, action) {
        Some(r) => r,
        None => return 0,
    };
    // SAFETY: the caller guarantees `out_off` points to RACK_RULE_FLOATS f64 in linear memory.
    let out = unsafe { core::slice::from_raw_parts_mut(out_off as *mut f64, RACK_RULE_FLOATS) };
    out[0] = rule.sensor as f64;
    out[1] = rule.op as u8 as f64;
    out[2] = rule.threshold;
    out[3] = rule.action as f64;
    RACK_RULE_FLOATS as u32
}

/// Evaluate a compiled rack rule against a sensor reading (§5.5).
///
/// Reads the `RackRule` back from `rule_off` (4 f64: `[sensor, op, threshold, action]`, as written
/// by [`rack_compile`]) and returns 1 if the rule fires, 0 if not. A NaN reading does not fire. The
/// rule must have been produced by [`rack_compile`]; a rule written with a different layout is the
/// caller's bug, not the kernel's.
#[unsafe(no_mangle)]
pub extern "C" fn rack_fires(rule_off: u32, reading: f64) -> u32 {
    // SAFETY: the caller guarantees `rule_off` points to RACK_RULE_FLOATS f64 written by
    // `rack_compile`.
    let raw = unsafe { core::slice::from_raw_parts(rule_off as *const f64, RACK_RULE_FLOATS) };
    let op = match odderon_astro::anomaly::Op::from_code(raw[1] as u8) {
        Some(op) => op,
        // A rule whose op code is not a valid operator was never produced by `rack_compile`; treat
        // it as not firing rather than trapping.
        None => return 0,
    };
    let rule = odderon_astro::anomaly::RackRule {
        sensor: raw[0] as u32,
        op,
        threshold: raw[2],
        action: raw[3] as u32,
    };
    if odderon_astro::anomaly::rack_fires(&rule, reading) {
        1
    } else {
        0
    }
}

/// The deterministic design hash of the anomaly/drawer/rack constants.
#[unsafe(no_mangle)]
pub extern "C" fn anomaly_design_hash() -> u64 {
    odderon_astro::anomaly::anomaly_design_hash()
}

// =============================================================================================
// The museum gallery (Block 7: the archive as a place, bible §11.4)
// =============================================================================================
//
// The museum is the archive made physical: a gallery that grows as the career grows. The kernel
// owns the LAYOUT — where each exhibit sits, how big it is, which way it faces — and the page owns
// the RENDERING of the parametric geometry. The same archive produces the same gallery on every
// platform, which is what makes it verifiable: a stranger who loads the archive and calls
// `museum_layout` walks the same aisles the player walked.
//
// The layout crosses the ABI as a buffer call (the same shape as `essence_summary` and
// `anomaly_from_mission`), because an exhibit is a struct and a struct cannot be a return value in
// a `no_std` cdylib with no allocator. The page writes one entry descriptor per archive entry into
// linear memory, the kernel computes the layout, and writes the exhibits back into caller-provided
// memory. All `unsafe` is confined to this ABI boundary; the kernel module stays `forbid(unsafe_code)`.
//
// The layouts, so a host and the gate agree:
//   * `museum_layout` reads `n` entry descriptors from `desc_off` (each 4 f64: `[kind, size,
//     orient, id]`) and writes `n` exhibits to `out_off` (each 6 f64: `[x, z, scale, rotation,
//     kind, id]`). Returns `n` on success, 0 if `n` exceeds the bound or a pointer is out of bounds.
//   * `museum_exhibit_kind` maps an exhibit kind code to a name index (0 = Vehicle, 1 = Wreck,
//     2 = Report, 3 = Comms, 4 = Mail, 5 = Mission, 6 = Briefing, -1 for unknown).
//   * `museum_design_hash` pins the gallery's layout constants.

/// The maximum number of exhibits a single layout call accepts. Bounded in practice by the archive
/// size; beyond this the call refuses (returns 0) rather than reading past a sane bound.
const MUSEUM_MAX_N: usize = 256;

/// The number of f64 per entry descriptor in linear memory: `[kind, size, orient, id]`.
const MUSEUM_DESC_FLOATS: usize = 4;

/// The number of f64 per exhibit in linear memory: `[x, z, scale, rotation, kind, id]`.
const MUSEUM_EXHIBIT_FLOATS: usize = 6;

/// Read `n` entry descriptors from linear memory at `desc_off` into a fixed-size stack array.
/// `None` if `n` exceeds the bound or the pointer would read out of bounds.
///
/// # Safety
///
/// The caller must guarantee `desc_off` points to `n * MUSEUM_DESC_FLOATS` f64 in linear memory.
fn read_museum_descs(desc_off: u32, n: u32) -> Option<[odderon_astro::museum::EntryDesc; MUSEUM_MAX_N]> {
    let n = n as usize;
    if n > MUSEUM_MAX_N {
        return None;
    }
    // SAFETY: bounded by the caller's guarantee, checked against MUSEUM_MAX_N above.
    let raw = unsafe { core::slice::from_raw_parts(desc_off as *const f64, n * MUSEUM_DESC_FLOATS) };
    let mut arr = [odderon_astro::museum::EntryDesc { kind: 0, size: 0.0, orient: 0.0, id: 0 }; MUSEUM_MAX_N];
    let mut i = 0;
    while i < n {
        let base = i * MUSEUM_DESC_FLOATS;
        arr[i] = odderon_astro::museum::EntryDesc {
            kind: raw[base] as u8,
            size: raw[base + 1],
            orient: raw[base + 2],
            id: raw[base + 3] as u32,
        };
        i += 1;
    }
    Some(arr)
}

/// Compute the museum gallery layout for `n` archive entries.
///
/// Reads `n` entry descriptors from linear memory at `desc_off` (each 4 f64:
/// `[kind, size, orient, id]`) and writes `n` exhibits to `out_off` (each 6 f64:
/// `[x, z, scale, rotation, kind, id]`). Returns `n` on success, 0 if `n` exceeds the bound or a
/// pointer was out of bounds.
///
/// The layout is a pure function of the entry descriptors: the same entries, in the same order,
/// produce the same gallery on every platform, every run.
#[unsafe(no_mangle)]
pub extern "C" fn museum_layout(desc_off: u32, n: u32, out_off: u32) -> u32 {
    let descs = match read_museum_descs(desc_off, n) {
        Some(d) => d,
        None => return 0,
    };
    let n = n as usize;
    let mut exhibits = [odderon_astro::museum::Exhibit { x: 0.0, z: 0.0, scale: 0.0, rotation: 0.0, kind: 0, id: 0 }; MUSEUM_MAX_N];
    let count = odderon_astro::museum::museum_layout(&descs[..n], &mut exhibits);
    // SAFETY: the caller guarantees `out_off` points to `count * MUSEUM_EXHIBIT_FLOATS` f64.
    let out = unsafe { core::slice::from_raw_parts_mut(out_off as *mut f64, count * MUSEUM_EXHIBIT_FLOATS) };
    let mut i = 0;
    while i < count {
        let base = i * MUSEUM_EXHIBIT_FLOATS;
        let e = exhibits[i];
        out[base] = e.x;
        out[base + 1] = e.z;
        out[base + 2] = e.scale;
        out[base + 3] = e.rotation;
        out[base + 4] = e.kind as f64;
        out[base + 5] = e.id as f64;
        i += 1;
    }
    count as u32
}

/// Map an exhibit kind code to a name index for display.
///
/// 0 = Vehicle, 1 = Wreck, 2 = Report, 3 = Comms, 4 = Mail, 5 = Mission, 6 = Briefing. Returns
/// -1 for a code this kernel does not know, so a future kind added by an older page is visible
/// rather than silently mislabelled.
#[unsafe(no_mangle)]
pub extern "C" fn museum_exhibit_kind(code: u32) -> i32 {
    match odderon_astro::museum::ExhibitKind::from_code(code as u8) {
        Some(kind) => kind.code() as i32,
        None => -1,
    }
}

/// The deterministic design hash of the museum layout's constants.
///
/// Folds in the golden angle, the base radius, the radial step, and the scale bounds, so a replay
/// bundle or a verifier can detect a stale gallery layout. A pure function: the same kernel
/// produces the same hash on every platform, every run.
#[unsafe(no_mangle)]
pub extern "C" fn museum_design_hash() -> u64 {
    odderon_astro::museum::museum_design_hash()
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
