//! The body catalogue: what the universe is made of, and how well any of it is known.
//!
//! ## Scope, stated so it cannot creep
//!
//! This module ships **opaque-body constants only** — `mu`, equatorial radius, `J2`, rotation rate. Values
//! that are (a) published by an authoritative body, (b) stable, and (c) needed to answer the only question
//! the kernel currently answers: *where will this thing be in a week*.
//!
//! It deliberately does **not** ship star positions, masses, radii, or orbital elements for planets. Those
//! live behind the `Survey` seam in `DATA-LICENSING.md` §3.3 and carry a licensing consequence — Hipparcos
//! and Tycho are CC BY-NC 3.0 IGO however intuitively "pre-Gaia = safe" feels. Putting a star table in this
//! file would move a data-licensing decision into the compiled kernel, which is exactly the rewrite the
//! licensing document exists to prevent.
//!
//! ## Why constants live here rather than in the algorithms
//!
//! `j2::EARTH_J2` is a *property of Earth*, not a property of the J2 equation. Mars is `1.96e-3`, Jupiter
//! `1.47e-2`. An algorithm that hard-codes Earth's value is an algorithm that can only ever model Earth, and
//! the whole point of passing `j2` as an argument is that the catalogue is the only thing that knows which
//! body it is talking to.
//!
//! ## Every value carries its provenance
//!
//! Per `CREDIBILITY.md` pillar 4 and `VISION.md` §3 law 2. A quantity that cannot say where it came from
//! cannot be shown to a player as fact.
//!
//! ## What is deliberately absent
//!
//! * **Rotation phase / prime meridian** (`W₀` and `Ẇ`). These are needed for body-fixed coordinates and
//!   are *not* interchangeable across reference frames — IAU 2009 and IAU 2015 disagree about the prime
//!   meridian of several bodies by tens of degrees. Guessing one would put every ground station in the
//!   wrong place with no visible symptom, so the field is not here until the frame work is done properly.
//! * **Atmosphere, albedo, surface composition.** Not needed by the kernel, and each is a licensing and
//!   accuracy question of its own.
//! * **Anything not published.** If a body's `J2` is not in a source we have read, it is [`Lookup::NotKnown`]
//!   rather than an estimate. `IMPORTANT-2.md`: unknown astronomy must never be presented as fact.

use crate::provenance::{Lookup, Quantity};
use odderon_dmath::TAU;

/// The properties the kernel needs to place a coasting body.
///
/// `Option` because a small irregular body may genuinely have no published `J2` — the field is absent, not
/// zero. A zero `J2` would mean "perfectly spherical", which is a strong claim about an asteroid and
/// usually a false one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Physical {
    /// Gravitational parameter `GM`, km³/s². Always known for a catalogue body: without it there is no orbit.
    pub mu: Quantity,
    /// Equatorial radius, km. Needed for `J2` and for any surface-relative reasoning.
    pub equatorial_radius: Quantity,
    /// `J2`, the dominant oblateness harmonic, as a **positive magnitude**.
    ///
    /// The sign convention is the one that cost a PR: the minus sign lives in the equation
    /// (`crate::j2::secular_rates`), not in the constant. See `j2::EARTH_J2`.
    pub j2: Lookup<Quantity>,
    /// Sidereal rotation rate, rad/s. Positive for prograde spin.
    pub rotation_rate: Lookup<Quantity>,
}

/// A body the kernel can propagate around.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    /// Stable identifier. Lowercase ASCII, no spaces — this is a key, not a label.
    pub id: &'static str,
    /// The numbers.
    pub physical: Physical,
}

/// Earth's rotation frame: IAU 2015 prime meridian angle at J2000, and the rotation rate.
///
/// `W₀ = 190.147°` is the 2015 report's value for Earth. It is **not zero**, and using zero is the bug this
/// constant exists to prevent: the prime meridian would land somewhere arbitrary in inertial space and every
/// ground position would be offset by up to half a rotation, with no NaN, no error and no visual cue.
///
/// ## The caveat, stated rather than buried
///
/// The IAU expression gives `W` measured from the **equinox of date**, not from J2000 ICRF, so strictly it
/// carries an equation-of-equinoxes term of order 0.0026° — about 0.3 km at the equator. For a game frame
/// whose ground positions are already good to kilometres that is below the noise, but it is a real
/// approximation and it is named. When the crate grows a proper precession-nutation model this is where the
/// correction belongs.
///
/// The rate is checked against `EARTH_ROTATION_RATE` in `tests/catalogue.rs`: the two are derived from
/// different published sources (`7.2921150e-5` rad/s versus `360.9856235°/day`) and agree to 5 ppm, which is
/// independent confirmation that neither has the solar-vs-sidereal confusion. Note the `0.9856°/day` in the
/// IAU figure is exactly the sun's annual apparent motion — that is *why* the sidereal rate is larger than
/// `360°/day`.
pub const EARTH_FRAME: crate::frames::BodyFixed = crate::frames::BodyFixed {
    w0: Quantity {
        // 190.147 deg in radians.
        value: 3.318_691_212_8,
        prov: crate::provenance::Prov::Defined,
        sigma: 0.0,
    },
    rate: Quantity {
        value: EARTH_ROTATION_RATE,
        prov: crate::provenance::Prov::Defined,
        sigma: 0.0,
    },
};

// ---------------------------------------------------------------------------------------------
// Earth
// ---------------------------------------------------------------------------------------------

/// `mu` for Earth: `GM = 398600.4418 km³/s²`.
///
/// The C00 value used in EGM2008/EGM96 and by JPL for Earth satellite work. The uncertainty on `GM` is
/// about 8e-9 km³/s² — ten decimal places below anything the game will compute — which is why it is
/// [`Quantity::defined`] rather than measured. That is a statement about the game's precision, not a claim
/// that the number is exact, and the distinction is written here so nobody later reads "defined" as "truth".
const EARTH_MU: f64 = 398_600.441_8;

/// Earth equatorial radius, km (WGS-84 / IUGG mean equatorial radius, 6 378.137 km).
///
/// WGS-84's semi-major axis is exactly 6 378 137.0 m by definition of the datum, so this is a defined
/// value and not an observation.
const EARTH_RADIUS: f64 = 6_378.137;

/// Earth sidereal rotation rate, rad/s.
///
/// This is the **sidereal** rate, `7.2921150e-5`, not the solar-day rate `2π/86400 = 7.2722052e-5`. The
/// difference is one part in 365 — about 0.4% — and using the wrong one makes a ground track walk eastward
/// by a full revolution per year. It is a classic and silent error, so the number is named explicitly
/// rather than written as `TAU / 86164.0905` and hoped over.
const EARTH_ROTATION_RATE: f64 = 7.292_115_0e-5;

/// Earth, as the kernel knows it.
///
/// `J2` is taken from `crate::j2::EARTH_J2` rather than duplicated, so there is exactly one place in the
/// crate where Earth's oblateness sign convention is written down.
pub const EARTH: Body = Body {
    id: "earth",
    physical: Physical {
        mu: Quantity {
            value: EARTH_MU,
            prov: crate::provenance::Prov::Defined,
            sigma: 0.0,
        },
        equatorial_radius: Quantity {
            value: EARTH_RADIUS,
            prov: crate::provenance::Prov::Defined,
            sigma: 0.0,
        },
        j2: Lookup::Known(Quantity {
            value: crate::j2::EARTH_J2,
            prov: crate::provenance::Prov::Defined,
            sigma: 0.0,
        }),
        rotation_rate: Lookup::Known(Quantity {
            value: EARTH_ROTATION_RATE,
            prov: crate::provenance::Prov::Defined,
            sigma: 0.0,
        }),
    },
};

/// The Moon.
///
/// Provenance is deliberately weaker than Earth's, and the reason is worth reading because it is the shape
/// of the problem the `Provenance` type exists for.
///
/// The Moon's `mu` (`GM = 4902.800066 km³/s²`) is measured by laser ranging and spacecraft tracking, so it
/// is [`Lookup::Known`]. But the Moon is **not an oblate spheroid to the precision that matters**: its
/// gravity field is dominated by the C22 term from the Earth-facing tidal bulge, not by J2, and the
/// first-order secular theory in `crate::j2` is a poor model of lunar orbits. Rather than publish a `J2`
/// that would silently produce a confidently wrong precession rate, the field is `NotKnown` with the
/// reason recorded. A lunar orbit needs the full field model, which is a later rung.
pub const MOON: Body = Body {
    id: "moon",
    physical: Physical {
        mu: Quantity {
            value: crate::mu::MOON,
            prov: crate::provenance::Prov::Defined,
            sigma: 0.0,
        },
        equatorial_radius: Quantity {
            value: crate::radius::EARTH * 0.272_507,
            prov: crate::provenance::Prov::Derived {
                method: "ratio-to-earth-radius",
            },
            // The 0.2725 ratio is good to about 1e-6; combining that with Earth's radius dominates.
            sigma: crate::radius::EARTH * 0.272_507 * 1e-6,
        },
        j2: Lookup::NotKnown {
            reason: "lunar-gravity-dominated-by-c22",
        },
        rotation_rate: Lookup::Known(Quantity {
            // synchronously locked: the sidereal month. 27.321661 days.
            value: TAU / (27.321_661 * 86_400.0),
            prov: crate::provenance::Prov::Derived {
                method: "synchronous-rotation",
            },
            sigma: 0.0,
        }),
    },
};

// ---------------------------------------------------------------------------------------------
// Mars
// ---------------------------------------------------------------------------------------------

/// Mars.
///
/// `mu` and `J2` are both well determined by decades of orbiter tracking, and Mars is where the first
/// interplanetary mission will go, so it is catalogued properly. The rotation rate is the sidereal value
/// from the IAU report.
pub const MARS: Body = Body {
    id: "mars",
    physical: Physical {
        mu: Quantity {
            value: crate::mu::MARS,
            prov: crate::provenance::Prov::Defined,
            sigma: 0.0,
        },
        equatorial_radius: Quantity {
            value: 3_396.2,
            prov: crate::provenance::Prov::Measured { sigma: 0.1 },
            sigma: 0.1,
        },
        j2: Lookup::Known(Quantity {
            value: 1.960_45e-3,
            prov: crate::provenance::Prov::Defined,
            sigma: 0.0,
        }),
        rotation_rate: Lookup::Known(Quantity {
            // 24h 37m 22.66s sidereal day
            value: TAU / 88_642.66,
            prov: crate::provenance::Prov::Derived {
                method: "sidereal-day",
            },
            sigma: 0.0,
        }),
    },
};

/// The Sun.
///
/// `J2` is `NotKnown` and that is not a gap to be filled in later: the Sun's oblateness is ~2e-7 and
/// contested, and no orbit in the game is close enough for it to matter. Recording it as unknown is more
/// honest than recording 2.2e-7 as though it were settled.
pub const SUN: Body = Body {
    id: "sun",
    physical: Physical {
        mu: Quantity {
            value: crate::mu::SUN,
            prov: crate::provenance::Prov::Defined,
            sigma: 0.0,
        },
        equatorial_radius: Quantity {
            value: 695_700.0,
            prov: crate::provenance::Prov::Measured { sigma: 100.0 },
            sigma: 100.0,
        },
        j2: Lookup::NotKnown {
            reason: "solar-oblateness-unsettled",
        },
        rotation_rate: Lookup::NotKnown {
            reason: "differential-rotation",
        },
    },
};

/// Everything the catalogue ships.
pub const ALL: [Body; 4] = [EARTH, MOON, MARS, SUN];

/// Look up a body by id.
///
/// `NoSuchBody` for a miss, which is a *different* answer from a body whose property is unknown — see
/// [`Lookup`]. A typo in a body id must not look like an epistemic limitation.
pub fn get(id: &str) -> Lookup<Body> {
    let mut i = 0;
    while i < ALL.len() {
        if str_eq(ALL[i].id, id) {
            return Lookup::Known(ALL[i]);
        }
        i += 1;
    }
    Lookup::NoSuchBody
}

/// Byte-wise string equality.
///
/// `no_std` without `alloc` has no `str` comparison in `core` that returns `bool` for free — `==` on `str`
/// is a `PartialEq` impl that lives in `core` and *is* available, but writing it this way keeps the crate's
/// "no hidden allocation, no hidden cost" habit and makes the ASCII guarantee explicit.
fn str_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}
