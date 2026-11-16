//! `oracle_tank_volume_zero_discrepancy`.
//!
//! #103 used to be described two ways: a hand-typed 38.6 m³ airframe in the
//! contract card, and a tank model that split the same 12,800 kg load by O/F,
//! divided by each fluid's liquid density and padded by 5% *per tank*. Those
//! two numbers disagreed by 1.9 m³ and the bug hunt filed it as open.
//!
//! There is exactly one derivation now — `TankGeometry` — and this oracle
//! proves the disagreement is zero rather than merely small:
//!
//! * the catalog volume equals the derivation for every card that has tanks;
//! * a stage built from the card never clamps at full load, and its two tank
//!   volumes sum to the catalog airframe exactly;
//! * the flown load at 3% ullage fills that airframe to within one part per
//!   billion, because it is the same formula.
//!
//! A genuinely oversized load still has to clamp, so the check is not vacuous.

use odd_contracts::Contract;
use odd_systems::geometry::{TankGeometry, DEFAULT_ULLAGE_FRACTION};
use odd_systems::Fluid;

const REL: f64 = 1e-9;

fn geometry_for(contract: &Contract, load_kg: f64) -> Option<TankGeometry> {
    let spec = contract.engine_spec;
    let (ox, fuel) = Fluid::pair(spec.propellant);
    TankGeometry::derive(ox, fuel, spec.of_ratio, load_kg, DEFAULT_ULLAGE_FRACTION)
}

#[test]
fn oracle_tank_volume_zero_discrepancy() {
    for id in [101u32, 102, 103] {
        let contract = Contract::get(id).unwrap();
        let derived = contract.derived_tank_volume_m3();

        // The card ships the derivation, not a second opinion.
        assert!(
            (contract.tank_volume_m3 - derived).abs() <= REL * derived,
            "contract {id} carries {:.9} m3 but derives {:.9} m3",
            contract.tank_volume_m3,
            derived
        );

        let vehicle = contract.vehicle(contract.engine_spec).unwrap();
        let load = vehicle.propellant_kg;
        let design = vehicle.effective.thermal;
        let Some(stage) = design.stage(&contract, &vehicle.effective.engine, load) else {
            // A storable pair has no cryogenic tank model to disagree with.
            assert!(geometry_for(&contract, load).is_some());
            continue;
        };

        let expected = geometry_for(&contract, load).expect("the load has a geometry");
        // Liquid volume at bulk density plus the shared ullage is the airframe.
        assert!(
            (expected.airframe_volume_m3 - derived).abs() <= REL * derived,
            "contract {id}: geometry says {:.9} m3 of airframe, catalogue says {derived:.9}",
            expected.airframe_volume_m3
        );

        // The signed tank spec is that geometry, not a re-derivation.
        let signed = stage.oxidizer.volume_m3 + stage.fuel.volume_m3;
        assert!(
            (signed - derived).abs() <= REL * derived,
            "contract {id} signs {signed:.9} m3 of tank against a {derived:.9} m3 airframe"
        );
        for (tank, want) in [
            (&stage.oxidizer, expected.tank_volumes_m3[0]),
            (&stage.fuel, expected.tank_volumes_m3[1]),
        ] {
            assert!(
                (tank.volume_m3 - want).abs() <= REL * want,
                "contract {id} signs {:.9} m3 for {:?} against {want:.9}",
                tank.volume_m3,
                tank.fluid
            );
        }

        // The load itself at the fixed ullage fills exactly that airframe.
        let carried = derived * (1.0 - DEFAULT_ULLAGE_FRACTION) * expected.bulk_density_kg_m3;
        assert!(
            (carried - load).abs() <= REL * load,
            "contract {id}: {derived:.9} m3 carries {carried:.6} kg, not the {load:.6} kg asked for"
        );

        // A 5% overload has to clamp: the airframe is a real constraint.
        let overload = load * 1.05;
        let over_geometry = geometry_for(&contract, overload).expect("an overload still derives");
        let clamped = design
            .stage(&contract, &vehicle.effective.engine, overload)
            .expect("an overload still evaluates");
        let clamped_volume = clamped.oxidizer.volume_m3 + clamped.fuel.volume_m3;
        assert!(
            clamped_volume < over_geometry.airframe_volume_m3 * (1.0 - REL),
            "contract {id} accepted {overload:.1} kg in an airframe that holds {load:.1} kg: \
             clipped {clamped_volume:.6} of {:.6} m3",
            over_geometry.airframe_volume_m3
        );
    }

    // The specific number the bug hunt called out, stated once so the fix is
    // visible rather than implied. The tug's airframe is 39.740301 m3 and its
    // usable volume at 3% ullage is 38.548092 m3.
    //
    // That is the diagnosis: the typed 38.6 m3 was the *usable* volume
    // (38.548 + rounding), and the model was using it as the whole airframe.
    // Turned into an airframe it holds 37.442 m3 usable and 368 kg less
    // propellant than the card asks for; turned into two tanks with a 5%
    // pad each it grows to 40.475 m3 and does not fit the stage it describes.
    let tug = Contract::get(103).unwrap();
    let volume = tug.derived_tank_volume_m3();
    let usable = volume * (1.0 - DEFAULT_ULLAGE_FRACTION);
    let geometry = geometry_for(&tug, tug.propellant_budget_kg).unwrap();
    println!(
        "tug airframe {volume:.6} m3, usable {usable:.6} m3, bulk {:.3} kg/m3",
        geometry.bulk_density_kg_m3
    );
    assert!((volume - 39.740_301).abs() < 1e-5, "{volume}");
    assert!((usable - 38.548_092).abs() < 1e-5, "{usable}");
    // The typed 38.6 m3 was within a rounding error of the usable volume, which
    // is what makes the old card look plausible and be wrong at the same time.
    assert!(
        (usable - 38.6).abs() < 0.06,
        "the catalog number {usable:.6} should reproduce the typed 38.6 m3 as usable volume"
    );
    // Read as an airframe it is short; read as two padded tanks it is fat.
    let typed_as_airframe = 38.6 * (1.0 - DEFAULT_ULLAGE_FRACTION);
    let shortfall = (usable - typed_as_airframe) * geometry.bulk_density_kg_m3;
    assert!(
        (shortfall - 368.0).abs() < 5.0,
        "the typed envelope would have carried {shortfall:.1} kg less propellant"
    );
    let per_tank_padding = geometry.usable_volume_m3 * 0.05 * 2.0 / (1.0 - 0.05);
    assert!(
        per_tank_padding > 2.0,
        "the old 5%-per-tank pad inflated the stage by {per_tank_padding:.3} m3"
    );
}
