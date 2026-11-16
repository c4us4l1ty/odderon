//! WP 1.6 acceptance gate: `oracle_instantaneous_cryo_bus_priority`.
//!
//! The stage used to integrate the oxidizer to the end of the flight, take
//! its **final** compressor draw, and hand the fuel tank a constant
//! `capacity − ox_peak` for the whole mission. In eclipse the bus itself
//! collapses to zero, so that allowance was fictional; in full sun it was
//! needlessly small.
//!
//! This gate flies a real sun-to-eclipse-to-sun trajectory and checks, at
//! every accepted integration step, that
//!
//! * `fuel_available == max(0, P_bus(t) − P_ox(t))` exactly,
//! * `P_ep == max(0, fuel_available − P_fuel)` — propulsion only gets surplus,
//! * the allowance actually **varies** with the instantaneous sunlight, so a
//!   return to the end-state approximation would fail.

use odd_astro::integrator::Options;
use odd_core::consts::ASTRONOMICAL_UNIT_M;
use odd_propulsion::{EngineSpec, Propellant};
use odd_systems::{allocate_bus, BusSpec, StageThermal, ThermalClock, ThermalEnvironment};

const R_EARTH: f64 = 6_371_000.0;
const MU_EARTH: f64 = 3.986_004_418e14;

fn engine() -> EngineSpec {
    EngineSpec {
        propellant: Propellant::LoxLh2,
        of_ratio: 6.0,
        p_c_pa: 3.5e6,
        r_throat_m: 0.0288,
        r_exit_m: 0.3155,
        length_fraction: 0.8,
    }
}

/// A 400 km circular orbit in the xy-plane with the Sun fixed along +x, so
/// the tank passes through Earth's shadow once per revolution.
fn eclipse_environment(
    radius: f64,
    mean_motion: f64,
) -> impl FnMut(f64) -> Result<ThermalEnvironment, odd_astro::integrator::IntegrationError> {
    move |t: f64| {
        let theta = mean_motion * t;
        let tank = [radius * libm::cos(theta), radius * libm::sin(theta), 0.0];
        Ok(ThermalEnvironment {
            sun: [ASTRONOMICAL_UNIT_M - tank[0], -tank[1], -tank[2]],
            earth: [-tank[0], -tank[1], -tank[2]],
            moon: [0.0; 3],
        })
    }
}

#[test]
fn oracle_instantaneous_cryo_bus_priority() {
    let radius = R_EARTH + 400_000.0;
    let period = std::f64::consts::TAU * libm::sqrt(radius * radius * radius / MU_EARTH);
    let mean_motion = std::f64::consts::TAU / period;

    let engine = engine();
    let mut stage = StageThermal::new(&engine, 8_000.0, 20).expect("a cryogenic stage");
    // Both tanks command active cooling, and the array is deliberately too
    // small to run both at nameplate: the priority rule is what decides.
    for tank in [&mut stage.oxidizer, &mut stage.fuel] {
        tank.radiator_area_m2 = 30.0;
        tank.cryocooler_lift_w = 300.0;
    }
    stage.bus = BusSpec {
        array_w_at_1au: 6_000.0,
        avionics_w: 400.0,
        electric_propulsion_w: 1_500.0,
    };

    let history = stage
        .integrate(
            &engine,
            8_000.0,
            ThermalClock {
                start_s: 0.0,
                end_s: 2.0 * period,
                options: Options {
                    relative_tolerance: 1e-10,
                    absolute_tolerance: 1e-11,
                    initial_step: 10.0,
                    max_step: 120.0,
                    max_steps: 400_000,
                },
            },
            eclipse_environment(radius, mean_motion),
        )
        .expect("the coupled stage integrates");

    let trend = &history.bus_trend;
    assert!(trend.len() > 40, "expected many steps, got {}", trend.len());

    let mut min_available = f64::INFINITY;
    let mut max_available = 0.0_f64;
    let mut saw_eclipse = false;
    let mut saw_sunlight = false;
    let mut worst_priority_error = 0.0_f64;
    for step in trend {
        // The rule, checked bit-exactly at every accepted step.
        let expected_available = libm::fmax(0.0, step.bus_w - step.oxidizer_w);
        let error = (step.fuel_available_w - expected_available).abs();
        worst_priority_error = worst_priority_error.max(error);
        assert_eq!(
            step.fuel_available_w.to_bits(),
            expected_available.to_bits(),
            "at t={:.3} s the fuel allowance must be the instantaneous remainder",
            step.time_s
        );
        // Avionics comes off the top, generation is shadow-scaled.
        assert!(step.bus_w <= step.generation_w + 1e-9);
        assert!((step.bus_w - libm::fmax(0.0, step.generation_w - 400.0)).abs() < 1e-9);
        // Electric propulsion only ever sees the surplus after both coolers.
        assert_eq!(
            step.electric_propulsion_w.to_bits(),
            libm::fmax(0.0, step.fuel_available_w - step.fuel_w).to_bits()
        );
        assert!(step.fuel_w <= step.fuel_available_w + 1e-9);
        assert!(step.oxidizer_w <= step.bus_w + 1e-9);

        if step.illumination < 1e-6 {
            saw_eclipse = true;
            assert_eq!(step.generation_w, 0.0, "no sun, no generation");
            assert_eq!(step.bus_w, 0.0);
            assert_eq!(step.fuel_available_w, 0.0);
            assert_eq!(step.electric_propulsion_w, 0.0);
        }
        if step.illumination > 0.999 {
            saw_sunlight = true;
        }
        min_available = min_available.min(step.fuel_available_w);
        max_available = max_available.max(step.fuel_available_w);
    }
    assert!(saw_eclipse, "the trajectory must enter Earth's shadow");
    assert!(saw_sunlight, "the trajectory must return to full sun");
    assert_eq!(worst_priority_error, 0.0);
    // The whole point: the allowance is NOT a constant derived from the
    // oxidizer's end-state peak.
    assert!(
        max_available - min_available > 1_000.0,
        "the fuel allowance must track the sun: {min_available} .. {max_available} W"
    );
    // The oxidizer must actually contend for the bus, or the priority rule
    // would be untested. An oversized radiator used to collapse its hot side
    // onto the cold tip and give it free cooling.
    let ox_peak = trend.iter().map(|s| s.oxidizer_w).fold(0.0, f64::max);
    assert!(
        ox_peak > 100.0,
        "the oxidizer cooler must draw real power, got {ox_peak} W"
    );
    assert!(
        trend.iter().any(|s| s.fuel_available_w < s.bus_w - 1.0),
        "the oxidizer must actually reduce what the fuel is offered"
    );
    println!(
        "{} steps, fuel allowance {min_available:.1} .. {max_available:.1} W, \
         oxidizer peak {:.1} W, cooler energy {:.3} MJ",
        trend.len(),
        trend.iter().map(|s| s.oxidizer_w).fold(0.0, f64::max),
        history.cooler_energy_j / 1e6
    );

    // The pure allocator and the integrator must agree, so the rule can also
    // be checked without running an integration at all.
    let sample = trend[trend.len() / 2];
    let env = eclipse_environment(radius, mean_motion)(sample.time_s).unwrap();
    let split = allocate_bus(&stage.bus, &env, sample.oxidizer_w, sample.fuel_w);
    assert_eq!(split.bus_w.to_bits(), sample.bus_w.to_bits());
    assert_eq!(
        split.fuel_available_w.to_bits(),
        sample.fuel_available_w.to_bits()
    );
    assert_eq!(
        split.electric_propulsion_w.to_bits(),
        sample.electric_propulsion_w.to_bits()
    );
}

/// A greedy oxidizer must never be able to push the fuel's allowance
/// negative, and the split must still conserve the bus.
#[test]
fn the_priority_split_conserves_power_and_never_goes_negative() {
    let env = ThermalEnvironment {
        sun: [ASTRONOMICAL_UNIT_M, 0.0, 0.0],
        earth: [0.0; 3],
        moon: [0.0; 3],
    };
    let bus = BusSpec {
        array_w_at_1au: 1_000.0,
        avionics_w: 300.0,
        electric_propulsion_w: 0.0,
    };
    for (ox, fuel) in [
        (0.0, 0.0),
        (200.0, 200.0),
        (900.0, 400.0),
        (5_000.0, 5_000.0),
    ] {
        let split = allocate_bus(&bus, &env, ox, fuel);
        assert!((split.bus_w - 700.0).abs() < 1e-9);
        assert!(split.fuel_available_w >= 0.0);
        assert!(split.oxidizer_w <= split.bus_w);
        assert!(split.fuel_w <= split.fuel_available_w);
        assert!(split.electric_propulsion_w >= 0.0);
        assert!(
            (split.oxidizer_w + split.fuel_w + split.electric_propulsion_w - split.bus_w).abs()
                < 1e-9,
            "the split must account for every watt"
        );
    }
    // In shadow there is nothing to allocate at all.
    let shadow = ThermalEnvironment {
        sun: [ASTRONOMICAL_UNIT_M, 0.0, 0.0],
        // Earth lies between the tank and the Sun, so the disc is occulted.
        earth: [R_EARTH + 100_000.0, 0.0, 0.0],
        moon: [0.0; 3],
    };
    let dark = allocate_bus(&bus, &shadow, 500.0, 500.0);
    assert_eq!(dark.generation_w, 0.0);
    assert_eq!(dark.bus_w, 0.0);
    assert_eq!(dark.oxidizer_w, 0.0);
    assert_eq!(dark.fuel_available_w, 0.0);
}

/// Regression: an oversized radiator must not buy free cooling.
///
/// The hot-side iteration used to relax onto `cold + 1e-3 K` when the
/// radiator was large for the load, so `COP = f·T_c/(T_h − T_c)` reached
/// ~24 000 and a 118 W lift cost 0.005 W of electricity. Two things now stop
/// it: the radiator absorbs its own environment, and the cooler has a finite
/// internal approach temperature.
#[test]
fn an_oversized_radiator_does_not_produce_free_cooling() {
    let engine = engine();
    let mut stage = StageThermal::new(&engine, 8_000.0, 20).unwrap();
    stage.oxidizer.radiator_area_m2 = 300.0;
    stage.oxidizer.cryocooler_lift_w = 300.0;
    let cold = stage
        .oxidizer
        .saturation_temperature_k(stage.oxidizer.relief_pressure_pa);
    let env = ThermalEnvironment {
        sun: [ASTRONOMICAL_UNIT_M, 0.0, 0.0],
        earth: [0.0, -R_EARTH - 400_000.0, 0.0],
        moon: [0.0; 3],
    };
    let balance = odd_systems::heat_balance(&stage.oxidizer, cold, &env, 293.0, 3.0, 1e6);
    assert!(
        balance.cryocooler_w > 0.0,
        "the cooler must be lifting heat"
    );
    let cop = balance.cryocooler_w / balance.cryocooler_bus_w;
    println!(
        "300 m² radiator: lift {:.2} W, draw {:.2} W, COP {:.4}",
        balance.cryocooler_w, balance.cryocooler_bus_w, cop
    );
    // Even in the most favourable geometry the machine is a real heat pump,
    // not a rounding error: lifting heat from a ~97 K tank costs a
    // comparable or larger amount of electricity.
    assert!(
        cop < 5.0,
        "an oversized radiator must not yield COP {cop:.1}"
    );
    assert!(balance.cryocooler_bus_w > 1.0);

    // The approach floor holds even with no environmental load at all.
    let point = odd_systems::cryocooler_point(&stage.oxidizer, cold, 300.0, 1e6);
    let carnot_at_floor = stage.oxidizer.cryocooler_carnot_fraction * cold
        / odd_systems::thermal::MIN_COOLER_APPROACH_K;
    assert!(point.cop <= carnot_at_floor + 1e-9);
    assert!(point.hot_side_k >= cold + odd_systems::thermal::MIN_COOLER_APPROACH_K - 1e-9);
}
