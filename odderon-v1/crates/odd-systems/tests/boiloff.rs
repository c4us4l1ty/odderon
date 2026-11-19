//! WP 1.6 acceptance gates.
//!
//! Three of the work-package card's numbers are physically impossible as
//! written. Each gate below states the correction and why, rather than
//! quietly bending a coefficient until the card's number appeared.
//!
//! 1. **"eps_MLI = 0.03, 4 m², full 1-AU sun, 300 K env → 1.5 %/day."**
//!    Those inputs give **12.2 %/day**, not 1.5. A 300 K sink radiates ~459 W/m²
//!    onto the blanket; even at eps_eff = 0.03 that is tens of watts into a
//!    100 kg tank. The published ~1.5 %/day figure belongs to the
//!    **35 K sunshielded, conduction-dominated** regime the same sentence
//!    names. This gate tests that regime, deriving strut conductance from
//!    real strut geometry, and separately pins the sunlit case at its true
//!    value so the distinction cannot silently collapse.
//!
//! 2. **"LEO with 60-min orbital period."** A 60-minute Earth orbit has
//!    a = 5076.85 km — about 1294 km *below the surface*. The shortest
//!    possible Earth orbit is 84.3 min. This gate uses a real 400 km LEO,
//!    period 92.41 min, and compares against an independent SciPy integration.
//!
//! 3. **"COP ≈ 0.1 at 20 K."** Carnot between 20 K and a 300 K sink is
//!    20/280 = 0.0714, so COP 0.1 would exceed the Second Law by 40 %. The
//!    gate keeps the card's *observable* requirement — 2 kW of lift at 20 K
//!    must draw **at least** 20 kW — which a Carnot-fraction model satisfies
//!    with a large margin, and asserts the Carnot ceiling is never crossed.

use odd_astro::integrator::Options;
use odd_core::consts::ASTRONOMICAL_UNIT_M;
use odd_systems::{
    cryocooler_point, heat_balance, integrate_tank, BusSpec, Fluid, ShellMaterial, TankSpec,
    TankState, ThermalClock, ThermalContext, ThermalEnvironment,
};

/// Matches `tools/astro-oracle/boiloff_eclipse_scipy.py`.
const CSV: &str = include_str!("../../../data/oracles/boiloff-eclipse.csv");

const R_EARTH: f64 = 6_371_000.0;
const MU_EARTH: f64 = 3.986_004_418e14;
const H_VAP_LH2: f64 = 445_600.0;

fn options() -> Options {
    Options {
        relative_tolerance: 1e-11,
        absolute_tolerance: 1e-12,
        initial_step: 10.0,
        max_step: 20.0,
        max_steps: 400_000,
    }
}

/// The oracle tank: 100 kg LH2, 4 m², eps_eff 0.03, relief at 200 kPa.
fn oracle_tank() -> TankSpec {
    TankSpec {
        fluid: Fluid::Lh2,
        volume_m3: 100.0 / 70.85,
        area_m2: 4.0,
        // eps_eff 0.03 is worse than ideal for any realistic layer count, so
        // one layer is enough to satisfy the emittance floor.
        mli_layers: 1,
        epsilon_eff: 0.03,
        shell: ShellMaterial::Aluminium2219,
        absorptivity: 0.2,
        outer_epsilon: 0.8,
        strut_conductance_w_k: 0.028,
        relief_pressure_pa: 200_000.0,
        radiator_area_m2: 0.0,
        radiator_epsilon: 0.8,
        cryocooler_lift_w: 0.0,
        cryocooler_carnot_fraction: 0.25,
    }
}

fn deep_space_sun() -> ThermalEnvironment {
    ThermalEnvironment {
        sun: [ASTRONOMICAL_UNIT_M, 0.0, 0.0],
        earth: [0.0; 3],
        moon: [0.0; 3],
    }
}

/// Six stainless support struts, 6.4 mm diameter, 100 mm long. Conductance is
/// `k·A/L` summed over the set — a physical design, not a fitted coefficient.
fn strut_conductance() -> f64 {
    let count = 6.0;
    let radius = 3.2e-3;
    let length = 0.100;
    let k = ShellMaterial::Stainless304.conductivity_w_m_k();
    count * k * (std::f64::consts::PI * radius * radius) / length
}

/// Gate 1 — self-pressurization and steady boil-off in the published regime.
#[test]
fn oracle_lh2_self_pressurization_rate() {
    // --- (a) the sunshielded, conduction-dominated 1.5 %/day regime ---------
    let mut spec = oracle_tank();
    spec.strut_conductance_w_k = strut_conductance();
    // Sunshielded: the blanket sees a 35 K shield, not the 300 K structure,
    // and no direct sun reaches the skin.
    spec.absorptivity = 0.0;
    let shielded = ThermalEnvironment {
        sun: [ASTRONOMICAL_UNIT_M, 0.0, 0.0],
        earth: [0.0; 3],
        moon: [0.0; 3],
    };
    let vent_temperature = spec.saturation_temperature_k(spec.relief_pressure_pa);
    let balance = heat_balance(&spec, vent_temperature, &shielded, 293.0, 35.0, 0.0);
    // Struts must dominate radiation in this regime, which is what makes the
    // published scaling conduction-limited.
    assert!(
        balance.strut_w > 100.0 * balance.mli_w.abs(),
        "struts {:.4} W should dominate radiation {:.4} W",
        balance.strut_w,
        balance.mli_w
    );
    let history = integrate_tank(
        &spec,
        0,
        TankState {
            temperature_k: vent_temperature,
            liquid_kg: 100.0,
            vented_kg: 0.0,
        },
        ThermalClock {
            start_s: 0.0,
            end_s: 86_400.0,
            options: options(),
        },
        ThermalContext {
            structure_temperature_k: 293.0,
            sink_temperature_k: 35.0,
            bus_w: 0.0,
        },
        |_| Ok(shielded),
    )
    .unwrap();
    let percent_per_day = 100.0 * history.state.vented_kg / 100.0;
    println!(
        "sunshielded 35 K: G {:.5} W/K, Q {:.4} W, {:.4} %/day (published ~1.5)",
        strut_conductance(),
        balance.net_w,
        percent_per_day
    );
    assert!(
        (percent_per_day - 1.5).abs() / 1.5 < 0.15,
        "{percent_per_day} %/day is outside 15% of the published 1.5 %/day"
    );

    // --- (b) closed-tank pressurization follows Clausius–Clapeyron ---------
    // Before the valve opens, dP/dt = (dP/dT)(dT/dt) with dP/dT = P·scale/T²
    // and dT/dt = Q/(m·c). Check the integrated rise against that chain rule.
    let mut closed = spec;
    closed.relief_pressure_pa = 400_000.0;
    let start_temperature = closed.saturation_temperature_k(200_000.0);
    let span = 600.0;
    let q = heat_balance(&closed, start_temperature, &shielded, 293.0, 35.0, 0.0).net_w;
    let props = closed.properties();
    let scale = props.heat_of_vaporization_j_kg * props.molar_mass_kg_mol / 8.314_462_618_153_24;
    let dtemperature = q / (100.0 * props.liquid_specific_heat_j_kg_k);
    let predicted_rise =
        200_000.0 * scale / (start_temperature * start_temperature) * dtemperature * span;
    let pressurized = integrate_tank(
        &closed,
        0,
        TankState {
            temperature_k: start_temperature,
            liquid_kg: 100.0,
            vented_kg: 0.0,
        },
        ThermalClock {
            start_s: 0.0,
            end_s: span,
            options: options(),
        },
        ThermalContext {
            structure_temperature_k: 293.0,
            sink_temperature_k: 35.0,
            bus_w: 0.0,
        },
        |_| Ok(shielded),
    )
    .unwrap();
    assert_eq!(
        pressurized.state.vented_kg, 0.0,
        "a closed tank must lose no propellant"
    );
    assert!(pressurized.vents.is_empty());
    let actual_rise = pressurized.peak_pressure_pa - 200_000.0;
    println!(
        "closed tank: dP/dt {:.4} Pa/s, rise {:.2} Pa vs chain-rule {:.2} Pa",
        actual_rise / span,
        actual_rise,
        predicted_rise
    );
    assert!(
        (actual_rise - predicted_rise).abs() / predicted_rise < 0.01,
        "{actual_rise} vs {predicted_rise}"
    );

    // --- (c) pin the sunlit 300 K case at its TRUE value -------------------
    // The card claimed 1.5 %/day for these inputs. It is actually ~12 %/day.
    // Pinning it here means a future "fix" that quietly made this 1.5 would
    // have to delete this assertion, which is the point.
    let sunlit = oracle_tank();
    let sunlit_balance = heat_balance(&sunlit, 20.27, &deep_space_sun(), 300.0, 300.0, 0.0);
    let sunlit_percent = 100.0 * (sunlit_balance.net_w / H_VAP_LH2) * 86_400.0 / 100.0;
    println!(
        "sunlit 300 K sink: skin {:.1} K, Q {:.3} W, {:.3} %/day (NOT 1.5)",
        sunlit_balance.skin_temperature_k, sunlit_balance.net_w, sunlit_percent
    );
    // 62.96 W of blanket leakage + 7.83 W of strut conduction = 70.79 W.
    assert!(
        (13.0..14.5).contains(&sunlit_percent),
        "the sunlit 300 K case changed: {sunlit_percent} %/day"
    );
    assert!(
        sunlit_percent > 5.0 * percent_per_day,
        "sunlit and sunshielded regimes must stay clearly distinct"
    );
}

/// Gate 2 — the eclipse thermal cycle against an independent SciPy oracle.
#[test]
fn oracle_eclipse_thermal_cycle() {
    let altitude = 400_000.0;
    let radius = R_EARTH + altitude;
    let period = std::f64::consts::TAU * libm::sqrt(radius * radius * radius / MU_EARTH);
    let mean_motion = std::f64::consts::TAU / period;
    // The card asked for a 60-minute period, which is sub-surface. Assert the
    // real one so the correction is visible in the test itself.
    assert!(
        period > 84.3 * 60.0,
        "no Earth orbit can be shorter than 84.3 min"
    );
    println!(
        "400 km LEO period {:.4} s = {:.4} min",
        period,
        period / 60.0
    );

    let spec = oracle_tank();
    let vent_temperature = spec.saturation_temperature_k(spec.relief_pressure_pa);
    // Same geometry as the Python: circular orbit in the xy-plane, Sun fixed
    // along +x, Earth vector from the tank back to the centre.
    let environment = |t: f64| {
        let theta = mean_motion * t;
        let tank = [radius * libm::cos(theta), radius * libm::sin(theta), 0.0];
        Ok(ThermalEnvironment {
            sun: [ASTRONOMICAL_UNIT_M - tank[0], -tank[1], -tank[2]],
            earth: [-tank[0], -tank[1], -tank[2]],
            moon: [0.0; 3],
        })
    };

    let rows: Vec<(usize, f64)> = CSV
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let f: Vec<&str> = line.split(',').collect();
            (f[0].parse::<usize>().unwrap(), f[2].parse::<f64>().unwrap())
        })
        .collect();
    assert_eq!(rows.len(), 4, "the fixture covers four orbits");

    let mut worst = 0.0_f64;
    let mut saw_eclipse = false;
    let mut saw_sunlight = false;
    for (orbit, expected) in rows {
        let start = (orbit - 1) as f64 * period;
        let history = integrate_tank(
            &spec,
            0,
            TankState {
                temperature_k: vent_temperature,
                liquid_kg: 100.0,
                vented_kg: 0.0,
            },
            ThermalClock {
                start_s: start,
                end_s: start + period,
                options: options(),
            },
            ThermalContext {
                structure_temperature_k: 293.0,
                sink_temperature_k: 3.0,
                bus_w: 0.0,
            },
            environment,
        )
        .unwrap();
        let actual = history.state.vented_kg;
        let error = (actual - expected).abs() / expected;
        worst = worst.max(error);
        println!(
            "orbit {orbit}: Rust {actual:.9} kg vs SciPy {expected:.9} kg  ({:.4}%)",
            100.0 * error
        );
        // Confirm the orbit really crossed both regimes.
        for step in 0..64 {
            let t = start + period * f64::from(step) / 64.0;
            let env = environment(t).unwrap();
            let illum = env.illumination();
            saw_eclipse |= illum < 0.01;
            saw_sunlight |= illum > 0.99;
        }
        assert!(
            error < 0.05,
            "orbit {orbit}: {actual} vs {expected} ({:.2}%)",
            100.0 * error
        );
    }
    assert!(saw_eclipse, "the scenario must enter umbra");
    assert!(saw_sunlight, "the scenario must reach full sun");
    // The integrators are independent, so agreement should be far tighter
    // than the 5% the card allows. Require 0.5% to catch real drift.
    assert!(worst < 0.005, "worst disagreement {:.4}%", 100.0 * worst);
    println!("worst SciPy disagreement: {:.5}%", 100.0 * worst);
}

/// Gate 3 — a 2 kW / 20 K cryocooler must drain the bus, end to end.
#[test]
fn oracle_cryocooler_bus_drain() {
    let mut spec = oracle_tank();
    spec.cryocooler_lift_w = 2_000.0;
    spec.radiator_area_m2 = 320.0;
    spec.cryocooler_carnot_fraction = 0.25;
    // A deliberately badly-mounted tank: 8 W/K of strut conduction gives a
    // ~2.2 kW passive load, so a 2 kW cooler runs at full lift instead of
    // being thermostatically modulated down. This is the case the card's
    // >=20 kW figure describes.
    spec.strut_conductance_w_k = 8.0;

    let point = cryocooler_point(&spec, 20.0, 2_000.0, 1e9);
    println!(
        "2 kW at 20 K: draw {:.0} W, COP {:.5}, radiator {:.1} K",
        point.power_w, point.cop, point.hot_side_k
    );
    // The card's observable requirement.
    assert!(
        point.power_w >= 20_000.0,
        "2 kW of lift at 20 K must cost at least 20 kW, got {:.0} W",
        point.power_w
    );
    assert!(
        (point.lift_w - 2_000.0).abs() < 1.0,
        "lift must be delivered"
    );
    // The Second Law is not negotiable: the card's COP 0.1 would break it.
    let carnot = 20.0 / (point.hot_side_k - 20.0);
    assert!(
        point.cop <= carnot + 1e-12,
        "COP {:.5} exceeds Carnot {:.5}",
        point.cop,
        carnot
    );
    assert!(
        point.cop < 0.0714,
        "COP against a ~300 K sink cannot reach the card's 0.1"
    );

    // End to end: that draw must come off the same bus that feeds electric
    // propulsion, through the tank integration and the real eclipse geometry.
    let bus = BusSpec {
        array_w_at_1au: 200_000.0,
        avionics_w: 1_000.0,
        // Greedy EP: it wants more than the array makes, so it is bus-limited
        // and every cooler watt comes directly out of its allocation.
        electric_propulsion_w: 250_000.0,
    };
    let radius = R_EARTH + 400_000.0;
    let period = std::f64::consts::TAU * libm::sqrt(radius * radius * radius / MU_EARTH);
    let mean_motion = std::f64::consts::TAU / period;
    let environment = |t: f64| {
        let theta = mean_motion * t;
        let tank = [radius * libm::cos(theta), radius * libm::sin(theta), 0.0];
        Ok(ThermalEnvironment {
            sun: [ASTRONOMICAL_UNIT_M - tank[0], -tank[1], -tank[2]],
            earth: [-tank[0], -tank[1], -tank[2]],
            moon: [0.0; 3],
        })
    };
    let context = ThermalContext {
        structure_temperature_k: 293.0,
        sink_temperature_k: 3.0,
        bus_w: bus.discretionary_capacity(),
    };
    let vent_temperature = spec.saturation_temperature_k(spec.relief_pressure_pa);
    let initial = TankState {
        temperature_k: vent_temperature,
        liquid_kg: 100.0,
        vented_kg: 0.0,
    };
    let clock = ThermalClock {
        start_s: 0.0,
        end_s: period,
        options: options(),
    };
    let mut passive_spec = spec;
    passive_spec.cryocooler_lift_w = 0.0;
    let passive = integrate_tank(&passive_spec, 0, initial, clock, context, environment).unwrap();
    let cooled = integrate_tank(&spec, 0, initial, clock, context, environment).unwrap();

    assert!(
        cooled.cryocooler_energy_j > 0.0,
        "the cooler must actually run"
    );
    let mean_draw = cooled.cryocooler_energy_j / period;
    println!(
        "passive loss {:.6} kg, cooled loss {:.6} kg; mean cooler draw {:.0} W",
        passive.state.vented_kg, cooled.state.vented_kg, mean_draw
    );
    assert!(
        cooled.state.vented_kg < passive.state.vented_kg,
        "active cooling must reduce boil-off"
    );
    assert!(
        mean_draw >= 20_000.0,
        "the integrated draw must reflect the >=20 kW requirement, got {mean_draw:.0} W"
    );
    // And that draw must remove exactly that much from electric propulsion.
    let idle = bus.resolve(1.0, 0.0);
    let cooling = bus.resolve(1.0, mean_draw);
    assert!(
        (idle.electric_propulsion_w - cooling.electric_propulsion_w - mean_draw).abs() < 1e-6,
        "cooler watts must come straight out of electric propulsion"
    );
    println!(
        "electric propulsion: {:.0} W idle -> {:.0} W while cooling",
        idle.electric_propulsion_w, cooling.electric_propulsion_w
    );
    // Sanity: the blanket really is seeing a warm skin in sunlight.
    let lit = heat_balance(&spec, vent_temperature, &deep_space_sun(), 293.0, 3.0, 0.0);
    assert!(lit.skin_temperature_k > 100.0);
}
