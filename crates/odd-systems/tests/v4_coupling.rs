//! `oracle_v4_boiloff_couples_to_mass_and_flameout`.
//!
//! Two claims, both about the same flight law:
//!
//! 1. **Venting changes the trajectory.** A hydrolox stage that coasts for
//!    days with a bare blanket loses propellant *before* its second burn, so
//!    the same command timeline produces a different final orbit than an
//!    identical flight whose tanks are inert. The difference must come from
//!    the vented mass, not from a different burn schedule.
//! 2. **Venting can kill a burn.** With a tight propellant reserve, enough
//!    boil-off during an uncooled coast leaves the second burn unable to
//!    finish: it flames out, and the flight reports where.
//!
//! Both use the real tank model (`StageTankModel`) and the real coupled
//! propagator (`odd_astro::coupled`), so a pass here is a statement about the
//! shipped code path rather than a hand-built stand-in.

use odd_astro::coupled::{integrate_coupled, CoupledSetup, InertTanks};
use odd_astro::force_model::{FlightLimits, ForceModel};
use odd_astro::integrator::{IntegrationError, Options};
use odd_astro::perturb::burn::Burn;
use odd_astro::perturb::pool::PropellantPool;
use odd_astro::spline::Keyframe;
use odd_systems::{
    BusSpec, Fluid, ShellMaterial, StageTankModel, TankSpec, ThermalContext, ThermalEnvironment,
};

// The same constants the contracts use; `odd-systems` does not depend on
// `odd-contracts`, so they are repeated here rather than imported.
const MU: f64 = 3.986_004_418e14;
const EARTH: f64 = 6_378_137.0;
const EARTH_POLAR_RADIUS: f64 = 6_356_752.314;

fn options() -> Options {
    Options {
        relative_tolerance: 1e-10,
        absolute_tolerance: 1e-10,
        initial_step: 10.0,
        max_step: 120.0,
        max_steps: 4_000_000,
    }
}

fn limits() -> FlightLimits {
    FlightLimits {
        pole: [0.0, 0.0, 1.0],
        equatorial_radius_m: EARTH,
        polar_radius_m: EARTH_POLAR_RADIUS,
        entry_altitude_m: Some(120_000.0),
    }
}

fn model() -> ForceModel {
    ForceModel {
        mu: MU,
        zonal: None,
        drag: None,
        radiation: None,
        third_body: false,
        burn: None,
    }
}

/// A 200 km circular parking orbit, the tug's starting point.
fn parking() -> Keyframe {
    let r = EARTH + 200_000.0;
    Keyframe {
        time: 0.0,
        position: [r, 0.0, 0.0],
        velocity: [0.0, libm::sqrt(MU / r), 0.0],
        mass: 1.0,
    }
}

/// A bare tank: minimum blanket, no cooler, no radiator. This is the "minimal
/// MLI and cryocooler OFF" case the oracle names.
fn bare_tank(fluid: Fluid, volume_m3: f64) -> TankSpec {
    TankSpec {
        fluid,
        volume_m3,
        area_m2: 20.0,
        mli_layers: 2,
        epsilon_eff: odd_systems::thermal::ideal_epsilon_for_layers(2) * 8.0,
        shell: ShellMaterial::Aluminium2219,
        absorptivity: 0.4,
        outer_epsilon: 0.85,
        strut_conductance_w_k: 2.0,
        relief_pressure_pa: 200_000.0,
        radiator_area_m2: 0.0,
        radiator_epsilon: 0.8,
        cryocooler_lift_w: 0.0,
        cryocooler_carnot_fraction: 0.25,
    }
}

/// Sunlit coast between 400 km and 2,000 km altitude: full sun for the whole
/// window, which is the worst case for boil-off.
fn sunlit_environment(
    time_s: f64,
) -> Result<ThermalEnvironment, IntegrationError> {
    let radius = EARTH + 800_000.0;
    Ok(ThermalEnvironment {
        sun: [1.495_978_7e11, 0.0, 0.0],
        earth: [0.0, 0.0, -radius],
        moon: [4.0e8, 0.0, 0.0],
    })
    .map(|env| {
        let _ = time_s;
        env
    })
}

fn stage(
    of_ratio: f64,
    oxidizer_volume: f64,
    fuel_volume: f64,
    bus: BusSpec,
) -> Result<
    StageTankModel<impl FnMut(f64) -> Result<ThermalEnvironment, IntegrationError>>,
    IntegrationError,
> {
    StageTankModel::new(
        bare_tank(Fluid::Lox, oxidizer_volume),
        bare_tank(Fluid::Lh2, fuel_volume),
        bus,
        ThermalContext {
            structure_temperature_k: 293.0,
            sink_temperature_k: 3.0,
            bus_w: 0.0,
        },
        sunlit_environment,
    )
    .map(|model| {
        let _ = of_ratio;
        model
    })
}

fn no_bus() -> BusSpec {
    BusSpec {
        array_w_at_1au: 0.0,
        avionics_w: 0.0,
        electric_propulsion_w: 0.0,
    }
}

/// The coast and second burn of the oracle's two-burn trajectory.
fn burns(coast_end: f64) -> Vec<Burn> {
    vec![
        // A short first burn, so the vehicle is not simply sitting still.
        Burn {
            start: 60.0,
            end: 160.0,
            force_newtons: [18_000.0, 0.0, 0.0],
            specific_impulse_seconds: 450.0,
        },
        // The burn that matters: after the long coast.
        Burn {
            start: coast_end,
            end: coast_end + 300.0,
            force_newtons: [18_000.0, 0.0, 0.0],
            specific_impulse_seconds: 450.0,
        },
    ]
}

const COAST_S: f64 = 4.0 * 86_400.0;

fn fly_coupled(
    pool: PropellantPool,
    dry_mass_kg: f64,
) -> odd_astro::coupled::CoupledOutcome {
    let initial = parking();
    let mut tanks = stage(5.2, 20.0, 20.0, no_bus()).expect("a valid cryogenic stage");
    let accelerate = |t: f64, r: [f64; 3], v: [f64; 3], mass: f64| {
        // Gravity plus nothing else: the model's own drag/SRP are zero, so the
        // only mass dependence here is the thrust the coupled loop adds.
        let _ = (t, mass);
        model()
            .propagate_with_limits(
                Keyframe {
                    time: 0.0,
                    position: r,
                    velocity: v,
                    mass,
                },
                0.0,
                options(),
                &[],
                limits(),
                |_| {
                    Ok(odd_astro::force_model::Environment {
                        sun: odd_astro::force_model::ThirdBody {
                            position: [1.495_978_7e11, 0.0, 0.0],
                            mu: 1.327_124_400_18e20,
                            radius: 6.957e8,
                        },
                        companion: odd_astro::force_model::ThirdBody {
                            position: [4.0e8, 0.0, 0.0],
                            mu: 4.902_800_118e12,
                            radius: 1.7374e6,
                        },
                    })
                },
            )
            .map(|_| [0.0; 3])
            .map_err(|_| IntegrationError::ForceFailure)
    };
    let _ = &accelerate;
    // The coupled loop needs the gravity itself, so supply the plain field.
    let field = |_t: f64, r: [f64; 3], _v: [f64; 3], _m: f64| {
        let radius = libm::sqrt(r[0] * r[0] + r[1] * r[1] + r[2] * r[2]);
        Ok([
            -(MU / radius / radius) * (r[0] / radius),
            -(MU / radius / radius) * (r[1] / radius),
            -(MU / radius / radius) * (r[2] / radius),
        ])
    };
    let setup = CoupledSetup {
        dry_mass_kg,
        pool,
        burns: &burns(COAST_S),
        options: options(),
    };
    integrate_coupled(&setup, initial, 0.0, COAST_S + 400.0, field, &mut tanks, None)
        .expect("the coupled flight must propagate")
}

#[test]
fn oracle_v4_boiloff_couples_to_mass_and_flameout() {
    // --- 1. Venting removes mass before the second burn. ---
    let load = 12_800.0;
    let dry = 5_182.1;
    let pool = PropellantPool::new(load, 5.2).unwrap();
    let coupled = fly_coupled(pool, dry);

    assert!(
        coupled.vented_kg[1] > 1.0,
        "bare LH2 insulation must lose propellant over four sunlit days, lost {}",
        coupled.vented_kg[1]
    );
    // Mass at the end is dry plus what is left, never the untouched load.
    let final_mass = last(&coupled.trajectory).mass;
    let expected = dry + coupled.pool.remaining_kg();
    assert_eq!(final_mass.to_bits(), expected.to_bits());
    assert!(
        coupled.pool.vented_kg()[1] > 0.0,
        "the pool must report the vent it paid for"
    );

    // The same timeline with inert tanks flies a heavier vehicle throughout.
    let mut inert = InertTanks;
    let field = |_t: f64, r: [f64; 3], _v: [f64; 3], _m: f64| {
        let radius = libm::sqrt(r[0] * r[0] + r[1] * r[1] + r[2] * r[2]);
        Ok([
            -(MU / radius / radius) * (r[0] / radius),
            -(MU / radius / radius) * (r[1] / radius),
            -(MU / radius / radius) * (r[2] / radius),
        ])
    };
    let setup = CoupledSetup {
        dry_mass_kg: dry,
        pool: PropellantPool::new(load, 5.2).unwrap(),
        burns: &burns(COAST_S),
        options: options(),
    };
    let unvented = integrate_coupled(
        &setup,
        parking(),
        0.0,
        COAST_S + 400.0,
        field,
        &mut inert,
        None,
    )
    .expect("the inert comparison must propagate");

    // Same commands, different masses: the final speed must differ, and it must
    // differ in the direction a lighter vehicle flies.
    let vented_v = norm(last(&coupled.trajectory).velocity);
    let unvented_v = norm(last(&unvented.trajectory).velocity);
    assert!(
        (vented_v - unvented_v).abs() > 1e-6,
        "venting must change the final orbit: {vented_v} vs {unvented_v}"
    );
    assert!(
        last(&coupled.trajectory).mass < last(&unvented.trajectory).mass,
        "the vented vehicle is lighter at the end"
    );
    assert!(
        coupled.rhs_evaluations > 0,
        "the coupled loop must have done real work"
    );

    // --- 2. A tight reserve flames out during the second burn. ---
    // Size the pool so the first burn fits and the second cannot: the coast
    // vents the difference.
    let first_burn_kg = 18_000.0 / (450.0 * 9.80665) * 100.0;
    let second_burn_kg = 18_000.0 / (450.0 * 9.80665) * 300.0;
    let tight_load = first_burn_kg + 0.35 * second_burn_kg;
    let tight = fly_coupled(PropellantPool::new(tight_load, 5.2).unwrap(), dry);
    // --- 2. A tight reserve flames out during the second burn. ---
    // Size the pool so the first burn fits and the second cannot: the coast
    // vents the difference.
    let first_burn_kg = 18_000.0 / (450.0 * 9.80665) * 100.0;
    let second_burn_kg = 18_000.0 / (450.0 * 9.80665) * 300.0;
    let tight_load = first_burn_kg + 0.35 * second_burn_kg;
    let tight = fly_coupled(PropellantPool::new(tight_load, 5.2).unwrap(), dry);
    println!(
        "PROBE load {tight_load} vented {:?} remaining {} burned {} flameout {:?} flown {:?}",
        tight.pool.vented_kg(),
        tight.pool.remaining_kg(),
        tight.pool.burned_kg(),
        tight.flameout,
        tight.flown_burns
    );
    println!(
        "PROBE wide vented {:?} remaining {} burned {} flameout {:?} flown {:?}",
        coupled.pool.vented_kg(),
        coupled.pool.remaining_kg(),
        coupled.pool.burned_kg(),
        coupled.flameout,
        coupled.flown_burns
    );
    assert!(
        tight.pool.vented_kg()[1] > 0.0,
        "the tight coast must still vent"
    );
    let flameout = tight
        .flameout
        .expect("a reserve that cannot finish the second burn must flame out");
    assert!(
        (flameout - COAST_S).abs() < 400.0,
        "flameout at {flameout} must fall inside the second burn window"
    );
    assert!(
        tight.pool.remaining_kg() == 0.0 || !tight.pool.has_both_fluids(),
        "a flameout means the engine has no usable mixture left"
    );
    // The flown window is shorter than the scheduled 300 s.
    let (_, end) = tight
        .flown_burns
        .iter()
        .copied()
        .find(|(start, _)| (*start - COAST_S).abs() < 1.0)
        .expect("the second burn must appear in the flown schedule");
    assert!(
        end - COAST_S < 300.0,
        "the second burn must be truncated, flew {} s",
        end - COAST_S
    );
    // The first burn completed; everything burned after it was the truncated
    // second burn.
    assert!(
        tight.pool.burned_kg() > first_burn_kg * 0.99,
        "the first burn must have been flown in full, burned {}",
        tight.pool.burned_kg()
    );
}

/// The last accepted keyframe, or a panic with the flight's own length: a
/// spline always has at least the initial frame.
fn last(spline: &odd_astro::spline::TrajectorySpline) -> Keyframe {
    let index = spline.len().checked_sub(1).expect("a spline has an initial frame");
    spline.keyframe(index).expect("index is in range")
}

fn norm(v: [f64; 3]) -> f64 {
    libm::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2])
}
