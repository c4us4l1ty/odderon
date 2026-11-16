//! A bipropellant stage's two tanks, sized from the engine's mixture ratio.
//!
//! The engine already fixes O/F and bulk density; splitting the load into an
//! oxidizer tank and a fuel tank is therefore derived, not chosen twice. Each
//! tank gets its own fluid, volume and thermal state, and the pair is
//! integrated on one shared flight clock.

use crate::bus::BusSpec;
use crate::environment::ThermalEnvironment;
use crate::fluid::Fluid;
use crate::thermal::{
    ShellMaterial, TankHistory, TankSpec, TankState, ThermalClock, ThermalContext, VentEvent,
};
use odd_astro::integrator::IntegrationError;
use odd_propulsion::{EngineSpec, Propellant};

/// Thermal design of both tanks on one stage.
#[derive(Clone, Copy, Debug)]
pub struct StageThermal {
    pub oxidizer: TankSpec,
    pub fuel: TankSpec,
    pub bus: BusSpec,
    /// Warm end of the support structure, K.
    pub structure_temperature_k: f64,
    /// Radiative sink the blanket sees, K.
    pub sink_temperature_k: f64,
}

/// Combined result for a stage.
#[derive(Clone, Debug)]
pub struct StageThermalHistory {
    pub oxidizer: TankHistory,
    pub fuel: TankHistory,
    /// All vent openings, chronological.
    pub vents: Vec<VentEvent>,
    /// Total usable propellant lost, kg.
    pub lost_kg: f64,
    /// Total cryocooler electrical energy, J.
    pub cooler_energy_j: f64,
    /// The bus split the integrator applied at every accepted step. The
    /// priority rule is checkable from this rather than asserted.
    pub bus_trend: Vec<crate::coupled::BusAllocation>,
}

impl StageThermalHistory {
    pub fn remaining_kg(&self) -> f64 {
        self.oxidizer.state.liquid_kg + self.fuel.state.liquid_kg
    }
}

/// Build the two tank specs for an engine and a total propellant load.
///
/// `load_kg` is split by the mixture ratio; each tank's volume follows from its
/// own saturated-liquid density with a 5% ullage allowance. Area is the sphere
/// of that volume, which is the minimum-area shape and therefore the optimistic
/// bound a designer starts from.
pub fn tanks_for(
    engine: &EngineSpec,
    load_kg: f64,
    template: &TankSpec,
) -> Result<(TankSpec, TankSpec), IntegrationError> {
    if !load_kg.is_finite() || load_kg <= 0.0 {
        return Err(IntegrationError::InvalidInput);
    }
    let (oxidizer_fluid, fuel_fluid) = Fluid::pair(engine.propellant);
    let of = engine.of_ratio;
    let fuel_kg = load_kg / (1.0 + of);
    let oxidizer_kg = load_kg - fuel_kg;
    let build = |fluid: Fluid, mass: f64| -> TankSpec {
        let props = fluid.properties();
        let volume = mass / props.liquid_density_kg_m3 * 1.05;
        let radius = libm::cbrt(3.0 * volume / (4.0 * std::f64::consts::PI));
        let area = 4.0 * std::f64::consts::PI * radius * radius;
        TankSpec {
            fluid,
            volume_m3: volume,
            area_m2: area,
            ..*template
        }
    };
    Ok((
        build(oxidizer_fluid, oxidizer_kg),
        build(fuel_fluid, fuel_kg),
    ))
}

impl StageThermal {
    /// A conservative default blanket for a stage flying the given engine.
    pub fn new(
        engine: &EngineSpec,
        load_kg: f64,
        mli_layers: u32,
    ) -> Result<Self, IntegrationError> {
        let template = TankSpec {
            fluid: Fluid::Lh2,
            volume_m3: 1.0,
            area_m2: 1.0,
            mli_layers,
            epsilon_eff: crate::thermal::ideal_epsilon_for_layers(mli_layers) * 4.0,
            shell: ShellMaterial::Aluminium2219,
            absorptivity: 0.2,
            outer_epsilon: 0.8,
            strut_conductance_w_k: 0.5,
            relief_pressure_pa: 200_000.0,
            radiator_area_m2: 0.0,
            radiator_epsilon: 0.8,
            cryocooler_lift_w: 0.0,
            cryocooler_carnot_fraction: 0.25,
        };
        let (oxidizer, fuel) = tanks_for(engine, load_kg, &template)?;
        Ok(Self {
            oxidizer,
            fuel,
            bus: BusSpec {
                array_w_at_1au: 0.0,
                avionics_w: 0.0,
                electric_propulsion_w: 0.0,
            },
            structure_temperature_k: 293.0,
            sink_temperature_k: crate::environment::SPACE_TEMPERATURE_K,
        })
    }

    /// Initial liquid masses for a load, split by mixture ratio.
    pub fn initial_states(&self, engine: &EngineSpec, load_kg: f64) -> (TankState, TankState) {
        let fuel_kg = load_kg / (1.0 + engine.of_ratio);
        let oxidizer_kg = load_kg - fuel_kg;
        let state = |spec: &TankSpec, mass: f64| TankState {
            // Start saturated at the relief setting: a fuelled tank on the pad
            // is already at its vent point, which is the worst honest case.
            temperature_k: spec.saturation_temperature_k(spec.relief_pressure_pa),
            liquid_kg: mass,
            vented_kg: 0.0,
        };
        (
            state(&self.oxidizer, oxidizer_kg),
            state(&self.fuel, fuel_kg),
        )
    }

    /// Integrate both tanks across the flight. The oxidizer is tank 0 and the
    /// fuel is tank 1, matching the signed vent-event ids.
    pub fn integrate<F>(
        &self,
        engine: &EngineSpec,
        load_kg: f64,
        clock: ThermalClock,
        mut environment: F,
    ) -> Result<StageThermalHistory, IntegrationError>
    where
        F: FnMut(f64) -> Result<ThermalEnvironment, IntegrationError>,
    {
        let (ox_initial, fuel_initial) = self.initial_states(engine, load_kg);
        // Both tanks advance in one state vector so the fuel's allowance is
        // the exact instantaneous remainder of the bus, not the oxidizer's
        // end-of-flight peak subtracted from a constant.
        let context = ThermalContext {
            structure_temperature_k: self.structure_temperature_k,
            sink_temperature_k: self.sink_temperature_k,
            // Superseded inside the coupled integrator by the trajectory's
            // own sunlight; kept in the struct for the single-tank API.
            bus_w: self.bus.discretionary_capacity(),
        };
        let (oxidizer, fuel, bus_trend) = crate::coupled::integrate_pair(
            &crate::coupled::TankPair {
                oxidizer: self.oxidizer,
                fuel: self.fuel,
                oxidizer_initial: ox_initial,
                fuel_initial,
            },
            &self.bus,
            clock,
            context,
            &mut environment,
        )?;
        let mut vents = oxidizer.vents.clone();
        vents.extend_from_slice(&fuel.vents);
        vents.sort_by(|a, b| a.time_us.cmp(&b.time_us).then(a.tank.cmp(&b.tank)));
        Ok(StageThermalHistory {
            lost_kg: oxidizer.state.vented_kg + fuel.state.vented_kg,
            cooler_energy_j: oxidizer.cryocooler_energy_j + fuel.cryocooler_energy_j,
            oxidizer,
            fuel,
            vents,
            bus_trend,
        })
    }
}

impl BusSpec {
    /// Upper bound the bus could ever give the cooler, at full sun. Used to
    /// size the per-tank allowance before the instantaneous split is known.
    pub fn discretionary_capacity(&self) -> f64 {
        libm::fmax(0.0, self.array_w_at_1au - self.avionics_w)
    }
}

/// True when this propellant pair needs a cryogenic thermal model at all.
pub fn needs_thermal_model(propellant: Propellant) -> bool {
    let (ox, fuel) = Fluid::pair(propellant);
    ox.properties().cryogenic || fuel.properties().cryogenic
}

#[cfg(test)]
mod tests {
    use super::*;
    use odd_astro::integrator::Options;
    use odd_core::consts::ASTRONOMICAL_UNIT_M;

    fn engine() -> EngineSpec {
        EngineSpec {
            propellant: Propellant::LoxLh2,
            of_ratio: 5.2,
            p_c_pa: 3.5e6,
            r_throat_m: 0.0288,
            r_exit_m: 0.0288 * libm::sqrt(120.0),
            length_fraction: 0.8,
        }
    }

    fn sun() -> ThermalEnvironment {
        ThermalEnvironment {
            sun: [ASTRONOMICAL_UNIT_M, 0.0, 0.0],
            earth: [0.0; 3],
            moon: [0.0; 3],
        }
    }

    fn options() -> Options {
        Options {
            relative_tolerance: 1e-9,
            absolute_tolerance: 1e-8,
            initial_step: 60.0,
            max_step: 600.0,
            max_steps: 200_000,
        }
    }

    #[test]
    fn the_load_splits_by_mixture_ratio_and_conserves_mass() {
        let stage = StageThermal::new(&engine(), 12_800.0, 20).unwrap();
        let (ox, fuel) = stage.initial_states(&engine(), 12_800.0);
        assert!((ox.liquid_kg + fuel.liquid_kg - 12_800.0).abs() < 1e-9);
        assert!((ox.liquid_kg / fuel.liquid_kg - 5.2).abs() < 1e-9);
        // Hydrogen is bulky. The volume ratio is fixed by the two saturated
        // liquid densities and the mixture ratio, so derive it rather than
        // asserting an arbitrary factor.
        let expected = (Fluid::Lox.properties().liquid_density_kg_m3
            / Fluid::Lh2.properties().liquid_density_kg_m3)
            / 5.2;
        let actual = stage.fuel.volume_m3 / stage.oxidizer.volume_m3;
        assert!((actual - expected).abs() < 1e-12, "{actual} vs {expected}");
        assert!(actual > 3.0, "hydrogen dominates the volume: {actual}");
    }

    #[test]
    fn a_five_day_coast_loses_propellant_and_reports_it() {
        let stage = StageThermal::new(&engine(), 12_800.0, 20).unwrap();
        let history = stage
            .integrate(
                &engine(),
                12_800.0,
                ThermalClock {
                    start_s: 0.0,
                    end_s: 5.0 * 86_400.0,
                    options: options(),
                },
                |_| Ok(sun()),
            )
            .unwrap();
        assert!(history.lost_kg > 0.0, "a real coast must boil off");
        let fraction = history.lost_kg / 12_800.0;
        // A 20-layer blanket at 1 AU should cost a few percent, not half.
        assert!(fraction < 0.15, "lost {:.3} of the load", fraction);
        assert!(history.remaining_kg() < 12_800.0);
        assert!(!history.vents.is_empty());
    }

    #[test]
    fn invalid_bus_or_thermal_context_cannot_be_integrated() {
        let clock = ThermalClock {
            start_s: 0.0,
            end_s: 1.0,
            options: options(),
        };
        let base = StageThermal::new(&engine(), 12_800.0, 20).unwrap();
        for field in [
            "ep negative",
            "ep nan",
            "array",
            "avionics",
            "structure",
            "sink",
        ] {
            let mut stage = base;
            match field {
                "ep negative" => stage.bus.electric_propulsion_w = -1.0,
                "ep nan" => stage.bus.electric_propulsion_w = f64::NAN,
                "array" => stage.bus.array_w_at_1au = f64::INFINITY,
                "avionics" => stage.bus.avionics_w = -1.0,
                "structure" => stage.structure_temperature_k = f64::NAN,
                "sink" => stage.sink_temperature_k = 0.0,
                _ => unreachable!(),
            }
            assert!(
                matches!(
                    stage.integrate(&engine(), 12_800.0, clock, |_| Ok(sun())),
                    Err(IntegrationError::InvalidInput)
                ),
                "invalid {field} must fail closed"
            );
        }
    }

    #[test]
    fn storables_need_no_thermal_model() {
        assert!(needs_thermal_model(Propellant::LoxLh2));
        assert!(needs_thermal_model(Propellant::LoxLch4));
        // LOX/RP-1 still has a cryogenic oxidizer.
        assert!(needs_thermal_model(Propellant::LoxRp1));
        assert!(!needs_thermal_model(Propellant::NtoMmh));
    }
}
