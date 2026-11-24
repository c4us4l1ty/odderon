//! `Provenance` as a type: what the epistemic machinery promises, and whether it keeps it.
//!
//! This module is the one the whole project's honesty rests on. `BUILD-PLAN.md` §4b calls it "done, first
//! surface", and until this file existed that claim had **no dedicated test** — the type was exercised
//! incidentally by `catalogue.rs`, `frames.rs`, `observe.rs` and `passes.rs`, each of which used whatever
//! constructor it needed and asserted about *bodies*, not about provenance. A type can be used everywhere and
//! tested nowhere.
//!
//! The tests are grouped by what a wrong implementation would break:
//!
//! 1. **The constructors refuse what they promise to refuse.** A zero-sigma measurement is a missing error bar,
//!    not infinite precision; a `Defined` NaN is not a constant. If these quietly succeeded, every downstream
//!    uncertainty would be built on a lie that type-checks.
//! 2. **`Unknown` fails loudly.** Its whole design is that arithmetic on it produces `NaN` rather than a
//!    plausible number, because a `0.0` placeholder becomes "massless planet" somewhere downstream.
//! 3. **`is_knowledge` draws the line in the right place.** `Derived` is knowledge; `Assumed` is not, even
//!    though a human wrote it down carefully.
//! 4. **`Lookup` keeps two failures apart.** "No such body" is a wiring bug; "body exists, property unknown" is
//!    the honest answer for a small asteroid. Collapsing them is the mistake the enum exists to prevent.
//! 5. **The tags are stable.** `DATA-LICENSING.md` §3.1 has a build-time check keyed on them.

use odderon_astro::provenance::{Lookup, Prov, Quantity};

// ---------------------------------------------------------------------------------------------
// 1. The constructors refuse what they promise to refuse
// ---------------------------------------------------------------------------------------------

/// A measurement with no error bar is refused, in every spelling of "no error bar".
///
/// The doc is specific: "a zero uncertainty measurement is not a strong measurement, it is a missing one:
/// somebody forgot to fill in the error bar". So zero, negative, and both kinds of non-finite must all be
/// refused — and `NaN` is the one that a naive `sigma > 0.0` test gets right by accident while a
/// `!(sigma <= 0.0)` test gets wrong.
#[test]
fn a_measurement_without_an_error_bar_is_refused() {
    for bad in [0.0, -0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            Prov::measured(bad).is_none(),
            "sigma {bad} must be refused as a measurement"
        );
        assert!(
            Quantity::measured(1.0, bad).is_none(),
            "Quantity::measured with sigma {bad} must be refused too"
        );
    }

    // And the smallest legitimate sigma is accepted, so the refusal above is not a blanket one.
    let tiny = Prov::measured(f64::MIN_POSITIVE).expect("a denormal is still a positive number");
    assert!(matches!(tiny, Prov::Measured { .. }));
}

/// `Quantity::measured` refuses a non-finite value even when the sigma is good.
///
/// Separate from the sigma check because the two are independent: a finite error bar on an infinite value is
/// still not a measurement, and the constructor checks them in that order.
#[test]
fn a_non_finite_value_is_refused_even_with_a_good_sigma() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            Quantity::measured(bad, 1.0).is_none(),
            "value {bad} with a finite sigma must still be refused"
        );
        assert!(
            Quantity::defined(bad).is_none(),
            "a constant cannot be {bad}"
        );
        assert!(
            Quantity::assumed(bad, 1.0, "test").is_none(),
            "an assumption cannot be {bad}"
        );
    }
}

/// An assumption must carry a usable sigma, because that is the whole point of the variant.
///
/// `Assumed` exists so that downstream arithmetic still has something to propagate. An assumption with no
/// error bar would be exact by default, which is how a placeholder becomes a fact.
#[test]
fn an_assumption_must_carry_a_usable_sigma() {
    let assumed = Quantity::assumed(45.0, 15.0, "envelope conduction").expect("valid");
    assert_eq!(assumed.value, 45.0);
    assert_eq!(assumed.sigma, 15.0);
    assert_eq!(assumed.prov.tag(), "assumed");
    assert!(
        !assumed.is_knowledge(),
        "an assumption is a choice, not a fact about the universe"
    );

    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(
            Quantity::assumed(45.0, bad, "test").is_none(),
            "an assumption with sigma {bad} has nothing to propagate"
        );
    }
}

/// A `Defined` quantity is exact, and says so in both places the sigma is reported.
///
/// `sigma` the field and `prov.own_sigma()` must agree, because a caller that reads one and a caller that
/// reads the other must reach the same conclusion about precision.
#[test]
fn a_defined_quantity_is_exact_in_both_sigma_reports() {
    let c = Quantity::defined(299_792_458.0).expect("finite");
    assert_eq!(c.sigma, 0.0, "the field must be zero");
    assert_eq!(
        c.prov.own_sigma(),
        Some(0.0),
        "and the method must agree with the field"
    );
    assert_eq!(c.prov.tag(), "defined");
    assert!(
        c.is_knowledge(),
        "a published constant is knowledge; it is the best kind"
    );
}

// ---------------------------------------------------------------------------------------------
// 2. Unknown fails loudly
// ---------------------------------------------------------------------------------------------

/// `Unknown` poisons arithmetic with `NaN` rather than behaving like a number.
///
/// The doc is explicit that the value is `NaN` "on purpose": a `0.0` placeholder would silently mean
/// "massless planet" somewhere downstream, and nothing would notice. This is the single most important
/// property of the type, and it is the one a well-meaning refactor to `Option`-like ergonomics could destroy.
#[test]
fn unknown_poisons_arithmetic_instead_of_lying() {
    let u = Quantity::unknown();
    assert!(u.value.is_nan(), "the value must be NaN, not a placeholder");
    assert!(
        !u.is_knowledge(),
        "nothing is known, so this cannot be shown as a fact"
    );
    assert_eq!(u.prov.tag(), "unknown");

    // Any arithmetic touching it is NaN, which is a loud failure rather than a plausible 0 or a plausible mass.
    let poisoned = u.value * 5.0 + 1.0;
    assert!(poisoned.is_nan(), "arithmetic on unknown must stay NaN");
    assert!(
        !(u.value > 0.0) && !(u.value < 0.0),
        "an unknown compares false in both directions, so `if mass > 0` is unreachable rather than wrong"
    );
}

/// The sigma of an unknown is unbounded in the field, and `None` from the method — a documented asymmetry.
///
/// This test pins behaviour that looks like an inconsistency and is not, so that the next reader does not
/// "fix" it. The field is the *uncertainty*, and an unknown quantity's uncertainty is genuinely infinite:
/// a finite placeholder would be a lie that arithmetic propagates as if it meant something. The method is
/// asked a narrower question — "does this provenance imply a number with a meaningful sigma" — and for
/// `Unknown` the honest answer is that it does not, so `None` rather than `Some(inf)`. Two different
/// questions, two different answers, and the distinction is that `None` also covers `Derived` and `Assumed`,
/// where the sigma is *not* infinite but merely not owned by the provenance.
#[test]
fn the_sigma_of_an_unknown_is_infinite_in_the_field_and_none_from_the_method() {
    let u = Quantity::unknown();
    assert_eq!(
        u.sigma,
        f64::INFINITY,
        "the field carries unbounded uncertainty"
    );
    assert_eq!(
        u.prov.own_sigma(),
        None,
        "the method declines to name a sigma at all"
    );

    // And the two agree for the variants where a sigma *is* owned by the provenance.
    assert_eq!(
        Quantity::defined(1.0).expect("finite").prov.own_sigma(),
        Some(0.0)
    );
    assert_eq!(
        Quantity::measured(1.0, 0.5)
            .expect("valid")
            .prov
            .own_sigma(),
        Some(0.5)
    );
    // Assumed owns a sigma in the *field* but not in the provenance, because the basis is a string, not a number.
    let a = Quantity::assumed(1.0, 0.5, "test").expect("valid");
    assert_eq!(a.sigma, 0.5);
    assert_eq!(a.prov.own_sigma(), None);
}

// ---------------------------------------------------------------------------------------------
// 3. is_knowledge draws the line in the right place
// ---------------------------------------------------------------------------------------------

/// The five variants partition into exactly two groups, and the boundary is where the doc says it is.
///
/// `Derived` is knowledge because a derivation from measured parents is still knowledge. `Assumed` is not,
/// even though a human wrote it down carefully — that is the distinction the type exists to make, and it is
/// the one most likely to drift if someone decides assumptions "count".
#[test]
fn knowledge_is_exactly_measured_defined_and_derived() {
    let measured = Quantity::measured(1.0, 0.1).expect("valid");
    let defined = Quantity::defined(1.0).expect("finite");
    let assumed = Quantity::assumed(1.0, 0.1, "test").expect("valid");
    let unknown = Quantity::unknown();
    let derived = Quantity {
        value: 1.0,
        prov: Prov::Derived {
            method: "two-body Kepler propagation",
        },
        sigma: 0.1,
    };

    assert!(measured.is_knowledge());
    assert!(defined.is_knowledge());
    assert!(
        derived.is_knowledge(),
        "a derivation from measured parents is knowledge"
    );
    assert!(!assumed.is_knowledge(), "an assumption is not a fact");
    assert!(!unknown.is_knowledge());

    // `is_knowledge` on Prov and on Quantity must not disagree, since the latter delegates to the former.
    for q in [measured, defined, assumed, unknown, derived] {
        assert_eq!(
            q.is_knowledge(),
            q.prov.is_knowledge(),
            "Quantity and Prov must agree for tag {}",
            q.prov.tag()
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 4. The tags are stable
// ---------------------------------------------------------------------------------------------

/// Every tag is the exact string a build check keys on, and no two variants share one.
///
/// `DATA-LICENSING.md` §3.1 requires a build-time check keyed on these strings, and the doc says a check that
/// keys on a string somebody renames is not a check. Pinning the literals here is what makes renaming one a
/// test failure instead of a silent licence-gate bypass.
#[test]
fn the_tags_are_the_stable_literals_a_build_check_keys_on() {
    assert_eq!(Prov::Measured { sigma: 1.0 }.tag(), "measured");
    assert_eq!(Prov::Defined.tag(), "defined");
    assert_eq!(Prov::Derived { method: "m" }.tag(), "derived");
    assert_eq!(Prov::Assumed { basis: "b" }.tag(), "assumed");
    assert_eq!(Prov::Unknown.tag(), "unknown");

    // Uniqueness, which the match arms happen to give but nothing else enforces.
    let tags = [
        Prov::Measured { sigma: 1.0 }.tag(),
        Prov::Defined.tag(),
        Prov::Derived { method: "m" }.tag(),
        Prov::Assumed { basis: "b" }.tag(),
        Prov::Unknown.tag(),
    ];
    for i in 0..tags.len() {
        for j in (i + 1)..tags.len() {
            assert_ne!(tags[i], tags[j], "two variants share the tag {}", tags[i]);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 5. Lookup keeps two failures apart
// ---------------------------------------------------------------------------------------------

/// "No such body" and "body exists, property unknown" are different answers, and the type keeps them apart.
///
/// This is the reason `Lookup` is not `Option`. Collapsing the two would let a missing catalogue entry look
/// like an epistemic limitation — or worse, let a genuinely unknown property look like a wiring bug and send
/// someone to debug the catalogue instead of the physics.
#[test]
fn lookup_distinguishes_a_missing_body_from_a_missing_fact() {
    let known: Lookup<f64> = Lookup::Known(5.972e24);
    let no_fact: Lookup<f64> = Lookup::NotKnown {
        reason: "small-body-mass-unmeasured",
    };
    let no_body: Lookup<f64> = Lookup::NoSuchBody;

    assert_eq!(known.known(), Some(5.972e24));
    assert_eq!(no_fact.known(), None);
    assert_eq!(no_body.known(), None);

    // The distinction that survives `known()` being `None` for both.
    assert!(known.body_exists(), "the catalogue has this body");
    assert!(
        no_fact.body_exists(),
        "the catalogue has this body even though the fact is absent — this is the whole point of the enum"
    );
    assert!(!no_body.body_exists(), "the catalogue has no such body");

    // A reason is carried, so a UI can key on a stable string instead of parsing English.
    if let Lookup::NotKnown { reason } = no_fact {
        assert_eq!(reason, "small-body-mass-unmeasured");
    } else {
        panic!("NotKnown must carry its reason");
    }
}

/// `Lookup` is `Copy` for a `Copy` payload, so a lookup can be read twice without cloning.
///
/// Not incidental: the catalogue returns these by value and callers legitimately inspect the enum and then
/// take the payload.
#[test]
fn lookup_can_be_inspected_and_then_consumed() {
    let l: Lookup<u32> = Lookup::Known(7);
    assert!(l.body_exists());
    assert_eq!(l.known(), Some(7));
    // A third read, after two by-value uses, which only compiles because the enum is Copy.
    assert!(l.body_exists());
}

// ---------------------------------------------------------------------------------------------
// 6. The constructors and the hand-built literals agree
// ---------------------------------------------------------------------------------------------

/// A quantity built by literal must be indistinguishable from one built by constructor.
///
/// The fields are public on purpose — "there is nothing dishonest a caller can do with them that the type
/// system could stop" — and `catalogue.rs` uses literals throughout. That makes the constructors' invariants
/// a *convention* for literal users rather than a guarantee, so the least this file can do is prove that the
/// two paths produce the same value.
#[test]
fn a_literal_and_a_constructor_produce_the_same_quantity() {
    let by_constructor = Quantity::defined(3_986_000.0).expect("finite");
    let by_literal = Quantity {
        value: 3_986_000.0,
        prov: Prov::Defined,
        sigma: 0.0,
    };
    assert_eq!(by_constructor, by_literal);

    let by_constructor = Quantity::measured(3_986_000.0, 120.0).expect("valid");
    let by_literal = Quantity {
        value: 3_986_000.0,
        prov: Prov::Measured { sigma: 120.0 },
        sigma: 120.0,
    };
    assert_eq!(by_constructor, by_literal);
}
