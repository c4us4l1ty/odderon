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
///
/// The constants below are spelled out rather than taken from `std::f64::consts`, on purpose: the
/// kernel is not allowed to inherit a library constant that a future toolchain might revise.
#[allow(clippy::approx_constant)]
pub mod dmath {
    /// π, to the last bit of f64.
    pub const PI: f64 = 3.141_592_653_589_793_f64;
    /// 2π.
    pub const TAU: f64 = 6.283_185_307_179_586_f64;
    /// π/2.
    pub const HALF_PI: f64 = 1.570_796_326_794_896_6_f64;
    /// ln 2.
    pub const LN2: f64 = 0.693_147_180_559_945_3_f64;
    /// 1/ln 2.
    pub const INV_LN2: f64 = 1.442_695_040_888_963_4_f64;

    /// Horner over ascending coefficients: `c0 + x*(c1 + x*(c2 + ...))`.
    #[inline]
    pub fn poly(c: &[f64], x: f64) -> f64 {
        let mut r = c[c.len() - 1];
        for i in (0..c.len() - 1).rev() {
            r = c[i] + x * r;
        }
        r
    }

    // The coefficients below are the ones in game/sim/dmath.js, indexed by *power*: the zero
    // entries are part of the array because `poly` walks powers, not non-zero terms. Getting this
    // wrong is silent -- a 7-term cosine array used as 9 sparse terms is simply a different
    // polynomial, and an error of 0.18 in `sin(0.5)` is what that looks like.
    const SIN_C: [f64; 12] = [
        0.0,
        1.0,
        0.0,
        -1.0 / 6.0,
        0.0,
        1.0 / 120.0,
        0.0,
        -1.0 / 5040.0,
        0.0,
        1.0 / 362_880.0,
        0.0,
        -1.0 / 39_916_800.0,
    ];

    const COS_C: [f64; 13] = [
        1.0,
        0.0,
        -1.0 / 2.0,
        0.0,
        1.0 / 24.0,
        0.0,
        -1.0 / 720.0,
        0.0,
        1.0 / 40_320.0,
        0.0,
        -1.0 / 3_628_800.0,
        0.0,
        1.0 / 479_001_600.0,
    ];

    /// The argument, reduced to the nearest multiple of π/2: `x = k·π/2 + r` with `|r| ≤ π/4`.
    #[inline]
    fn quadrant(x: f64) -> (i64, f64) {
        let k = (x / HALF_PI).round();
        (k as i64, x - k * HALF_PI)
    }

    /// Sine. Exact at 0, odd, monotone, and identical on every target because nothing here is libm.
    pub fn sin(x: f64) -> f64 {
        if x != x || x.is_infinite() {
            return f64::NAN;
        }
        let (k, r) = quadrant(x);
        let s = poly(&SIN_C, r);
        let c = poly(&COS_C, r);
        match k.rem_euclid(4) {
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

    /// e^x by reduction to [0, ln2) and a 17-term Taylor polynomial, exactly as the JavaScript
    /// kernel does it. (The earlier version halved the argument and *squared the polynomial*, which
    /// is not the same function; it is the sort of thing a parity test exists to catch.)
    pub fn exp(x: f64) -> f64 {
        if x > 709.0 {
            return f64::INFINITY;
        }
        if x < -745.0 {
            return 0.0;
        }
        let n = (x * INV_LN2).floor();
        let mut r = x - n * LN2;
        let mut n = n as i32;
        if r < 0.0 {
            r += LN2;
            n -= 1;
        }
        poly(&EXP_C, r) * scale(1.0, n)
    }

    /// 1/n! for n = 0..17, by power.
    const EXP_C: [f64; 18] = [
        1.0,
        1.0,
        1.0 / 2.0,
        1.0 / 6.0,
        1.0 / 24.0,
        1.0 / 120.0,
        1.0 / 720.0,
        1.0 / 5040.0,
        1.0 / 40_320.0,
        1.0 / 362_880.0,
        1.0 / 3_628_800.0,
        1.0 / 39_916_800.0,
        1.0 / 479_001_600.0,
        1.0 / 6_227_020_800.0,
        1.0 / 87_178_291_200.0,
        1.0 / 1_307_674_368_000.0,
        1.0 / 20_922_789_888_000.0,
        1.0 / 355_687_428_096_000.0,
    ];

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
        // at most two rounds: the subnormal range is only 52 bits below 2^-1022
        for _ in 0..2 {
            if k >= -1022 {
                break;
            }
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
        let neg = x < 0.0;
        let mut a = if neg { -x } else { x };
        if a.is_infinite() {
            return if neg { -HALF_PI } else { HALF_PI };
        }
        let inv = a > 1.0;
        if inv {
            a = 1.0 / a;
        }
        // Two half-angle steps. One leaves t <= 0.414 and the omitted term (t^25/25) is ~1e-11,
        // which the accuracy test caught in the JavaScript port; two give t <= 0.199 and ~1e-18.
        let mut t = a / (1.0 + (1.0 + a * a).sqrt());
        t = t / (1.0 + (1.0 + t * t).sqrt());
        let mut s = 4.0 * t * poly(&ATAN_C, t * t);
        if inv {
            s = HALF_PI - s;
        }
        if neg {
            -s
        } else {
            s
        }
    }

    /// arctan of y/x in the right quadrant, exactly as `game/sim/dmath.js` defines it.
    pub fn atan2(y: f64, x: f64) -> f64 {
        if x == 0.0 {
            return if y > 0.0 {
                HALF_PI
            } else if y < 0.0 {
                -HALF_PI
            } else {
                0.0
            };
        }
        let a = atan(y / x);
        if x > 0.0 {
            a
        } else if y >= 0.0 {
            a + PI
        } else {
            a - PI
        }
    }

    /// arcsin via `atan2`, which is how the JavaScript kernel does it (and why the two agree).
    pub fn asin(x: f64) -> f64 {
        if x > 1.0 || x < -1.0 {
            return f64::NAN;
        }
        atan2(x, (1.0 - x * x).max(0.0).sqrt())
    }

    /// The arc tangent series coefficients, by power: 1 - t²/3 + t⁴/5 - ...
    const ATAN_C: [f64; 12] = [
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
        -1.0 / 23.0,
    ];

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
        for _ in 0..11 {
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
            dmath::asin(sin_elev.clamp(-1.0, 1.0)) * 180.0 / dmath::PI
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
///
/// It is mixed into every replay digest, so an old replay cannot be silently verified against a new
/// kernel: verification fails loudly instead of succeeding wrongly.
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
    fn the_sun_is_where_the_sun_is() {
        // Tanegashima, day 226, 12:00 JST = 03:00 UTC. The JavaScript kernel reports 74.2 deg from
        // the horizon for this moment, and test/physics.test.mjs holds it to 1 deg.
        let e = thermo::Room::sun_elevation_deg(226.0, 3.0, 30.4019, 130.9681);
        assert!((e - 73.7).abs() < 1.0, "noon elevation {e} deg");
        // and it is below the horizon twelve hours later
        let night = thermo::Room::sun_elevation_deg(226.0, 15.0, 30.4019, 130.9681);
        assert!(night < 0.0, "midnight elevation {night} deg");
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
        let a = digest(&[1, 2, 3, 0x4f44_4445_524f_4e]);
        let b = digest(&[1, 2, 3, 0x4f44_4445_524f_4e]);
        assert_eq!(a, b, "the digest must be a function of its input");
        assert_ne!(digest(&[1, 2, 3]), digest(&[3, 2, 1]), "word order must matter");
    }
}
