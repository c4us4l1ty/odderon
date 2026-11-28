//! Tests for the clipboard: the checklist with its wrong item, and the static fire.
//!
//! The bible's Block 2 gate is *a reproducible failure from the input log*. The checklist is the
//! input log. These tests verify that the reference checklist has exactly one wrong item, that the
//! corrected checklist validates clean, that the same checklist produces the same hash, and that the
//! static fire reads the same cryo network the pad uses.

use odderon_astro::clipboard::{Checklist, ChecklistError, FireDebrief, StaticFire};

// =============================================================================================
// The checklist
// =============================================================================================

#[test]
fn the_reference_checklist_has_exactly_one_wrong_item() {
    let mut cl = Checklist::reference();
    let wrong = cl
        .validate()
        .expect("the reference checklist must validate");
    assert_eq!(wrong, 0, "the first item (chill_s) is the wrong one");
    assert!(
        cl.wrong_item().is_some(),
        "wrong_item must be set after validate"
    );
}

#[test]
fn the_wrong_item_is_the_chill_time() {
    let mut cl = Checklist::reference();
    cl.validate().expect("must validate");
    let item = cl.wrong_item().expect("must have a wrong item");
    assert_eq!(item.quantity, "chill_s");
    assert!(
        item.expected < Checklist::CORRECT_CHILL_S,
        "the chill time is too short"
    );
}

#[test]
fn the_corrected_checklist_validates_clean() {
    let mut cl = Checklist::corrected();
    let result = cl.validate();
    // The corrected checklist has NO wrong item — every item is correct.
    assert_eq!(result, Err(ChecklistError::NoWrongItem));
}

#[test]
fn an_empty_checklist_is_refused() {
    let mut cl = Checklist::reference();
    cl.count = 0;
    cl.wrong_index = None;
    assert_eq!(cl.validate(), Err(ChecklistError::Empty));
}

#[test]
fn a_checklist_with_no_wrong_item_is_refused() {
    // All items correct: no productive failure to catch.
    let mut cl = Checklist::corrected();
    assert_eq!(cl.validate(), Err(ChecklistError::NoWrongItem));
}

#[test]
fn a_checklist_with_multiple_wrong_items_is_refused() {
    // Two items wrong: cannot attribute to one cause.
    let mut cl = Checklist::reference();
    // Make a second item wrong: set bleeds to 0 (below the 3-cycle threshold).
    cl.items[1].expected = 0.0;
    let result = cl.validate();
    // Now chill_s AND bleeds are both wrong.
    assert_eq!(result, Err(ChecklistError::MultipleWrong));
}

#[test]
fn the_debrief_names_the_wrong_item_and_points_at_the_remedy() {
    let mut cl = Checklist::reference();
    let wrong = cl.validate().expect("must validate");
    let db = cl.debrief(wrong);
    assert_eq!(db.wrong_quantity, "chill_s");
    assert_eq!(db.remedy_value, Checklist::CORRECT_CHILL_S);
    assert!(db.remedy_note.contains("chill"));
}

#[test]
fn the_design_hash_is_stable_across_clones() {
    let cl = Checklist::reference();
    let h1 = cl.design_hash();
    let cl2 = cl.clone();
    let h2 = cl2.design_hash();
    assert_eq!(h1, h2, "the same checklist must produce the same hash");
}

#[test]
fn a_different_checklist_has_a_different_hash() {
    let ref_hash = Checklist::reference().design_hash();
    let corrected_hash = Checklist::corrected().design_hash();
    assert_ne!(
        ref_hash, corrected_hash,
        "different checklists must have different hashes"
    );
}

#[test]
fn the_module_design_hash_is_stable() {
    // The module's own design hash folds in the correct chill threshold.
    let h1 = odderon_astro::clipboard::design_hash();
    let h2 = odderon_astro::clipboard::design_hash();
    assert_eq!(h1, h2);
}

// =============================================================================================
// The static fire
// =============================================================================================

/// The pad's fill parameters, matching `cryo.rs`'s reference fill.
fn pad_fill() -> (f64, f64) {
    // fill rate kg/s and window s — the same values the pad page uses.
    (2.0, 300.0)
}

#[test]
fn the_underspecified_checklist_fire_does_not_light() {
    // The reference checklist's chill time (5 s) is under-specified, so the pad cavitates
    // and the fire never lights.
    let (fill_rate, window) = pad_fill();
    let fire = StaticFire::run(5.0, 0, fill_rate, window)
        .expect("the fire must return Some even when it does not light");
    assert!(
        !fire.fired,
        "an under-specified chilldown must not light the fire"
    );
    // The vapour fraction must be above the cavitation threshold.
    assert!(
        fire.inlet_vapour_fraction >= odderon_astro::cryo::CAVITATION_THRESHOLD.value,
        "inlet vapour fraction must be at or above cavitation threshold"
    );
}

#[test]
fn the_corrected_checklist_fire_lights() {
    // The corrected checklist's chill time is sufficient, so the pad runs clean and the fire lights.
    let (fill_rate, window) = pad_fill();
    let fire = StaticFire::run(Checklist::CORRECT_CHILL_S, 3, fill_rate, window)
        .expect("the fire must return Some");
    assert!(fire.fired, "a sufficient chilldown must light the fire");
    // The vapour fraction must be below the cavitation threshold.
    assert!(
        fire.inlet_vapour_fraction < odderon_astro::cryo::CAVITATION_THRESHOLD.value,
        "inlet vapour fraction must be below cavitation threshold"
    );
}

#[test]
fn the_fire_readings_are_the_same_for_the_same_checklist() {
    // The bible's gate: a reproducible failure from the input log.
    let (fill_rate, window) = pad_fill();
    let f1 = StaticFire::run(5.0, 0, fill_rate, window).expect("must return Some");
    let f2 = StaticFire::run(5.0, 0, fill_rate, window).expect("must return Some");
    assert_eq!(
        f1, f2,
        "the same checklist must produce the same fire readings"
    );
}

#[test]
fn the_fire_refuses_non_finite_inputs() {
    let (fill_rate, window) = pad_fill();
    assert!(StaticFire::run(f64::NAN, 0, fill_rate, window).is_none());
    assert!(StaticFire::run(f64::INFINITY, 0, fill_rate, window).is_none());
    assert!(StaticFire::run(-1.0, 0, fill_rate, window).is_none());
    assert!(StaticFire::run(5.0, 0, f64::NAN, window).is_none());
    assert!(StaticFire::run(5.0, 0, 0.0, window).is_none());
    assert!(StaticFire::run(5.0, 0, fill_rate, 0.0).is_none());
}

#[test]
fn the_fire_debrief_measures_the_players_divergence() {
    let (fill_rate, window) = pad_fill();
    let fire = StaticFire::run(Checklist::CORRECT_CHILL_S, 3, fill_rate, window)
        .expect("must return Some");
    // The player marks the vapour fraction at 0.0 (close to the clean-run reading) and
    // pressure at 500 kPa (wrong).
    let db = StaticFire::debrief(&fire, 0.0, 500.0).expect("must return Some");
    assert!(db.fired);
    // The vapour divergence should be small (the mark is near the actual clean-run reading).
    assert!(db.divergence_vapour < 0.1);
    // The pressure divergence should be large.
    assert!(db.divergence_pressure > 100.0);
}

#[test]
fn the_fire_debrief_refuses_non_finite_marks() {
    let (fill_rate, window) = pad_fill();
    let fire = StaticFire::run(Checklist::CORRECT_CHILL_S, 3, fill_rate, window)
        .expect("must return Some");
    assert!(StaticFire::debrief(&fire, f64::NAN, 100.0).is_none());
    assert!(StaticFire::debrief(&fire, 0.05, f64::INFINITY).is_none());
}

#[test]
fn divergence_is_symmetric_and_absolute() {
    // The divergence between a mark and an actual reading is the absolute difference,
    // regardless of which is larger.
    let d1 = StaticFire::divergence(0.1, 0.05).expect("must return Some");
    let d2 = StaticFire::divergence(0.05, 0.1).expect("must return Some");
    assert_eq!(d1, d2);
    assert!(d1 > 0.0);
}

#[test]
fn divergence_refuses_non_finite() {
    assert!(StaticFire::divergence(f64::NAN, 0.1).is_none());
    assert!(StaticFire::divergence(0.1, f64::INFINITY).is_none());
}

#[test]
fn the_fire_debrief_carries_both_actual_readings() {
    let (fill_rate, window) = pad_fill();
    let fire = StaticFire::run(Checklist::CORRECT_CHILL_S, 3, fill_rate, window)
        .expect("must return Some");
    let db: FireDebrief = StaticFire::debrief(&fire, 0.0, 100.0).expect("must return Some");
    // The actual readings are the fire's own values.
    assert!((db.actual_vapour_fraction - fire.inlet_vapour_fraction).abs() < 1e-12);
    assert!((db.actual_pressure_kpa - fire.pressure_kpa).abs() < 1e-9);
}
