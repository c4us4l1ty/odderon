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
