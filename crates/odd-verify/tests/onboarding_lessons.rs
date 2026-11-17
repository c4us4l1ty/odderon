//! Measurement harness for the first-30-minutes onboarding flow.
//!
//! `docs/onboarding.md` proposes six lessons. Law 7 says a spec card is a
//! hypothesis, and every lesson in that document is a card: it claims that a
//! particular "tempting first attempt" fails in a particular, *teachable* way,
//! and that a particular fix succeeds. This file is the measurement of those
//! claims. Each test flies the tempting attempt and the intended fix through the
//! shipped `odd_verify::compute` path and prints both outcomes.
//!
//! These are not gates on player success — an unscored lesson has no pass
//! threshold. They gate the **discovery**: if the tempting attempt does not
//! produce the failure the lesson claims, or the fix does not produce the
//! correction, the lesson is not viable and the test fails. A lesson whose
//! physics does not produce the lesson is removed from the document, not
//! softened in the test.
//!
//! Run with `--nocapture` to read the measurements:
//!
//! ```text
//! source scripts/env.sh
//! cargo test -p odd-verify --test onboarding_lessons -- --nocapture
//! ```
//!
//! # Corrections this harness had to make to itself
//!
//! Each of these was a lesson that the kernel refused, and each changed the
//! design document rather than the physics:
//!
//! 1. **Over-budget attempts are refused, not flown.** Contract 101 carries
//!    180 kg of propellant on a 1,020 kg dry stage at 310.6 s, which is
//!    495.1 m/s. A "tempting" 2,000 m/s burn is not tempting, it is impossible,
//!    and it returns `PropellantDeficit` before any physics runs.
//! 2. **Burns are centred on their node.** `compile_burn` starts a burn at
//!    `node_us - dur/2`, so a 100 m/s burn on this stage is ~50 s long and a
//!    node at t=10 s is refused with `BurnOutsidePlan`.
//! 3. **TNW is `[T, N, W]`, and `N` is in-plane.** A burn along index 1 is a
//!    radial burn and moves inclination by 0.009 deg. Only index 2 rotates the
//!    plane. The first draft of lesson 3 burned along index 1.
//! 4. **Guidance is not optional.** `split_tug_for_stage` returns, per node,
//!    the state at the periapsis it targeted. Compiling with `guidance = &[]`
//!    instead of `Some(split.guidance)` silently discards the targeting: the
//!    same three-kick plan then falls into the Earth instead of reaching
//!    387,653 km.

use odd_contracts::{Contract, DelegationMask};
use odd_core::frames::State;
use odd_ephem::Ephemeris;
use odd_sim::mission::{LockedMission, PredictionBaseline};
use odd_sim::planner::{ManeuverNode, Plan, PlanError};
use odd_verify::FlightSettings;

const EARTH: f64 = odd_contracts::EARTH_EQUATORIAL_RADIUS;
const MU: f64 = odd_contracts::EARTH_MU;
/// Standard gravity, for the rocket-equation budget.
const G0: f64 = 9.806_65;
/// Sea-level pressure, for lesson 4's ambient.
const SEA_LEVEL_PA: f64 = 101_325.0;

/// Report indices used below, from the frozen 58-field layout.
const APOAPSIS: usize = 1;
const PERIAPSIS: usize = 2;
const ECCENTRICITY: usize = 3;
const SCORE: usize = 4;
const COMMANDED_DV: usize = 5;
/// The isolated two-body finite-burn decomposition: ideal delta-v, steering
/// loss, gravity loss, burn duration, and the swept angle.
const IDEAL_DV: usize = 20;
const STEERING_LOSS: usize = 21;
const GRAVITY_LOSS: usize = 22;
const BURN_DURATION_S: usize = 25;
const DURATION_S: usize = 29;
const IMPACTED: usize = 30;
const INCLINATION: usize = 26;
const NODE: usize = 27;
const SCORE_PREDICTION: usize = 34;
const SCORE_OBJECTIVES: usize = 35;
const SWEEP_DEG: usize = 57;

/// Report fields 1 and 2 are **altitudes above the equatorial radius**, not
/// radii. Subtracting the radius again yields negative "altitudes", which is
/// how this harness first reported a 5,820 km crater.
fn km(altitude_m: f64) -> f64 {
    altitude_m / 1_000.0
}

fn degrees(radians: f64) -> f64 {
    radians / std::f64::consts::PI * 180.0
}

/// Total delta-v a contract's own stage can deliver from full tanks to dry.
fn budget_m_s(contract: &Contract) -> f64 {
    contract.engine.specific_impulse_seconds
        * G0
        * libm::log(contract.initial.mass / contract.dry_mass_kg)
}

fn settings_for(contract: &Contract) -> FlightSettings {
    FlightSettings {
        contract_id: contract.id,
        epoch: odd_contracts::START_EPOCH,
        initial: contract.initial,
        duration_us: contract.duration_us,
        dry_mass: contract.dry_mass_kg,
        engine: contract.engine,
        node_metric: false,
        thermal: None,
        delegation: DelegationMask::MANUAL,
        revision: odd_core::PhysicsRevision::CoupledFitted,
    }
}

/// One flight. Returns the report, or the reason the plan was refused — the
/// refusal itself is a measured outcome and often the lesson.
fn flight(contract: &Contract, nodes: &[ManeuverNode]) -> Result<[f64; 58], PlanError> {
    flight_guided(contract, nodes, None)
}

/// A flight with an explicit guidance reference per node.
///
/// Guidance matters: `split_tug_for_stage` returns, per node, the state at the
/// periapsis it targeted. Compiling with `guidance = &[]` re-derives each burn
/// from the *unperturbed* coast, which silently discards the targeting and turns
/// a validated three-kick transfer into three mistimed burns. The browser always
/// passes it (`mission_split_tug` → `settings.guidance`), so the harness must too.
fn flight_guided(
    contract: &Contract,
    nodes: &[ManeuverNode],
    guidance: Option<Vec<Option<State>>>,
) -> Result<[f64; 58], PlanError> {
    let eph = Ephemeris::bundled().unwrap();
    let plan = Plan::compile_guided(
        contract.initial,
        MU,
        contract.duration_us,
        contract.dry_mass_kg,
        contract.engine,
        nodes,
        guidance.as_deref().unwrap_or(&[]),
    )?;
    odd_verify::compute(&settings_for(contract), &plan, None, &eph)
        .map(|o| o.report)
        .ok_or(PlanError::NumericalRange)
}

/// One flight scored against a locked prediction.
fn scored_flight(
    contract: &Contract,
    nodes: &[ManeuverNode],
    prediction: PredictionBaseline,
) -> Option<[f64; 58]> {
    let eph = Ephemeris::bundled().unwrap();
    let plan = Plan::compile(
        contract.initial,
        MU,
        contract.duration_us,
        contract.dry_mass_kg,
        contract.engine,
        nodes,
    )
    .ok()?;
    let lock = LockedMission::new(plan.clone(), prediction, contract.dry_mass_kg).ok()?;
    odd_verify::compute(&settings_for(contract), &plan, Some(&lock), &eph).map(|o| o.report)
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// Every lesson contract starts already in orbit, above the atmosphere. This is
/// what lets the whole first 30 minutes avoid ascent, surface launch, gravity
/// turn and launch-site physics — none of which is shipped.
#[test]
fn every_lesson_contract_starts_already_in_orbit() {
    for id in [101u32, 102, 103] {
        let contract = Contract::get(id).unwrap();
        let r = flight(&contract, &[]).expect("a coast must compute");
        let (apo, peri) = (km(r[APOAPSIS]), km(r[PERIAPSIS]));
        println!(
            "contract {id}: start {peri:.1} x {apo:.1} km, e {:.5}, \
             dv budget {:.1} m/s, window {:.0} min",
            r[ECCENTRICITY],
            budget_m_s(&contract),
            contract.duration_us as f64 / 6e7
        );
        assert!(
            peri > 100.0,
            "contract {id} must start clear of the atmosphere, got {peri:.1} km"
        );
    }
}

// ---------------------------------------------------------------------------
// Lesson 1 — "Your satellite is falling"
//
// Claim: one prograde burn raises the far side and leaves the near side where
// it was, so the low side stays in the atmosphere; a second burn at the far side
// is needed. That is a Hohmann transfer, discovered by hand.
// ---------------------------------------------------------------------------

/// The whole 495.1 m/s budget on one prograde burn: the single most obvious
/// thing a new player can do, and the most they can possibly do.
fn lesson_1_tempting() -> [ManeuverNode; 1] {
    [ManeuverNode {
        epoch_us: 200_000_000,
        tnw_m_s: [495.0, 0.0, 0.0],
    }]
}

/// The intended fix: half the transfer now, half at the far side.
fn lesson_1_fix() -> [ManeuverNode; 2] {
    let r0 = EARTH + 260_000.0;
    let r1 = EARTH + 550_000.0;
    let a = (r0 + r1) * 0.5;
    let coast = std::f64::consts::PI * libm::sqrt(a * a * a / MU);
    let v0 = libm::sqrt(MU / r0);
    let v1 = libm::sqrt(MU / r1);
    [
        ManeuverNode {
            epoch_us: 200_000_000,
            tnw_m_s: [v0 * (libm::sqrt(a / r0) - 1.0), 0.0, 0.0],
        },
        ManeuverNode {
            epoch_us: ((200.0 + coast) * 1e6) as i64,
            tnw_m_s: [v1 * (1.0 - libm::sqrt(r0 / a)), 0.0, 0.0],
        },
    ]
}

#[test]
fn lesson_1_the_tempting_max_burn_leaves_the_low_side_in_the_atmosphere() {
    let contract = Contract::get(101).unwrap();
    let r = flight(&contract, &lesson_1_tempting()).expect("495 m/s is inside the budget");
    println!(
        "lesson 1 / tempting (495 m/s, one burn): apoapsis {:.1} km, periapsis {:.1} km, e {:.4}",
        km(r[APOAPSIS]),
        km(r[PERIAPSIS]),
        r[ECCENTRICITY]
    );
    assert!(
        km(r[APOAPSIS]) > 1_500.0,
        "the burn must visibly raise the far side, got {:.0} km",
        km(r[APOAPSIS])
    );
    assert!(
        km(r[PERIAPSIS]) < 280.0,
        "the low side must stay near where it started, got {:.0} km",
        km(r[PERIAPSIS])
    );
    // The objective is 550 km. The tempting burn overshoots it wildly *and*
    // leaves the orbit unusable — the failure the lesson is built on.
    assert!(
        km(r[APOAPSIS]) > 550.0 * 2.0,
        "the lesson needs a spectacular overshoot"
    );
}

#[test]
fn lesson_1_the_two_burn_fix_circularises_at_the_target() {
    let contract = Contract::get(101).unwrap();
    let r = flight(&contract, &lesson_1_fix()).expect("the two-burn fix must fly");
    println!(
        "lesson 1 / fix ({:.1} + {:.1} m/s): apoapsis {:.1} km, periapsis {:.1} km, e {:.4}",
        lesson_1_fix()[0].tnw_m_s[0],
        lesson_1_fix()[1].tnw_m_s[0],
        km(r[APOAPSIS]),
        km(r[PERIAPSIS]),
        r[ECCENTRICITY]
    );
    assert!(
        (km(r[APOAPSIS]) - 550.0).abs() < 15.0 && (km(r[PERIAPSIS]) - 550.0).abs() < 15.0,
        "the fix must land near the 550 km objective, got {:.0} x {:.0} km",
        km(r[PERIAPSIS]),
        km(r[APOAPSIS])
    );
    assert!(
        r[ECCENTRICITY] < 0.005,
        "the fix must circularise, got e {:.4}",
        r[ECCENTRICITY]
    );
}

#[test]
fn lesson_1_the_fix_stays_inside_the_measured_budget() {
    let contract = Contract::get(101).unwrap();
    let budget = budget_m_s(&contract);
    let r = flight(&contract, &lesson_1_fix()).unwrap();
    println!(
        "lesson 1 / fix spends {:.1} m/s of a {:.1} m/s budget ({:.0}%)",
        r[COMMANDED_DV],
        budget,
        r[COMMANDED_DV] / budget * 100.0
    );
    assert!(
        r[COMMANDED_DV] < budget * 0.9,
        "the lesson must leave the player room to be wrong once"
    );
}

#[test]
fn lesson_1_an_over_budget_attempt_is_refused_not_silently_flown() {
    let contract = Contract::get(101).unwrap();
    let budget = budget_m_s(&contract);
    let over = [ManeuverNode {
        epoch_us: 200_000_000,
        tnw_m_s: [2_000.0, 0.0, 0.0],
    }];
    // Which refusal wins depends on which limit trips first — a 2,000 m/s burn
    // on this stage both exceeds the tanks and runs past the end of the mission
    // window — so the lesson depends only on the plan being refused rather than
    // flown, not on the specific code.
    let refused = flight(&contract, &over);
    println!("lesson 1 / 2000 m/s attempt on a {budget:.0} m/s budget: {refused:?}");
    assert!(
        refused.is_err(),
        "an impossible burn must fail loudly at compile time"
    );
}

// ---------------------------------------------------------------------------
// Lesson 2 — "Guess first"
//
// Claim: a player can be asked to predict the outcome before seeing it, and the
// prediction can be scored against the flown result without any new physics.
// This test establishes the *spread* the guess has to resolve, which is what
// sizes the prediction marker's scale.
// ---------------------------------------------------------------------------

#[test]
fn lesson_2_a_guess_can_be_compared_against_the_flown_result() {
    let contract = Contract::get(101).unwrap();
    let flown = flight(&contract, &lesson_1_tempting()).unwrap();
    let actual_apo = km(flown[APOAPSIS]);
    // A plausible naive guess: "three times the burn, three times as high".
    let guess = 260.0 + (actual_apo - 260.0) * 0.66;
    println!(
        "lesson 2 / guessed apoapsis {guess:.1} km, flown {actual_apo:.1} km, \
         gap {:.1} km",
        (guess - actual_apo).abs()
    );
    // The gap has to be large enough to be visible on a screen and small enough
    // that a player who understands the lesson can close it.
    let gap = (guess - actual_apo).abs();
    assert!(
        (200.0..2_000.0).contains(&gap),
        "the guess must be visibly wrong but not absurd, gap {gap:.0} km"
    );
}

// ---------------------------------------------------------------------------
// Lesson 3 — "Tilting the orbit"
//
// Claim: the plane is a separate degree of freedom from the shape, and the
// plane change belongs where the vehicle is slowest. Contract 102 starts at
// 250 x 800 km, inclination 96.00, and wants 98.60.
// ---------------------------------------------------------------------------

/// TNW is `[T, N, W]` where `W` is the orbit normal (angular momentum). A burn
/// along `N` (index 1) is a *radial in-plane* burn: it changes the orbit's
/// shape and leaves the plane alone. Only index 2 buys inclination. The first
/// draft of this harness burned along index 1 and measured 0.0086 deg of tilt,
/// which is the lesson failing rather than the player.
#[test]
fn lesson_3_the_orbit_normal_component_is_the_one_that_tilts() {
    let contract = Contract::get(102).unwrap();
    // A 100 m/s burn on this stage is centred on its node, so the node must be
    // far enough in for the first half of the burn to stay inside the window.
    let node_epoch_us = 600_000_000;
    let mut tilts = [0.0; 2];
    for (slot, (label, component)) in [("in-plane N", 1usize), ("orbit normal W", 2usize)]
        .into_iter()
        .enumerate()
    {
        let mut tnw = [0.0; 3];
        tnw[component] = 100.0;
        let r = flight(
            &contract,
            &[ManeuverNode {
                epoch_us: node_epoch_us,
                tnw_m_s: tnw,
            }],
        )
        .expect("100 m/s is well inside the budget");
        tilts[slot] = degrees(r[INCLINATION]) - 96.0;
        println!(
            "lesson 3 / 100 m/s along {label}: inclination {:.4} deg ({:+.4}), \
             node {:.2} deg, apoapsis {:.0} km, periapsis {:.0} km",
            degrees(r[INCLINATION]),
            tilts[slot],
            degrees(r[NODE]),
            km(r[APOAPSIS]),
            km(r[PERIAPSIS]),
        );
    }
    assert!(
        tilts[0].abs() < 0.02,
        "the in-plane component must not tilt the plane, got {:+.4} deg",
        tilts[0]
    );
    assert!(
        tilts[1].abs() > 0.5,
        "the orbit-normal component must tilt it, got {:+.4} deg",
        tilts[1]
    );
}

/// The tempting first attempt for 102 is a *prograde* burn: it fixes the height
/// and leaves the plane exactly where it was. The fix is to spend part of the
/// same burn out of plane.
#[test]
fn lesson_3_a_prograde_burn_cannot_change_the_plane() {
    let contract = Contract::get(102).unwrap();
    let nodes = contract.example_nodes();
    let reference = flight(&contract, &nodes).expect("the reference must fly");
    let prograde_only = [ManeuverNode {
        epoch_us: nodes[0].epoch_us,
        tnw_m_s: [nodes[0].tnw_m_s[0], 0.0, 0.0],
    }];
    let prograde = flight(&contract, &prograde_only).expect("a 142 m/s burn flies");
    println!(
        "lesson 3 / prograde-only {:.0} m/s: inclination {:.3} deg, apoapsis {:.0} km, \
         periapsis {:.0} km",
        nodes[0].tnw_m_s[0],
        degrees(prograde[INCLINATION]),
        km(prograde[APOAPSIS]),
        km(prograde[PERIAPSIS])
    );
    println!(
        "lesson 3 / with the out-of-plane component: inclination {:.3} deg, \
         apoapsis {:.0} km, periapsis {:.0} km, e {:.4}",
        degrees(reference[INCLINATION]),
        km(reference[APOAPSIS]),
        km(reference[PERIAPSIS]),
        reference[ECCENTRICITY]
    );
    assert!(
        (degrees(prograde[INCLINATION]) - 96.0).abs() < 0.01,
        "a burn with no out-of-plane component must not tilt the orbit"
    );
    assert!(
        (degrees(reference[INCLINATION]) - 98.6).abs() < 0.05,
        "the reference must reach 98.60 deg, got {:.4}",
        degrees(reference[INCLINATION])
    );
}

/// The reference flight for 102 is one *combined* burn at apoapsis: a prograde
/// circularisation and a 2.6 degree plane change taken together where the
/// vehicle is slowest. It is a complete mission, so it can be checked against
/// the card's own 366.8 m/s reference instead of an idealised formula.
#[test]
fn lesson_3_the_reference_burn_fits_the_card_and_the_stage() {
    let contract = Contract::get(102).unwrap();
    let r = flight(&contract, &contract.example_nodes()).expect("the reference must fly");
    let budget = budget_m_s(&contract);
    println!(
        "lesson 3 / reference spends {:.1} m/s of the card's {:.1} m/s and the stage's \
         {:.1} m/s; margin {:.0} m/s",
        r[COMMANDED_DV],
        contract.reference_delta_v_m_s,
        budget,
        budget - r[COMMANDED_DV]
    );
    assert!(
        (r[COMMANDED_DV] - contract.reference_delta_v_m_s).abs() < 1.0,
        "the example nodes must reproduce the card's reference budget"
    );
    assert!(
        r[COMMANDED_DV] < budget,
        "the card's reference must be affordable on the card's own stage"
    );
}

/// The plane change belongs where the vehicle is slowest. Both apsides are
/// affordable, which is the precondition for the lesson offering a *choice*
/// rather than a single forced solution.
#[test]
fn lesson_3_the_plane_change_is_cheaper_at_apoapsis() {
    let contract = Contract::get(102).unwrap();
    let rp = EARTH + 250_000.0;
    let ra = EARTH + 800_000.0;
    let a = (rp + ra) * 0.5;
    let period_s = std::f64::consts::TAU * libm::sqrt(a * a * a / MU);
    let v_p = libm::sqrt(MU * (2.0 / rp - 1.0 / a));
    let v_a = libm::sqrt(MU * (2.0 / ra - 1.0 / a));
    let di = (98.6 - 96.0_f64).to_radians();
    let at_periapsis = 2.0 * v_p * libm::sin(di / 2.0);
    let at_apoapsis = 2.0 * v_a * libm::sin(di / 2.0);
    println!(
        "lesson 3 / 2.60 deg costs {at_periapsis:.0} m/s at the 250 km periapsis ({v_p:.0} m/s) \
         and {at_apoapsis:.0} m/s at the 800 km apoapsis ({v_a:.0} m/s), period {period_s:.0} s"
    );
    assert!(
        at_apoapsis < at_periapsis,
        "the turn must be cheaper where the vehicle is slower"
    );
    let budget = budget_m_s(&contract);
    assert!(
        at_periapsis < budget,
        "even the expensive end of the trade must fit the {budget:.0} m/s budget"
    );
}

// ---------------------------------------------------------------------------
// Lesson 4 — "The shaking engine"
//
// Claim: the same engine behaves differently at sea level than in vacuum, and
// the difference is not a scalar "less efficient" — it is a *mechanism*: the
// ambient pressure the bell was not shaped for makes the exhaust detach from the
// wall. This lesson needs no flight at all; it is the engine lab posed at two
// ambient pressures, which is why it is the one lesson that can be taught
// entirely inside the ground-side solver.
// ---------------------------------------------------------------------------

#[test]
fn lesson_4_the_same_bell_flows_full_in_space_and_separates_at_sea_level() {
    use odd_propulsion::solve_engine;

    // Contract 103's baseline engine, which the engine lab opens on.
    let spec = odd_contracts::TUG_BASELINE;
    let mut solutions = Vec::new();
    for (label, ambient) in [("vacuum", 0.0), ("sea level", SEA_LEVEL_PA)] {
        let solved = solve_engine(&spec, ambient).expect("the baseline must solve at any ambient");
        println!(
            "lesson 4 / {label} ({ambient:.0} Pa): thrust {:.0} N, Isp {:.1} s, \
             exit pressure {:.0} Pa, expansion ratio {:.1}",
            solved.thrust_n,
            solved.specific_impulse_s,
            solved.exit_pressure_pa,
            solved.expansion_ratio
        );
        match solved.separation {
            None => println!("lesson 4 /   flow: attached, the bell runs full"),
            Some(sep) => println!(
                "lesson 4 /   flow: separates at x/r = {:.3}, area ratio {:.1}, \
                 side load {:.2} kN",
                sep.x_norm, sep.area_ratio_sep, sep.side_load_kn
            ),
        }
        solutions.push(solved);
    }
    let (vacuum, sea) = (&solutions[0], &solutions[1]);
    println!(
        "lesson 4 / thrust {:.0} -> {:.0} N ({:.0}%), Isp {:.1} -> {:.1} s",
        vacuum.thrust_n,
        sea.thrust_n,
        (sea.thrust_n / vacuum.thrust_n - 1.0) * 100.0,
        vacuum.specific_impulse_s,
        sea.specific_impulse_s
    );
    assert!(
        vacuum.separation.is_none(),
        "a vacuum bell must flow full in vacuum"
    );
    assert!(
        sea.separation.is_some(),
        "the same bell must separate at sea level — this is the lesson"
    );
    assert!(
        sea.thrust_n < vacuum.thrust_n,
        "the separated engine must make less thrust, got {:.0} vs {:.0}",
        sea.thrust_n,
        vacuum.thrust_n
    );
}

/// The lesson's second beat: separation is a *threshold*, not a slope. Sweeping
/// altitude has to show a clean transition the player can find, because a
/// gradual drift is not a discoverable event.
#[test]
fn lesson_4_separation_appears_at_a_findable_altitude() {
    use odd_propulsion::solve_engine;
    let spec = odd_contracts::TUG_BASELINE;
    let mut crossing_km: Option<f64> = None;
    let mut previous = true;
    for altitude_km in 0..=40u32 {
        let ambient = SEA_LEVEL_PA * libm::exp(-(altitude_km as f64) * 1_000.0 / 8_500.0);
        let solved = solve_engine(&spec, ambient).unwrap();
        let separated = solved.separation.is_some();
        if previous && !separated && crossing_km.is_none() {
            crossing_km = Some(altitude_km as f64);
        }
        previous = separated;
        if altitude_km % 5 == 0 || crossing_km == Some(altitude_km as f64) {
            println!(
                "lesson 4 / {altitude_km:2} km ({ambient:8.0} Pa): thrust {:6.0} N, \
                 Isp {:5.1} s, {}",
                solved.thrust_n,
                solved.specific_impulse_s,
                if separated { "SEPARATED" } else { "attached" }
            );
        }
    }
    println!("lesson 4 / separation clears near {crossing_km:?} km");
    assert!(
        crossing_km.is_some(),
        "the lab must present a findable altitude where the flow reattaches"
    );
}

// ---------------------------------------------------------------------------
// Lesson 5 — "The long-burn trap"
//
// Claim: the delta-v is fixed by the orbit change, but the *loss* is set by how
// long the engine stays lit near periapsis. Contract 103 is an 18 t tug at
// 18 kN — a thrust-to-weight near 0.1 — so a 3,240 m/s transfer is a 2,284 s
// burn that swings 149 deg while the engine is on and loses ~3,600 m/s to
// gravity and steering.
// ---------------------------------------------------------------------------

/// The unavoidable cost: engine-on time is fixed by the mass, the thrust and the
/// delta-v, and the sweep is what that time buys at orbital rate. Nothing about
/// the player's plan changes these numbers.
#[test]
fn lesson_5_measures_the_finite_burn_penalty_itself() {
    let contract = Contract::get(103).unwrap();
    let eph = Ephemeris::bundled().unwrap();
    let split = odd_contracts::split_tug_for_stage(&contract, 3, 3_240.0, 1600.0, &eph).unwrap();
    // Engine-on time is set by the rocket equation, not by the naive
    // delta-v-over-acceleration ratio: the vehicle lightens as it burns, so its
    // thrust-to-weight *rises* during the burn. The naive ratio understates the
    // duration by a factor of nearly three here (790 s against 2,284 s), which
    // is itself worth saying out loud in the lesson.
    let exhaust = contract.engine.specific_impulse_seconds * G0;
    let propellant = contract.initial.mass * (1.0 - libm::exp(-3_240.0 / exhaust));
    println!(
        "lesson 5 / tug TWR at ignition {:.2} (rising as it burns), {:.0} m/s needs \
         {:.0} kg of propellant and {:.0} s of engine-on time",
        contract.engine.thrust_newtons / (contract.initial.mass * G0),
        3_240.0,
        propellant,
        propellant * exhaust / contract.engine.thrust_newtons
    );
    let r = flight_guided(&contract, &split.nodes, Some(split.guidance.clone()))
        .expect("the splitter's own plan must fly");
    println!(
        "lesson 5 / shipped 3-kick split: ideal {:.0} m/s, gravity loss {:.0} m/s, \
         steering loss {:.0} m/s, burn time {:.0} s, sweep {:.0} deg",
        r[IDEAL_DV],
        r[GRAVITY_LOSS],
        r[STEERING_LOSS],
        r[BURN_DURATION_S],
        degrees(r[SWEEP_DEG])
    );
    // Losses must be large enough to be the lesson, and the sweep large enough
    // that `LongFiniteBurnArc` (burn_sweep_deg > 25) fires on the tempting plan.
    assert!(
        r[GRAVITY_LOSS] > 100.0,
        "the lesson needs a visible gravity loss, got {:.0} m/s",
        r[GRAVITY_LOSS]
    );
    assert!(
        degrees(r[SWEEP_DEG]) > 25.0,
        "the tempting plan must trip the long-arc rule, sweep {:.0} deg",
        degrees(r[SWEEP_DEG])
    );
}

/// The tempting attempt: spend the whole 3,240 m/s in one burn at the first
/// periapsis. It is *legal* — the stage has the propellant — and it is a
/// catastrophe. Measured: the arc swings 149 deg while the engine is lit, the
/// vehicle drives its periapsis 6,365 km below the surface and re-enters, and
/// 2,707 m/s of the 3,240 m/s commanded — 84% of it — is lost to gravity and
/// steering before the vehicle reaches any of the altitude it paid for.
///
/// This is the lesson's whole point, and it is a better failure than a refusal:
/// the plan compiles, the engine lights, and the result is a crater.
#[test]
fn lesson_5_one_burn_is_flown_and_re_enters() {
    let contract = Contract::get(103).unwrap();
    let r = flight(
        &contract,
        &[ManeuverNode {
            epoch_us: 1_600_000_000,
            tnw_m_s: [3_240.0, 0.0, 0.0],
        }],
    )
    .expect("the single burn is legal: the stage has the propellant for it");
    println!(
        "lesson 5 / tempting single burn: apoapsis {:.0} km, periapsis {:.0} km, e {:.3}, \
         impacted {}, gravity loss {:.0} m/s, steering loss {:.0} m/s, sweep {:.0} deg",
        km(r[APOAPSIS]),
        km(r[PERIAPSIS]),
        r[ECCENTRICITY],
        r[IMPACTED] > 0.5,
        r[GRAVITY_LOSS],
        r[STEERING_LOSS],
        degrees(r[SWEEP_DEG])
    );
    assert!(
        r[IMPACTED] > 0.5,
        "the tempting burn must re-enter, not merely miss"
    );
    assert!(
        km(r[PERIAPSIS]) < 0.0,
        "the periapsis must be driven below the surface, got {:.0} km",
        km(r[PERIAPSIS])
    );
    assert!(
        r[GRAVITY_LOSS] + r[STEERING_LOSS] > 2_500.0,
        "the lesson needs most of the burn thrown away, got {:.0} m/s lost of {:.0} m/s spent",
        r[GRAVITY_LOSS] + r[STEERING_LOSS],
        r[COMMANDED_DV]
    );
    // The same 3,240 m/s in one burn, earlier in the window when the burn cannot
    // even be centred, is refused instead. Both are recorded: the lesson must
    // not assume which one the player's node epoch produces.
    let early = flight(
        &contract,
        &[ManeuverNode {
            epoch_us: 200_000_000,
            tnw_m_s: [3_240.0, 0.0, 0.0],
        }],
    );
    println!("lesson 5 / the same burn at t=200 s: {early:?}");
}

/// The fix the game ships: `split_tug_for_stage`. Three kicks at three real
/// periapses, each retargeted against the perturbed trajectory, and each flown
/// with the guidance state that targeting produced.
#[test]
fn lesson_5_the_shipped_splitter_reaches_translunar_apoapsis() {
    let contract = Contract::get(103).unwrap();
    let eph = Ephemeris::bundled().unwrap();
    let mut results = Vec::new();
    for kicks in 2..=3usize {
        let split = odd_contracts::split_tug_for_stage(&contract, kicks, 3_240.0, 1600.0, &eph)
            .expect("the shipped splitter must produce a plan");
        let epochs: Vec<f64> = split
            .nodes
            .iter()
            .map(|n| n.epoch_us as f64 / 1e6)
            .collect();
        let r = flight_guided(&contract, &split.nodes, Some(split.guidance.clone()))
            .expect("the splitter's own plan must fly");
        // The objective wants an apoapsis 380,000 km from Earth's centre.
        let radius_km = km(r[APOAPSIS]) + EARTH / 1_000.0;
        println!(
            "lesson 5 / {kicks} kicks at {epochs:?} s: apoapsis {:.0} km radius \
             ({:.0} km altitude), periapsis {:.0} km, gravity loss {:.0} m/s, \
             steering loss {:.0} m/s",
            radius_km,
            km(r[APOAPSIS]),
            km(r[PERIAPSIS]),
            r[GRAVITY_LOSS],
            r[STEERING_LOSS]
        );
        results.push((kicks, radius_km, r[GRAVITY_LOSS] + r[STEERING_LOSS]));
    }
    // Three kicks reach the Moon's neighbourhood; two do not. This is the
    // measured discovery the lesson is built on, and it is *not* monotone in the
    // naive direction — more burns do not automatically mean less loss, because
    // a badly placed later burn costs more steering.
    let (_, three_radius, three_loss) = results[1];
    let (_, two_radius, two_loss) = results[0];
    assert!(
        three_radius > 380_000.0 && (three_radius - 380_000.0).abs() / 380_000.0 < 0.15,
        "three kicks must land near the 380,000 km objective, got {three_radius:.0} km"
    );
    assert!(
        two_radius < three_radius,
        "three kicks must reach higher than two, got {two_radius:.0} vs {three_radius:.0} km"
    );
    assert!(
        three_loss < two_loss,
        "three kicks must also lose less, got {three_loss:.0} vs {two_loss:.0} m/s"
    );
}

// ---------------------------------------------------------------------------
// Lesson 6 — the first scored contract
//
// Claim: the same rescue from lesson 1, flown with a locked prediction, earns a
// real score. Two honesty checks come with it: a score requires a prediction,
// and this particular scored vehicle is identical to the flown one *only*
// because it fits no hardware.
// ---------------------------------------------------------------------------

/// The prediction a player who has just learned lesson 1 could plausibly lock:
/// the right delta-v, the right target orbit, and no idea about boil-off.
fn lesson_6_prediction(contract: &Contract) -> PredictionBaseline {
    let flown = flight(contract, &lesson_1_fix()).unwrap();
    PredictionBaseline {
        delta_v_m_s: flown[COMMANDED_DV],
        apoapsis_altitude_m: flown[APOAPSIS],
        periapsis_altitude_m: flown[PERIAPSIS],
        remaining_propellant_kg: flown[0] - contract.dry_mass_kg,
        time_of_flight_s: flown[DURATION_S],
        // The 550 km objective defines no target position, so there is no
        // encounter geometry to miss.
        target_miss_distance_m: None,
    }
}

#[test]
fn lesson_6_the_reference_rescue_earns_a_real_score() {
    let contract = Contract::get(101).unwrap();
    let report = scored_flight(&contract, &lesson_1_fix(), lesson_6_prediction(&contract))
        .expect("a locked flight must score");
    println!(
        "lesson 6 / reference rescue: score {:.4}, apoapsis {:.1} km, periapsis {:.1} km, \
         objectives {:.4}, prediction {:.4}",
        report[SCORE],
        km(report[APOAPSIS]),
        km(report[PERIAPSIS]),
        report[SCORE_OBJECTIVES],
        report[SCORE_PREDICTION]
    );
    assert!(
        report[SCORE] > 0.5,
        "the reference must earn a real score, got {:.4}",
        report[SCORE]
    );
    assert!(
        report[SCORE] <= 1.0 && report[SCORE] >= 0.0,
        "the cap must hold, got {:.4}",
        report[SCORE]
    );
}

/// The discontinuity the document has to fix: without a locked prediction there
/// is no score at all, not a low one.
#[test]
fn lesson_6_has_no_score_without_a_locked_prediction() {
    let contract = Contract::get(101).unwrap();
    let report = flight(&contract, &lesson_1_fix()).unwrap();
    println!("lesson 6 / score without a prediction: {}", report[SCORE]);
    assert!(
        report[SCORE].is_nan(),
        "an unlocked flight must not produce a number that reads as a bad score"
    );
}

/// Scoring honesty: the catalog stage fits no hardware, so its dry mass is
/// exactly its fitted dry mass and the scored vehicle equals the flown one.
/// Any lesson 6 that let the player fit an engine would break this equality and
/// import the prediction-term penalty measured in the previous session.
#[test]
fn lesson_6_is_only_honest_because_it_fits_no_hardware() {
    let contract = Contract::get(101).unwrap();
    let vehicle = contract
        .vehicle(contract.engine_spec)
        .expect("the stage must solve");
    println!(
        "lesson 6 / catalog dry {:.3} kg, fitted dry {:.3} kg, delta {:.3} kg",
        contract.dry_mass_kg,
        vehicle.dry_mass_kg,
        (vehicle.dry_mass_kg - contract.dry_mass_kg).abs()
    );
    assert!(
        (vehicle.dry_mass_kg - contract.dry_mass_kg).abs() < 1e-6,
        "lesson 6's honesty argument requires a stage that fits no hardware"
    );
}
