//! `oracle_v4_fitted_mass_trade_study`.
//!
//! Coupling boil-off into the mass is only worth having if it makes the design
//! trade real. The claim is asymmetric, and that asymmetry is the whole point:
//!
//! * On **Contract 101**, a three-hour storable LEO rescue, there is nothing
//!   cryogenic to insulate. Fitting 80 MLI layers and a cryocooler to it buys
//!   nothing and, if the hardware is signed anyway, costs dry mass — the
//!   verifier refuses a stray thermal block on a storable stage outright.
//! * On **Contract 103**, a multi-day hydrolox tug, the trade runs the other
//!   way. A bare blanket loses propellant for days before the final perigee
//!   kick; the recommended card costs a few hundred kilograms of hardware and
//!   saves more propellant than it costs, because every vented kilogram is a
//!   kilogram that cannot be burned.
//!
//! Both sides are measured with the shipped tank model and the shipped mass
//! accounting. Nothing here is a separate estimate.

use odd_contracts::{Contract, DelegationMask, ThermalDesign};
use odd_ephem::Ephemeris;
use odd_verify::FlightSettings;

const EPOCH: i64 = odd_contracts::START_EPOCH;

/// A deliberately excessive cryogenic fit-out: maximum blanket, maximum
/// cooler, maximum radiator, maximum array to run it.
fn maximal_design() -> ThermalDesign {
    let mut design = ThermalDesign::baseline();
    design.mli_layers = 80;
    design.epsilon_eff = 4.0 * odd_systems::ideal_epsilon_for_layers(80);
    design.radiator_area_m2 = 80.0;
    design.cryocooler_lift_w = 4_000.0;
    design.strut_conductance_w_k = 0.1;
    design.bus.array_w_at_1au = 24_000.0;
    design
}

/// A tank with almost no insulation and no active cooling: the "bare" case.
fn bare_design() -> ThermalDesign {
    let mut design = ThermalDesign::baseline();
    design.mli_layers = 2;
    design.epsilon_eff = 8.0 * odd_systems::ideal_epsilon_for_layers(2);
    design.strut_conductance_w_k = 2.0;
    design.cryocooler_lift_w = 0.0;
    design.radiator_area_m2 = 0.0;
    design
}

fn rescue_settings(design: &ThermalDesign) -> FlightSettings {
    let contract = Contract::get(101).unwrap();
    let vehicle = contract
        .vehicle_delegated(contract.engine_spec, DelegationMask::MANUAL, design)
        .unwrap();
    FlightSettings {
        contract_id: contract.id,
        epoch: EPOCH,
        initial: contract.initial,
        duration_us: contract.duration_us,
        dry_mass: vehicle.dry_mass_kg,
        engine: vehicle.engine,
        node_metric: false,
        delegation: DelegationMask::MANUAL,
        thermal: Some(odd_verify::ThermalInputs {
            engine_spec: vehicle.effective.engine,
            design: vehicle.effective.thermal,
            load_kg: vehicle.propellant_kg,
        }),
    }
}

/// One evaluation of a cryogenic stage along the tug's reference coast.
struct Trade {
    /// Dry mass the hardware actually costs.
    dry_mass_kg: f64,
    /// Propellant still aboard at the scoring instant.
    remaining_kg: f64,
    /// Mass the flight actually has to accelerate, kg: dry plus remaining.
    usable_kg: f64,
    /// Propellant lost to the vent, kg.
    vented_kg: f64,
}

fn tug_trade(design: &ThermalDesign) -> Trade {
    let contract = Contract::get(103).unwrap();
    let eph = Ephemeris::bundled().unwrap();
    let vehicle = contract
        .vehicle_delegated(contract.engine_spec, DelegationMask::MANUAL, design)
        .unwrap();
    let settings = FlightSettings {
        contract_id: contract.id,
        epoch: EPOCH,
        initial: contract.initial,
        duration_us: contract.duration_us,
        dry_mass: vehicle.dry_mass_kg,
        engine: vehicle.engine,
        node_metric: false,
        delegation: DelegationMask::MANUAL,
        thermal: Some(odd_verify::ThermalInputs {
            engine_spec: vehicle.effective.engine,
            design: vehicle.effective.thermal,
            load_kg: vehicle.propellant_kg,
        }),
    };
    let plan = odd_sim::planner::Plan::compile(
        contract.initial,
        odd_contracts::EARTH_MU,
        contract.duration_us,
        vehicle.dry_mass_kg,
        vehicle.engine,
        &[],
    )
    .unwrap();
    let outcome = odd_verify::compute(&settings, &plan, None, &eph).expect("the coast evaluates");
    let vented = outcome.thermal.lost_kg;
    let remaining = vehicle.propellant_kg - vented;
    Trade {
        dry_mass_kg: vehicle.dry_mass_kg,
        remaining_kg: remaining,
        usable_kg: vehicle.dry_mass_kg + remaining,
        vented_kg: vented,
    }
}

#[test]
fn oracle_v4_fitted_mass_trade_study() {
    // --- 101: there is nothing to insulate, so hardware is pure cost. ---
    let contract = Contract::get(101).unwrap();
    let plain = contract.vehicle(contract.engine_spec).unwrap();
    // A storable stage cannot even build a cryogenic tank pair, so the tank
    // model contributes exactly zero loss whatever blanket is signed.
    assert!(plain
        .effective
        .thermal
        .stage(&contract, &plain.effective.engine, plain.propellant_kg)
        .is_none());
    let overbuilt = contract
        .vehicle_delegated(
            contract.engine_spec,
            DelegationMask::MANUAL,
            &maximal_design(),
        )
        .unwrap();
    // A storable stage signs no thermal block, so the excess hardware is not
    // charged and cannot buy anything either: the two are the same vehicle.
    assert_eq!(
        overbuilt.dry_mass_kg.to_bits(),
        plain.dry_mass_kg.to_bits(),
        "a storable airframe must not pay for a cryogenic blanket it cannot use"
    );
    // Compute the rescue with no tank model at all: zero boil-off, by
    // construction, not by a claim.
    let settings = rescue_settings(&ThermalDesign::baseline());
    let eph = Ephemeris::bundled().unwrap();
    let plan = odd_sim::planner::Plan::compile(
        contract.initial,
        odd_contracts::EARTH_MU,
        contract.duration_us,
        settings.dry_mass,
        settings.engine,
        &[],
    )
    .unwrap();
    let outcome = odd_verify::compute(&settings, &plan, None, &eph).unwrap();
    assert!(
        outcome.thermal.inert,
        "a three-hour storable hop has no cryogenic loss to trade against"
    );
    assert_eq!(outcome.thermal.lost_kg, 0.0);

    // --- 103: insulation pays for itself over a multi-day coast. ---
    let bare = tug_trade(&bare_design());
    let card = tug_trade(&contract.thermal_card());
    let maximal = tug_trade(&maximal_design());

    println!(
        "tug trade: bare vented {:.3} kg (dry {:.1}), card vented {:.3} kg (dry {:.1}), \
         maximal vented {:.3} kg (dry {:.1})",
        bare.vented_kg, bare.dry_mass_kg, card.vented_kg, card.dry_mass_kg, maximal.vented_kg,
        maximal.dry_mass_kg
    );

    // The card is heavier than a bare tank. That is the purchase price.
    assert!(
        card.dry_mass_kg > bare.dry_mass_kg,
        "insulation and a cooler are real hardware: {:.1} vs {:.1} kg",
        card.dry_mass_kg,
        bare.dry_mass_kg
    );
    // It loses far less propellant to the vent.
    assert!(
        card.vented_kg < bare.vented_kg,
        "the recommended blanket must cut the vent: {:.3} vs {:.3} kg",
        card.vented_kg,
        bare.vented_kg
    );
    // And the purchase is worth it: the cooled stage carries more propellant
    // to the burn than the bare one, net of everything the hardware weighs.
    assert!(
        card.remaining_kg > bare.remaining_kg + (card.dry_mass_kg - bare.dry_mass_kg),
        "cooling must save more propellant than the hardware costs; \
         saved {:.3} kg, spent {:.3} kg",
        card.remaining_kg - bare.remaining_kg,
        card.dry_mass_kg - bare.dry_mass_kg
    );
    // Over-building is not free either: the maximal blanket is heavier than
    // the card and saves essentially nothing further, so `usable_kg` — the
    // mass the vehicle actually has to accelerate for a given payload — is
    // worse. That is the fitted-mass efficiency term in miniature.
    assert!(
        maximal.dry_mass_kg > card.dry_mass_kg,
        "80 layers, 4 kW of cooling and 80 m² of radiator must cost more than the card"
    );
    assert!(
        maximal.usable_kg > card.usable_kg,
        "over-building must be a net loss: {:.1} vs {:.1} kg to accelerate",
        maximal.usable_kg,
        card.usable_kg
    );
}
