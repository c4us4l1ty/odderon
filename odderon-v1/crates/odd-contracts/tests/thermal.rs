//! A thermally limited engine is a real solution with a named cause. The
//! contract layer must refuse it by name so the lab can tell the designer
//! what to change, rather than reporting a generic convergence failure.
use odd_contracts::{Contract, ThermalDesign};
use odd_propulsion::{solve_engine, EngineSpec, Propellant, PropulsionError};

#[test]
fn a_hot_throat_is_refused_by_name_not_as_a_numerical_failure() {
    let contract = Contract::get(101).unwrap();
    // A tiny throat at high chamber pressure drives the Bartz flux up hard.
    let hot = EngineSpec {
        propellant: Propellant::LoxRp1,
        of_ratio: 2.7,
        p_c_pa: 30.0e6,
        r_throat_m: 0.006,
        r_exit_m: 0.006 * 4.0,
        length_fraction: 0.8,
    };
    let solved = solve_engine(&hot, 0.0).expect("a hot design still solves");
    assert!(
        solved.thermal_limit_exceeded,
        "wall {:.0} K",
        solved.throat_wall_temperature_k
    );
    assert!(!solved.flight_ready());
    assert_eq!(
        contract.vehicle(hot).err(),
        Some(PropulsionError::ThermalLimitExceeded)
    );
}

#[test]
fn tug_geometry_matches_the_airframe_it_is_signed_into() {
    // This used to document a v3 gap: the vehicle filled the contract's usable
    // volume, then each tank independently added a 5% pad, so 38.6 m3 of
    // airframe somehow held 40.475496 m3 of tank. v4 derives both numbers from
    // one `TankGeometry`, so the gap is now closed by construction and this
    // test is the proof rather than the disclosure.
    let contract = Contract::get(103).unwrap();
    let vehicle = contract.vehicle(contract.engine_spec).unwrap();
    let stage = ThermalDesign::baseline()
        .stage(&contract, &contract.engine_spec, vehicle.propellant_kg)
        .unwrap();
    let total = stage.oxidizer.volume_m3 + stage.fuel.volume_m3;
    println!(
        "tug tank volumes including ullage: {total:.6} m³; contract volume: {:.6} m³",
        contract.tank_volume_m3
    );
    assert!((total - contract.tank_volume_m3).abs() < 1e-9);
    assert!((total - 39.740301).abs() < 1e-5);
    // The usable volume really is the old typed 38.6 m3, which is why the
    // catalog number looked right while describing the wrong quantity.
    let usable = total * (1.0 - odd_systems::DEFAULT_ULLAGE_FRACTION);
    assert!((usable - 38.548092).abs() < 1e-5, "{usable}");
}
