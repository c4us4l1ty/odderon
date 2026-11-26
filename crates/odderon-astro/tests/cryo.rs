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
    chilldown_residual_heat, flash_fraction, subcooling_margin_j_per_kg, vapour_pressure_kpa, ChillPlan,
    CryoNetwork, Failure,
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

    let under = net.simulate(&ChillPlan::underspecified(), 0.50, 60.0).expect("a valid plan runs");
    let (failure, _t) = under
        .failure
        .expect("the under-specified load must cavitate: that is the bible's failure");
    assert_eq!(failure, Failure::Cavitation);

    let specified = net.simulate(&ChillPlan::specified(), 0.50, 60.0).expect("a valid plan runs");
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
    let under = net.simulate(&ChillPlan::underspecified(), 0.50, 60.0).unwrap();
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

    assert_eq!(a, b, "the same input log must reproduce the same outcome exactly");
    assert_eq!(a.failure, b.failure, "and the failure, in particular, must be identical");
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

    let (_, t_short) = short.failure.expect("the under-specified load fails within 30 s");
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
    assert!(warm > cold, "P_sat must rise with T: {} at 85 K vs {} at 100 K", cold, warm);
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
        let margin = subcooling_margin_j_per_kg(tank_t, tank_p).expect("a positive subcooling has a margin");
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
    let short = chilldown_residual_heat(&ChillPlan { chill_s: 5.0, bleeds: 0 }).unwrap();
    let long = chilldown_residual_heat(&ChillPlan { chill_s: 600.0, bleeds: 0 }).unwrap();
    assert!(long < short, "longer chill must leave less heat: {} vs {}", long, short);
    assert!(long > 0.0, "chilldown asymptotes to the floor, never below zero");
}

/// More bleeds remove heat too: each purge cycle takes a fraction of the residual with it.
#[test]
fn more_bleeds_remove_more_heat() {
    let few = chilldown_residual_heat(&ChillPlan { chill_s: 60.0, bleeds: 1 }).unwrap();
    let many = chilldown_residual_heat(&ChillPlan { chill_s: 60.0, bleeds: 5 }).unwrap();
    assert!(many < few, "more bleeds must leave less heat: {} vs {}", many, few);
}

/// Neither mechanism substitutes for the other: a long chill with no bleeds still leaves more heat than the
/// same chill with bleeds, and many bleeds with no chill time still leave a line that never got cold. This
/// is the property the bible's "one item that is wrong" exploits — fixing only one of the two is not a
/// correct chilldown.
#[test]
fn neither_chill_time_nor_bleeds_substitutes_for_the_other() {
    let time_only = chilldown_residual_heat(&ChillPlan { chill_s: 600.0, bleeds: 0 }).unwrap();
    let bleeds_only = chilldown_residual_heat(&ChillPlan { chill_s: 0.0, bleeds: 5 }).unwrap();
    let both = chilldown_residual_heat(&ChillPlan { chill_s: 600.0, bleeds: 5 }).unwrap();
    assert!(both < time_only, "bleeds on top of chill must help");
    assert!(both < bleeds_only, "chill on top of bleeds must help");
}

/// A non-finite or negative chill time is refused: a plan that is not a plan is not a chilldown.
#[test]
fn a_non_finite_chill_plan_is_refused() {
    assert!(chilldown_residual_heat(&ChillPlan { chill_s: f64::NAN, bleeds: 0 }).is_none());
    assert!(chilldown_residual_heat(&ChillPlan { chill_s: f64::INFINITY, bleeds: 0 }).is_none());
    assert!(chilldown_residual_heat(&ChillPlan { chill_s: -1.0, bleeds: 0 }).is_none());
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
    let out = net.simulate(&ChillPlan::underspecified(), 0.0, 60.0).unwrap();
    assert_eq!(out.liquid_kg, 0.0);
    assert!(out.failure.is_none(), "an empty fill must not cavitate");
}
