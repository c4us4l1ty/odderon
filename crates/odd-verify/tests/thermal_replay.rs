//! WP 1.6 wiring: boil-off is evaluated inside the Level-3 code path, so the
//! browser and a headless replay produce the same vent history from the same
//! physical specs. Nothing here trusts a claimed loss figure.

use odd_astro::spline::Keyframe;
use odd_contracts::{Contract, ThermalDesign};
use odd_ephem::Ephemeris;
use odd_sim::planner::{ManeuverNode, Plan};
use odd_verify::{compute, FlightSettings, ThermalInputs};

const EPOCH: i64 = 1_767_225_600;

fn settings(contract: &Contract, thermal: Option<ThermalInputs>) -> FlightSettings {
    FlightSettings {
        contract_id: contract.id,
        epoch: EPOCH,
        initial: contract.initial,
        duration_us: contract.duration_us,
        dry_mass: contract.dry_mass_kg,
        engine: contract.engine,
        node_metric: contract.id == 102,
        delegation: odd_contracts::DelegationMask::MANUAL,
        thermal,
    }
}

fn coast(contract: &Contract) -> Plan {
    Plan::compile(
        contract.initial,
        odd_contracts::EARTH_MU,
        contract.duration_us,
        contract.dry_mass_kg,
        contract.engine,
        &[],
    )
    .expect("an empty coast compiles")
}

fn inputs(contract: &Contract, design: ThermalDesign) -> ThermalInputs {
    ThermalInputs {
        engine_spec: contract.engine_spec,
        design,
        load_kg: contract.initial.mass - contract.dry_mass_kg,
    }
}

/// The five-day cislunar tug really loses propellant, and the loss is a
/// design lever: a better blanket loses less, active cooling loses less
/// still, and every watt of cooling comes off the bus.
#[test]
fn the_tug_boils_off_and_the_design_changes_it() {
    let eph = Ephemeris::bundled().unwrap();
    let tug = Contract::get(103).unwrap();
    let plan = coast(&tug);
    let load = tug.initial.mass - tug.dry_mass_kg;
    println!(
        "tug load {load:.1} kg over {} days",
        tug.duration_us as f64 / 86_400e6
    );

    let baseline = ThermalDesign::baseline();
    let flown = compute(
        &settings(&tug, Some(inputs(&tug, baseline))),
        &plan,
        None,
        &eph,
    )
    .expect("the tug coast evaluates");
    assert!(!flown.thermal.inert, "a hydrolox tug must model its tanks");
    let base_loss = flown.thermal.lost_kg;
    let fraction = base_loss / load;
    println!(
        "20-layer baseline: {base_loss:.3} kg = {:.3}% of load, {} vent events, cooler {:.0} J",
        100.0 * fraction,
        flown.thermal.vents.len(),
        flown.thermal.cooler_energy_j
    );
    // The contract card's target: a few percent of 12.8 t over five days.
    assert!(
        fraction > 0.0,
        "a real cryogenic coast must lose propellant"
    );
    assert!(
        fraction < 0.10,
        "20 layers should cost a few percent, not {:.1}%",
        100.0 * fraction
    );
    assert!(!flown.thermal.vents.is_empty(), "venting must be recorded");
    // The 58-field report is fully allocated, so the thermal outcome travels
    // beside it rather than overwriting the Shapley coalition metrics.
    assert!(flown.report[41..57]
        .iter()
        .any(|v| v.is_finite() && *v != base_loss));

    // A thicker blanket must lose strictly less.
    let mut better = baseline;
    better.mli_layers = 40;
    better.epsilon_eff = odd_systems::ideal_epsilon_for_layers(40) * 4.0;
    let insulated = compute(
        &settings(&tug, Some(inputs(&tug, better))),
        &plan,
        None,
        &eph,
    )
    .unwrap();
    println!(
        "40-layer blanket: {:.3} kg ({:.3}%)",
        insulated.thermal.lost_kg,
        100.0 * insulated.thermal.lost_kg / load
    );
    assert!(
        insulated.thermal.lost_kg < base_loss,
        "more insulation must reduce boil-off"
    );

    // Active cooling: less boil-off, and it is paid for in bus joules.
    let mut cooled = baseline;
    cooled.cryocooler_lift_w = 400.0;
    cooled.radiator_area_m2 = 12.0;
    cooled.bus.array_w_at_1au = 60_000.0;
    let active = compute(
        &settings(&tug, Some(inputs(&tug, cooled))),
        &plan,
        None,
        &eph,
    )
    .unwrap();
    println!(
        "400 W cooler: {:.3} kg lost, {:.4} MJ drawn from the bus",
        active.thermal.lost_kg,
        active.thermal.cooler_energy_j / 1e6
    );
    assert!(
        active.thermal.lost_kg < base_loss,
        "active cooling must reduce boil-off"
    );
    assert!(
        active.thermal.cooler_energy_j > 0.0,
        "cooling must cost bus energy"
    );
    // At 20 K the electrical cost is far larger than the heat lifted.
    let seconds = tug.duration_us as f64 / 1e6;
    let mean_bus_w = active.thermal.cooler_energy_j / seconds;
    assert!(
        mean_bus_w > 400.0,
        "a 400 W lift at cryogenic temperature must cost more than 400 W of bus"
    );
}

/// Storables have no tank model, and a legacy record with no thermal spec
/// replays with exactly zero loss — so older certificates stay valid.
#[test]
fn storables_and_legacy_records_have_no_boil_off() {
    let eph = Ephemeris::bundled().unwrap();
    let rescue = Contract::get(101).unwrap();
    let plan = coast(&rescue);
    let with_design = compute(
        &settings(&rescue, Some(inputs(&rescue, ThermalDesign::baseline()))),
        &plan,
        None,
        &eph,
    )
    .unwrap();
    assert!(with_design.thermal.inert);
    assert_eq!(with_design.thermal.lost_kg, 0.0);
    assert!(with_design.thermal.vents.is_empty());

    let legacy = compute(&settings(&rescue, None), &plan, None, &eph).unwrap();
    assert_eq!(legacy.thermal.lost_kg, 0.0);
    assert!(legacy.thermal.inert);
    // The trajectory itself is untouched by the tank model.
    assert_eq!(legacy.report[0], with_design.report[0]);
    assert_eq!(legacy.report[1], with_design.report[1]);
}

/// Replay determinism: the same specs flown twice give a bit-identical vent
/// history. This is what lets the FDR sign the events rather than a summary.
#[test]
fn vent_history_is_bit_reproducible() {
    let eph = Ephemeris::bundled().unwrap();
    let tug = Contract::get(103).unwrap();
    let plan = coast(&tug);
    let s = settings(&tug, Some(inputs(&tug, ThermalDesign::baseline())));
    let a = compute(&s, &plan, None, &eph).unwrap();
    let b = compute(&s, &plan, None, &eph).unwrap();
    assert_eq!(a.thermal.vents, b.thermal.vents);
    assert_eq!(a.thermal.lost_kg.to_bits(), b.thermal.lost_kg.to_bits());
    assert_eq!(
        a.thermal.cooler_energy_j.to_bits(),
        b.thermal.cooler_energy_j.to_bits()
    );
    // Every event is timestamped and ordered.
    assert!(a
        .thermal
        .vents
        .windows(2)
        .all(|w| w[0].time_us <= w[1].time_us));
    println!("{} reproducible vent events", a.thermal.vents.len());
}

/// A guided burn still integrates the tanks on the flight clock, not a
/// separate one: adding a maneuver changes the eclipse geometry and therefore
/// the loss, without any wall-time involvement.
#[test]
fn maneuvers_change_the_thermal_environment() {
    let eph = Ephemeris::bundled().unwrap();
    let tug = Contract::get(103).unwrap();
    let s = settings(&tug, Some(inputs(&tug, ThermalDesign::baseline())));
    let coasted = compute(&s, &coast(&tug), None, &eph).unwrap();
    let nodes = [ManeuverNode {
        epoch_us: 1_600_000_000,
        tnw_m_s: [3_240.0, 0.0, 0.0],
    }];
    let plan = Plan::compile(
        tug.initial,
        odd_contracts::EARTH_MU,
        tug.duration_us,
        tug.dry_mass_kg,
        tug.engine,
        &nodes,
    )
    .unwrap();
    let kicked = compute(&s, &plan, None, &eph).unwrap();
    println!(
        "coast {:.3} kg vs kicked {:.3} kg",
        coasted.thermal.lost_kg, kicked.thermal.lost_kg
    );
    assert!(!kicked.thermal.inert);
    assert!(kicked.thermal.lost_kg > 0.0);
    // The kick raises apogee, changing the Earth-IR view factor and the
    // eclipse duty cycle, so the integrated loss must differ.
    assert_ne!(
        coasted.thermal.lost_kg.to_bits(),
        kicked.thermal.lost_kg.to_bits(),
        "a different trajectory must produce a different thermal history"
    );
    let _ = Keyframe {
        time: 0.0,
        position: [0.0; 3],
        velocity: [0.0; 3],
        mass: 0.0,
    };
}
