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
fn frozen_tug_geometry_discloses_the_ullage_capacity_gap() {
    // This documents a v3 modeling gap, not a physical-fit assertion. The
    // vehicle fills the contract's usable volume, then each tank independently
    // adds a 5% ullage allowance. Reducing the load or changing the signed
    // geometry here would change existing v3 trajectories and certificates.
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
    assert!(total > contract.tank_volume_m3);
    assert!((total - 40.475496).abs() < 1e-5);
}
