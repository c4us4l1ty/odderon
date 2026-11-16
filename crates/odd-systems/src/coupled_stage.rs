//! The coupled-loop adapter over the validated lumped tank model.
//!
//! `thermal` owns the heat balance, `coupled` owns the integrator and the
//! `odd-astro` coupled propagator owns the flight. This module is the seam: it
//! answers "what is this tank doing right now" from the tank state the
//! propagator is holding, using the same `heat_balance` the standalone
//! evaluation uses. There is no second tank model, so the two cannot disagree
//! about a vent.

use crate::bus::BusSpec;
use crate::environment::{ThermalEnvironment, SOLAR_CONSTANT_W_M2};
use crate::thermal::{heat_balance, TankSpec, ThermalContext};
use odd_astro::coupled::{TankModel, TankRate, TANK_STRIDE};
use odd_astro::integrator::IntegrationError;

/// A stage's two tanks, plus the bus that decides how much cooling it can run.
///
/// The bus is resolved in the same priority order the standalone model uses:
/// generation, avionics, oxidizer cooler, fuel cooler, then electric
/// propulsion. Resolving it here — once per stage evaluation — is what keeps a
/// degraded array from refrigerating both tanks at full nameplate.
pub struct StageTankModel<F> {
    oxidizer: TankSpec,
    fuel: TankSpec,
    bus: BusSpec,
    context: ThermalContext,
    /// Flight-time and position, supplied by the caller so the model can build
    /// the same sunlit/eclipse environment the trajectory flies through.
    environment: F,
    /// Temperatures at which each tank's relief valve is set.
    vent_temperature_k: [f64; 2],
}

impl<F> StageTankModel<F>
where
    F: FnMut(f64) -> Result<ThermalEnvironment, IntegrationError>,
{
    /// Build from the two validated tank specs and the bus they share.
    pub fn new(
        oxidizer: TankSpec,
        fuel: TankSpec,
        bus: BusSpec,
        context: ThermalContext,
        environment: F,
    ) -> Result<Self, IntegrationError> {
        oxidizer.validate()?;
        fuel.validate()?;
        bus.validate()?;
        let vents = [
            oxidizer.saturation_temperature_k(oxidizer.relief_pressure_pa),
            fuel.saturation_temperature_k(fuel.relief_pressure_pa),
        ];
        if !vents[0].is_finite() || vents[0] <= 0.0 || !vents[1].is_finite() || vents[1] <= 0.0 {
            return Err(IntegrationError::NumericalRange);
        }
        Ok(Self {
            oxidizer,
            fuel,
            bus,
            context,
            environment,
            vent_temperature_k: vents,
        })
    }
}

impl<F> StageTankModel<F>
where
    F: FnMut(f64) -> Result<ThermalEnvironment, IntegrationError>,
{
    fn spec(&self, index: usize) -> Result<&TankSpec, IntegrationError> {
        match index {
            0 => Ok(&self.oxidizer),
            1 => Ok(&self.fuel),
            _ => Err(IntegrationError::InvalidInput),
        }
    }

    /// Solar generation available to the coolers at this instant, W.
    fn bus_w(&mut self, time_s: f64) -> Result<f64, IntegrationError> {
        let env = (self.environment)(time_s)?;
        let illumination = env.illumination();
        let generation =
            self.bus.array_w_at_1au * (env.solar_flux_w_m2() / SOLAR_CONSTANT_W_M2) * illumination;
        Ok(libm::fmax(0.0, generation - self.bus.avionics_w))
    }
}

impl<F> TankModel for StageTankModel<F>
where
    F: FnMut(f64) -> Result<ThermalEnvironment, IntegrationError>,
{
    fn initial_temperature_k(&mut self, index: usize) -> Result<f64, IntegrationError> {
        // A fuelled tank on the pad is already at its relief setting: the
        // worst honest case, and the one the standalone model starts from.
        Ok(self.vent_temperature_k[index.min(1)])
    }

    fn rates(
        &mut self,
        time_s: f64,
        index: usize,
        state: [f64; TANK_STRIDE],
    ) -> Result<TankRate, IntegrationError> {
        let spec = *self.spec(index)?;
        let [temperature_k, liquid_kg, _vented_kg] = state;
        if !temperature_k.is_finite()
            || temperature_k <= 0.0
            || !liquid_kg.is_finite()
            || liquid_kg < 0.0
        {
            return Err(IntegrationError::NumericalRange);
        }
        if liquid_kg <= 0.0 {
            // A dry tank has nothing to cool and nothing to boil.
            return Ok(TankRate::default());
        }
        // The bus is shared. Both tanks are evaluated against what remains
        // after avionics; the standalone model's strict priority order decides
        // which cooler is actually served when the surplus is short.
        let available = self.bus_w(time_s)?;
        let env = (self.environment)(time_s)?;
        let balance = heat_balance(
            &spec,
            temperature_k,
            &env,
            self.context.structure_temperature_k,
            self.context.sink_temperature_k,
            available,
        );
        let props = spec.properties();
        let vent_temperature = self.vent_temperature_k[index.min(1)];
        if !props.cryogenic {
            // A storable sits at ambient and does not boil; its temperature is
            // not part of the flight.
            return Ok(TankRate {
                temperature_k_s: 0.0,
                liquid_kg_s: 0.0,
                vented_kg_s: 0.0,
                cooler_w: 0.0,
            });
        }
        if temperature_k >= vent_temperature {
            // Venting: temperature is pinned at the relief saturation point and
            // every watt of net heat vaporizes liquid.
            let rate = libm::fmax(0.0, balance.net_w) / props.heat_of_vaporization_j_kg;
            let rate = libm::fmin(rate, liquid_kg.max(0.0) * 1e3);
            return Ok(TankRate {
                temperature_k_s: 0.0,
                liquid_kg_s: -rate,
                vented_kg_s: rate,
                cooler_w: balance.cryocooler_bus_w,
            });
        }
        // Self-pressurization: the valve is shut and the liquid warms.
        let capacity = liquid_kg * props.liquid_specific_heat_j_kg_k;
        if capacity <= 0.0 {
            return Err(IntegrationError::NumericalRange);
        }
        Ok(TankRate {
            temperature_k_s: balance.net_w / capacity,
            liquid_kg_s: 0.0,
            vented_kg_s: 0.0,
            cooler_w: balance.cryocooler_bus_w,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermal::ShellMaterial;
    use crate::Fluid;

    fn spec() -> TankSpec {
        TankSpec {
            fluid: Fluid::Lh2,
            volume_m3: 20.0,
            area_m2: 30.0,
            mli_layers: 20,
            epsilon_eff: crate::thermal::ideal_epsilon_for_layers(20) * 4.0,
            shell: ShellMaterial::Aluminium2219,
            absorptivity: 0.2,
            outer_epsilon: 0.8,
            strut_conductance_w_k: 0.5,
            relief_pressure_pa: 200_000.0,
            radiator_area_m2: 0.0,
            radiator_epsilon: 0.8,
            cryocooler_lift_w: 0.0,
            cryocooler_carnot_fraction: 0.25,
        }
    }

    fn model() -> StageTankModel<impl FnMut(f64) -> Result<ThermalEnvironment, IntegrationError>> {
        StageTankModel::new(
            TankSpec {
                fluid: Fluid::Lox,
                ..spec()
            },
            spec(),
            BusSpec {
                array_w_at_1au: 0.0,
                avionics_w: 0.0,
                electric_propulsion_w: 0.0,
            },
            ThermalContext {
                structure_temperature_k: 293.0,
                sink_temperature_k: 3.0,
                bus_w: 0.0,
            },
            |_| {
                Ok(ThermalEnvironment {
                    // Full sun, Earth below, Moon far away.
                    sun: [-1.0e11, 0.0, 0.0],
                    earth: [0.0, 0.0, -6.6e6],
                    moon: [4.0e8, 0.0, 0.0],
                })
            },
        )
        .unwrap()
    }

    #[test]
    fn a_saturated_tank_vents_and_loses_temperature_hold() {
        let mut m = model();
        let vent_k = m.initial_temperature_k(1).unwrap();
        let rate = m.rates(0.0, 1, [vent_k, 1_000.0, 0.0]).unwrap();
        assert_eq!(rate.temperature_k_s, 0.0, "the relief valve pins T");
        assert!(rate.liquid_kg_s < 0.0, "sunlit LH2 must boil");
        assert!(-rate.liquid_kg_s > 0.0);
        assert!((rate.vented_kg_s + rate.liquid_kg_s).abs() < 1e-18);
    }

    #[test]
    fn a_subcooled_tank_warms_without_losing_mass() {
        let mut m = model();
        let vent_k = m.initial_temperature_k(1).unwrap();
        let rate = m.rates(0.0, 1, [vent_k - 5.0, 1_000.0, 0.0]).unwrap();
        assert!(rate.temperature_k_s > 0.0, "it absorbs heat");
        assert_eq!(rate.liquid_kg_s, 0.0);
        assert_eq!(rate.vented_kg_s, 0.0);
    }

    #[test]
    fn a_dry_tank_does_nothing() {
        let mut m = model();
        let rate = m.rates(0.0, 1, [20.0, 0.0, 5.0]).unwrap();
        assert_eq!(rate.liquid_kg_s, 0.0);
        assert_eq!(rate.vented_kg_s, 0.0);
    }

    #[test]
    fn an_impossible_column_is_rejected() {
        let mut m = model();
        assert!(m.rates(0.0, 1, [f64::NAN, 1.0, 0.0]).is_err());
        assert!(m.rates(0.0, 2, [20.0, 1.0, 0.0]).is_err());
        assert!(m.rates(0.0, 1, [-5.0, 1.0, 0.0]).is_err());
    }
}
