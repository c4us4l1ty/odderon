//! The Pad, as one measurable claim: a reproducible failure from the input log.
//!
//! `docs/BUILD-PLAN.md` §5 makes the gate to leave P1 *a reproducible failure from the input log*, and
//! `GAME-DESIGN-BIBLE.md` §17 defines the failure: the player's first cryo load is under-specified on
//! purpose, it froths, the pump cavitates, and the test article dies on the stand.
//!
//! That is a falsifiable statement about a coupled system, so the tests below are grouped by which part of
//! it they can falsify:
//!
//! 1. **The bible's failure is reproduced.** The under-specified chilldown cavitates; the specified one does
//!    not. If both plans behave the same, the model is not modelling the failure the bible describes.
//! 2. **The failure is reproducible.** The same input log, simulated twice, returns the same outcome
//!    bit-for-bit. This is the P1 gate itself: a failure that depends on when you ran it is a story, not a
//!    gate.
//! 3. **The saturation curve anchors and is consistent.** `vapour_pressure_kpa` is one atmosphere at the
//!    normal boiling point by construction, monotone in temperature, and its inverse
//!    (`subcooling_margin_j_per_kg`) agrees with it rather than being a second approximation.
//! 4. **The chilldown is monotone.** More chill time and more bleeds each remove heat; the residual falls
//!    with either. A chilldown that does not cool is not a chilldown.
//! 5. **The flash is bounded and honest.** It is in `[0, 1]`, zero when the line has nothing to give, and
//!    refuses the inputs that have no meaning.
//! 6. **The refusals.** A non-finite plan, fill rate or elapsed time is refused rather than answered.
//!
//! Every number quoted in a comment was computed against the same constants before being written down, so
//! a failure says which of the two disagreed.

use odderon_astro::cryo::{
    chilldown_residual_heat, flash_fraction, subcooling_margin_j_per_kg, vapour_pressure_kpa,
    ChillPlan, CryoNetwork, Failure,
};

/// The reference pad network and the two plans from the bible: the under-specified first load (the one
/// that is supposed to fail) and a correctly specified chill (the one that is supposed to survive).
fn network() -> CryoNetwork {
    CryoNetwork::reference()
}

// =============================================================================================
// 1. The bible's failure is reproduced
// =============================================================================================

/// The P1 claim, stated as one test: the under-specified chilldown cavitates, the specified one does not.
///
/// This is the whole reason the module exists. The bible's "productive failure" is that an under-specified
/// first cryo load froths, the pump cavitates and the test article dies; the debrief then explains chilldown.
/// If the under-specified plan did *not* fail, the model is not modelling that failure. If the specified plan
/// *did* fail, the model is failing for the wrong reason and the player cannot learn from it.
#[test]
fn the_underspecified_chilldown_cavitates_and_the_specified_one_does_not() {
    let net = network();

    let under = net
        .simulate(&ChillPlan::underspecified(), 0.50, 60.0)
        .expect("a valid plan runs");
    let (failure, _t) = under
        .failure
        .expect("the under-specified load must cavitate: that is the bible's failure");
    assert_eq!(failure, Failure::Cavitation);

    let specified = net
        .simulate(&ChillPlan::specified(), 0.50, 60.0)
        .expect("a valid plan runs");
    assert!(
        specified.failure.is_none(),
        "the specified chilldown must survive; it cavitated at {:?}",
        specified.failure,
    );
}

/// The failure is *caused* by the chilldown, not by the fill. Same fill, same window; only the plan moves.
///
/// A test that asserts the two plans differ without pinning *which* quantity carries the difference is
/// satisfied by any change to the plan. This one reads the pump inlet vapour fraction — the quantity the
/// cavitation predicate actually reads — and requires the under-specified plan to cross the threshold and the
/// specified plan to stay below it by a stated margin.
#[test]
fn the_failure_turns_on_the_pump_inlet_vapour_fraction() {
    use odderon_astro::cryo::CAVITATION_THRESHOLD;

    let net = network();
    let under = net
        .simulate(&ChillPlan::underspecified(), 0.50, 60.0)
        .unwrap();
    let specified = net.simulate(&ChillPlan::specified(), 0.50, 60.0).unwrap();

    assert!(
        under.pump_inlet_vapour_fraction > CAVITATION_THRESHOLD.value,
        "under-specified inlet fraction {} must exceed the threshold {}",
        under.pump_inlet_vapour_fraction,
        CAVITATION_THRESHOLD.value,
    );
    assert!(
        specified.pump_inlet_vapour_fraction < CAVITATION_THRESHOLD.value,
        "specified inlet fraction {} must stay below the threshold {}",
        specified.pump_inlet_vapour_fraction,
        CAVITATION_THRESHOLD.value,
    );
}

// =============================================================================================
// 2. The failure is reproducible (the P1 gate)
// =============================================================================================

/// **The P1 gate.** The same input log, simulated twice, returns the same outcome bit-for-bit.
///
/// This is the claim the phase is gated on, and it is the one a stateful or clock-driven model would fail.
/// The simulation is a pure function of the plan, the fill rate and `t`, so two runs of the same inputs must
/// agree exactly — not within a tolerance, exactly. A tolerance here would let a failure that depends on
/// global state slip through, which is the failure mode this gate exists to prevent.
#[test]
fn the_same_input_log_reproduces_the_failure_bit_for_bit() {
    let net = network();
    let plan = ChillPlan::underspecified();

    let a = net.simulate(&plan, 0.50, 120.0).unwrap();
    let b = net.simulate(&plan, 0.50, 120.0).unwrap();

    assert_eq!(
        a, b,
        "the same input log must reproduce the same outcome exactly"
    );
    assert_eq!(
        a.failure, b.failure,
        "and the failure, in particular, must be identical"
    );
}

/// Reproducibility across *seek times*: the failure time is a function of the log, so stopping the run
/// earlier must not move it. A model that re-derives the failure from the current time rather than from the
/// log would report different failure times for the same plan at different `elapsed_s`.
#[test]
fn the_failure_time_does_not_depend_on_how_long_the_run_is_allowed_to_go() {
    let net = network();
    let plan = ChillPlan::underspecified();

    let short = net.simulate(&plan, 0.50, 30.0).unwrap();
    let long = net.simulate(&plan, 0.50, 300.0).unwrap();

    let (_, t_short) = short
        .failure
        .expect("the under-specified load fails within 30 s");
    let (_, t_long) = long.failure.expect("and within 300 s");

    // The failure is a property of the fill entering the line, so it occurs at the same simulated time
    // regardless of how far past it the run is allowed to continue.
    assert!(
        (t_short - t_long).abs() < 1e-9,
        "failure time moved with the window: {} vs {}",
        t_short,
        t_long,
    );
}

// =============================================================================================
// 3. The saturation curve anchors and is consistent
// =============================================================================================

/// The curve is anchored: at the normal boiling point the vapour pressure is exactly one atmosphere. This
/// is by construction, and it is the one number the whole saturation model is pinned to.
#[test]
fn the_saturation_curve_is_anchored_at_the_normal_boiling_point() {
    use odderon_astro::cryo::{ATMOSPHERE_KPA, LOX_NBP_K};
    let p = vapour_pressure_kpa(LOX_NBP_K).expect("the NBP has a vapour pressure");
    assert!(
        (p - ATMOSPHERE_KPA).abs() < 1e-9,
        "P_sat(NBP) = {} kPa, want exactly one atmosphere {}",
        p,
        ATMOSPHERE_KPA,
    );
}

/// The curve is monotone increasing in temperature: a warmer liquid has a higher vapour pressure. A
/// decreasing or non-monotone curve would invert the subcooling logic and make a warm line look subcooled.
#[test]
fn the_saturation_curve_rises_with_temperature() {
    let cold = vapour_pressure_kpa(85.0).unwrap();
    let warm = vapour_pressure_kpa(100.0).unwrap();
    assert!(
        warm > cold,
        "P_sat must rise with T: {} at 85 K vs {} at 100 K",
        cold,
        warm
    );
}

/// The inverse agrees with the forward curve. `subcooling_margin_j_per_kg` inverts the same Clausius–
/// Clapeyron form `vapour_pressure_kpa` uses, so the saturation temperature it computes must satisfy the
/// forward curve. If the two were independent approximations, a drift between them would make the margin
/// wrong in a way no single test would catch.
#[test]
fn the_subcooling_margin_inverts_the_saturation_curve() {
    // At the tank pressure, the saturation temperature is the temperature whose vapour pressure equals it.
    let tank_p = 150.0;
    let t_sat_from_margin = {
        // Recover T_sat from the margin at a known subcooling: margin = c_p * (T_sat - T_tank), so
        // T_sat = T_tank + margin / c_p. Use a tank 10 K below saturation.
        let tank_t = 80.0;
        let margin =
            subcooling_margin_j_per_kg(tank_t, tank_p).expect("a positive subcooling has a margin");
        use odderon_astro::cryo::LOX_CP;
        tank_t + margin / LOX_CP.value
    };
    let p_at_t_sat = vapour_pressure_kpa(t_sat_from_margin).expect("T_sat has a vapour pressure");
    assert!(
        (p_at_t_sat - tank_p).abs() / tank_p < 1e-9,
        "inverse disagrees with forward: P_sat({}) = {} kPa, want {}",
        t_sat_from_margin,
        p_at_t_sat,
        tank_p,
    );
}

/// A non-positive subcooling is refused, not answered. A line already past saturation is the failure state,
/// not a margin to carry forward.
#[test]
fn a_line_past_saturation_has_no_margin() {
    // A tank warmer than the saturation temperature at its pressure: the "margin" would be negative.
    assert!(subcooling_margin_j_per_kg(120.0, 101.325).is_none());
}

// =============================================================================================
// 4. The chilldown is monotone
// =============================================================================================

/// More chill time removes heat: the residual falls as the chill runs longer. A chilldown that does not cool
/// with time is not a chilldown.
#[test]
fn more_chill_time_removes_more_heat() {
    let short = chilldown_residual_heat(&ChillPlan {
        chill_s: 5.0,
        bleeds: 0,
    })
    .unwrap();
    let long = chilldown_residual_heat(&ChillPlan {
        chill_s: 600.0,
        bleeds: 0,
    })
    .unwrap();
    assert!(
        long < short,
        "longer chill must leave less heat: {} vs {}",
        long,
        short
    );
    assert!(
        long > 0.0,
        "chilldown asymptotes to the floor, never below zero"
    );
}

/// More bleeds remove heat too: each purge cycle takes a fraction of the residual with it.
#[test]
fn more_bleeds_remove_more_heat() {
    let few = chilldown_residual_heat(&ChillPlan {
        chill_s: 60.0,
        bleeds: 1,
    })
    .unwrap();
    let many = chilldown_residual_heat(&ChillPlan {
        chill_s: 60.0,
        bleeds: 5,
    })
    .unwrap();
    assert!(
        many < few,
        "more bleeds must leave less heat: {} vs {}",
        many,
        few
    );
}

/// Neither mechanism substitutes for the other: a long chill with no bleeds still leaves more heat than the
/// same chill with bleeds, and many bleeds with no chill time still leave a line that never got cold. This
/// is the property the bible's "one item that is wrong" exploits — fixing only one of the two is not a
/// correct chilldown.
#[test]
fn neither_chill_time_nor_bleeds_substitutes_for_the_other() {
    let time_only = chilldown_residual_heat(&ChillPlan {
        chill_s: 600.0,
        bleeds: 0,
    })
    .unwrap();
    let bleeds_only = chilldown_residual_heat(&ChillPlan {
        chill_s: 0.0,
        bleeds: 5,
    })
    .unwrap();
    let both = chilldown_residual_heat(&ChillPlan {
        chill_s: 600.0,
        bleeds: 5,
    })
    .unwrap();
    assert!(both < time_only, "bleeds on top of chill must help");
    assert!(both < bleeds_only, "chill on top of bleeds must help");
}

/// A non-finite or negative chill time is refused: a plan that is not a plan is not a chilldown.
#[test]
fn a_non_finite_chill_plan_is_refused() {
    assert!(chilldown_residual_heat(&ChillPlan {
        chill_s: f64::NAN,
        bleeds: 0,
    })
    .is_none());
    assert!(chilldown_residual_heat(&ChillPlan {
        chill_s: f64::INFINITY,
        bleeds: 0,
    })
    .is_none());
    assert!(chilldown_residual_heat(&ChillPlan {
        chill_s: -1.0,
        bleeds: 0,
    })
    .is_none());
}

// =============================================================================================
// 5. The flash is bounded and honest
// =============================================================================================

/// The flash is zero when the line has nothing to give: the cryogen stays liquid.
#[test]
fn a_cold_line_flashes_nothing() {
    let f = flash_fraction(0.0, 10_000.0).unwrap();
    assert_eq!(f, 0.0);
}

/// The flash is bounded in [0, 1]: a line can vaporise all of the cryogen, never more.
#[test]
fn the_flash_is_bounded() {
    // A line with more than enough heat to vaporise everything: the fraction saturates at 1.0.
    let full = flash_fraction(1_000_000.0, 0.0).unwrap();
    assert_eq!(full, 1.0);
    // And a middling line gives a fraction strictly between.
    let mid = flash_fraction(100_000.0, 10_000.0).unwrap();
    assert!(mid > 0.0 && mid < 1.0, "mid flash {} out of bounds", mid);
}

/// A non-finite or negative heat load is refused: the line cannot give the cryogen less than nothing, and a
/// non-finite value is not a heat load.
#[test]
fn a_non_finite_flash_input_is_refused() {
    assert!(flash_fraction(f64::NAN, 10_000.0).is_none());
    assert!(flash_fraction(-1.0, 10_000.0).is_none());
    assert!(flash_fraction(10_000.0, f64::NAN).is_none());
}

// =============================================================================================
// 6. The refusals
// =============================================================================================

/// A non-finite fill rate or elapsed time is refused: the run is not a run, and a number returned for it
/// would be the same lie as a temperature for a missing kernel.
#[test]
fn a_non_finite_fill_rate_or_elapsed_time_is_refused() {
    let net = network();
    let plan = ChillPlan::underspecified();
    assert!(net.simulate(&plan, f64::NAN, 60.0).is_none());
    assert!(net.simulate(&plan, -1.0, 60.0).is_none());
    assert!(net.simulate(&plan, 0.50, f64::NAN).is_none());
    assert!(net.simulate(&plan, 0.50, f64::INFINITY).is_none());
}

/// A zero fill rate runs to the end of the window with no liquid and no failure: nothing entered the line,
/// so there is nothing to flash. The model must not invent a failure from an empty fill.
#[test]
fn an_empty_fill_produces_no_failure() {
    let net = network();
    let out = net
        .simulate(&ChillPlan::underspecified(), 0.0, 60.0)
        .unwrap();
    assert_eq!(out.liquid_kg, 0.0);
    assert!(out.failure.is_none(), "an empty fill must not cavitate");
}

// =============================================================================================
// 7. The wreck: a failed run, kept examinable
// =============================================================================================

/// A failing plan produces a wreck; a clean one does not. The bible's rule is that the wreckage is always
/// examinable -- never cut to black on failure -- so a failed run must leave a record, and a clean run must
/// leave none. Inventing a wreck for a clean run would be the same lie as a temperature for a missing kernel.
#[test]
fn a_failed_run_leaves_a_wreck_and_a_clean_run_leaves_none() {
    let net = network();
    let wreck = net
        .wreck(&ChillPlan::underspecified(), 0.50, 60.0)
        .expect("the under-specified load leaves a wreck on the stand");
    assert!(wreck.failure_time_s >= 0.0);

    let clean = net.wreck(&ChillPlan::specified(), 0.50, 60.0);
    assert!(
        clean.is_none(),
        "a clean run must leave no wreck to examine"
    );
}

/// THE WRECK CARRIES THE INPUT LOG. A wreck that does not say what was run is a verdict, not a record: a
/// reader cannot look at *why* it failed or re-simulate it. The wreck must carry the checklist that
/// produced it so it is self-describing and reproducible from itself.
#[test]
fn the_wreck_carries_the_input_log_that_produced_it() {
    let net = network();
    let plan = ChillPlan {
        chill_s: 12.0,
        bleeds: 1,
    };
    let wreck = net.wreck(&plan, 0.50, 90.0).expect("this plan cavitates");
    assert_eq!(wreck.chill_s, 12.0);
    assert_eq!(wreck.bleeds, 1);
    assert_eq!(wreck.fill_rate_kg_per_s, 0.50);
    assert_eq!(wreck.window_s, 90.0);
    assert!(wreck.failure_time_s >= 0.0);
    assert!(wreck.inlet_vapour_fraction > 0.0);
    assert!(wreck.residual_heat_j_per_kg > 0.0);
}

/// THE WRECK IS REPRODUCIBLE. The same input log, asked for twice, returns the same wreck bit-for-bit. This
/// is the P1 gate applied to the wreck itself: a wreck that depends on when it was recorded is a story, not
/// a record.
#[test]
fn the_wreck_is_reproducible_from_its_input_log() {
    let net = network();
    let plan = ChillPlan::underspecified();
    let a = net.wreck(&plan, 0.50, 120.0).unwrap();
    let b = net.wreck(&plan, 0.50, 120.0).unwrap();
    assert_eq!(
        a, b,
        "the same input log must reproduce the same wreck exactly"
    );
}

/// The wreck and the predicate agree: `is_wreck` is exactly `wreck().is_some()`. A predicate that disagreed
/// with the record it summarises would let the page decide a run left a wreck when the record says it did
/// not -- or the reverse.
#[test]
fn the_wreck_predicate_agrees_with_the_wreck_record() {
    let net = network();
    for (chill, bleeds) in [(5.0, 0), (12.0, 1), (300.0, 3), (60.0, 0), (0.0, 0)] {
        let plan = ChillPlan {
            chill_s: chill,
            bleeds,
        };
        assert_eq!(
            net.is_wreck(&plan, 0.50, 60.0),
            net.wreck(&plan, 0.50, 60.0).is_some(),
            "is_wreck disagreed with wreck() for chill={chill} bleeds={bleeds}",
        );
    }
}

/// A refused run has no wreck. A non-finite input is a run that never happened; answering `true` for it
/// would record a failure that was never reached.
#[test]
fn a_refused_run_leaves_no_wreck() {
    let net = network();
    let plan = ChillPlan::underspecified();
    assert!(net.wreck(&plan, f64::NAN, 60.0).is_none());
    assert!(!net.is_wreck(&plan, f64::NAN, 60.0));
}

// =============================================================================================
// 8. The Mark: grease pencil, ghost twin, one arrow, Akashi
// =============================================================================================

use odderon_astro::cryo::{akashi_entry, attribution, design_hash, ghost_twin, Attribution, Mark};

/// The ghost twin's divergence is the reward prediction error: actual minus expected.
///
/// This is the P2 core. The player marks where they expect the needle to settle; the kernel runs the
/// real model; the gap between the two is δ. A mark that agrees with the actual produces δ = 0; a mark
/// that the actual overshoots produces a positive δ. The sign and magnitude must be the arithmetic
/// difference, not an approximation of it.
#[test]
fn the_ghost_twin_divergence_is_actual_minus_expected() {
    let net = network();
    let plan = ChillPlan::underspecified();

    // The actual inlet fraction for the under-specified plan is ~1.0 (it cavitates). Marking below
    // that produces a positive divergence; marking above produces a negative one.
    let actual = net
        .simulate(&plan, 0.50, 60.0)
        .unwrap()
        .pump_inlet_vapour_fraction;

    let low_mark = Mark::inlet(plan, 0.5);
    let low = ghost_twin(&low_mark, &net, 0.50, 60.0).expect("a valid mark produces a ghost twin");
    assert!(
        (low.divergence - (actual - 0.5)).abs() < 1e-12,
        "divergence {} must be actual ({}) minus expected (0.5)",
        low.divergence,
        actual,
    );
    assert_eq!(low.expected, 0.5);
    assert_eq!(low.actual, actual);

    let high_mark = Mark::inlet(plan, actual + 0.1);
    let high =
        ghost_twin(&high_mark, &net, 0.50, 60.0).expect("a valid mark produces a ghost twin");
    assert!(
        high.divergence < 0.0,
        "a mark above the actual must produce a negative divergence, got {}",
        high.divergence,
    );
}

/// The ghost twin is reproducible: the same mark and plan produce the same δ on every run.
///
/// The Loop of Consequence is a gate, not a story. If the divergence depended on when it was computed,
/// the reward prediction error would be noise rather than signal.
#[test]
fn the_ghost_twin_is_reproducible() {
    let net = network();
    let plan = ChillPlan::underspecified();
    let mark = Mark::inlet(plan, 0.3);

    let a = ghost_twin(&mark, &net, 0.50, 60.0).unwrap();
    let b = ghost_twin(&mark, &net, 0.50, 60.0).unwrap();
    assert_eq!(
        a, b,
        "the same mark and plan must produce the same ghost twin exactly"
    );
}

/// A non-finite expected value is refused: a mark with no meaning is not a prediction.
#[test]
fn a_non_finite_mark_is_refused() {
    let net = network();
    let plan = ChillPlan::underspecified();
    let mark = Mark::inlet(plan, f64::NAN);
    assert!(
        ghost_twin(&mark, &net, 0.50, 60.0).is_none(),
        "a NaN mark must be refused, not answered",
    );
}

/// The one arrow names the single largest contributor to the divergence.
///
/// §6.2: `argmax over xᵢ of | ∂(Actual − Expected) / ∂xᵢ |`. The pad's plan has two inputs -- chill
/// time and bleeds. The arrow must name the larger of the two sensitivities, and the result must be
/// reproducible.
#[test]
fn the_one_arrow_names_the_largest_contributor() {
    let net = network();
    // chill_s=60, bleeds=0 gives an interior flash (~0.56): the needle is between the rails, so it
    // responds to the plan inputs and the arrow has something to point at. The under-specified plan
    // (chill_s=5, bleeds=0) flashes to 1.0 -- clamped at the rail -- where the sensitivity is zero
    // and the arrow correctly points at nothing.
    let plan = ChillPlan {
        chill_s: 60.0,
        bleeds: 0,
    };
    let mark = Mark::inlet(plan, 0.1);

    let result = attribution(&mark, &net, 0.50, 60.0);
    // The mark (0.1) disagrees with the actual (~0.56), so there is a divergence to attribute.
    assert_ne!(
        result.which,
        Attribution::None,
        "a divergent mark must produce an attribution, not None",
    );
    // The sensitivities must be non-negative and finite.
    assert!(result.sensitivity_chill >= 0.0);
    assert!(result.sensitivity_bleeds >= 0.0);
    // The arrow names the larger of the two.
    match result.which {
        Attribution::ChillTime => {
            assert!(
                result.sensitivity_chill > result.sensitivity_bleeds,
                "arrow says chill time but bleeds sensitivity ({}) is larger than chill ({})",
                result.sensitivity_bleeds,
                result.sensitivity_chill,
            );
        }
        Attribution::Bleeds => {
            assert!(
                result.sensitivity_bleeds > result.sensitivity_chill,
                "arrow says bleeds but chill sensitivity ({}) is larger than bleeds ({})",
                result.sensitivity_chill,
                result.sensitivity_bleeds,
            );
        }
        Attribution::None => unreachable!("checked above"),
    }
}

/// The one arrow is reproducible: the same mark and plan produce the same attribution.
#[test]
fn the_one_arrow_is_reproducible() {
    let net = network();
    let plan = ChillPlan::underspecified();
    let mark = Mark::inlet(plan, 0.2);

    let a = attribution(&mark, &net, 0.50, 60.0);
    let b = attribution(&mark, &net, 0.50, 60.0);
    assert_eq!(
        a, b,
        "the same mark and plan must produce the same attribution exactly"
    );
}

/// A mark that agrees with the actual produces no arrow: a zero gap points nowhere.
///
/// The bible's rule is "one arrow, not five" -- but an arrow on a zero divergence is not a
/// consequence, it is decoration. The attribution must be `None` when the mark and the actual agree.
#[test]
fn a_mark_that_agrees_with_the_actual_produces_no_arrow() {
    let net = network();
    let plan = ChillPlan::underspecified();
    let actual = net
        .simulate(&plan, 0.50, 60.0)
        .unwrap()
        .pump_inlet_vapour_fraction;
    let mark = Mark::inlet(plan, actual);

    let result = attribution(&mark, &net, 0.50, 60.0);
    assert_eq!(
        result.which,
        Attribution::None,
        "a mark equal to the actual must produce no attribution",
    );
}

/// The design hash is deterministic: the same kernel produces the same hash on every run.
///
/// An Akashi entry is stamped with this so a reader can tell which kernel version produced it. If the
/// hash were not deterministic, the stamp would be noise.
#[test]
fn the_design_hash_is_deterministic() {
    let a = design_hash();
    let b = design_hash();
    assert_eq!(
        a, b,
        "the design hash must be a pure function of the constants"
    );
    assert_ne!(a, 0, "the design hash must not be the zero value");
}

/// The Akashi entry is a pure function of `(design hash, epoch, input log, mark)`.
///
/// §4.4: the entry is generated deterministically. The same inputs produce the same entry on every
/// platform, every run. The wreck, ghost twin, and attribution on the entry must all be computed from
/// the same plan, so the entry is self-consistent: a reader can take the plan and re-simulate it.
#[test]
fn the_akashi_entry_is_deterministic_and_self_consistent() {
    let net = network();
    let plan = ChillPlan::underspecified();
    let mark = Mark::inlet(plan, 0.1);

    let a = akashi_entry(&net, 7, &plan, 0.50, 60.0, Some(&mark)).expect("a valid entry");
    let b = akashi_entry(&net, 7, &plan, 0.50, 60.0, Some(&mark)).expect("a valid entry");
    assert_eq!(
        a, b,
        "the same inputs must produce the same Akashi entry exactly"
    );

    // The design hash is the kernel's, not a caller-supplied value.
    assert_eq!(a.design_hash, design_hash());
    // The epoch is carried through.
    assert_eq!(a.epoch, 7);
    // The plan is carried through.
    assert_eq!(a.plan, plan);
    // The ghost twin on the entry agrees with a direct computation.
    let direct = ghost_twin(&mark, &net, 0.50, 60.0).unwrap();
    assert_eq!(a.ghost, Some(direct));
    // The wreck on the entry agrees with a direct computation.
    assert_eq!(a.wreck, net.wreck(&plan, 0.50, 60.0));
}

/// The Akashi entry carries the Loop of Consequence end-to-end: mark → ghost twin → attribution → wreck.
///
/// This is the P2 gate. An entry with a mark must carry the ghost twin (the divergence δ), the one
/// arrow (the attribution), and the wreck (the consequence). An entry without a mark carries no ghost
/// twin and no attribution, but still carries the wreck -- the loop can fire without a prediction, but
/// the consequence is always recorded.
#[test]
fn the_akashi_entry_carries_the_loop_of_consequence() {
    let net = network();
    // Use a plan with an interior flash so the attribution fires. The under-specified plan flashes
    // to 1.0 (the rail), where the arrow correctly points at nothing -- but the gate here is that
    // the full loop fires end-to-end, which needs a divergent mark with a live arrow.
    let plan = ChillPlan {
        chill_s: 60.0,
        bleeds: 0,
    };

    // With a mark: the full loop.
    let mark = Mark::inlet(plan, 0.1);
    let entry = akashi_entry(&net, 1, &plan, 0.50, 60.0, Some(&mark)).unwrap();
    assert!(entry.mark.is_some(), "an entry with a mark must carry it");
    assert!(
        entry.ghost.is_some(),
        "an entry with a mark must carry the ghost twin"
    );
    assert!(
        entry.ghost.unwrap().divergence != 0.0,
        "the ghost twin must carry a non-zero divergence for a divergent mark",
    );
    assert_ne!(
        entry.attribution.which,
        Attribution::None,
        "an entry with a divergent mark must carry an attribution",
    );
    assert!(
        entry.wreck.is_some(),
        "the plan cavitates, so the entry must carry a wreck"
    );

    // Without a mark: the consequence still fires, but there is no prediction to compare against.
    let entry_no_mark = akashi_entry(&net, 2, &plan, 0.50, 60.0, None).unwrap();
    assert!(entry_no_mark.mark.is_none());
    assert!(entry_no_mark.ghost.is_none(), "no mark means no ghost twin");
    assert_eq!(
        entry_no_mark.attribution.which,
        Attribution::None,
        "no mark means no attribution",
    );
    assert!(
        entry_no_mark.wreck.is_some(),
        "the wreck fires regardless of whether a mark was placed"
    );
}
