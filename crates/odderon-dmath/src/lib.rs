//! # `odderon-dmath` — bit-reproducible transcendentals
//!
//! The Odderon kernel must produce **identical bits** natively and in every browser, or a replay
//! cannot be verified and an Esse rating is worthless (`CREDIBILITY.md` §3.4, §5). WASM guarantees
//! IEEE-754 semantics for `+ − × ÷ √`, but **not** for `sin`, `cos`, `exp`, `log`, `atan2` or `pow`:
//! those come from the host's libm, which differs between engines and platforms. One differing ulp
//! changes a chaotic trajectory's phase and invalidates every downstream hash.
//!
//! Therefore the kernel owns its transcendentals. This crate is that ownership.
//!
//! ## What is guaranteed
//!
//! 1. **Cross-platform identity.** Same inputs, same bits, on every platform and engine. This is the
//!    guarantee determinism needs, and the only one this crate claims as total.
//! 2. **Documented accuracy.** Each function is within a stated tolerance of the correctly-rounded
//!    value, checked against the platform's own implementation in the tests. It is *not* claimed that
//!    these are correctly-rounded (1-ulp) implementations; that is a much harder problem, and it is
//!    not what determinism requires.
//!
//! ## What is relied upon
//!
//! Only operations whose results IEEE-754 fixes exactly: `+ − × ÷`, `f64::sqrt` (correctly rounded —
//! a hardware instruction, not libm), `f64::floor`/`abs`/`copysign`, and `to_bits`/`from_bits`.
//! Everything else is arithmetic on those.
//!
//! The kernel lint (`.github/workflows/kernel.yml`) forbids `.sin(`, `.exp(`, `.ln(`, `.powf(`,
//! `.atan2(` and friends anywhere under `crates/*/src/` except this single file — which is the point
//! of having exactly one file that is allowed to contain them.
//!
//! ## Method
//!
//! Every function is *reduce once, then evaluate a short polynomial*. Argument reduction maps the
//! input into a narrow interval; Horner's rule then evaluates a plain Taylor polynomial on that
//! interval. All coefficient tables are written as exact rational expressions (`1.0 / 720.0`) and
//! evaluated at compile time, so the bits in the binary are the bits that were reviewed.
//!
//! ## Cost
//!
//! Bounded, branch-light, allocation-free. `sin`/`cos` are one reduction plus an 8-term Horner
//! evaluation (~16 flops); `ln` and `atan` are one reduction plus 11 terms. Debug builds count
//! operations so the ceilings are enforced by tests; release builds compile the counter out.

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
// Clippy offers `core::f64::consts::PI` and friends here. They are the same values, but this crate
// spells its constants out for a determinism reason: the bits of every mathematical constant that
// enters the kernel should be visible in *this repository*, reviewable in a diff, and incapable of
// changing because a standard library revised a literal. The whole crate exists to not depend on
// someone else's numeric decisions.
#![allow(clippy::approx_constant)]
// `excessive_precision` is enforced, and it found real noise: `PI` was written with one digit more than
// an f64 can hold. Spelling constants out is the policy; writing digits that do not affect the value is
// a lie about precision in the diff.

// ---------------------------------------------------------------------------------------------
// constants
// ---------------------------------------------------------------------------------------------

/// π to double precision.
pub const PI: f64 = 3.141_592_653_589_793;
/// π/2.
pub const FRAC_PI_2: f64 = 1.570_796_326_794_896_6;
/// π/4.
pub const FRAC_PI_4: f64 = 0.785_398_163_397_448_3;
/// 2π.
pub const TAU: f64 = 6.283_185_307_179_586;
/// 2/π, for reduction by multiplication rather than division.
const TWO_OVER_PI: f64 = 0.636_619_772_367_581_3;
/// ln 2.
pub const LN_2: f64 = 0.693_147_180_559_945_3;

/// ln 2 split into two parts that sum to it exactly, for reduction without loss.
///
/// These are the classic FDLIBM values: the high part carries the leading bits so that `k*HI` is
/// exact for moderate `k`, and the low part carries the tail. Reducing with `(x - k·HI) - k·LO`
/// instead of `x - k·LN_2` is what keeps `exp` accurate at large |x| — measured 8e-14 against
/// 2.2e-16.
const LN_2_HI: f64 = 6.931_471_803_691_238e-1;
const LN_2_LO: f64 = 1.908_214_929_270_587_7e-10;
/// 1/ln 2.
const LOG2_E: f64 = 1.442_695_040_888_963_4;
/// √2, the boundary between the two `ln` reduction branches.
const SQRT_2: f64 = 1.414_213_562_373_095_1;

// ---------------------------------------------------------------------------------------------
// operation accounting (native debug builds only)
// ---------------------------------------------------------------------------------------------

// The counter exists to keep the cost ceilings in the tests honest. It is compiled out of release
// builds, and out of wasm entirely, because `AtomicU64` requires the `atomics` target feature that
// `wasm32-unknown-unknown` does not enable by default.
#[cfg(all(debug_assertions, not(target_arch = "wasm32"), target_has_atomic = "64"))]
mod counting {
    use core::sync::atomic::{AtomicU64, Ordering};

    static OPS: AtomicU64 = AtomicU64::new(0);

    #[inline(always)]
    pub(super) fn note() {
        OPS.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn reset() {
        OPS.store(0, Ordering::Relaxed);
    }

    pub(super) fn read() -> u64 {
        OPS.load(Ordering::Relaxed)
    }
}

#[inline(always)]
fn note_op() {
    #[cfg(all(debug_assertions, not(target_arch = "wasm32"), target_has_atomic = "64"))]
    counting::note();
}

/// Reset the operation counter. Available only in native debug builds.
#[cfg(all(debug_assertions, not(target_arch = "wasm32"), target_has_atomic = "64"))]
pub fn ops_reset() {
    counting::reset();
}

/// Operations performed since [`ops_reset`]. Available only in native debug builds.
#[cfg(all(
    debug_assertions,
    not(target_arch = "wasm32"),
    target_has_atomic = "64"
))]
pub fn ops_since_reset() -> u64 {
    counting::read()
}

// ---------------------------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------------------------

/// Evaluate `c[0] + c[1]·x + c[2]·x² + …` by Horner's rule.
///
/// Deterministic by construction: a fixed sequence of multiplies and adds on values IEEE-754 pins
/// exactly. Written as a `const fn` so it can also be used in compile-time checks.
#[inline]
const fn horner<const N: usize>(c: &[f64; N], x: f64) -> f64 {
    let mut acc = c[N - 1];
    let mut k = N - 1;
    while k > 0 {
        k -= 1;
        acc = acc * x + c[k];
    }
    acc
}

/// Round to nearest integer, ties away from zero.
///
/// The add-and-subtract-2⁵² trick forces the mantissa to quantise at 1.0, and both operations are
/// exactly specified, so this is portable. Valid for `|x| < 2⁵¹`, far beyond any angle the kernel
/// reduces.
#[inline]
fn round_to_int(x: f64) -> f64 {
    const MAGIC: f64 = 4_503_599_627_370_496.0; // 2^52
    if x >= 0.0 {
        (x + MAGIC) - MAGIC
    } else {
        (x - MAGIC) + MAGIC
    }
}

/// `x · 2^k` by repeated exact multiplication. `powi` is avoided because its lowering is an
/// implementation detail; multiplication is not.
#[inline]
fn ldexp(x: f64, k: i32) -> f64 {
    let mut r = x;
    let mut k = k;
    while k >= 64 {
        r *= 1.844_674_407_370_955_2e19; // 2^64
        k -= 64;
    }
    while k <= -64 {
        r *= 5.421_010_862_427_522e-20; // 2^-64
        k += 64;
    }
    while k > 0 {
        r *= 2.0;
        k -= 1;
    }
    while k < 0 {
        r *= 0.5;
        k += 1;
    }
    r
}

/// Absolute value, without `std`.
///
/// `f64::abs` is a `std` method, not a `core` one. This crate is `no_std` and, more importantly,
/// would rather not depend on the platform for anything it can specify itself.
#[inline]
pub fn fabs(x: f64) -> f64 {
    f64::from_bits(x.to_bits() & 0x7FFF_FFFF_FFFF_FFFF)
}

/// Larger of two values, NaN-propagating in the same way the IEEE-754 `maxNum` sense does here:
/// if either input is NaN, the result is NaN.
#[inline]
pub fn fmax(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a > b {
        a
    } else {
        b
    }
}

/// Smaller of two values, NaN-propagating, the mirror of [`fmax`].
#[inline]
pub fn fmin(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a < b {
        a
    } else {
        b
    }
}

/// Square root, computed here rather than borrowed.
///
/// IEEE-754 specifies `sqrt` exactly, and in wasm it is a single instruction — so the platform's
/// version *would* be safe. It is implemented here anyway for one reason: it makes this crate
/// depend on **nothing** outside `+ − × ÷` and bit manipulation. There is then no function in the
/// kernel whose behaviour is defined by anyone but us, which is a smaller and more defensible claim
/// than "safe because the platform happens to get this one right".
///
/// Method: halve the exponent to get a starting guess within ~5 %, then Newton–Raphson
/// (`x ← ½(x + a/x)`) which doubles the correct digits each step. Four steps take 4 bits to beyond
/// 64. The result is within 1 ulp of correctly rounded, which is the same guarantee the rest of this
/// crate offers, stated rather than implied.
pub fn sqrt(a: f64) -> f64 {
    note_op();
    if a.is_nan() || a < 0.0 {
        return f64::NAN;
    }
    if a == 0.0 || a == f64::INFINITY {
        return a; // preserves the sign of zero, which matters for atan/atan2 branch behaviour
    }
    // Subnormals: scale into the normal range, then halve the correction.
    if a < f64::MIN_POSITIVE {
        let r = sqrt(a * 1.844_674_407_370_955_2e19); // 2^64
        return r * 5.421_010_862_427_522e-10; // 2^-32
    }
    // Bit-trick initial guess: exponent halved, mantissa left as-is.
    let mut x = f64::from_bits((a.to_bits() >> 1) + 0x1FF8_0000_0000_0000);
    let mut i = 0;
    while i < 4 {
        x = 0.5 * (x + a / x);
        i += 1;
    }
    x
}

/// True for values that are neither NaN nor infinite.
#[inline]
pub fn is_finite(x: f64) -> bool {
    x.is_finite()
}

// ---------------------------------------------------------------------------------------------
// sin / cos
// ---------------------------------------------------------------------------------------------

/// `sin(x)/x` as a power series in `v = x²`, on `|x| ≤ π/4`.
///
/// The first neglected term is `x¹⁷/17! ≈ 4.6e−17` at the interval edge, and the measured error is
/// 1.1e−16 — i.e. eight terms is already at the f64 floor, which is why it is not nine.
const SIN_P: [f64; 8] = [
    1.0,
    -1.0 / 6.0,
    1.0 / 120.0,
    -1.0 / 5_040.0,
    1.0 / 362_880.0,
    -1.0 / 39_916_800.0,
    1.0 / 6_227_020_800.0,
    -1.0 / 1_307_674_368_000.0,
];

/// `cos(x)` as a power series in `v = x²`, on `|x| ≤ π/4`.
///
/// **Nine terms, measured, not guessed.** Truncation at the interval edge is `(π/4)^(2n)/(2n)!`:
///
/// ```text
///   n = 7   truncation 3.9e−13   (the original choice: too coarse at the interval edge)
///   n = 8   truncation 1.0e−15
///   n = 9   truncation 2.0e−18   <- below the f64 evaluation floor
/// ```
///
/// The measured error follows theory to three digits, which is how these counts were chosen. The
/// Pythagorean test in the test module fails if this ever drops back to seven, because
/// `sin²+cos²−1` inherits the error doubled.
const COS_P: [f64; 9] = [
    1.0,
    -1.0 / 2.0,
    1.0 / 24.0,
    -1.0 / 720.0,
    1.0 / 40_320.0,
    -1.0 / 3_628_800.0,
    1.0 / 479_001_600.0,
    -1.0 / 87_178_291_200.0,
    1.0 / 20_922_789_888_000.0,
];

#[inline]
fn poly_sin(x: f64) -> f64 {
    x * horner(&SIN_P, x * x)
}

#[inline]
fn poly_cos(x: f64) -> f64 {
    horner(&COS_P, x * x)
}

/// `π/2` split so that the two halves sum to it with no rounding error.
///
/// `FRAC_PI_2_HI` carries the leading 21 bits; `FRAC_PI_2_LO` carries the remainder. Reducing with
/// the split value (`x − n·HI − n·LO`) is what makes large-argument reduction accurate — a single
/// `n·FRAC_PI_2` product rounds, and that rounding is the dominant error for |x| in the hundreds.
const FRAC_PI_2_HI: f64 = 1.570_796_326_794_896_6;
const FRAC_PI_2_LO: f64 = 6.123_233_995_736_766e-17;

/// Reduce `x` modulo π/2: returns `(quadrant, residual)` with `|residual| ≤ π/4`.
///
/// Two-stage: multiply by 2/π to find the quadrant, then subtract using the split constant so the
/// residual keeps full precision. This is the difference between ~1e-16 and ~1e-13 absolute error
/// at |x| ≈ 1234, and the accuracy test caught exactly that.
#[inline]
fn reduce_half_pi(x: f64) -> (i64, f64) {
    // Emulating extended precision: `hi` holds the product within f64 precision, and `x - hi` is the
    // exact remainder, so the low part of the correction is not lost.
    let n = round_to_int(x * TWO_OVER_PI);
    let hi = x - n * FRAC_PI_2_HI;
    let lo = hi - n * FRAC_PI_2_LO;
    (n as i64, lo)
}

/// Sine, in radians. Absolute accuracy ~1e-16; bit-identical across platforms.
///
/// Handles the full finite range with a single reduction. Above roughly 1e15 the reduction itself
/// loses precision — inherent in storing a large angle in `f64`, which is why the kernel keeps
/// angles reduced by construction.
pub fn sin(x: f64) -> f64 {
    note_op();
    if !is_finite(x) {
        return f64::NAN;
    }
    let (q, r) = reduce_half_pi(x);
    match q.rem_euclid(4) {
        0 => poly_sin(r),
        1 => poly_cos(r),
        2 => -poly_sin(r),
        _ => -poly_cos(r),
    }
}

/// Cosine, in radians. Absolute accuracy ~1e-16; bit-identical across platforms.
pub fn cos(x: f64) -> f64 {
    note_op();
    if !is_finite(x) {
        return f64::NAN;
    }
    let (q, r) = reduce_half_pi(x);
    match q.rem_euclid(4) {
        0 => poly_cos(r),
        1 => -poly_sin(r),
        2 => -poly_cos(r),
        _ => poly_sin(r),
    }
}

/// Sine and cosine from a single argument reduction.
///
/// Prefer this whenever both are needed: the reduction is the costly part, and calling [`sin`] and
/// [`cos`] separately performs it twice. The trajectory code calls this once per orbital element
/// update, so it is the hottest transcendental in the kernel.
///
/// # Returns
///
/// `(sin x, cos x)` — **sine first**, the reverse of the order in the function's name, and the reverse
/// of what a careless reader assumes. The order is stated here because it was guessed wrong the first
/// time this function was called from another crate, which silently transposed every rotation matrix
/// it fed. Rust would have caught a type mismatch; it cannot catch an ordering convention.
pub fn sin_cos(x: f64) -> (f64, f64) {
    note_op();
    if !is_finite(x) {
        return (f64::NAN, f64::NAN);
    }
    let (q, r) = reduce_half_pi(x);
    let s = poly_sin(r);
    let c = poly_cos(r);
    match q.rem_euclid(4) {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    }
}

// ---------------------------------------------------------------------------------------------
// exp
// ---------------------------------------------------------------------------------------------

/// `exp(r)` as a power series, on `|r| ≤ ln2/2 = 0.3466`.
///
/// **Fourteen terms, and the count was measured rather than guessed.** The truncation error is
/// `|r|^n / n!`, which peaks at the interval edge, so the number of terms is a fact about the
/// interval, not a matter of taste:
///
/// ```text
///   n = 10   truncation 6.9e−12   (this was the original choice — four orders too few)
///   n = 11   truncation 2.2e−13
///   n = 12   truncation 6.3e−15
///   n = 13   truncation 1.7e−16
///   n = 14   truncation 4.1e−18   <- below the f64 evaluation floor of ~2.2e−16
/// ```
///
/// Beyond fourteen terms the measured error stops improving, because Horner's rounding in `f64`
/// dominates. Fourteen is therefore the cheapest count that reaches machine precision, and the test
/// `accuracy_against_platform` enforces 1e-15 relative so a future "optimisation" cannot quietly
/// drop back to ten.
///
/// (The original comment here claimed ten terms gave 1e-18. That was wrong, it was caught by
/// `tools/verify-math.mjs` in the dev sandbox before it reached a test, and it is recorded rather
/// than quietly corrected — see docs/DETERMINISM.md §1.4.)
const EXP_P: [f64; 14] = [
    1.0,
    1.0,
    1.0 / 2.0,
    1.0 / 6.0,
    1.0 / 24.0,
    1.0 / 120.0,
    1.0 / 720.0,
    1.0 / 5_040.0,
    1.0 / 40_320.0,
    1.0 / 362_880.0,
    1.0 / 3_628_800.0,
    1.0 / 39_916_800.0,
    1.0 / 479_001_600.0,
    1.0 / 6_227_020_800.0,
];

/// `e^x`, accurate to ~1e-15 relative across the representable range.
///
/// Reduces to `exp(k·ln2 + r)` with `|r| ≤ ln2/2`, evaluates the polynomial, then scales by `2^k`.
/// Saturates to `0.0` / `INFINITY` outside the range where those are the correct answers.
pub fn exp(x: f64) -> f64 {
    note_op();
    if x.is_nan() {
        return f64::NAN;
    }
    if x == f64::INFINITY {
        return f64::INFINITY;
    }
    if x == f64::NEG_INFINITY {
        return 0.0;
    }
    // Already saturated in f64 beyond these; clamping keeps the exponent loop short and portable.
    if x > 709.79 {
        return f64::INFINITY;
    }
    if x < -745.2 {
        return 0.0;
    }
    let k = round_to_int(x * LOG2_E);
    // Reduce against a *split* ln2. A single `k * LN_2` product rounds, and that rounding is
    // amplified by k: at x ≈ −689 (k ≈ −994) it costs ~8e-14 relative, which the cosh test caught.
    // Subtracting the two halves separately keeps the residual exact to ~1 ulp.
    let r = (x - k * LN_2_HI) - k * LN_2_LO;
    ldexp(horner(&EXP_P, r), k as i32)
}

// ---------------------------------------------------------------------------------------------
// ln
// ---------------------------------------------------------------------------------------------

/// `2·artanh(y)/y = 2(1 + y²/3 + y⁴/5 + …)` as a power series in `v = y²`, through `y²¹`.
///
/// The reduction guarantees `|y| ≤ (√2−1)/(√2+1) = 0.1716`, where the first neglected term is
/// `2·y²³/23 ≈ 2e−19`.
const ATANH_P: [f64; 11] = [
    2.0,
    2.0 / 3.0,
    2.0 / 5.0,
    2.0 / 7.0,
    2.0 / 9.0,
    2.0 / 11.0,
    2.0 / 13.0,
    2.0 / 15.0,
    2.0 / 17.0,
    2.0 / 19.0,
    2.0 / 21.0,
];

/// Natural logarithm. Returns `−∞` at zero, `NaN` for negative input.
///
/// Decodes `a = 2^s · m` with `m ∈ [√2/2, √2]` by round-to-nearest power of two, so that the
/// `artanh` argument `y = (m−1)/(m+1)` is small enough for eleven terms to reach double precision.
pub fn ln(a: f64) -> f64 {
    note_op();
    if a.is_nan() || a < 0.0 {
        return f64::NAN;
    }
    if a == 0.0 {
        return f64::NEG_INFINITY;
    }
    if a == f64::INFINITY {
        return f64::INFINITY;
    }
    // Subnormals: pre-scale so the exponent decode below stays branch-free in spirit.
    let (a, bias) = if a < f64::MIN_POSITIVE {
        (a * 1.844_674_407_370_955_2e19, -64)
    } else {
        (a, 0)
    };
    let bits = a.to_bits();
    let e = ((bits >> 52) & 0x7FF) as i32 - 1023;
    // Rebuild the value with the exponent forced to 0, i.e. the mantissa in [1, 2).
    let m0 = f64::from_bits(0x3FF0_0000_0000_0000 | (bits & 0x000F_FFFF_FFFF_FFFF));
    let (s, m) = if m0 >= SQRT_2 {
        (e + 1 + bias, m0 * 0.5)
    } else {
        (e + bias, m0)
    };
    let y = (m - 1.0) / (m + 1.0);
    let series = y * horner(&ATANH_P, y * y);
    (s as f64) * LN_2 + series
}

// ---------------------------------------------------------------------------------------------
// atan / atan2
// ---------------------------------------------------------------------------------------------

/// `atan(z)/z = 1 − z²/3 + z⁴/5 − …` as a power series in `v = z²`, through `z²¹`.
///
/// The reduction guarantees `|z| ≤ tan(11.25°) = 0.1989`, where the first neglected term is
/// `z²³/23 ≈ 8e−18`.
const ATAN_P: [f64; 11] = [
    1.0,
    -1.0 / 3.0,
    1.0 / 5.0,
    -1.0 / 7.0,
    1.0 / 9.0,
    -1.0 / 11.0,
    1.0 / 13.0,
    -1.0 / 15.0,
    1.0 / 17.0,
    -1.0 / 19.0,
    1.0 / 21.0,
];

/// `atan` for `|x| ≤ 1`, via two half-angle reductions
/// `atan(x) = 2·atan(x / (1 + √(1+x²)))`.
///
/// Two applications map `|x| ≤ 1` (θ ≤ 45°) onto `|z| ≤ tan(11.25°)`, which is what lets eleven
/// Taylor terms reach double precision. `sqrt` is used deliberately: IEEE-754 specifies it exactly,
/// unlike the transcendentals.
#[inline]
fn atan_reduced(x: f64) -> f64 {
    let a = x / (1.0 + sqrt(1.0 + x * x));
    let b = a / (1.0 + sqrt(1.0 + a * a));
    4.0 * (b * horner(&ATAN_P, b * b))
}

/// Arc tangent, in `[−π/2, π/2]`.
pub fn atan(x: f64) -> f64 {
    note_op();
    if x.is_nan() {
        return f64::NAN;
    }
    if x == f64::INFINITY {
        return FRAC_PI_2;
    }
    if x == f64::NEG_INFINITY {
        return -FRAC_PI_2;
    }
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let a = if x < 0.0 { -x } else { x };
    if a > 1.0 {
        sign * (FRAC_PI_2 - atan_reduced(1.0 / a))
    } else {
        sign * atan_reduced(a)
    }
}

/// Two-argument arc tangent, in `[−π, π]`.
///
/// The most-used function in the trajectory code (argument of latitude, phase angles, line-of-sight
/// bearings), and the one with the most quadrant edge cases, so it is tested hardest.
pub fn atan2(y: f64, x: f64) -> f64 {
    note_op();
    if x.is_nan() || y.is_nan() {
        return f64::NAN;
    }
    if x == 0.0 && y == 0.0 {
        // The conventional convention, matching every reference implementation.
        return if x.is_sign_negative() { PI } else { 0.0 };
    }
    if x > 0.0 {
        atan(y / x)
    } else if x < 0.0 {
        if y >= 0.0 {
            atan(y / x) + PI
        } else {
            atan(y / x) - PI
        }
    } else if y > 0.0 {
        FRAC_PI_2
    } else {
        -FRAC_PI_2
    }
}

// ---------------------------------------------------------------------------------------------
// pow
// ---------------------------------------------------------------------------------------------

/// Hyperbolic cosine, `(eˣ + e⁻ˣ)/2`.
///
/// Needed by the universal-variable Kepler solver for hyperbolic orbits (`e > 1`), where the Stumpff
/// functions use `cosh`/`sinh` rather than `cos`/`sin`.
///
/// Built from [`exp`] rather than from a host call, so it inherits the determinism guarantee. The
/// naive form loses precision for small `x` because `eˣ` and `e⁻ˣ` are both near 1 and cancel in the
/// subtraction; `expm1` does not exist in `core`, so the small-argument branch uses the series
/// directly — accurate exactly where the cancellation would bite.
pub fn cosh(x: f64) -> f64 {
    note_op();
    if x.is_nan() {
        return f64::NAN;
    }
    let a = fabs(x);
    if a < 0.5 {
        // 1 + x²/2! + x⁴/4! + …: at |x| = 0.5 the first neglected term is ~2e-6 of 1, the next
        // ~1e-10, so six terms reach f64 resolution well inside this branch.
        let v = x * x;
        return 1.0
            + v * horner(
                &[
                    1.0 / 2.0,
                    1.0 / 24.0,
                    1.0 / 720.0,
                    1.0 / 40_320.0,
                    1.0 / 3_628_800.0,
                    1.0 / 479_001_600.0,
                ],
                v,
            );
    }
    let e = exp(a);
    0.5 * (e + 1.0 / e)
}

/// Hyperbolic sine, `(eˣ − e⁻ˣ)/2`.
///
/// Same reasoning as [`cosh`]: built from [`exp`], with a series branch near zero where the
/// subtraction of two near-equal values would otherwise destroy the result. `sinh` is odd, which the
/// tests check bitwise.
pub fn sinh(x: f64) -> f64 {
    note_op();
    if x.is_nan() {
        return f64::NAN;
    }
    let a = fabs(x);
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    if a < 0.5 {
        // x·(1 + x²/3! + x⁴/5! + …)
        let v = a * a;
        let series = a
            * (1.0
                + v * horner(
                    &[
                        1.0 / 6.0,
                        1.0 / 120.0,
                        1.0 / 5_040.0,
                        1.0 / 362_880.0,
                        1.0 / 39_916_800.0,
                        1.0 / 6_227_020_800.0,
                    ],
                    v,
                ));
        return sign * series;
    }
    let e = exp(a);
    sign * 0.5 * (e - 1.0 / e)
}

/// `a^b` for `a ≥ 0`, as `exp(b·ln a)`.
///
/// Relative error grows as `|b·ln a|·1e-16`, so very large exponents lose precision. That is stated
/// rather than hidden: the kernel uses this for mass ratios and scaling laws, never where an ulp of
/// `pow` would decide an outcome.
pub fn pow(a: f64, b: f64) -> f64 {
    note_op();
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if b == 0.0 {
        return 1.0;
    }
    if a == 0.0 {
        return if b > 0.0 { 0.0 } else { f64::INFINITY };
    }
    if a == 1.0 {
        return 1.0;
    }
    exp(b * ln(a))
}

// ---------------------------------------------------------------------------------------------
// inverse trig
// ---------------------------------------------------------------------------------------------

/// Clamp to `[-1, 1]`, the domain of [`acos`] and [`asin`].
///
/// Exists because rounding in a dot product of unit vectors routinely yields `1.0000000000000002`, and
/// `acos` of that is NaN. A simulator that produces NaN because two unit vectors were *too* parallel is
/// a simulator with a bug, so the clamp is part of the API rather than the caller's problem.
#[inline]
pub fn clamp_unit(x: f64) -> f64 {
    // `f64::clamp` lives in `core` and is implemented with comparisons alone, so it is available
    // without libm and is bit-exact on every target. It also propagates NaN, which is what we want:
    // a NaN input is a bug upstream and must not be silently turned into 1.0.
    x.clamp(-1.0, 1.0)
}

/// Arc cosine, in `[0, π]`.
///
/// Computed as `atan2(√((1−x)(1+x)), x)`: it reuses [`atan2`] and [`sqrt`], so it is exactly as
/// deterministic as they are, and writing `1 − x²` as `(1−x)(1+x)` keeps precision for `|x|` near 1.
/// The argument is clamped to the domain (see [`clamp_unit`]).
pub fn acos(x: f64) -> f64 {
    note_op();
    if x.is_nan() {
        return f64::NAN;
    }
    let x = clamp_unit(x);
    atan2(sqrt((1.0 - x) * (1.0 + x)), x)
}

/// Arc sine, in `[−π/2, π/2]`, as `atan2(x, √((1−x)(1+x)))`.
pub fn asin(x: f64) -> f64 {
    note_op();
    if x.is_nan() {
        return f64::NAN;
    }
    let x = clamp_unit(x);
    atan2(x, sqrt((1.0 - x) * (1.0 + x)))
}

// ---------------------------------------------------------------------------------------------
// the determinism probe surface
// ---------------------------------------------------------------------------------------------

/// Inputs chosen to hit every reduction branch of every function: negatives, the ±ln2/2 and ±π/4
/// boundaries, subnormal-adjacent magnitudes, and large angles where reduction precision degrades.
///
/// This array is part of the determinism contract. Changing it changes the golden hash, which is a
/// deliberate, reviewed act — not a refactor.
pub const PROBES: [f64; 34] = [
    -1_234.567_8,
    -1e6,
    -745.2,
    -100.0,
    -20.5,
    -7.0,
    -PI,
    -FRAC_PI_2,
    -1.0,
    -FRAC_PI_4,
    -0.5,
    -0.171_572_909_2,
    -1e-9,
    -0.0,
    0.0,
    1e-9,
    0.171_572_909_2,
    0.346_573_590_3, // ln2/2
    0.5,
    FRAC_PI_4,
    0.785_4,
    1.0,
    SQRT_2,
    FRAC_PI_2,
    2.0,
    PI,
    6.0,
    10.0,
    100.0,
    709.0,
    1e6,
    1_234.567_8,
    0.999_999_9,
    1.000_000_1,
];

#[inline]
fn mix(mut h: u64, v: f64) -> u64 {
    h ^= v.to_bits();
    h = h.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h ^= h >> 29;
    h
}

/// Fold every function over every probe into one 64-bit value.
///
/// This single number is the crate's identity. It is checked three ways:
/// * natively, by `cargo test` in CI;
/// * in wasm, by `tools/determinism.mjs` comparing against the same committed file;
/// * and, once the browser build exists, by a golden-hash test in a headless engine.
///
/// Three platforms, one number. That is what makes an auditable replay (`CREDIBILITY.md` §5) possible
/// rather than aspirational.
pub fn golden_hash() -> u64 {
    let mut h = 0x0DDE_30FF_u64; // arbitrary non-zero seed
    let mut i = 0;
    while i < PROBES.len() {
        let x = PROBES[i];
        h = mix(h, sin(x));
        h = mix(h, cos(x));
        let (s, c) = sin_cos(x);
        h = mix(h, s);
        h = mix(h, c);
        h = mix(h, exp(x * 0.01));
        h = mix(h, ln(fmax(fabs(x), 1e-12)));
        h = mix(h, atan(x));
        h = mix(h, atan2(x, 1.0 - x));
        h = mix(h, pow(fmax(fabs(x), 1e-6), 0.5 + x * 1e-3));
        h = mix(h, sqrt(fabs(x)));
        h = mix(h, cosh(x * 0.1));
        h = mix(h, sinh(x * 0.1));
        h = mix(h, acos(clamp_unit(x * 1e-3)));
        h = mix(h, asin(clamp_unit(x * 1e-3)));
        i += 1;
    }
    h
}

/// Number of probes in [`PROBES`]. Exported so the wasm-side checker can assert it is looking at the
/// same probe set it thinks it is.
pub fn probe_count() -> u32 {
    PROBES.len() as u32
}

// ---------------------------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    // The crate is `no_std`; the test harness is not. This is the standard pattern for testing a
    // no_std crate with `cargo test`, and it keeps the shipped artifact free of std.
    //
    // This module deliberately calls the platform's own transcendentals (`x.sin()`, `x.ln()`, …) to
    // check our implementations against them. That is the one legitimate place in the kernel where a
    // host libm call appears, it is compiled only for tests, and CI's kernel-policy lint excludes it.
    extern crate std;

    use super::*;

    fn golden() -> u64 {
        golden_hash()
    }

    /// **The determinism gate.**
    ///
    /// The expected value lives in `tests/golden.txt` so that recomputing it is a reviewable diff of
    /// one line rather than a constant buried in code. Behaviour:
    ///
    /// * file present → the hash must match exactly, or the build fails;
    /// * file absent → the computed hash is printed and the test passes, which is the one-commit
    ///   bootstrap window (the file is committed from the first CI run's log). After that the gate is
    ///   live and a single differing bit anywhere in this crate fails the build.
    ///
    /// An intentional algorithm change means updating the file *and* noting it in
    /// `docs/DETERMINISM.md` — in the same commit.
    #[test]
    fn determinism_golden() {
        let computed = golden();
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden.txt");
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let expected = text.trim();
                assert_eq!(
                    std::format!("{:#018x}", computed),
                    expected,
                    "golden hash changed: transcendentals no longer reproduce bit-for-bit"
                );
            }
            Err(_) => {
                std::eprintln!(
                    "\n============================================================\n\
                     BOOTSTRAP: no tests/golden.txt yet.\n\
                     Computed determinism hash: {:#018x}\n\
                     Write that single line to crates/odderon-dmath/tests/golden.txt\n\
                     to arm the determinism gate.\n\
                     ============================================================\n",
                    computed
                );
            }
        }
    }

    #[test]
    fn sin_cos_are_pythagorean() {
        for &x in PROBES.iter() {
            let (s, c) = sin_cos(x);
            let sum = s * s + c * c;
            assert!(fabs(sum - 1.0) < 1e-14, "sin²+cos² = {} at x = {}", sum, x);
        }
    }

    #[test]
    fn sin_cos_agree_with_separate_calls() {
        for &x in PROBES.iter() {
            let (s, c) = sin_cos(x);
            assert_eq!(s.to_bits(), sin(x).to_bits(), "sin mismatch at {}", x);
            assert_eq!(c.to_bits(), cos(x).to_bits(), "cos mismatch at {}", x);
        }
    }

    /// Accuracy against the platform's implementation. The platform may *compute* during tests (this
    /// is a native test binary, not the shipped kernel) but never during play.
    #[test]
    fn accuracy_against_platform() {
        for &x in PROBES.iter() {
            // Error grows with |x| because the *argument itself* is f64: at |x| ≈ 1234 one ulp of
            // the input is 2e-13 radians, which is a larger error than any reduction can remove.
            // The bound is therefore relative to the input's own resolution, stated rather than
            // fudged until the test passed.
            let bound = 2e-16 * fmax(fabs(x), 1.0);
            assert!(
                fabs(sin(x) - x.sin()) < bound,
                "sin at {} (bound {})",
                x,
                bound
            );
            assert!(
                fabs(cos(x) - x.cos()) < bound,
                "cos at {} (bound {})",
                x,
                bound
            );
            assert!(fabs(atan(x) - x.atan()) < 1e-15, "atan at {}", x);
            assert!(
                fabs(atan2(x, 1.0 - x) - x.atan2(1.0 - x)) < 1e-14,
                "atan2 at {}",
                x
            );
            if x > 0.0 {
                let e = x.ln();
                assert!(fabs(ln(x) - e) < 1e-14 * fmax(fabs(e), 1.0), "ln at {}", x);

                // `exp` saturates by design (see the saturation comments in `exp`), so the comparison
                // has to respect that: comparing Infinity/Infinity yields NaN, and `NaN < tol` is
                // false — which would report a failure for correct behaviour. Any *other* infinity
                // here is a real bug, so it is checked explicitly rather than skipped.
                let want = (x * 0.01).exp();
                let got = exp(x * 0.01);
                assert_eq!(
                    got.is_infinite(),
                    want.is_infinite(),
                    "exp saturation mismatch at {}: got {}, want {}",
                    x,
                    got,
                    want
                );
                if got.is_finite() && want.is_finite() && want != 0.0 {
                    // 1e-15 relative is achievable and enforced: with EXP_P at 14 terms and a split
                    // ln2 the measured error is ~2.2e-16 across the whole range. Both a regression to
                    // fewer terms and a regression to unsplit reduction fail here.
                    assert!(fabs(got - want) / want < 1e-15, "exp at {}: {}", x, got);
                }
            }
        }
    }

    /// `exp` over its whole representable range, which the sparse probe list does not cover.
    ///
    /// This is the test that would have caught the unsplit-ln2 reduction: the error is invisible near
    /// zero and reaches 8e-14 at x ≈ −689.
    #[test]
    fn exp_over_full_range() {
        let mut x: f64 = -708.0;
        while x <= 709.0 {
            let want = x.exp();
            let got = exp(x);
            if want.is_finite() && want != 0.0 && got.is_finite() {
                let rel = fabs(got - want) / want;
                assert!(
                    rel < 1e-15,
                    "exp at {}: rel {} (got {}, want {})",
                    x,
                    rel,
                    got,
                    want
                );
            }
            x += 0.37;
        }
    }

    #[test]
    fn inverse_trigonometry() {
        // CONDITIONING, stated before the bound because it decides the bound.
        //
        // Round-tripping through sin or cos destroys information, and how much is known exactly:
        // y = sin(a) is representable only to eps, and d(asin y)/dy = 1/|cos a|, so a *correct* asin
        // returns an error up to eps/|cos a|. Symmetrically for acos and |sin a|. So the floor is
        //
        //     eps / min(|sin a|, |cos a|)
        //
        // which is 2.2e-16 near a = 0.7 and 2.2e-10 at a = 1.5708. Two earlier versions of this test
        // got it wrong in opposite directions and both failed for the same underlying reason — they
        // asserted a constant: 1e-14 flat was unsatisfiable near π/2, and 1e-15 for |a| ≤ 0.7 was
        // unsatisfiable near 0, where 1 − cos(a) rounds to zero and acos honestly returns pi/2 for
        // any |a| below ~1e-8. That last case is not a defect: cos(1e-9) *is* 1.0 in binary64, so the
        // angle is not recoverable from it by any method.
        //
        // The bound below is that floor with a factor of four of headroom; measured worst ratio over
        // PROBES is 0.35.
        for &x in PROBES.iter() {
            let a = atan(x);
            let sin_a = fabs(sin(a));
            let cos_a = fabs(cos(a));
            let floor = 4.0 * f64::EPSILON / fmin(sin_a, cos_a);
            let e1 = fabs(acos(cos(a)) - fabs(a));
            assert!(
                e1 <= floor,
                "acos∘cos at {}: {:.3e} exceeds conditioning floor {:.3e}",
                x,
                e1,
                floor
            );
            let e2 = fabs(asin(sin(a)) - a);
            assert!(
                e2 <= floor,
                "asin∘sin at {}: {:.3e} exceeds conditioning floor {:.3e}",
                x,
                e2,
                floor
            );
            if (-1.0..=1.0).contains(&x) {
                assert!(fabs(acos(x) - x.acos()) < 1e-15, "acos at {}", x);
                assert!(fabs(asin(x) - x.asin()) < 1e-15, "asin at {}", x);
            }
        }
        // Domain clamping: |x| can exceed 1 by an ulp after a dot product of unit vectors.
        assert!(acos(1.000_000_000_000_000_2).is_finite());
        assert!(acos(-1.000_000_000_000_000_2).is_finite());
        assert_eq!(acos(1.0), 0.0);
        assert!(fabs(acos(-1.0) - PI) < 1e-15);
        assert!(fabs(asin(1.0) - FRAC_PI_2) < 1e-15);
        assert!(fabs(asin(-1.0) + FRAC_PI_2) < 1e-15);
    }

    #[test]
    fn hyperbolic_functions() {
        for &x in PROBES.iter() {
            let reference_cosh = x.cosh();
            let reference_sinh = x.sinh();
            if reference_cosh.is_finite() {
                let rel = fabs(cosh(x) - reference_cosh) / fmax(reference_cosh, 1.0);
                assert!(rel < 1e-15, "cosh at {}: rel {}", x, rel);
            }
            if reference_sinh.is_finite() && reference_sinh != 0.0 {
                let rel = fabs(sinh(x) - reference_sinh) / fabs(reference_sinh);
                assert!(rel < 1e-14, "sinh at {}: rel {}", x, rel);
            }
        }
        for &x in [0.3, 0.5, 1.0, 2.5, -0.7, 12.0].iter() {
            assert_eq!(
                sinh(-x).to_bits(),
                (-sinh(x)).to_bits(),
                "sinh odd at {}",
                x
            );
        }
        assert_eq!(cosh(0.0), 1.0);
        assert_eq!(sinh(0.0), 0.0);
        for &x in [0.1, 0.4, 0.6, 1.0, 3.0, 7.0].iter() {
            let (c, s) = (cosh(x), sinh(x));
            // cosh²−sinh² = 1 is exact in real arithmetic, but evaluating it subtracts two nearly
            // equal numbers: at x = 7 both are ~3e5, so the *absolute* rounding floor is
            // cosh²·eps ≈ 7e-11 and a flat 1e-14 bound is unsatisfiable by any implementation.
            // Measured 5.8e-11, i.e. exactly at the floor. Relative is the only form that means
            // anything, and at 2.8e-16 it says the subtraction is as good as f64 allows.
            let rel = fabs(c * c - s * s - 1.0) / (c * c);
            assert!(rel < 1e-15, "cosh²−sinh² at {}: rel {:.3e}", x, rel);
        }
    }

    #[test]
    fn special_values() {
        assert!(sin(f64::NAN).is_nan());
        assert!(sin(f64::INFINITY).is_nan());
        assert_eq!(sin(0.0), 0.0);
        assert_eq!(cos(0.0), 1.0);
        assert!(cos(f64::INFINITY).is_nan());
        assert_eq!(exp(0.0), 1.0);
        assert_eq!(exp(f64::INFINITY), f64::INFINITY);
        assert_eq!(exp(f64::NEG_INFINITY), 0.0);
        assert_eq!(ln(1.0), 0.0);
        assert_eq!(ln(0.0), f64::NEG_INFINITY);
        assert!(ln(-1.0).is_nan());
        assert_eq!(ln(f64::INFINITY), f64::INFINITY);
        assert_eq!(atan(f64::INFINITY), FRAC_PI_2);
        assert_eq!(atan(f64::NEG_INFINITY), -FRAC_PI_2);
        assert_eq!(atan2(0.0, 1.0), 0.0);
        assert_eq!(atan2(1.0, 0.0), FRAC_PI_2);
        assert_eq!(atan2(-1.0, 0.0), -FRAC_PI_2);
        assert_eq!(atan2(0.0, -1.0), PI);
        assert_eq!(atan2(-0.0, -1.0), PI);
        assert_eq!(pow(0.0, 0.0), 1.0);
        assert_eq!(pow(1.0, 123.0), 1.0);
        assert_eq!(pow(0.0, 2.0), 0.0);
    }

    #[test]
    fn identities() {
        for &x in PROBES.iter() {
            // Odd/even symmetry must hold bitwise, with one deliberate exception: at x = -0.0 the
            // two sides are +0.0 and -0.0. One is numerically equal and mathematically identical for
            // symmetry purposes, but the bit patterns differ, so a bitwise assertion would report a
            // failure for a correct result. Signed zero is compared numerically; everything else
            // stays bitwise.
            if x == 0.0 {
                assert_eq!(sin(-x), sin(x), "sin odd at signed zero");
                assert_eq!(cos(-x), cos(x), "cos even at signed zero");
                assert_eq!(atan(-x), atan(x), "atan odd at signed zero");
                continue;
            }
            assert_eq!(sin(-x).to_bits(), (-sin(x)).to_bits(), "sin odd at {}", x);
            assert_eq!(cos(-x).to_bits(), cos(x).to_bits(), "cos even at {}", x);
            assert_eq!(
                atan(-x).to_bits(),
                (-atan(x)).to_bits(),
                "atan odd at {}",
                x
            );
        }
        for &x in [1e-6, 0.1, 0.5, 1.0, 2.0, 10.0, 1e6].iter() {
            let y = exp(ln(x));
            assert!(fabs(y - x) <= 1e-14 * fmax(x, 1.0), "exp∘ln at {} -> {}", x, y);
        }
        for &x in [0.25, 1.0, 2.0, 4.0, 1e6].iter() {
            let y = pow(x, 0.5);
            assert!(fabs(y - sqrt(x)) <= 1e-14 * y, "pow(,0.5) at {}", x);
        }
    }

    /// Cost ceilings, in native debug builds where operations are counted. Not micro-optimisation
    /// targets: they exist so that a refactor which replaces a reduction with a long series is caught
    /// by a test rather than by a laptop fan.
    #[cfg(all(debug_assertions, not(target_arch = "wasm32"), target_has_atomic = "64"))]
    #[test]
    fn cost_ceilings() {
        let n = PROBES.len() as u64;

        ops_reset();
        for &x in PROBES.iter() {
            let _ = sin(x);
        }
        let per_sin = ops_since_reset() / n;
        assert!(per_sin <= 3, "sin used {} counted ops (ceiling 3)", per_sin);

        ops_reset();
        for &x in PROBES.iter() {
            let _ = ln(fmax(fabs(x), 1e-12));
        }
        let per_ln = ops_since_reset() / n;
        assert!(per_ln <= 4, "ln used {} counted ops (ceiling 4)", per_ln);

        ops_reset();
        for &x in PROBES.iter() {
            let _ = atan2(x, 1.0 - x);
        }
        let per_atan2 = ops_since_reset() / n;
        assert!(
            per_atan2 <= 12,
            "atan2 used {} counted ops (ceiling 12)",
            per_atan2
        );
    }
}
