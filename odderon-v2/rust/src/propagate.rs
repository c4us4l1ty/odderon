//! Universal-variable Kepler propagation (Vallado, Algorithm 8), ported from
//! `propagateKepler` in `src/core/orbit.ts`.
//!
//! Exact for two-body motion; no perturbations. Time warp relies on this so a
//! node an hour away costs the same as one a minute away.

use crate::elements::Vec3;

const MAX_NEWTON: usize = 80;
const MAX_BISECTION: usize = 100;

#[inline]
fn c2(z: f64) -> f64 {
    if z > 1e-6 {
        (1.0 - z.sqrt().cos()) / z
    } else if z < -1e-6 {
        ((-z).sqrt().cosh() - 1.0) / -z
    } else {
        0.5
    }
}

#[inline]
fn c3(z: f64) -> f64 {
    if z > 1e-6 {
        let s = z.sqrt();
        (s - s.sin()) / (s * s * s)
    } else if z < -1e-6 {
        let s = (-z).sqrt();
        (s.sinh() - s) / (s * s * s)
    } else {
        1.0 / 6.0
    }
}

struct Residual {
    f: f64,
    fp: f64,
}

pub fn propagate_kepler(r0: Vec3, v0: Vec3, dt: f64, mu: f64) -> (Vec3, Vec3) {
    if !dt.is_finite() || dt == 0.0 {
        return (r0, v0);
    }
    let rm = r0.len();
    let vm = v0.len();
    let sqrt_mu = mu.sqrt();
    let alpha = 2.0 / rm - (vm * vm) / mu;
    let sigma0 = r0.dot(v0) / sqrt_mu;

    let mut chi = if alpha > 1e-10 {
        sqrt_mu * dt * alpha
    } else if alpha < -1e-10 {
        let a = 1.0 / alpha;
        let num = -2.0 * mu * alpha * dt;
        let den = sigma0 + dt.signum() * (-mu * a).sqrt() * (1.0 - rm * alpha);
        let guess = (num / den).max(1e-12).ln();
        let chi = dt.signum() * (-a).sqrt() * guess;
        if chi.is_finite() {
            chi
        } else {
            (sqrt_mu * dt) / rm
        }
    } else {
        (sqrt_mu * dt) / rm
    };

    let residual = |x: f64| -> Residual {
        let z = alpha * x * x;
        let zc2 = c2(z);
        let zc3 = c3(z);
        let r = x * x * zc2 + sigma0 * x * (1.0 - z * zc3) + rm * (1.0 - z * zc2);
        let f = sigma0 * x * x * zc2 + (1.0 - alpha * rm) * x * x * x * zc3 + rm * x - sqrt_mu * dt;
        Residual { f, fp: r }
    };

    let mut solved = false;
    for _ in 0..MAX_NEWTON {
        let res = residual(chi);
        if res.f.abs() < 1e-9 {
            solved = true;
            break;
        }
        if !res.f.is_finite() || !res.fp.is_finite() || res.fp.abs() < 1e-14 {
            break;
        }
        let next = chi - res.f / res.fp;
        if !next.is_finite() {
            break;
        }
        if (next - chi).abs() < 1e-12 {
            chi = next;
            solved = true;
            break;
        }
        chi = next;
    }

    if !solved {
        // Bisection fallback keeps a bad frame from poisoning the state.
        let span = sqrt_mu * dt.abs() / rm + 1.0;
        let mut lo = chi.min(0.0) - span;
        let mut hi = chi.max(0.0) + span;
        for _ in 0..MAX_BISECTION {
            let mid = 0.5 * (lo + hi);
            let f_mid = residual(mid).f;
            if f_mid.abs() < 1e-6 {
                break;
            }
            if (residual(lo).f < 0.0) == (f_mid < 0.0) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        chi = 0.5 * (lo + hi);
    }

    let z = alpha * chi * chi;
    let zc2 = c2(z);
    let zc3 = c3(z);
    let f = 1.0 - chi * chi / rm * zc2;
    let g = dt - chi * chi * chi / sqrt_mu * zc3;
    let r_new = r0.scale(f).add_scaled(v0, g);
    let rm_new = r_new.len();
    let fdot = (sqrt_mu / (rm * rm_new)) * (alpha * chi * chi * chi * zc3 - chi);
    let gdot = 1.0 - chi * chi / rm_new * zc2;
    let v_new = r0.scale(fdot).add_scaled(v0, gdot);

    if !r_new.x.is_finite() || rm_new < 1.0 {
        return (r0, v0);
    }
    (r_new, v_new)
}
