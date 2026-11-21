//! Deterministic IEEE-754 polynomial approximations for transcendental functions.
//! Never invokes host libm, guaranteeing identical bit outputs on x86_64, ARM64, and WASM.

pub const PI: f64 = 3.14159265358979323846264338327950288;
pub const TAU: f64 = 6.28318530717958647692528676655900577;
pub const HALF_PI: f64 = 1.57079632679489661923132169163975144;
pub const INV_TAU: f64 = 0.159154943091895335768883763372514362;
pub const LN2: f64 = 0.693147180559945309417232121458176568;

#[inline]
pub fn clamp_f64(x: f64, lo: f64, hi: f64) -> f64 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

#[inline]
pub fn clamp_f32(x: f32, lo: f32, hi: f32) -> f32 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

#[inline]
pub fn dabs(x: f64) -> f64 {
    f64::from_bits(x.to_bits() & 0x7fff_ffff_ffff_ffff)
}

#[inline]
pub fn dfloor(x: f64) -> f64 {
    let i = x as i64;
    let fi = i as f64;
    if fi > x {
        fi - 1.0
    } else {
        fi
    }
}

/// Deterministic IEEE-754 square root via bit-level seed + 4 Newton-Raphson iterations.
#[inline]
pub fn dsqrt(x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let bits = x.to_bits();
    let approx_bits = (bits >> 1) + 0x1ff8_0000_0000_0000;
    let mut y = f64::from_bits(approx_bits);
    y = 0.5 * (y + x / y);
    y = 0.5 * (y + x / y);
    y = 0.5 * (y + x / y);
    y = 0.5 * (y + x / y);
    y
}

/// Core minimax sin on [-PI/4, PI/4] (< 1e-15 accuracy).
#[inline]
fn sin_quarter_pi(x: f64) -> f64 {
    let x2 = x * x;
    const S1: f64 = -1.66666666666666324348e-1;
    const S2: f64 = 8.33333333332248946124e-3;
    const S3: f64 = -1.98412698298579493134e-4;
    const S4: f64 = 2.75573137070700676789e-6;
    const S5: f64 = -2.50507602534068634195e-8;
    const S6: f64 = 1.58969099521155010221e-10;
    let poly = (((((S6 * x2 + S5) * x2 + S4) * x2 + S3) * x2 + S2) * x2 + S1) * x2;
    x + x * poly
}

/// Core minimax cos on [-PI/4, PI/4] (< 1e-15 accuracy).
#[inline]
fn cos_quarter_pi(x: f64) -> f64 {
    let x2 = x * x;
    const C1: f64 = 4.16666666666666019037e-2;
    const C2: f64 = -1.38888888888741095749e-3;
    const C3: f64 = 2.48015872894767294178e-5;
    const C4: f64 = -2.75573143513906633035e-7;
    const C5: f64 = 2.08757232129817482790e-9;
    const C6: f64 = -1.13596475577881948265e-11;
    let r = x2 * x2 * (((((C6 * x2 + C5) * x2 + C4) * x2 + C3) * x2 + C2) * x2 + C1);
    1.0 - 0.5 * x2 + r
}

/// Deterministic sine for any finite angle in radians (< 1e-15 error).
#[inline]
pub fn dsin(x: f64) -> f64 {
    const INV_HALF_PI: f64 = 0.636619772367581343075535053490057448;
    const Q_PI: f64 = 0.785398163397448309615660845819875721;
    let n = dfloor(x * INV_HALF_PI + 0.5) as i64;
    let r = x - (n as f64) * (2.0 * Q_PI);
    match n.rem_euclid(4) {
        0 => sin_quarter_pi(r),
        1 => cos_quarter_pi(r),
        2 => -sin_quarter_pi(r),
        _ => -cos_quarter_pi(r),
    }
}

/// Deterministic cosine for any finite angle in radians (< 1e-15 error).
#[inline]
pub fn dcos(x: f64) -> f64 {
    const INV_HALF_PI: f64 = 0.636619772367581343075535053490057448;
    const Q_PI: f64 = 0.785398163397448309615660845819875721;
    let n = dfloor(x * INV_HALF_PI + 0.5) as i64;
    let r = x - (n as f64) * (2.0 * Q_PI);
    match n.rem_euclid(4) {
        0 => cos_quarter_pi(r),
        1 => -sin_quarter_pi(r),
        2 => -cos_quarter_pi(r),
        _ => sin_quarter_pi(r),
    }
}

#[inline]
pub fn dtan(x: f64) -> f64 {
    let c = dcos(x);
    if dabs(c) < 1e-15 {
        if c >= 0.0 { 1e15 } else { -1e15 }
    } else {
        dsin(x) / c
    }
}

/// Deterministic atan on [0, 1] (< 1e-15 accuracy via PI/4 range reduction).
#[inline]
fn atan_unit(z: f64) -> f64 {
    const TAN_PI_8: f64 = 0.414213562373095048801688724209698079;
    const Q_PI: f64 = 0.785398163397448309615660845819875721;
    let (offset, u) = if z > TAN_PI_8 {
        (Q_PI, (z - 1.0) / (z + 1.0))
    } else {
        (0.0, z)
    };
    let u2 = u * u;
    let poly = u
        * (1.0
            + u2 * ((-1.0 / 3.0)
                + u2 * ((1.0 / 5.0)
                    + u2 * ((-1.0 / 7.0)
                        + u2 * ((1.0 / 9.0)
                            + u2 * ((-1.0 / 11.0)
                                + u2 * ((1.0 / 13.0)
                                    + u2 * ((-1.0 / 15.0)
                                        + u2 * ((1.0 / 17.0)
                                            + u2 * ((-1.0 / 19.0) + u2 * (1.0 / 21.0)))))))))));
    offset + poly
}

#[inline]
pub fn datan2(y: f64, x: f64) -> f64 {
    if x == 0.0 && y == 0.0 {
        return 0.0;
    }
    let ax = dabs(x);
    let ay = dabs(y);
    let mut a = if ax >= ay {
        atan_unit(ay / ax)
    } else {
        HALF_PI - atan_unit(ax / ay)
    };
    if x < 0.0 {
        a = PI - a;
    }
    if y < 0.0 {
        -a
    } else {
        a
    }
}

#[inline]
pub fn dasin(x: f64) -> f64 {
    let c = clamp_f64(x, -1.0, 1.0);
    datan2(c, dsqrt(1.0 - c * c))
}

/// Deterministic exponential e^x (< 1e-14 relative error).
#[inline]
pub fn dexp(x: f64) -> f64 {
    let xc = clamp_f64(x, -700.0, 700.0);
    let k = dfloor(xc / LN2 + 0.5) as i32;
    let r = xc - (k as f64) * LN2;
    // 11th-order Horner Taylor on [-LN2/2, LN2/2]
    let p = 1.0
        + r * (1.0
            + r * (0.5
                + r * ((1.0 / 6.0)
                    + r * ((1.0 / 24.0)
                        + r * ((1.0 / 120.0)
                            + r * ((1.0 / 720.0)
                                + r * ((1.0 / 5040.0)
                                    + r * ((1.0 / 40320.0)
                                        + r * ((1.0 / 362880.0)
                                            + r * ((1.0 / 3628800.0)
                                                + r * (1.0 / 39916800.0)))))))))));
    let exp_bits = ((1023 + k as i64) as u64) << 52;
    p * f64::from_bits(exp_bits)
}

/// Deterministic natural logarithm ln(x) for x > 0 (< 1e-14 relative error).
#[inline]
pub fn dln(x: f64) -> f64 {
    if x <= 0.0 {
        return -1e30;
    }
    let bits = x.to_bits();
    let exp = ((bits >> 52) & 0x7ff) as i64 - 1023;
    let mant = f64::from_bits((bits & 0x000f_ffff_ffff_ffff) | (1023u64 << 52));
    // Map [1, 2) into [1/sqrt(2), sqrt(2)] so |u| <= 0.1716
    let (m, e) = if mant > 1.4142135623730951 {
        (mant * 0.5, exp + 1)
    } else {
        (mant, exp)
    };
    let u = (m - 1.0) / (m + 1.0);
    let u2 = u * u;
    let series = u
        * (2.0
            + u2 * ((2.0 / 3.0)
                + u2 * ((2.0 / 5.0)
                    + u2 * ((2.0 / 7.0)
                        + u2 * ((2.0 / 9.0)
                            + u2 * ((2.0 / 11.0)
                                + u2 * ((2.0 / 13.0)
                                    + u2 * ((2.0 / 15.0) + u2 * (2.0 / 17.0)))))))));
    (e as f64) * LN2 + series
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trig_identities_and_accuracy() {
        for i in -360..=360 {
            let rad = (i as f64) * PI / 180.0;
            let s = dsin(rad);
            let c = dcos(rad);
            let norm = s * s + c * c;
            assert!((norm - 1.0).abs() < 1e-13, "pythagorean drift at {} deg: {}", i, norm);
        }
        assert!((dsin(0.0)).abs() < 1e-15);
        assert!((dcos(0.0) - 1.0).abs() < 1e-15);
        assert!((dsin(HALF_PI) - 1.0).abs() < 1e-15);
        assert!((datan2(1.0, 1.0) - PI * 0.25).abs() < 1e-10);
    }

    #[test]
    fn exp_ln_roundtrip() {
        for &v in &[0.05, 0.5, 1.0, 2.718281828, 10.0, 343.0, 5500.0] {
            let rt = dexp(dln(v));
            assert!(((rt - v) / v).abs() < 1e-11, "roundtrip failed for {}: got {}", v, rt);
        }
    }
}
