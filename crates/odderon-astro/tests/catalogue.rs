//! The catalogue's job is not to hold numbers. It is to be **incapable of presenting a guess as a fact**.
//!
//! So these tests are about the *type discipline* as much as the values, and the most important ones assert
//! that known-unknowns stay marked as unknown. `IMPORTANT-2.md` is binding on this: unknown astronomy must
//! never be presented as fact. A test suite that only checked `mu::EARTH` against a table would let someone
//! "complete" the catalogue by inventing the gaps, and every test would still pass.

use odderon_astro::catalogue::{self, EARTH, MARS, MOON, SUN};
use odderon_astro::j2::{secular_rates, EARTH_J2};
use odderon_astro::provenance::{Lookup, Prov, Quantity};
use odderon_astro::{mu, radius};
use odderon_dmath::fabs;

const MU: f64 = mu::EARTH;
const RE: f64 = radius::EARTH;

// ---------------------------------------------------------------------------------------------
// 1. The type refuses to fabricate
// ---------------------------------------------------------------------------------------------

/// A zero-uncertainty "measurement" is rejected, because it is a missing error bar, not infinite precision.
#[test]
fn a_measurement_must_carry_a_positive_uncertainty() {
    assert!(Prov::measured(0.0).is_none(), "zero sigma must be refused");
    assert!(Prov::measured(-1.0).is_none(), "negative sigma must be refused");
    assert!(Prov::measured(f64::NAN).is_none(), "NaN sigma must be refused");
    assert!(Prov::measured(f64::INFINITY).is_none(), "infinite sigma must be refused");
    assert!(Prov::measured(0.1).is_some(), "an ordinary sigma must be accepted");
}

/// `Unknown` carries NaN, so arithmetic on it fails loudly instead of silently meaning zero.
///
/// This is the whole reason the variant exists. If an unknown mass propagated as `0.0`, it would become a
/// massless body in a Newtonian propagator — an infinite acceleration, or more likely a division that
/// quietly produced `Inf` three modules away from the cause. NaN is loud; zero is a lie.
#[test]
fn unknown_poisons_arithmetic_instead_of_reading_as_zero() {
    let q = Quantity::unknown();
    assert!(!q.is_knowledge(), "unknown is not knowledge");
    assert!(q.value.is_nan(), "an unknown value must be NaN, not a placeholder number");
    assert!(q.sigma.is_infinite(), "an unknown quantity has unbounded uncertainty");
}

/// An assumption is explicitly *not* knowledge, even though it has a usable number and uncertainty.
///
/// This is the distinction that makes the type worth its weight. A design assumption is legitimate — a mass
/// budget is full of them — and it must never render the same way as a measurement.
#[test]
fn an_assumption_is_not_knowledge_even_though_it_has_a_number() {
    let q = Quantity::assumed(1500.0, 300.0, "placeholder-density").expect("valid assumption");
    assert!((q.value - 1500.0).abs() < 1e-12, "the number is usable");
    assert!(q.sigma > 0.0, "and so is the uncertainty");
    assert!(!q.is_knowledge(), "but it is not knowledge");
    assert_eq!(q.prov.tag(), "assumed");
}

/// A derived value is knowledge, because its parents were.
#[test]
fn derivation_preserves_epistemic_status() {
    let q = Quantity {
        value: 1.0,
        prov: Prov::Derived { method: "kepler-third-law" },
        sigma: 1e-6,
    };
    assert!(q.is_knowledge());
    assert_eq!(q.prov.tag(), "derived");
}

// ---------------------------------------------------------------------------------------------
// 2. Unknown stays unknown — the anti-fabrication tests
// ---------------------------------------------------------------------------------------------

/// Bodies whose parameters are not defensibly known must stay `NotKnown` while still *existing*.
///
/// The Moon's gravity is dominated by C22 rather than J2, so first-order J2 secular theory would give a
/// confidently wrong precession rate. Recording that as `NotKnown { reason }` is the honest answer; a `J2`
/// of 2.03e-4 copied from a table without asking whether the model applies would be a fabrication with a
/// decimal point on it.
#[test]
fn the_moon_says_it_does_not_know_its_j2_rather_than_guessing() {
    match MOON.physical.j2 {
        Lookup::NotKnown { reason } => {
            assert_eq!(reason, "lunar-gravity-dominated-by-c22", "the reason must be machine-readable");
        }
        Lookup::Known(_) => panic!("the Moon's J2 must not be presented as a known basis for J2 theory"),
        Lookup::NoSuchBody => panic!("the Moon exists; only the property is unknown"),
    }
    // and it must still be a real catalogue body
    assert!(MOON.physical.j2.body_exists(), "the body exists even though the property does not");
}

/// The Sun's oblateness and rotation are not settled, and are recorded as such.
#[test]
fn the_sun_does_not_claim_a_settled_oblateness_or_rotation() {
    assert!(!SUN.physical.j2.body_exists() || SUN.physical.j2.known().is_none());
    assert!(SUN.physical.rotation_rate.known().is_none(), "the Sun rotates differentially");
    // but its mu is solid, and that is what an orbit needs
    assert!(SUN.physical.mu.is_knowledge());
}

/// Every body in the catalogue can answer `mu`, or it cannot be propagated around at all.
#[test]
fn every_catalogued_body_knows_its_gravitational_parameter() {
    for b in catalogue::ALL.iter() {
        assert!(
            b.physical.mu.is_knowledge(),
            "body {} must know mu - without it there is no orbit",
            b.id
        );
        assert!(b.physical.mu.value > 0.0, "body {} has non-positive mu", b.id);
    }
}

// ---------------------------------------------------------------------------------------------
// 3. The values agree with the ones the kernel already uses
// ---------------------------------------------------------------------------------------------

/// The catalogue and the older loose constants must not drift apart.
///
/// `mu::EARTH` and `radius::EARTH` predate the catalogue and are used all over the kernel. Two sources of
/// truth for Earth's radius is a bug waiting to happen, so the crate now has one: this test. When the loose
/// constants are eventually retired, this test is what proves the replacement was faithful.
#[test]
fn catalogue_agrees_with_the_kernel_constants() {
    assert_eq!(EARTH.physical.mu.value, MU, "Earth mu: catalogue vs mu::EARTH");
    assert_eq!(EARTH.physical.equatorial_radius.value, RE, "Earth radius: catalogue vs radius::EARTH");
    assert_eq!(EARTH.physical.j2.known().unwrap().value, EARTH_J2, "Earth J2: catalogue vs j2::EARTH_J2");
    assert_eq!(MOON.physical.mu.value, mu::MOON, "Moon mu");
    assert_eq!(MARS.physical.mu.value, mu::MARS, "Mars mu");
    assert_eq!(SUN.physical.mu.value, mu::SUN, "Sun mu");
}

/// The catalogue's Earth constants actually drive the J2 machinery.
///
/// A catalogue nobody reads is a table, not a kernel. This pulls `mu`, `radius` and `j2` out of the
/// catalogue and pushes them through the secular-rate function, then checks the answer against the published
/// ISS nodal regression. It proves the wiring, not the arithmetic — the arithmetic has its own tests.
#[test]
fn the_catalogue_drives_the_perturbation_machinery() {
    let earth = catalogue::get("earth").known().expect("earth is catalogued");
    let mu = earth.physical.mu.value;
    let re = earth.physical.equatorial_radius.value;
    let j2 = earth.physical.j2.known().expect("earth knows j2").value;

    let a = re + 400.0;
    let rates = secular_rates(a, 0.0005, 51.6 * core::f64::consts::PI / 180.0, mu, j2, re)
        .expect("valid orbit");

    let deg_per_day = 86_400.0 * 180.0 / core::f64::consts::PI;
    let raan = rates.raan * deg_per_day;
    assert!(
        (-5.6..-4.4).contains(&raan),
        "the catalogue's own constants should reproduce the ISS nodal regression, got {raan} deg/day"
    );
}

// ---------------------------------------------------------------------------------------------
// 4. Lookup distinguishes a typo from an epistemic limit
// ---------------------------------------------------------------------------------------------

/// A missing body and an unknown property are different answers, and must not collapse to one.
#[test]
fn lookup_distinguishes_a_typo_from_an_unknown_property() {
    // a body that is not in the catalogue
    let missing = catalogue::get("pluto");
    assert_eq!(missing, Lookup::NoSuchBody);
    assert!(!missing.body_exists(), "a body we do not have does not exist to us");
    assert!(missing.known().is_none());

    // a body that is, with a property that is not
    let moon = catalogue::get("moon").known().expect("moon is catalogued");
    assert!(moon.physical.j2.body_exists(), "the Moon exists");
    assert!(moon.physical.j2.known().is_none(), "but its J2 is not known");
}

/// Lookup is case-sensitive and exact.
///
/// Ids are keys. `"Earth"` failing to resolve is correct behaviour and better than a case-insensitive
/// search that hides a typo until it matters.
#[test]
fn lookup_is_exact() {
    assert_eq!(catalogue::get("Earth"), Lookup::NoSuchBody);
    assert_eq!(catalogue::get("eth"), Lookup::NoSuchBody);
    assert_eq!(catalogue::get(""), Lookup::NoSuchBody);
    assert!(catalogue::get("earth").body_exists());
}

/// Ids are unique, lowercase and ASCII.
///
/// Enforced because a duplicate id silently shadows an entry — the first match wins and the second is
/// dead data that still gets tested. The build should not be able to contain that state.
#[test]
fn ids_are_unique_and_well_formed() {
    let all = catalogue::ALL;
    for i in 0..all.len() {
        let id = all[i].id;
        assert!(!id.is_empty(), "empty id at index {i}");
        assert!(
            id.bytes().all(|b| b.is_ascii_lowercase()),
            "id {id} must be lowercase ASCII"
        );
        for other in all.iter().skip(i + 1) {
            assert_ne!(id, other.id, "duplicate catalogue id: {id}");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 5. Sidereal, not solar
// ---------------------------------------------------------------------------------------------

/// Earth's rotation rate is the **sidereal** one.
///
/// The distinction is 0.4% — one part in 365 — and using the solar-day rate instead makes a ground track
/// walk east by a full revolution per year with no error message anywhere. It is one of the most common
/// silent errors in orbit code, so it gets a test with the two candidate values named.
#[test]
fn earth_rotation_rate_is_sidereal_not_solar() {
    let w = EARTH.physical.rotation_rate.known().expect("earth knows its spin").value;

    let sidereal = core::f64::consts::TAU / 86164.0905;
    let solar = core::f64::consts::TAU / 86_400.0;

    // 1e-10, not machine epsilon. The catalogue stores the published figure 7.2921150e-5 (eight
    // significant figures, which is what WGS-84 and the IAU report give), while `TAU/86164.0905`
    // recomputes it from a rounded day length. The two differ by 8.6e-12, which is the *rounding of the
    // published constant*, not an error in it. A bound tighter than that would be testing which
    // arithmetic was used rather than which rate was meant.
    assert!(
        fabs(w - sidereal) < 1e-10,
        "rotation rate should be the sidereal rate {sidereal:e}, got {w:e}"
    );

    // And it must be distinguishable from the wrong answer. The margin here is small and that is the
    // point: the two rates differ by only 2e-7 rad/s (one part in 365), so the test has to sit between
    // them rather than at some comfortable round number. 1e-8 is two orders below the separation and
    // eight above the rounding, which is the widest window available.
    let separation = fabs(sidereal - solar);
    assert!(
        fabs(w - solar) > separation * 0.99,
        "the stored rate is closer to the solar-day rate {solar:e} than to the sidereal one -          a ground track would walk east by a full revolution per year"
    );
}

// ---------------------------------------------------------------------------------------------
// 6. A catalogue value that is wrong by a lot is worse than one that is missing
// ---------------------------------------------------------------------------------------------

/// Every `Known` quantity is finite, and every `Measured` one has a sigma consistent with its value.
///
/// A sigma of 1e6 on a radius of 3396 km is not a measurement, it is a shrug with units. This catches a
/// decimal point in the wrong place in either field, which is the realistic way a hand-entered catalogue
/// fails.
#[test]
fn every_known_quantity_is_finite_and_plausibly_bounded() {
    for b in catalogue::ALL.iter() {
        let candidates = [
            b.physical.mu,
            b.physical.equatorial_radius,
        ];
        for q in candidates {
            assert!(q.value.is_finite(), "body {} has a non-finite value", b.id);
            assert!(q.value > 0.0, "body {} has a non-positive value", b.id);
            if let Prov::Measured { sigma } = q.prov {
                assert!(
                    sigma < q.value,
                    "body {} has a sigma ({sigma:e}) larger than its value ({:e}) - that is a shrug, not a measurement",
                    b.id,
                    q.value
                );
            }
        }
        // j2, where known, is a small dimensionless number
        if let Some(j2) = b.physical.j2.known() {
            assert!(j2.value >= 0.0, "body {}: J2 is a positive magnitude in this crate", b.id);
            assert!(j2.value < 0.1, "body {}: J2 of {} is not plausible", b.id, j2.value);
        }
    }
}
