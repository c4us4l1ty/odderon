//! Odderon 2 flight core.
//!
//! Pure, deterministic astrodynamics for the browser and desktop builds. The
//! crate has no dependencies, allocates nothing and exposes a flat C ABI so the
//! wasm module can be instantiated without wasm-bindgen.
//!
//! The JavaScript side (`src/core/core.ts`) owns the memory view: every export
//! writes its result into a shared 64-slot scratch buffer and returns how many
//! doubles it wrote.

#![allow(clippy::too_many_arguments)]

pub mod dynamics;
pub mod elements;
pub mod propagate;
pub mod score;

use core::ptr::addr_of_mut;
use elements::{Elements, Vec3};

pub const EARTH_MU: f64 = 3.986_004_418e14;
pub const EARTH_RADIUS: f64 = 6_371_000.0;
pub const EARTH_J2: f64 = 1.082_626_68e-3;

/// Bumped whenever the packed layouts below change; the JS side refuses a
/// module whose version it does not know.
pub const VERSION: i32 = 2;

const SCRATCH_LEN: usize = 64;

static mut SCRATCH: [f64; SCRATCH_LEN] = [0.0; SCRATCH_LEN];

fn write(values: &[f64]) -> i32 {
    if values.len() > SCRATCH_LEN {
        return -1;
    }
    unsafe {
        let out = addr_of_mut!(SCRATCH) as *mut f64;
        for (i, value) in values.iter().enumerate() {
            *out.add(i) = *value;
        }
    }
    values.len() as i32
}

/// Pointer to the shared scratch buffer, as an offset into wasm memory.
#[no_mangle]
pub extern "C" fn oc_scratch_ptr() -> *mut f64 {
    addr_of_mut!(SCRATCH) as *mut f64
}

#[no_mangle]
pub extern "C" fn oc_version() -> i32 {
    VERSION
}

/// Writes 23 doubles: a, e, i, raan, argp, nu, p, pe, ap, period, energy,
/// h.xyz, e_vec.xyz, p_hat.xyz, q_hat.xyz.
#[no_mangle]
pub extern "C" fn oc_elements(rx: f64, ry: f64, rz: f64, vx: f64, vy: f64, vz: f64, mu: f64) -> i32 {
    let el = elements::elements_from_state(
        Vec3::new(rx, ry, rz),
        Vec3::new(vx, vy, vz),
        mu,
    );
    write(&el.pack())
}

/// Writes 6 doubles: position then velocity.
#[no_mangle]
pub extern "C" fn oc_state_from_elements(
    px: f64,
    py: f64,
    pz: f64,
    qx: f64,
    qy: f64,
    qz: f64,
    p: f64,
    e: f64,
    nu: f64,
    mu: f64,
) -> i32 {
    let (r, v) = elements::state_from_elements(
        Vec3::new(px, py, pz),
        Vec3::new(qx, qy, qz),
        p,
        e,
        nu,
        mu,
    );
    write(&[r.x, r.y, r.z, v.x, v.y, v.z])
}

/// Writes 6 doubles: position then velocity after `dt` seconds on the conic.
#[no_mangle]
pub extern "C" fn oc_kepler(
    rx: f64,
    ry: f64,
    rz: f64,
    vx: f64,
    vy: f64,
    vz: f64,
    dt: f64,
    mu: f64,
) -> i32 {
    let (r, v) = propagate::propagate_kepler(
        Vec3::new(rx, ry, rz),
        Vec3::new(vx, vy, vz),
        dt,
        mu,
    );
    write(&[r.x, r.y, r.z, v.x, v.y, v.z])
}

/// One RK4 step under thrust. `j2` is 0 or 1.
#[no_mangle]
pub extern "C" fn oc_rk4(
    rx: f64,
    ry: f64,
    rz: f64,
    vx: f64,
    vy: f64,
    vz: f64,
    dt: f64,
    ax: f64,
    ay: f64,
    az: f64,
    j2: i32,
    mu: f64,
) -> i32 {
    let (r, v) = dynamics::step_rk4(
        Vec3::new(rx, ry, rz),
        Vec3::new(vx, vy, vz),
        dt,
        Vec3::new(ax, ay, az),
        j2 != 0,
        mu,
    );
    write(&[r.x, r.y, r.z, v.x, v.y, v.z])
}

#[no_mangle]
pub extern "C" fn oc_time_to_apsis(a: f64, e: f64, nu: f64, apoapsis: i32, mu: f64) -> f64 {
    elements::time_to_apsis(a, e, nu, apoapsis != 0, mu)
}

/// 0..1 composite score. The caller rounds and grades.
#[no_mangle]
pub extern "C" fn oc_score(
    pe_error: f64,
    ap_error: f64,
    tolerance: f64,
    fuel_left: f64,
    time_frac: f64,
) -> f64 {
    score::composite(pe_error, ap_error, tolerance, fuel_left, time_frac)
}

/// 0..1 apsis accuracy, exposed for the HUD progress ring.
#[no_mangle]
pub extern "C" fn oc_accuracy(pe_error: f64, ap_error: f64, tolerance: f64) -> f64 {
    score::accuracy(pe_error, ap_error, tolerance)
}

/// Sanity check used by the boot self-test: returns 1 when the module answers.
#[no_mangle]
pub extern "C" fn oc_self_test() -> i32 {
    let r = Vec3::new(7_000_000.0, 0.0, 0.0);
    let v = Vec3::new(0.0, 7_500.0, 0.0);
    let el = elements::elements_from_state(r, v, EARTH_MU);
    let ok = (el.a - 6_916_882.0).abs() < 5_000.0 && el.e > 0.0 && el.period > 5_000.0;
    if ok {
        1
    } else {
        0
    }
}

pub fn elements_from_state(r: Vec3, v: Vec3) -> Elements {
    elements::elements_from_state(r, v, EARTH_MU)
}
