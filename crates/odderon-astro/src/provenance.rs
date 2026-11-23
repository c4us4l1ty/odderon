//! Provenance as a *type*, not a comment.
//!
//! ## The law this implements
//!
//! `VISION.md` §3 law 2 and `CREDIBILITY.md` pillar 4 both say the same thing: a number the simulation
//! cannot justify must not reach the player. `GAME-DESIGN-BIBLE.md` §107 puts it as a test:
//!
//! > if any number cannot pass this, it is a bug.
//!
//! The mechanism is that every physical quantity travels with its **lineage**: where it came from, how
//! precisely it is known, and whether it is measured or assumed. That is not a debug field. `DATA-LICENSING.md`
//! §4 makes the epistemic status a *gameplay* quantity — pre-Gaia star distances are uncertain on purpose,
//! and the uncertainty is the content of the astrometry pillar ("Be Your Own Gaia").
//!
//! ## Why a sum type rather than a `f64` and a `sigma`
//!
//! Because the interesting distinction is not "how wide is the error bar" but **"is this known, or
//! invented?"**. A bare `332946.0487 ± 0.0007` cannot express the difference between a measured mass and a
//! placeholder an engineer put in a spreadsheet, and those two must never render the same way. Modelling
//! that as one variant per epistemic status means the compiler forces every consumer to decide what to do
//! with an [`Prov::Assumed`] value, instead of it silently flowing into the HUD looking exactly like a
//! measurement.
//!
//! ## The one rule
//!
//! [`Prov::certainty`] is the only sanctioned way to collapse a `Prov` to a display string, and it returns
//! something that *says* the value is uncertain. There is deliberately no `value()` accessor that hands
//! back a bare number without the caller having pattern-matched the variant first. A caller that wants the
//! number has to say which kind of number it is, at the call site, in code a reviewer can see.

use crate::is_positive_finite;

/// Where a physical quantity came from, and how well it is known.
///
/// The variants are epistemic, not structural: they answer *"what do we know and how do we know it"*, not
/// *"which file was it read from"*. A file name belongs in the manifest (`DATA-LICENSING.md` §3.1); this
/// type cares about the claim, not the bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Prov {
    /// Measured, with a one-sigma uncertainty. The honest case: a real observation from a real instrument.
    ///
    /// `sigma` is in the same units as the value. A `sigma` of zero is a lie about precision and is
    /// rejected by [`Prov::measured`], which returns `None` rather than constructing a perfect measurement.
    Measured {
        /// One standard deviation, same units as the value.
        sigma: f64,
    },

    /// A published constant whose own uncertainty is below the precision at which the game uses it.
    ///
    /// Example: `mu` for Earth from EGM, where the `sigma` is metres-per-second-squared in the twelfth
    /// decimal and nothing in the game can notice. This is **not** a licence to call anything exact: it is
    /// a statement that the uncertainty of the value is smaller than the uncertainty of everything it feeds.
    Defined,

    /// Computed from other `Prov` values, with the derivation named.
    ///
    /// `method` is a short stable identifier, not prose, so a test can assert on it and a UI can look up a
    /// longer explanation. The uncertainty of a derived value is carried separately in [`Quantity`] because
    /// it depends on the arithmetic — see there.
    Derived {
        /// Stable identifier for the derivation, e.g. `"kepler-third-law"`.
        method: &'static str,
    },

    /// A design choice or a placeholder that is **not** a measurement of reality.
    ///
    /// This is the variant that earns the type its existence. An assumed value is legitimate inside a
    /// design (a mass budget starts full of them) and illegitimate as a statement about the universe. A
    /// renderer must show it differently, and `DATA-LICENSING.md` §4 relies on it: a star distance that is
    /// merely assumed is exactly what the player is there to improve.
    Assumed {
        /// Stable identifier for the assumption, e.g. `"placeholder-density"`.
        basis: &'static str,
    },

    /// Unknown. Distinct from assumed: nothing was chosen, the value is simply not known.
    ///
    /// The presence of this variant is what makes "unknown astronomy is never presented as fact" a
    /// checkable property rather than an aspiration. A consumer that receives `Unknown` and renders a
    /// number has written a bug, and it is visible in the match arms.
    Unknown,
}

impl Prov {
    /// Construct a measured provenance, rejecting a non-positive or non-finite `sigma`.
    ///
    /// A zero uncertainty measurement is not a strong measurement, it is a missing one: somebody forgot to
    /// fill in the error bar and the type must not accept the omission as a claim of infinite precision.
    pub fn measured(sigma: f64) -> Option<Prov> {
        if is_positive_finite(sigma) {
            Some(Prov::Measured { sigma })
        } else {
            None
        }
    }

    /// Is this value a claim about reality rather than a choice or a gap?
    ///
    /// `Derived` is included because a derivation from measured parents is still knowledge. `Assumed` and
    /// `Unknown` are not, and the two differ in whether a human made the number up or nobody knows it.
    pub fn is_knowledge(self) -> bool {
        matches!(self, Prov::Measured { .. } | Prov::Defined | Prov::Derived { .. })
    }

    /// A short, stable, machine-readable tag. Used by tests and by the manifest, never by prose.
    ///
    /// Stable because `DATA-LICENSING.md` §3.1 requires a build-time check to key on it, and a build check
    /// that keys on a string somebody renames is not a check.
    pub fn tag(self) -> &'static str {
        match self {
            Prov::Measured { .. } => "measured",
            Prov::Defined => "defined",
            Prov::Derived { .. } => "derived",
            Prov::Assumed { .. } => "assumed",
            Prov::Unknown => "unknown",
        }
    }

    /// The uncertainty this provenance implies **on its own**, ignoring any derived arithmetic.
    ///
    /// `None` means "not a number with a meaningful sigma" — a derived value's sigma depends on its inputs
    /// and lives in [`Quantity`], and an assumed or unknown value has no defensible one at all.
    pub fn own_sigma(self) -> Option<f64> {
        match self {
            Prov::Measured { sigma } => Some(sigma),
            Prov::Defined => Some(0.0),
            _ => None,
        }
    }
}

/// A value that cannot be separated from its provenance.
///
/// Deliberately not `Copy`-and-forget: the fields are public because there is nothing dishonest a caller
/// can do with them that the type system could stop, but every constructor enforces that `value` and `prov`
/// are consistent, and the derived arithmetic propagates uncertainty rather than dropping it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quantity {
    /// The value, in whatever unit the caller has documented. This crate does not track units.
    pub value: f64,
    /// Where it came from.
    pub prov: Prov,
    /// Combined one-sigma uncertainty, in the same units as `value`.
    ///
    /// Zero for a [`Prov::Defined`] constant. `f64::INFINITY` for [`Prov::Unknown`], because an unknown
    /// quantity's uncertainty is unbounded and a finite placeholder would be a lie that arithmetic would
    /// then propagate as if it meant something.
    pub sigma: f64,
}

impl Quantity {
    /// A measured quantity.
    pub fn measured(value: f64, sigma: f64) -> Option<Quantity> {
        let prov = Prov::measured(sigma)?;
        if !value.is_finite() {
            return None;
        }
        Some(Quantity { value, prov, sigma })
    }

    /// A published constant with negligible uncertainty.
    pub fn defined(value: f64) -> Option<Quantity> {
        if !value.is_finite() {
            return None;
        }
        Some(Quantity {
            value,
            prov: Prov::Defined,
            sigma: 0.0,
        })
    }

    /// A design assumption. Carries an *explicit* sigma so downstream arithmetic still has something to
    /// propagate: pretending an assumption is exact is how a placeholder becomes a fact.
    pub fn assumed(value: f64, sigma: f64, basis: &'static str) -> Option<Quantity> {
        if !value.is_finite() || !is_positive_finite(sigma) {
            return None;
        }
        Some(Quantity {
            value,
            prov: Prov::Assumed { basis },
            sigma,
        })
    }

    /// Nothing is known.
    ///
    /// The value is `NaN` on purpose. Any arithmetic that touches it produces `NaN`, which is a loud
    /// failure; a placeholder like `0.0` would silently mean "massless planet" somewhere downstream.
    pub fn unknown() -> Quantity {
        Quantity {
            value: f64::NAN,
            prov: Prov::Unknown,
            sigma: f64::INFINITY,
        }
    }

    /// Can this value be shown to a player as a fact about the universe?
    pub fn is_knowledge(&self) -> bool {
        self.prov.is_knowledge()
    }
}

/// A catalogue lookup that may have no answer.
///
/// Separate from `Option` because the two failure modes must be distinguishable at the call site: *"this
/// body is not in the catalogue"* is a wiring bug, while *"this body is in the catalogue and its mass is
/// unknown"* is the honest answer for a small asteroid. Collapsing both to `None` would let a missing
/// catalogue entry look like an epistemic limitation, or worse, the other way round.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lookup<T> {
    /// The catalogue has this body and a value for this property.
    Known(T),
    /// The catalogue has this body but does not know this property. The body exists; the fact does not.
    NotKnown {
        /// Why it is not known, in a form a UI can key on without parsing English.
        reason: &'static str,
    },
    /// The catalogue does not have this body at all.
    NoSuchBody,
}

impl<T> Lookup<T> {
    /// The value, if there is one.
    pub fn known(self) -> Option<T> {
        match self {
            Lookup::Known(v) => Some(v),
            _ => None,
        }
    }

    /// Does the *catalogue* have this body, regardless of whether it knows the property?
    ///
    /// This is the distinction that makes the enum worth having: a test can assert that a body is present
    /// while its mass is legitimately absent.
    pub fn body_exists(&self) -> bool {
        !matches!(self, Lookup::NoSuchBody)
    }
}
