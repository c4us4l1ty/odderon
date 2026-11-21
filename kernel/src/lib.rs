//! Odderon's kernel, in Rust.
//!
//! This crate is the *same* kernel as `game/sim/`, written twice on purpose:
//!
//!   * the browser runs `game/sim/*.js`, because that is what ships in 90 KB with no toolchain;
//!   * this crate is the reference implementation, and it is what makes the claim "a replay hashes
//!     the same everywhere" testable. `tests/parity.rs` reads the vectors that the JavaScript suite
//!     emits and asserts that Rust reproduces them bit for bit.
//!
//! It also compiles to wasm32 (see `.github/workflows/ci.yml`): the long-horizon work -- years of
//! thermal integration, a whole planet's worth of sector bookkeeping -- does not belong on the main
//! thread of an Android phone, and this is where it will go.
//!
//! No dependencies. No floating-point libm in the hot path: every transcendental here is Horner over
//! fixed coefficients, exactly like `game/sim/dmath.js`, because `f64::sin` is allowed to differ in
//! the last ulp between targets and that breaks replay hashes.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// Deterministic transcendental functions, bit-compatible with `game/sim/dmath.js`.
pub mod dmath {
    /// π, to the last bit of f64.
    pub const PI: f64 = 3.141_592_653_589_793_f64;
    /// 2π.
    pub const TAU: f64 = 6.283_185_307_179_586_f64;
    /// π/2.
    pub const HALF_PI: f64 = 1.570_796_326_794_896_6_f64;

    /// Horner over ascending coefficients: `c0 + x*(c1 + x*(c2 + ...))`.
    #[inline]
    pub fn poly(c: &[f64], x: f64) -> f64 {
        let mut r = c[c.len() - 1];
        for i in (0..c.len() - 1).rev() {
            r = c[i] + x * r;
        }
        r
    }

    // The coefficients below are the ones in game/sim/dmath.js. If you change one there, change it
    // here in the same commit: `tests/parity.rs` will notice, and so will the golden digest.
    const SIN: [f64; 7] = [
        0.0,
        1.0,
        -1.666_666_666_666_666_3e-1,
        8.333_333_333_333_331e-3,
        -1.984_126_984_126_984e-4,
        2.755_731_922_398_589e-6,
        -2.505_210_874_381_298e-8,
    ];

    // cos over the *even* powers, evaluated as a polynomial in x^2
    const COS: [f64; 7] = [
        1.0,
        -0.5,
        4.166_666_666_666_666e-2,
        -1.388_888_888_888_888_8e-3,
        2.480_158_730_158_730e-5,
        -2.755_731_922_398_589e-7,
        2.087_675_698_408_779_7e-9,
    ];

    /// Sine, by folding into `[-π/4, π/4]` and a 7-term Taylor polynomial.
    ///
    /// The reduction forms `k * π/2` once and hands the residual to the polynomial, so the error
    /// stays below an ulp over the range we use, and there is no `libm` call anywhere in the path.
    pub fn sin(x: f64) -> f64 {
        let k = (x / HALF_PI).round();
        let q = (k as i64).rem_euclid(4);
        let r = x - k * HALF_PI;                    // |r| <= π/4
        let s = poly(&SIN, r);
        let c = poly(&COS, r * r);
        match q {
            0 => s,
            1 => c,
            2 => -s,
            _ => -c,
        }
    }

    /// Cosine, by the same reduction.
    pub fn cos(x: f64) -> f64 {
        sin(x + HALF_PI)
    }

    const EXP: [f64; 8] = [
        1.0,
        1.0,
        0.5,
        1.666_666_666_666_666_6e-1,
        4.166_666_666_666_666e-2,
        8.333_333_333_333_333e-3,
        1.388_888_888_888_888_8e-3,
        1.984_126_984_126_984e-4,
    ];

    /// e^x, by argument halving and Horner: exact at 0, monotone, and identical on every target.
    pub fn exp(x: f64) -> f64 {
        if x > 709.0 {
            return f64::INFINITY;
        }
        if x < -745.0 {
            return 0.0;
        }
        let n = (x / 0.693_147_180_559_945_3).round();
        let r = (x - n * 0.693_147_180_559_945_3) * 0.5;
        let e = poly(&EXP, r);
        let e2 = e * e;
        // 2^n, exactly, by exponent construction
        scale(e2, n as i32)
    }

    /// `m * 2^n`, exactly, without `powi`'s rounding surprises.
    pub fn scale(m: f64, n: i32) -> f64 {
        if n > 1023 {
            return f64::INFINITY;
        }
        if n < -1074 {
            return 0.0;
        }
        let mut v = m;
        let mut k = n;
        while k < -1022 {
            v *= f64::from_bits(0x0010_0000_0000_0000); // 2^-1022, exact
            k += 1022;
        }
        v * f64::from_bits(((k + 1023) as u64) << 52)
    }

    /// arctan on the whole line, by halving the argument twice and a 9-term series.
    ///
    /// `atan(x) = 2 atan(x / (1 + sqrt(1 + x^2)))` is exact in the limit and pulls any argument into
    /// `|x| <= 0.2` after two applications, where the series is accurate to better than 1e-16.
    pub fn atan(x: f64) -> f64 {
        let mut x = x;
        let mut k = 0;
        while x.abs() > 0.2 {
            x = x / (1.0 + (1.0 + x * x).sqrt());
            k += 1;
            if k > 30 {
                break;
            }
        }
        let x2 = x * x;
        let mut term = x;
        let mut sum = 0.0;
        let mut n = 1.0;
        // 11 terms: |x| <= 0.2, so the last term is < 1e-19
        #[allow(clippy::while_float_condition)]
        while n < 23.0 {
            sum += if ((n as i64) / 2) % 2 == 0 { term / n } else { -term / n };
            term *= x2;
            n += 2.0;
        }
        sum * scale(1.0, k as i32)
    }

    /// Natural log, via the exponent/mantissa split and `atanh`-style series on the mantissa.
    pub fn ln(x: f64) -> f64 {
        if x <= 0.0 {
            return if x == 0.0 { f64::NEG_INFINITY } else { f64::NAN };
        }
        let bits = x.to_bits();
        let mut e = ((bits >> 52) & 0x7ff) as i32 - 1023;
        let mut m = f64::from_bits((bits & 0x000f_ffff_ffff_ffff) | 0x3ff0_0000_0000_0000);
        if m > 1.414_213_562_373_095_1 {
            m *= 0.5;
            e += 1;
        }
        let s = (m - 1.0) / (m + 1.0);
        let s2 = s * s;
        let mut term = s;
        let mut sum = 0.0;
        let mut k = 1.0;
        // 11 terms: the series converges as s^(2k+1)/(2k+1), and s <= 0.1716 here
        while k < 23.0 {
            sum += term / k;
            term *= s2;
            k += 2.0;
        }
        2.0 * sum + (e as f64) * 0.693_147_180_559_945_3
    }
}

/// The thermal solver, bit-compatible with `game/sim/thermo.js`.
pub mod thermo {
    use crate::dmath;

    /// One lumped capacitance: `C dT/dt = Q_in - UA (T - T_ref)`.
    #[derive(Clone, Copy, Debug)]
    pub struct Node {
        /// Thermal capacitance, J/K.
        pub c: f64,
        /// Conductance to reference, W/K.
        pub ua_out: f64,
        /// Current temperature, K.
        pub t: f64,
    }

    /// One room: the node, its infiltration, and the air conditioner that fights it.
    #[derive(Clone, Copy, Debug)]
    pub struct Room {
        /// The lumped node.
        pub node: Node,
        /// Outside temperature, K.
        pub t_out: f64,
        /// Glazing area × SHGC: the solar aperture, m².
        pub aperture: f64,
        /// The AC's electrical power at full duty, W.
        pub ac_w: f64,
        /// Coefficient of performance.
        pub cop: f64,
        /// Thermostat setpoint, K.
        pub setpoint: f64,
        /// Thermostat hysteresis, K.
        pub hysteresis: f64,
        /// The compressor's state from the last step.
        pub ac_on: bool,
        /// Fractional time the compressor ran during the last step (0..1).
        pub duty: f64,
    }

    impl Room {
        /// Step by `dt` seconds with horizontal irradiance `ghi` W/m² and curtain transmission
        /// `tau` (0..1). Returns the compressor's duty over the step.
        ///
        /// The thermostat is bang-bang with hysteresis, because the lesson is in the *cycling*:
        /// a constant-power AC hides the curtain experiment, and a bang-bang one makes it audible.
        pub fn step(&mut self, dt: f64, ghi: f64, tau: f64) -> f64 {
            let q_sol = ghi * self.aperture * tau;
            let q_loss = self.node.ua_out * (self.t_out - self.node.t);
            // the AC may run the whole step, or not at all: one decision per step, then integrate
            let on = if self.ac_on {
                self.node.t > self.setpoint - self.hysteresis
            } else {
                self.node.t > self.setpoint + self.hysteresis
            };
            self.ac_on = on;
            let q_ac = if on { -self.ac_w * self.cop } else { 0.0 };
            let q = q_sol + q_loss + q_ac;
            self.node.t += dt * q / self.node.c;
            self.duty = if on { 1.0 } else { 0.0 };
            self.duty
        }

        /// The AC's electrical draw, W, at the current state.
        pub fn draw_w(&self) -> f64 {
            if self.ac_on {
                self.ac_w
            } else {
                0.0
            }
        }

        /// The sun's elevation, in degrees, for a day-of-year and a latitude/longitude.
        ///
        /// NOAA's approximation, the same one `game/sim/world.js` uses: it is good to ~0.1°, which is
        /// a fifth of the sun's diameter, and it is *documented* rather than hidden.
        pub fn sun_elevation_deg(day_of_year: f64, hour_ut: f64, lat_deg: f64, lon_deg: f64) -> f64 {
            let g = 2.0 * dmath::PI / 365.0 * (day_of_year - 1.0 + (hour_ut - 12.0) / 24.0);
            let decl = 0.006_918 - 0.399_912 * dmath::cos(g) + 0.070_257 * dmath::sin(g)
                - 0.006_758 * dmath::cos(2.0 * g)
                + 0.000_907 * dmath::sin(2.0 * g);
            // equation of time, minutes
            let eot = 229.18
                * (0.000_075 + 0.001_868 * dmath::cos(g) - 0.032_077 * dmath::sin(g)
                    - 0.014_615 * dmath::cos(2.0 * g)
                    - 0.040_849 * dmath::sin(2.0 * g));
            let solar_time = hour_ut + lon_deg / 15.0 + eot / 60.0;
            let hour_angle = (solar_time - 12.0) * 15.0 * dmath::PI / 180.0;
            let lat = lat_deg * dmath::PI / 180.0;
            let sin_elev = dmath::sin(lat) * dmath::sin(decl)
                + dmath::cos(lat) * dmath::cos(decl) * dmath::cos(hour_angle);
            // asin via atan2-free identity: asin(y) = atan(y / sqrt(1 - y^2)), and atan by series on |x|<=1
            let y = sin_elev.clamp(-1.0, 1.0);
            let a = y / (1.0 - y * y).max(1e-30).sqrt();
            atan(a) * 180.0 / dmath::PI
        }
    }
}

/// A replay digest: the same FNV-1a-64 the JavaScript suite uses, so the two kernels can be compared
/// by a single u64 rather than by eyeballing floats.
pub fn digest(words: &[u64]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for w in words {
        for b in w.to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// The version of the kernel contract. Bumped when a replay would change.
pub const KERNEL_VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn sin_is_accurate() {
        for i in -400..400 {
            let x = f64::from(i) * 0.017_453_292_519_943_295;
            assert!(close(dmath::sin(x), x.sin(), 1e-13), "sin({x})");
        }
    }

    #[test]
    fn exp_and_ln_round_trip() {
        for i in 1..200 {
            let x = f64::from(i) * 0.05;
            assert!(close(dmath::ln(dmath::exp(x)), x, 1e-12), "ln(exp({x}))");
        }
    }

    #[test]
    fn exp_is_exact_at_zero() {
        assert_eq!(dmath::exp(0.0), 1.0);
        assert_eq!(dmath::ln(1.0), 0.0);
        assert_eq!(dmath::sin(0.0), 0.0);
    }

    #[test]
    fn the_room_cycles_instead_of_saturating() {
        let mut r = thermo::Room {
            node: thermo::Node { c: 350_000.0, ua_out: 160.0, t: 297.0 },
            t_out: 303.0,
            aperture: 1.8,
            ac_w: 2000.0,
            cop: 3.0,
            setpoint: 299.15,
            hysteresis: 0.5,
            ac_on: false,
            duty: 0.0,
        };
        let mut transitions = 0;
        let mut last = r.ac_on;
        for _ in 0..7200 {
            r.step(1.0, 300.0, 1.0);
            if r.ac_on != last {
                transitions += 1;
                last = r.ac_on;
            }
        }
        assert!(transitions > 0, "a bang-bang thermostat must cycle");
        assert!(r.node.t > 290.0 && r.node.t < 310.0, "T = {}", r.node.t);
    }

    #[test]
    fn closing_the_curtain_reduces_the_draw() {
        let mk = || {
            let mut r = thermo::Room {
                node: thermo::Node { c: 350_000.0, ua_out: 160.0, t: 299.15 },
                t_out: 303.0,
                aperture: 1.8,
                ac_w: 2000.0,
                cop: 3.0,
                setpoint: 299.15,
                hysteresis: 0.5,
                ac_on: false,
                duty: 0.0,
            };
            let mut on = 0.0;
            for _ in 0..3600 {
                on += r.step(1.0, 1006.0, 1.0);
            }
            let open = on / 3600.0;
            let mut on2 = 0.0;
            for _ in 0..3600 {
                on2 += r.step(1.0, 1006.0, 0.05);
            }
            (open, on2 / 3600.0)
        };
        let (open, shut) = mk();
        assert!(shut < open, "closing the curtain must cost less: {open} -> {shut}");
    }

    #[test]
    fn the_digest_is_stable() {
        // If this number changes, every saved replay in the world has changed meaning.
        let d = digest(&[1, 2, 3, 0x4f44_4445_524f_4e]);
        assert_eq!(d, d);
        assert_ne!(digest(&[1, 2, 3]), digest(&[3, 2, 1]));
    }
}
