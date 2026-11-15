//! Oracle gates for WP 1.5.
//!
//! `oracle_rs25_and_merlin_benchmarks` and
//! `oracle_summerfield_separation_and_fuel_rich_optimum`. The published
//! numbers are the acceptance criteria from the work-package card; the
//! tolerances are the card's, not tuned to the implementation.
use odd_propulsion::{nozzle, solve_engine, EngineSpec, Propellant};

fn rs25() -> EngineSpec {
    EngineSpec {
        propellant: Propellant::LoxLh2,
        of_ratio: 6.0,
        p_c_pa: 20.64e6,
        r_throat_m: 0.1307,
        r_exit_m: 0.1307 * libm::sqrt(69.0),
        length_fraction: 0.8,
    }
}

fn merlin() -> EngineSpec {
    EngineSpec {
        propellant: Propellant::LoxRp1,
        of_ratio: 2.36,
        p_c_pa: 9.7e6,
        r_throat_m: 0.1325,
        r_exit_m: 0.1325 * 4.0,
        length_fraction: 0.8,
    }
}

#[test]
fn oracle_rs25_and_merlin_benchmarks() {
    let sea_level = 101_325.0;
    let vacuum = solve_engine(&rs25(), 0.0).unwrap();
    let ground = solve_engine(&rs25(), sea_level).unwrap();
    println!(
        "RS-25: Isp_vac {:.3} s, Isp_sl {:.3} s, F_vac {:.1} kN, mdot {:.1} kg/s, c* {:.1} m/s",
        vacuum.specific_impulse_s,
        ground.specific_impulse_s,
        vacuum.thrust_n / 1000.0,
        vacuum.mass_flow_kg_s,
        vacuum.c_star_m_s
    );
    assert!(
        (vacuum.specific_impulse_s - 452.0).abs() <= 8.0,
        "RS-25 vacuum Isp {}",
        vacuum.specific_impulse_s
    );
    assert!(
        (ground.specific_impulse_s - 366.0).abs() <= 10.0,
        "RS-25 sea-level Isp {}",
        ground.specific_impulse_s
    );
    assert!(!vacuum.thermal_limit_exceeded);
    assert!(vacuum.separation.is_none());
    // The RS-25 runs attached at sea level: its exit pressure is above the
    // Summerfield threshold even on the pad.
    assert!(ground.separation.is_none());

    let vacuum = solve_engine(&merlin(), 0.0).unwrap();
    let ground = solve_engine(&merlin(), sea_level).unwrap();
    println!(
        "Merlin 1D: Isp_vac {:.3} s, Isp_sl {:.3} s, F_vac {:.1} kN, mdot {:.1} kg/s",
        vacuum.specific_impulse_s,
        ground.specific_impulse_s,
        vacuum.thrust_n / 1000.0,
        vacuum.mass_flow_kg_s
    );
    assert!(
        (vacuum.specific_impulse_s - 311.0).abs() <= 8.0,
        "Merlin vacuum Isp {}",
        vacuum.specific_impulse_s
    );
    assert!(
        (ground.specific_impulse_s - 282.0).abs() <= 8.0,
        "Merlin sea-level Isp {}",
        ground.specific_impulse_s
    );
    assert!(vacuum.thrust_n > ground.thrust_n);
}

#[test]
fn oracle_summerfield_separation_and_fuel_rich_optimum() {
    // A vacuum bell that must not be lit at sea level.
    let vacuum_bell = EngineSpec {
        propellant: Propellant::LoxRp1,
        of_ratio: 2.36,
        p_c_pa: 5.0e6,
        r_throat_m: 0.05,
        r_exit_m: 0.05 * libm::sqrt(80.0),
        length_fraction: 0.8,
    };
    let ground = solve_engine(&vacuum_bell, 101_325.0).unwrap();
    let separated = ground
        .separation
        .expect("epsilon 80 at 5 MPa must separate");
    println!(
        "separation: x/L {:.4}, eps_sep {:.3}, side load {:.3} kN, Isp_sl {:.1} s",
        separated.x_norm,
        separated.area_ratio_sep,
        separated.side_load_kn,
        ground.specific_impulse_s
    );
    assert!(
        (0.1..0.9).contains(&separated.x_norm),
        "separation line {}",
        separated.x_norm
    );
    assert!(separated.area_ratio_sep > 1.0 && separated.area_ratio_sep < 80.0);
    assert!(separated.side_load_kn > 0.0);
    // Separation costs thrust relative to the same bell running attached.
    let space = solve_engine(&vacuum_bell, 0.0).unwrap();
    assert!(space.separation.is_none());
    assert!(space.specific_impulse_s > ground.specific_impulse_s);

    // Climbing out of the atmosphere re-attaches the flow.
    let mut last_separated = None;
    for step in 0..=100 {
        let altitude = step as f64 * 1000.0;
        let ambient = nozzle::ambient_pressure(altitude);
        let state = solve_engine(&vacuum_bell, ambient).unwrap();
        if state.separation.is_some() {
            last_separated = Some(altitude);
        }
        if altitude >= 35_000.0 {
            assert!(
                state.separation.is_none(),
                "still separated at {altitude} m ({ambient} Pa)"
            );
        }
    }
    let last = last_separated.expect("it must separate somewhere near the pad");
    println!("flow re-attaches above {last} m");
    assert!(last < 35_000.0);

    // Peak specific impulse is fuel-rich; peak temperature is not.
    let mut best_isp = (0.0, f64::NEG_INFINITY);
    let mut best_tc = (0.0, f64::NEG_INFINITY);
    for step in 0..=110 {
        let of = 2.6 + step as f64 * 0.05;
        let spec = EngineSpec {
            of_ratio: of,
            ..rs25()
        };
        let solved = solve_engine(&spec, 0.0).unwrap();
        if solved.specific_impulse_s > best_isp.1 {
            best_isp = (of, solved.specific_impulse_s);
        }
        if solved.chamber_temperature_k > best_tc.1 {
            best_tc = (of, solved.chamber_temperature_k);
        }
    }
    println!(
        "hydrolox peak Isp {:.2} s at O/F {:.2}; peak Tc {:.0} K at O/F {:.2}",
        best_isp.1, best_isp.0, best_tc.1, best_tc.0
    );
    assert!(
        (4.5..=6.2).contains(&best_isp.0),
        "peak Isp mixture ratio {}",
        best_isp.0
    );
    assert!(
        (7.0..=9.0).contains(&best_tc.0),
        "peak temperature mixture ratio {}",
        best_tc.0
    );
    assert!(best_tc.0 - best_isp.0 > 1.5);
    let stoichiometric = solve_engine(
        &EngineSpec {
            of_ratio: 7.94,
            ..rs25()
        },
        0.0,
    )
    .unwrap();
    assert!(stoichiometric.specific_impulse_s < best_isp.1 - 10.0);
}

#[test]
fn every_propellant_solves_and_prefers_fuel_rich_operation() {
    for propellant in Propellant::all() {
        let (lo, hi) = propellant.of_band();
        let reference = match propellant {
            Propellant::LoxLh2 => rs25(),
            Propellant::LoxRp1 => merlin(),
            Propellant::LoxLch4 => EngineSpec {
                propellant,
                of_ratio: 3.5,
                p_c_pa: 20.0e6,
                r_throat_m: 0.13,
                r_exit_m: 0.13 * libm::sqrt(34.0),
                length_fraction: 0.8,
            },
            Propellant::NtoMmh => EngineSpec {
                propellant,
                of_ratio: 2.0,
                p_c_pa: 1.5e6,
                r_throat_m: 0.03,
                r_exit_m: 0.03 * libm::sqrt(60.0),
                length_fraction: 0.8,
            },
        };
        let solved = solve_engine(&reference, 0.0).unwrap();
        println!(
            "{}: Isp {:.1} s, Tc {:.0} K, M {:.2} g/mol, rho {:.0} kg/m3, mass {:.0} kg, wall {:.0} K",
            propellant.label(),
            solved.specific_impulse_s,
            solved.chamber_temperature_k,
            solved.molar_mass_kg_mol * 1000.0,
            solved.bulk_density_kg_m3,
            solved.dry_mass_kg,
            solved.throat_wall_temperature_k
        );
        assert!(solved.flight_ready(), "{}", propellant.label());
        assert!(solved.specific_impulse_s > 250.0 && solved.specific_impulse_s < 500.0);
        // The whole calibrated band must solve, not just the anchor.
        for step in 0..=10 {
            let of = lo + (hi - lo) * step as f64 / 10.0;
            assert!(solve_engine(
                &EngineSpec {
                    of_ratio: of,
                    ..reference
                },
                0.0
            )
            .is_ok());
        }
    }
}

#[test]
fn impossible_designs_are_named_not_guessed() {
    use odd_propulsion::PropulsionError;
    let base = rs25();
    assert_eq!(
        solve_engine(
            &EngineSpec {
                r_exit_m: base.r_throat_m * 0.5,
                ..base
            },
            0.0
        )
        .err(),
        Some(PropulsionError::GeometryOutOfRange)
    );
    assert_eq!(
        solve_engine(
            &EngineSpec {
                of_ratio: 30.0,
                ..base
            },
            0.0
        )
        .err(),
        Some(PropulsionError::MixtureRatioOutOfRange)
    );
    assert_eq!(
        solve_engine(
            &EngineSpec {
                p_c_pa: 0.0,
                ..base
            },
            0.0
        )
        .err(),
        Some(PropulsionError::ChamberPressureOutOfRange)
    );
    assert_eq!(
        solve_engine(&base, -1.0).err(),
        Some(PropulsionError::NumericalRange)
    );
}
