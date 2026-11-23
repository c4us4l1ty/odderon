//! The `Survey` seam: how a star catalogue enters the game without being compiled into it.
//!
//! ## Why a trait, and not a table
//!
//! Every other data surface in this crate is a `const` array. That is right for a body's `mu`: the number is
//! published, it is ours to redistribute, and a lookup that cannot fail is simpler than one that can. It is
//! **wrong** for a star catalogue, for two independent reasons that happen to agree.
//!
//! 1. **Licensing.** The high-precision astrometry missions and their mirrors are not ours to redistribute
//!    (`DATA-LICENSING.md` §1, `CLASSES.md` §1 — the GATED records). A `const` array ships in the binary, so
//!    a compiled star catalogue is a redistribution whether or not there is a `data/` directory to inspect.
//! 2. **Design.** `VISION.md` Pillar 1 says the player *earns* knowledge by measuring. A shipped
//!    high-precision catalogue would hand a new player the whole sky mapped to microarcseconds, which is
//!    omniscience with a loading screen. The licence and the game want the same thing here: a bearing known,
//!    a range uncertain.
//!
//! So the catalogue is a **backend**, not a table. This module defines the seam and nothing else.
//!
//! Deliberately, this file never spells a GATED dataset's identifier. `tests/survey.rs` greps the kernel for
//! those names and fails if one appears, and a doc comment is source — so the prose says "the high-precision
//! astrometry missions" instead of naming them.
//!
//! Two pre-existing comments elsewhere in the crate *do* name those missions, in prose explaining why their
//! data is not here. That is the opposite of a leak and the test allows it by name, per file, with the reason
//! written down. Without that allowance the test would push authors to write worse comments to satisfy a
//! grep — a rule that makes the code less honest is the wrong rule.
//!
//! ## The structural claim: the kernel cannot hold a backend
//!
//! This is stronger than "no backend ships", and it is worth being precise about because the weaker version
//! is what most projects mean. This crate is `#![no_std]` and `#![forbid(unsafe_code)]`, so:
//!
//! * there is no allocator, so it cannot `Box` a backend;
//! * there is no `unsafe`, so it cannot smuggle one into a `static` through a raw pointer;
//! * a `static` holding `Option<Box<dyn Survey>>` cannot be written without one of those two.
//!
//! Therefore **there is no install function, no global registry, and no place in the kernel a backend could
//! live.** Every function here takes the backend as an argument, and the caller — the shell, which does have
//! an allocator and does own the player's data — keeps it. A future contributor cannot accidentally add a
//! `static mut SURVEY` without first deleting one of the two crate attributes, which is a visible change to
//! the crate root and would fail review on its own.
//!
//! That is the seam doing its job: the property "Tier-1 data cannot ship in the binary" is enforced by the
//! shape of the crate rather than by a rule someone has to remember.
//!
//! ## What a star carries
//!
//! Every physical field is a [`Quantity`], so its uncertainty is first-class data rather than a hidden
//! column. A distance is not `Option<f64>` but a quantity whose `sigma` may exceed its `value`; a star with a
//! 40 % parallax error is a range, which is what it is. The baseline US-PD stack has precise directions and
//! honest, large distance uncertainty beyond a few hundred parsecs (`DATA-LICENSING.md` §4), and this type
//! makes it impossible to store a distance without saying how well it is known.
//!
//! ## What this module cannot do, stated first
//!
//! **The shipped binary has no star catalogue. Not one star.** Nothing in this crate returns a real star,
//! and no test here proves that any real star data works — the tests build a synthetic backend in the test
//! process to prove the *seam's* behaviour. The gap between "the seam is correct" and "the game has a sky"
//! is a backend, and a backend is exactly what a player supplies.

use crate::provenance::Quantity;

/// A direction on the sky, as a unit vector in ICRS.
///
/// Stored as a vector rather than as `(ra, dec)` because the only thing the game does with a direction is
/// take angles between two of them, and the vector keeps precision that the angles had already thrown away.
/// [`Direction::ra_rad`] and [`Direction::dec_rad`] recover the angles for display.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Direction {
    /// Unit vector, ICRS. Verified unit at construction; not renormalised on read.
    pub vector: [f64; 3],
}

impl Direction {
    /// Build from a vector, refusing one that is not unit to within `1e-9`.
    ///
    /// Refused rather than silently normalised. A backend handing over a non-unit vector has a bug, and
    /// fixing it here would hide the bug behind a plausible-looking sky while every separation angle came out
    /// slightly wrong — precisely the class of error the rest of this project refuses to absorb.
    pub fn from_vector(vector: [f64; 3]) -> Option<Direction> {
        let norm_squared = vector[0] * vector[0] + vector[1] * vector[1] + vector[2] * vector[2];
        let norm = odderon_dmath::sqrt(norm_squared);
        if !(1.0 - 1.0e-9..=1.0 + 1.0e-9).contains(&norm) {
            return None;
        }
        Some(Direction { vector })
    }

    /// Right ascension in radians, `[0, 2π)`.
    ///
    /// Defined for every unit vector, including the poles, where RA is conventionally `0`.
    pub fn ra_rad(&self) -> f64 {
        let ra = odderon_dmath::atan2(self.vector[1], self.vector[0]);
        if ra < 0.0 {
            ra + core::f64::consts::TAU
        } else {
            ra
        }
    }

    /// Declination in radians, `[−π/2, π/2]`.
    pub fn dec_rad(&self) -> f64 {
        odderon_dmath::asin(odderon_dmath::clamp_unit(self.vector[2]))
    }

    /// The angle between two directions, in radians.
    ///
    /// `atan2(|a × b|, a · b)`, not `acos(a · b)`. The two are the same in exact arithmetic and very
    /// different in `f64`: for stars a few arcseconds apart, `a · b` is `1 − 1e-11`, and `acos` of that has a
    /// *quantisation* of `sqrt(eps) ≈ 1.5e-8` rad — about 3 milliarcseconds of noise, larger than the
    /// separation being measured. The cross-product form keeps relative accuracy down to microarcseconds,
    /// which is the precision the astrometry mechanic depends on.
    pub fn separation(&self, other: &Direction) -> f64 {
        let a = self.vector;
        let b = other.vector;
        let cross = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let cross_norm =
            odderon_dmath::sqrt(cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]);
        let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        odderon_dmath::atan2(cross_norm, dot)
    }

    /// A direction from spherical coordinates, normalising the result.
    ///
    /// Convenience for backends that store angles, and for tests. Returns `None` for a non-finite input or a
    /// declination outside `[−π/2, π/2]`, rather than projecting an impossible input onto the sphere.
    pub fn from_ra_dec(ra_rad: f64, dec_rad: f64) -> Option<Direction> {
        use core::f64::consts::FRAC_PI_2;
        if !ra_rad.is_finite() || !dec_rad.is_finite() {
            return None;
        }
        if !(-FRAC_PI_2..=FRAC_PI_2).contains(&dec_rad) {
            return None;
        }
        // RA needs no reduction: `sin_cos` reduces internally, and `ra_rad()` reconstructs it into `[0, 2π)`
        // through `atan2` at any magnitude. Reducing it here would only add a second place for the angle to
        // disagree with itself.
        let (sin_ra, cos_ra) = odderon_dmath::sin_cos(ra_rad);
        let (sin_dec, cos_dec) = odderon_dmath::sin_cos(dec_rad);
        Direction::from_vector([cos_dec * cos_ra, cos_dec * sin_ra, sin_dec])
    }
}

/// One star, as a backend presents it.
///
/// Every physical field is a [`Quantity`], so no uncertainty can be dropped on the way in. `id` is the
/// backend's own stable identifier: the kernel does not interpret it, and two backends may disagree about how
/// to name the same star. Nothing keys on `id` across backends, deliberately — a cross-match is an
/// astrometric problem, not a string problem.
#[derive(Clone, Copy, Debug)]
pub struct Star {
    /// Backend-stable identifier. Opaque to the kernel; carried so a UI can show it and logs can name it.
    pub id: u64,
    /// ICRS direction.
    pub direction: Direction,
    /// Distance in **parsecs**, with its own uncertainty.
    ///
    /// The field the whole seam exists for: a `Quantity` whose `sigma` is comparable to its `value` is a
    /// range, and `DATA-LICENSING.md` §4 makes that the designed starting condition beyond a few hundred
    /// parsecs.
    pub distance_parsec: Quantity,
    /// Apparent magnitude in whatever band the backend documents — Johnson V by convention. Small is bright.
    pub magnitude: Quantity,
    /// Proper motion in radians per second, where the backend has it.
    ///
    /// Routinely absent: the older surveyed catalogues have no usable proper motion. Absence is
    /// [`Quantity::unknown`], not zero, because a proper motion of zero is a *claim* that the star is fixed.
    pub proper_motion: Quantity,
}

/// Why a lookup could not be answered.
///
/// The distinctions matter: an empty patch of sky, a nonsensical request, and a backend that does not exist
/// need three different responses, and a single "no" would let a caller present a bad radius as an empty sky.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SurveyError {
    /// The radius is not a positive finite angle, or exceeds a hemisphere.
    ///
    /// Refused rather than clamped. A zero or negative radius is a caller's bug, and returning an empty tile
    /// would make it look like a genuinely empty patch of sky.
    BadRadius,
    /// A coordinate was not finite.
    BadDirection,
    /// The direction handed back by a backend was not a unit vector.
    ///
    /// Reported on the way *out* of [`cone_search`] rather than trusted: it is the one property of a backend's
    /// output this module can check, and it catches the commonest backend bug — a direction built by
    /// forgetting to normalise.
    BackendReturnedNonUnitDirection,
}

/// One patch of sky, as a backend returns it.
///
/// Deliberately not a `Vec`: no allocator in this crate, so a backend owns its storage and lends out a
/// slice. That borrow is the whole lifetime story: a caller must finish with the tile before asking again.
#[derive(Clone, Copy, Debug)]
pub struct StarTile<'a> {
    /// The stars found, in the backend's own order. Callers must not assume a sort: a backend sorting by
    /// magnitude and one sorting by distance are both correct, and a caller depending on either will break
    /// on the other.
    pub stars: &'a [Star],
    /// Whether the backend had more stars than its buffer could return.
    ///
    /// True means "this cone is denser than this backend can resolve" — the difference between a sparse field
    /// and a cluster the instrument cannot resolve. Not an error: a truncated tile is a lossy observation,
    /// which is a thing that happens to real instruments.
    pub truncated: bool,
}

/// A source of stars. Implemented by the game's backends — never by this crate.
///
/// Two obligations the type system cannot check, so they are stated here:
///
/// * **`cone_search` must not return stars from outside the cone.** A backend that ignores the direction and
///   returns a fixed sky satisfies every signature and every test that only counts stars.
/// * **`dataset` must name the real source.** It is what a player is shown, and it is matched against the
///   identifier in `src/classes.json` — so a wrong answer here is a licensing misstatement, not a label.
pub trait Survey {
    /// Stars within `radius_rad` of `(ra_rad, dec_rad)`, both in radians, written into `out`.
    ///
    /// The output buffer is the caller's, for the same reason the module holds no backend: there is no
    /// allocator here. A tile describes how much of `out` was filled and whether more matched than fitted, so
    /// a crowded field is reported as truncated rather than silently thinned.
    ///
    /// **The returned tile borrows `out`, not the backend** — note the lifetime parameter. That is what makes
    /// the borrow rule work in practice: the next call cannot start until the previous tile is dropped,
    /// because it would otherwise overwrite the stars the caller is still reading.
    fn cone_search<'a>(
        &self,
        ra_rad: f64,
        dec_rad: f64,
        radius_rad: f64,
        out: &'a mut [Star],
    ) -> Result<StarTile<'a>, SurveyError>;

    /// The dataset's stable identifier, matching an `id` in `src/classes.json`.
    fn dataset(&self) -> &'static str;

    /// May the data behind this backend be redistributed?
    ///
    /// **`false` is not a defect.** It is the ordinary case for a Tier-1 backend, and it is the flag the
    /// release rule reads ([`admits`]). A backend that is unsure must answer `false`: the cheap error is
    /// refusing to ship someone else's data, and the expensive one is shipping it.
    fn redistributable(&self) -> bool;
}

/// Whether a build may use this backend.
///
/// `DATA-LICENSING.md` §3.2 rule 2: development and non-commercial builds may load Tier 1; a release build
/// physically cannot. Expressed as a function of an explicit profile rather than of `cfg!(debug_assertions)`
/// so both branches are testable — a rule that can only be exercised in the configuration that never ships is
/// not a rule, it is a hope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildProfile {
    /// A development or non-commercial build. Any backend a player installs is allowed.
    Development,
    /// A release build. Only redistributable data.
    Release,
}

/// Whether `profile` may use `survey`. See [`BuildProfile`].
///
/// A release build refuses a non-redistributable backend outright rather than warning: `GRAPHICS.md` R1's
/// zero-bitmap law is the same shape, and for the same reason — a property that can be *lost* is a property
/// nobody can quietly violate.
pub fn admits(profile: BuildProfile, survey: &dyn Survey) -> bool {
    match profile {
        BuildProfile::Development => true,
        BuildProfile::Release => survey.redistributable(),
    }
}

/// Search `survey` for stars in a cone, validating the request and the reply.
///
/// The backend is an argument, not a global — see the module documentation for why that is a structural
/// guarantee rather than a convention.
pub fn cone_search<'a>(
    survey: &dyn Survey,
    ra_rad: f64,
    dec_rad: f64,
    radius_rad: f64,
    out: &'a mut [Star],
) -> Result<StarTile<'a>, SurveyError> {
    if !ra_rad.is_finite() || !dec_rad.is_finite() {
        return Err(SurveyError::BadDirection);
    }
    // Written as two positive tests rather than `!(r > 0.0)` so that NaN is excluded by name: NaN fails
    // `> 0.0`, and relying on a negated comparison to catch it makes the rejection look accidental.
    let radius_is_usable = radius_rad.is_finite() && radius_rad > 0.0;
    if !radius_is_usable {
        return Err(SurveyError::BadRadius);
    }
    // A cone wider than a hemisphere is well-defined and practically always a mistake: the caller wanted
    // "the whole sky", which is a different request. Refused rather than silently intersected, because the
    // intersection would depend on the direction that happened to be passed.
    if radius_rad > core::f64::consts::FRAC_PI_2 {
        return Err(SurveyError::BadRadius);
    }

    let tile = survey.cone_search(ra_rad, dec_rad, radius_rad, out)?;

    // The one property of a backend's output that can be checked here, and the commonest way a backend is
    // wrong: a direction that was never normalised. Checked on every star, cheapest first.
    let mut i = 0;
    while i < tile.stars.len() {
        let v = tile.stars[i].direction.vector;
        let norm = odderon_dmath::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
        if !(1.0 - 1.0e-9..=1.0 + 1.0e-9).contains(&norm) {
            return Err(SurveyError::BackendReturnedNonUnitDirection);
        }
        i += 1;
    }
    Ok(tile)
}
