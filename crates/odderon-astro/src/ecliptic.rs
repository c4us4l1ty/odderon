//! Ecliptic frame: the ICRF ↔ ecliptic transformation.
//!
//! ## Why this exists
//!
//! The inertial frame the kernel propagates in is **ICRF-aligned geocentric** — Earth-centred, aligned
//! with the International Celestial Reference Frame. The ecliptic frame is the same origin rotated so
//! the x-axis points toward the vernal equinox and the z-axis is normal to the ecliptic plane.
//!
//! Interplanetary work lives in the ecliptic: planetary orbits are nearly coplanar with the ecliptic,
//! so a transfer trajectory planned in the ecliptic is a two-dimensional problem. The ICRF frame, by
//! contrast, is tilted by the obliquity of the ecliptic (~23.4°), which makes every interplanetary
//! trajectory look like it has a significant out-of-plane component that is actually just the frame's
//! tilt. The ecliptic is the plane the solar system actually lives in.
//!
//! ## Why this does not need an ephemeris
//!
//! The [`crate::frames`] module's `BodyFixed` transform needs a body's prime-meridian angle `W₀`,
//! which is a per-body published constant. The ecliptic frame needs **one number**: the obliquity of
//! the ecliptic, `ε` — the angle between the ecliptic pole and the celestial pole. That is a single
//! published constant, not a table of per-body data. The transformation is a pure rotation about the
//! x-axis by `ε`, and it has no epoch dependence beyond the secular drift of `ε` itself, which is
//! ~47 arcseconds per century — negligible over any mission but stated for completeness.
//!
//! `CREDIBILITY.md` §3.1 lists this row as "partially implemented" with the note that "there is no
//! ephemeris yet, and no ecliptic frame, so there is nothing to rotate between." The second half of
//! that reasoning is wrong: the ecliptic frame is a pure rotation from ICRF and needs no ephemeris.
//! What the ephemeris is needed for is the *body-fixed* frames of other planets, which is a separate
//! data task. This module provides the ecliptic leg; the body-fixed leg for non-Earth bodies remains
//! a data task, not a mathematical one.
//!
//! ## The transformation
//!
//! ICRF → ecliptic is a rotation about the x-axis by `+ε`:
//!
//! ```text
//! x_ecl =  x_icrf
//! y_ecl =  y_icrf · cos ε + z_icrf · sin ε
//! z_ecl = -y_icrf · sin ε + z_icrf · cos ε
//! ```
//!
//! Ecliptic → ICRF is the inverse rotation about x by `−ε`. The two are exact inverses: composing
//! them returns the input vector to machine precision, which is what [`Ecliptic::round_trip`] checks.
//!
//! ## What must be true when this is right
//!
//! * **Round trip closes:** ICRF → ecliptic → ICRF returns the input to machine precision. A rotation
//!   and its inverse must compose to the identity.
//! * **Magnitudes are preserved:** a rotation does not change vector lengths. Checked on vectors of
//!   several magnitudes and directions.
//! * **Angles are preserved:** the angle between two vectors is the same in both frames.
//! * **The ecliptic north pole maps correctly:** the ecliptic north pole `(0, 0, 1)` in the ecliptic
//!   frame maps to `(0, −sin ε, cos ε)` in ICRF — the celestial pole, tilted from the ecliptic pole
//!   by exactly the obliquity. This is the physical meaning of `ε` and the cheapest possible check
//!   that the rotation is about the right axis in the right direction.
//! * **The vernal equinox is on both axes:** the direction `(1, 0, 0)` — the vernal equinox — is the
//!   x-axis in both frames, so it maps to itself. This catches a sign error on the rotation angle,
//!   which would tilt the x-axis away from the equinox.
//! * **The obliquity matches the published value:** at J2000, `ε = 23.439291°` (IAU 2006). This is
//!   the value the transformation is built from, and the test pins it against the published constant
//!   rather than against the code's own arithmetic.
//! * **The secular drift is in the right direction:** `ε` decreases over time (the ecliptic pole
//!   precesses toward the celestial pole). The rate is ~−47″/century, and the test checks that the
//!   computed rate at a future epoch is less than at J2000.
//!
//! Each of those is an assertion in `tests/ecliptic.rs`, with the tolerance written before the
//! measurement.

use crate::provenance::Quantity;
use crate::{cos_rad, sin_rad, Vec3};
use odderon_dmath::acos;

/// Seconds per day.
pub const SECONDS_PER_DAY: f64 = 86_400.0;

/// The obliquity of the ecliptic at J2000, in **radians**.
///
/// The published IAU 2006 value is 23.439291° (the mean obliquity at J2000, from the IAU 2006
/// precession model). This is the angle between the mean ecliptic pole and the mean celestial pole,
/// and it is the rotation angle the ICRF ↔ ecliptic transform uses.
///
/// **Source:** IAU 2006 precession model (Capitaine et al. 2003), as adopted by the IAU Resolution
/// B1 (2006). The value is a `Defined` quantity because it is a published physical constant, not a
/// modelling choice.
pub const OBLIQUITY_J2000_RAD: f64 = 23.439_291_0 * core::f64::consts::PI / 180.0;

/// The secular rate of change of the obliquity, in **radians per second**.
///
/// The obliquity decreases over time: the ecliptic pole precesses toward the celestial pole at
/// about −47 arcseconds per century. The published rate is −0.00000036° per day (IAU 2006), which
/// converts to this constant.
///
/// The magnitude is small: over a century the obliquity changes by ~47″ ≈ 0.00023°, which is
/// ~2e-6 of the total angle. The rate is included so the transformation can be evaluated at any
/// epoch, not just J2000, but it is negligible for any mission shorter than a century.
pub const OBLIQUITY_RATE_RAD_PER_S: f64 = -0.000_000_36_0 * core::f64::consts::PI / 180.0 / SECONDS_PER_DAY;

/// The ecliptic frame: a rotation from ICRF to the ecliptic, parameterised by the obliquity.
///
/// Deliberately *not* `Default`: a default obliquity would silently be zero, which would make the
/// ecliptic frame identical to the ICRF frame — a perfectly self-consistent transform that is wrong
/// by 23.4°. That is the same failure mode `crate::frames::BodyFixed` is designed to prevent, and
/// this type follows the same rule.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ecliptic {
    /// Obliquity at J2000, radians.
    pub obliquity_j2000: Quantity,
    /// Secular rate of change, radians per second.
    pub obliquity_rate: Quantity,
}

impl Ecliptic {
    /// The standard ecliptic frame: IAU 2006 obliquity at J2000, with its secular drift.
    pub fn standard() -> Self {
        Ecliptic {
            obliquity_j2000: Quantity::defined(OBLIQUITY_J2000_RAD)
                .expect("obliquity J2000 is a finite published constant"),
            obliquity_rate: Quantity::defined(OBLIQUITY_RATE_RAD_PER_S)
                .expect("obliquity rate is a finite published constant"),
        }
    }

    /// The obliquity at `dt` seconds after J2000, in radians.
    ///
    /// `ε(t) = ε₀ + rate · t`. The secular term is tiny (~2e-6 rad per century), but it is included
    /// so the frame is correct at any epoch, not just J2000.
    pub fn obliquity_at(&self, dt: f64) -> Option<f64> {
        if !dt.is_finite() {
            return None;
        }
        let eps_0 = self.obliquity_j2000.value;
        let rate = self.obliquity_rate.value;
        if !eps_0.is_finite() || !rate.is_finite() {
            return None;
        }
        Some(eps_0 + rate * dt)
    }

    /// ICRF → ecliptic. Rotates about the x-axis by the obliquity at `dt`.
    ///
    /// Returns `None` when the obliquity at `dt` is not available (non-finite `dt` or non-finite
    /// constants). The rotation preserves vector magnitudes exactly — it is a rigid rotation, not a
    /// scaling — which is what the round-trip and magnitude tests check.
    pub fn to_ecliptic(&self, v: Vec3, dt: f64) -> Option<Vec3> {
        let eps = self.obliquity_at(dt)?;
        let c = cos_rad(eps);
        let s = sin_rad(eps);
        // Rotation about x by +ε: y' = y·cos ε + z·sin ε, z' = −y·sin ε + z·cos ε
        Some(Vec3::new(v.x, v.y * c + v.z * s, -v.y * s + v.z * c))
    }

    /// Ecliptic → ICRF. The exact inverse of [`Ecliptic::to_ecliptic`].
    pub fn to_icrf(&self, v: Vec3, dt: f64) -> Option<Vec3> {
        let eps = self.obliquity_at(dt)?;
        let c = cos_rad(eps);
        let s = sin_rad(eps);
        // Rotation about x by −ε: y' = y·cos ε − z·sin ε, z' = y·sin ε + z·cos ε
        Some(Vec3::new(v.x, v.y * c - v.z * s, v.y * s + v.z * c))
    }

    /// Round-trip a vector: ICRF → ecliptic → ICRF.
    ///
    /// The result must equal the input to machine precision. This is the fundamental check that the
    /// two transformations are exact inverses, and it is what the round-trip test asserts.
    pub fn round_trip(&self, v: Vec3, dt: f64) -> Option<Vec3> {
        let to_ecl = self.to_ecliptic(v, dt)?;
        self.to_icrf(to_ecl, dt)
    }
}

/// The angle between two vectors, in radians, via the dot product.
///
/// Used by the tests to verify that rotations preserve angles. Kept public because it is a useful
/// geometric primitive and the tests should use the kernel's own arithmetic, not a hand-rolled
/// implementation.
pub fn angle_between(a: Vec3, b: Vec3) -> Option<f64> {
    let na = a.norm();
    let nb = b.norm();
    if !crate::is_positive_finite(na) || !crate::is_positive_finite(nb) {
        return None;
    }
    let cos_theta = (a.dot(b) / (na * nb)).clamp(-1.0, 1.0);
    Some(acos(cos_theta))
}

/// The ecliptic north pole expressed in ICRF, at epoch `dt`.
///
/// The ecliptic north pole is `(0, 0, 1)` in the ecliptic frame. Rotating it to ICRF gives
/// `(0, −sin ε, cos ε)` — the celestial pole, tilted from the ecliptic pole by exactly the obliquity.
/// This is the physical meaning of the obliquity and the cheapest check that the rotation is about
/// the right axis in the right direction.
pub fn ecliptic_north_pole_icrf(dt: f64) -> Option<Vec3> {
    let eps = Ecliptic::standard().obliquity_at(dt)?;
    Some(Vec3::new(0.0, -sin_rad(eps), cos_rad(eps)))
}

/// The vernal equinox direction, which is the x-axis in both frames.
///
/// This vector maps to itself under the ICRF ↔ ecliptic transform, because the rotation is about the
/// x-axis. It is the direction that defines both frames and the reason they share an x-axis.
pub fn vernal_equinox() -> Vec3 {
    Vec3::new(1.0, 0.0, 0.0)
}
