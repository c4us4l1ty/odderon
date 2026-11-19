//! Classical orbital elements, ported from `src/core/orbit.ts`.
//!
//! The arithmetic follows the TypeScript reference line for line, including
//! the order of operations, so both paths agree to within floating-point
//! noise. If you change one, change the other; `tests/parity.rs` guards it.

use crate::{EARTH_J2, EARTH_MU, EARTH_RADIUS};

pub const EPS: f64 = 1e-9;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
    pub fn dot(self, o: Self) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: Self) -> Self {
        Self::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    pub fn len(self) -> f64 {
        self.dot(self).sqrt()
    }
    pub fn scale(self, k: f64) -> Self {
        Self::new(self.x * k, self.y * k, self.z * k)
    }
    pub fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
    pub fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
    pub fn add_scaled(self, o: Self, k: f64) -> Self {
        Self::new(self.x + o.x * k, self.y + o.y * k, self.z + o.z * k)
    }
    pub fn normalized(self) -> Self {
        let l = self.len();
        if l > 0.0 {
            self.scale(1.0 / l)
        } else {
            self
        }
    }
}

pub fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    if v < lo {
        lo
    } else if v > hi {
        hi
    } else {
        v
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Elements {
    pub a: f64,
    pub e: f64,
    pub i: f64,
    pub raan: f64,
    pub argp: f64,
    pub nu: f64,
    pub p: f64,
    pub pe: f64,
    pub ap: f64,
    pub period: f64,
    pub energy: f64,
    pub h: Vec3,
    pub e_vec: Vec3,
    pub p_hat: Vec3,
    pub q_hat: Vec3,
    pub closed: bool,
}

impl Elements {
    /// Packed in the order the JavaScript bridge expects.
    pub fn pack(&self) -> [f64; 23] {
        [
            self.a, self.e, self.i, self.raan, self.argp, self.nu, self.p, self.pe, self.ap,
            self.period, self.energy, self.h.x, self.h.y, self.h.z, self.e_vec.x, self.e_vec.y,
            self.e_vec.z, self.p_hat.x, self.p_hat.y, self.p_hat.z, self.q_hat.x, self.q_hat.y,
            self.q_hat.z,
        ]
    }

    pub fn altitude(&self) -> f64 {
        self.pe - EARTH_RADIUS
    }
}

pub fn elements_from_state(r: Vec3, v: Vec3, mu: f64) -> Elements {
    let rm = r.len();
    let vm = v.len();
    let h = r.cross(v);
    let hm = h.len();
    let energy = vm * vm / 2.0 - mu / rm;
    let a = if energy.abs() < 1e-12 {
        f64::INFINITY
    } else {
        -mu / (2.0 * energy)
    };

    let e_vec = r
        .scale(vm * vm - mu / rm)
        .add_scaled(v, -r.dot(v))
        .scale(1.0 / mu);
    let e = clamp(e_vec.len(), 0.0, 1e6);

    let mut inc = 0.0;
    if hm > EPS {
        inc = clamp(h.z / hm, -1.0, 1.0).acos();
    }

    // Node vector n = z_hat x h = (-h.y, h.x, 0)
    let n = Vec3::new(-h.y, h.x, 0.0);
    let nm = n.len();

    let mut raan = 0.0;
    if nm > EPS {
        raan = n.y.atan2(n.x);
    }

    let mut argp = 0.0;
    if nm > EPS && e > 1e-8 {
        argp = (e_vec.dot(n.cross(h)) / h.len()).atan2(e_vec.dot(n));
    } else if e > 1e-8 {
        argp = e_vec.y.atan2(e_vec.x);
    }
    let tau = std::f64::consts::PI * 2.0;
    argp = ((argp % tau) + tau) % tau;

    let nu;
    if e > 1e-8 {
        let q = h.cross(e_vec).scale(1.0 / hm);
        let denom = (e * rm).max(EPS);
        nu = (r.dot(q) / denom).atan2(r.dot(e_vec) / denom);
    } else if nm > EPS {
        let q = h.cross(n).scale(1.0 / (hm * nm));
        nu = (r.dot(q) / rm.max(EPS)).atan2(r.dot(n) / nm.max(EPS));
    } else {
        nu = r.y.atan2(r.x);
    }

    let p = hm * hm / mu;
    let p_hat = if e > 1e-8 {
        e_vec.scale(1.0 / e)
    } else if nm > EPS {
        n.scale(1.0 / nm)
    } else {
        Vec3::new(1.0, 0.0, 0.0)
    };
    let q_hat = h.cross(p_hat).normalized();

    let pe = p / (1.0 + e);
    let ap = if e < 1.0 {
        p / (1.0 - e)
    } else {
        f64::INFINITY
    };
    let period = if e < 1.0 && a.is_finite() {
        period_of(a, mu)
    } else {
        f64::INFINITY
    };

    Elements {
        a,
        e,
        i: inc,
        raan,
        argp,
        nu,
        p,
        pe,
        ap,
        period,
        energy,
        h,
        e_vec,
        p_hat,
        q_hat,
        closed: e < 1.0,
    }
}

/// Inertial state from the perifocal basis plus conic parameters.
pub fn state_from_elements(
    p_hat: Vec3,
    q_hat: Vec3,
    p: f64,
    e: f64,
    nu: f64,
    mu: f64,
) -> (Vec3, Vec3) {
    let r = p / (1.0 + e * nu.cos());
    let cnu = nu.cos();
    let snu = nu.sin();
    let pos = p_hat.scale(r * cnu).add_scaled(q_hat, r * snu);
    let k = (mu / p).sqrt();
    let vel = p_hat.scale(-k * snu).add_scaled(q_hat, k * (e + cnu));
    (pos, vel)
}

/// Perifocal basis from the classical angles, matching `perifocalBasis`.
pub fn perifocal_basis(raan: f64, inc: f64, argp: f64) -> (Vec3, Vec3) {
    let p1 = Vec3::new(raan.cos(), raan.sin(), 0.0);
    let q1 = Vec3::new(-raan.sin(), raan.cos(), 0.0);
    let w = Vec3::new(0.0, 0.0, 1.0);
    let p = p1.scale(argp.cos()).add_scaled(q1, argp.sin());
    let q2 = w.cross(p1);
    let q = q2.scale(argp.cos()).add_scaled(p1, -argp.sin());
    let p = rotate_about(p, p1, inc);
    let q = rotate_about(q, p1, inc);
    (p.normalized(), q.normalized())
}

/// Rodrigues rotation of `v` about `axis` by `angle`.
fn rotate_about(v: Vec3, axis: Vec3, angle: f64) -> Vec3 {
    let k = axis.normalized();
    let cos = angle.cos();
    let sin = angle.sin();
    v.scale(cos)
        .add_scaled(k.cross(v), sin)
        .add_scaled(k, k.dot(v) * (1.0 - cos))
}

pub fn period_of(a: f64, mu: f64) -> f64 {
    2.0 * std::f64::consts::PI * (a * a * a / mu).sqrt()
}

/// Time to the next periapsis (`apoapsis == false`) or apoapsis, in seconds.
pub fn time_to_apsis(a: f64, e: f64, nu: f64, apoapsis: bool, mu: f64) -> f64 {
    if !(e < 1.0) || !a.is_finite() {
        return f64::INFINITY;
    }
    let n = (mu / (a * a * a)).sqrt();
    let target = if apoapsis {
        std::f64::consts::PI
    } else {
        0.0
    };
    let mut dnu = target - nu;
    if dnu <= 0.0 {
        dnu += std::f64::consts::PI * 2.0;
    }
    let e0 = 2.0
        * (((1.0 - e).sqrt() * (nu / 2.0).sin())
            .atan2((1.0 + e).sqrt() * (nu / 2.0).cos()));
    let e1 = 2.0
        * (((1.0 - e).sqrt() * ((nu + dnu) / 2.0).sin())
            .atan2((1.0 + e).sqrt() * ((nu + dnu) / 2.0).cos()));
    let m0 = e0 - e * e0.sin();
    let m1 = e1 - e * e1.sin();
    let mut dm = m1 - m0;
    if dm < 0.0 {
        dm += std::f64::consts::PI * 2.0;
    }
    dm / n
}

/// J2 is re-exported here so callers do not need to reach into the crate root.
pub const J2: f64 = EARTH_J2;

pub fn default_mu() -> f64 {
    EARTH_MU
}
