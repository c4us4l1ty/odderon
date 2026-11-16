//! The regression that the unit-level delegation oracle could not provide.
//!
//! `oracle_delegation_efficiency_cap` calls `chief_ai::cap_score` directly, so
//! it passed even while nothing in production called it. These tests fly a
//! real mission through `compute` and assert the consequences a player would
//! observe: a delegated stack **masses more**, and the same flight **scores
//! less**. If the wiring is ever removed again, these fail.

use odd_contracts::{Contract, DelegationMask, Department, ThermalDesign};
use odd_ephem::Ephemeris;
use odd_sim::mission::{LockedMission, PredictionBaseline};
use odd_sim::planner::{ManeuverNode, Plan};
use odd_verify::{compute, FlightSettings};

fn settings(contract: &Contract, mask: DelegationMask) -> FlightSettings {
    FlightSettings {
        contract_id: contract.id,
        epoch: odd_contracts::START_EPOCH,
        initial: contract.initial,
        duration_us: contract.duration_us,
        dry_mass: contract.dry_mass_kg,
        engine: contract.engine,
        node_metric: contract.id == 102,
        thermal: None,
        delegation: mask,
        revision: odd_core::PhysicsRevision::CoupledFitted,
    }
}

/// A perfect rescue: the reference Hohmann, flown exactly, with a locked
/// prediction that matches. Manual efficiency is 1.0, so the 0.65 cap bites.
fn rescue_plan(contract: &Contract) -> (Plan, LockedMission, [ManeuverNode; 2]) {
    let mu = odd_contracts::EARTH_MU;
    let r0 = odd_contracts::EARTH_EQUATORIAL_RADIUS + 260_000.0;
    let r1 = odd_contracts::EARTH_EQUATORIAL_RADIUS + 550_000.0;
    let a = (r0 + r1) * 0.5;
    let coast = std::f64::consts::PI * libm::sqrt(a * a * a / mu);
    let v0 = libm::sqrt(mu / r0);
    let v1 = libm::sqrt(mu / r1);
    let nodes = [
        ManeuverNode {
            epoch_us: 200_000_000,
            tnw_m_s: [v0 * (libm::sqrt(a / r0) - 1.0), 0.0, 0.0],
        },
        ManeuverNode {
            epoch_us: ((200.0 + coast) * 1e6) as i64,
            tnw_m_s: [v1 * (1.0 - libm::sqrt(r0 / a)), 0.0, 0.0],
        },
    ];
    let plan = Plan::compile(
        contract.initial,
        mu,
        contract.duration_us,
        contract.dry_mass_kg,
        contract.engine,
        &nodes,
    )
    .expect("the reference rescue compiles");
    let total: f64 = nodes.iter().map(|n| n.tnw_m_s[0]).sum();
    let prediction = PredictionBaseline {
        delta_v_m_s: total,
        apoapsis_altitude_m: 550_000.0,
        periapsis_altitude_m: 550_000.0,
        remaining_propellant_kg: 120.0,
        time_of_flight_s: 10_800.0,
        target_miss_distance_m: None,
    };
    let locked = LockedMission::new(plan.clone(), prediction, contract.dry_mass_kg)
        .expect("the prediction locks");
    (plan, locked, nodes)
}

/// A flown delegated mission must score strictly less than the identical
/// manual one. This is the assertion that was missing.
#[test]
fn a_flown_delegated_mission_scores_less() {
    let eph = Ephemeris::bundled().unwrap();
    let rescue = Contract::get(101).unwrap();
    let (plan, locked, _nodes) = rescue_plan(&rescue);

    let manual = compute(
        &settings(&rescue, DelegationMask::MANUAL),
        &plan,
        Some(&locked),
        &eph,
    )
    .expect("the manual rescue evaluates");
    let three = DelegationMask::MANUAL
        .with(Department::Propulsion, true)
        .with(Department::Thermal, true)
        .with(Department::Power, true);
    let delegated = compute(&settings(&rescue, three), &plan, Some(&locked), &eph)
        .expect("the delegated rescue evaluates");

    let manual_score = manual.report[4];
    let delegated_score = delegated.report[4];
    let manual_eff = manual.report[36];
    let delegated_eff = delegated.report[36];
    println!(
        "flown rescue: manual S {manual_score:.6} (Seff {manual_eff:.4}) vs \
         delegated S {delegated_score:.6} (Seff {delegated_eff:.4})"
    );
    assert!(manual_score.is_finite(), "the scenario must score");
    assert!(
        delegated_eff <= 0.65 + 1e-12,
        "a flown delegated flight must be capped, got Seff {delegated_eff}"
    );
    assert!(
        delegated_score < manual_score,
        "delegation must cost score on a real flight: {delegated_score} vs {manual_score}"
    );
    // The objective achieved is identical — only the efficiency is capped.
    assert_eq!(manual.report[35], delegated.report[35]);
    assert_eq!(manual.report[1], delegated.report[1]);
}

/// The margin must be real hardware, not a display annotation: delegating
/// raises the dry mass that flies and therefore lowers the propellant the
/// same tank volume can carry.
#[test]
fn delegation_adds_real_dry_mass_to_the_vehicle() {
    let tug = Contract::get(103).unwrap();
    let design = ThermalDesign::baseline();
    let manual = tug.vehicle(tug.engine_spec).unwrap();
    assert_eq!(manual.delegation_margin_kg, 0.0);
    assert_eq!(manual.delegation_cost_multiplier, 1.0);

    let all = tug
        .vehicle_delegated(tug.engine_spec, DelegationMask::ALL, &design)
        .unwrap();
    println!(
        "tug dry mass: manual {:.3} kg -> delegated {:.3} kg (+{:.3} kg), cost x{:.4}",
        manual.dry_mass_kg,
        all.dry_mass_kg,
        all.delegation_margin_kg,
        all.delegation_cost_multiplier
    );
    assert!(all.delegation_margin_kg > 0.0);
    // Substitution changes the hardware, so the mass difference is the
    // chief's fit-out plus its margin — both must be carried.
    assert!(
        (all.dry_mass_kg - manual.dry_mass_kg - all.thermal_fitout_kg - all.delegation_margin_kg)
            .abs()
            < 1e-9,
        "the substituted fit-out and the margin must both enter the dry mass"
    );
    assert!(all.initial_mass_kg > manual.initial_mass_kg);
    assert!((all.delegation_cost_multiplier - 1.30).abs() < 1e-12);
    // Thrust and Isp are physics, not margins: delegation must not invent
    // performance, only mass and cost.
    assert_eq!(all.engine.thrust_newtons, manual.engine.thrust_newtons);
    assert_eq!(
        all.engine.specific_impulse_seconds,
        manual.engine.specific_impulse_seconds
    );

    // One discipline costs less than four.
    let one = tug
        .vehicle_delegated(
            tug.engine_spec,
            DelegationMask::MANUAL.with(Department::Thermal, true),
            &design,
        )
        .unwrap();
    assert!(one.delegation_margin_kg < all.delegation_margin_kg);
    assert!(one.delegation_cost_multiplier < all.delegation_cost_multiplier);
}

/// The cap must survive a signed round trip: the verifier rebuilds the mask
/// from the record, so a delegated certificate cannot replay as manual and
/// collect the uncapped score.
#[test]
fn the_cap_is_recomputed_from_the_signed_mask() {
    let eph = Ephemeris::bundled().unwrap();
    let rescue = Contract::get(101).unwrap();
    let (plan, locked, nodes) = rescue_plan(&rescue);
    let mask = DelegationMask::ALL;
    let outcome = compute(&settings(&rescue, mask), &plan, Some(&locked), &eph).unwrap();

    let mut spec = odd_verify::FlightSpec {
        revision: odd_core::PhysicsRevision::CoupledFitted,
        contract_id: rescue.id,
        epoch_s: odd_contracts::START_EPOCH,
        duration_us: rescue.duration_us,
        initial_mass_kg: rescue.initial.mass,
        dry_mass_kg: rescue.dry_mass_kg,
        thrust_n: rescue.engine.thrust_newtons,
        specific_impulse_s: rescue.engine.specific_impulse_seconds,
        initial_state: [
            rescue.initial.position[0],
            rescue.initial.position[1],
            rescue.initial.position[2],
            rescue.initial.velocity[0],
            rescue.initial.velocity[1],
            rescue.initial.velocity[2],
        ],
        node_metric: false,
        prediction: [
            locked.prediction().delta_v_m_s,
            locked.prediction().apoapsis_altitude_m,
            locked.prediction().periapsis_altitude_m,
            120.0,
            10_800.0,
            f64::NAN,
        ],
        commands: nodes
            .iter()
            .map(|node| (node.epoch_us, node.tnw_m_s, None))
            .collect(),
        thermal: None,
        delegation: mask,
    };
    let replayed = odd_verify::replay_flight(&spec).expect("the delegated flight replays");
    assert_eq!(replayed.report[4], outcome.report[4]);
    assert!(replayed.report[36] <= 0.65 + 1e-12);

    // Claiming the same flight was manual must change the score, which is
    // exactly why the mask is inside the signed bytes.
    spec.delegation = DelegationMask::MANUAL;
    let as_manual = odd_verify::replay_flight(&spec).unwrap();
    assert!(
        as_manual.report[4] > replayed.report[4],
        "dropping the mask must not reproduce the capped score"
    );
    println!(
        "replay: delegated S {:.6}, forged-manual S {:.6}",
        replayed.report[4], as_manual.report[4]
    );
}
