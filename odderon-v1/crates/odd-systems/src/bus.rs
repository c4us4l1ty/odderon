//! The spacecraft electrical bus, so a cryocooler has something to steal from.
//!
//! Cryogenic cooling is not free: lifting heat at 20 K against a 300 K radiator
//! costs roughly ten watts of bus power per watt lifted. That draw comes off
//! the same budget as avionics and electric propulsion, which is the whole
//! design tension WP 1.6 is meant to expose.

/// Solar array, battery and fixed loads. All powers in W.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BusSpec {
    /// Array output at 1 AU, normal incidence, W.
    pub array_w_at_1au: f64,
    /// Non-negotiable avionics and comms draw, W.
    pub avionics_w: f64,
    /// Nameplate demand of the electric thruster, W. Zero for chemical stages.
    pub electric_propulsion_w: f64,
}

/// What the bus could supply this instant, and what is left after fixed loads.
#[derive(Clone, Copy, Debug)]
pub struct PowerBudget {
    /// Array output after `1/r²` and eclipse shadowing, W.
    pub generated_w: f64,
    /// Power available to the cryocooler and electric propulsion together, W.
    pub discretionary_w: f64,
    /// Power actually left for electric propulsion after cooling, W.
    pub electric_propulsion_w: f64,
    /// True when the cooler's demand exceeded what cooling could take.
    pub cooling_throttled: bool,
}

impl BusSpec {
    /// Every declared load is a physical, finite, non-negative power, even
    /// when this chemical flight does not consume the EP nameplate demand.
    pub fn validate(&self) -> Result<(), odd_astro::integrator::IntegrationError> {
        if [
            self.array_w_at_1au,
            self.avionics_w,
            self.electric_propulsion_w,
        ]
        .iter()
        .any(|power| !power.is_finite() || *power < 0.0)
        {
            return Err(odd_astro::integrator::IntegrationError::InvalidInput);
        }
        Ok(())
    }

    /// Resolve the bus for a given solar flux fraction and cooler draw.
    ///
    /// `illumination_flux_ratio` is `(1 AU / r)² × shadow`, i.e. exactly the
    /// product `ThermalEnvironment` already computes. Avionics are served
    /// first, then the cryocooler (a tank over pressure is a lost mission),
    /// and electric propulsion gets the remainder.
    pub fn resolve(&self, illumination_flux_ratio: f64, cooler_demand_w: f64) -> PowerBudget {
        let generated = self.array_w_at_1au * illumination_flux_ratio.max(0.0);
        let discretionary = libm::fmax(0.0, generated - self.avionics_w);
        let cooling = libm::fmin(cooler_demand_w.max(0.0), discretionary);
        let remaining = discretionary - cooling;
        PowerBudget {
            generated_w: generated,
            discretionary_w: discretionary,
            electric_propulsion_w: libm::fmin(self.electric_propulsion_w, remaining),
            cooling_throttled: cooling < cooler_demand_w - 1e-9,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avionics_are_served_before_anything_optional() {
        let bus = BusSpec {
            array_w_at_1au: 1_000.0,
            avionics_w: 300.0,
            electric_propulsion_w: 5_000.0,
        };
        let budget = bus.resolve(1.0, 0.0);
        assert_eq!(budget.discretionary_w, 700.0);
        assert_eq!(budget.electric_propulsion_w, 700.0);
    }

    #[test]
    fn a_cryocooler_directly_removes_electric_propulsion_power() {
        let bus = BusSpec {
            array_w_at_1au: 30_000.0,
            avionics_w: 500.0,
            // Nameplate above the bus, so delivered power is bus-limited and
            // the cooling draw subtracts one-for-one.
            electric_propulsion_w: 40_000.0,
        };
        let idle = bus.resolve(1.0, 0.0);
        let cooling = bus.resolve(1.0, 20_000.0);
        assert_eq!(idle.electric_propulsion_w, 29_500.0);
        assert_eq!(cooling.electric_propulsion_w, 9_500.0);
        assert!(
            (idle.electric_propulsion_w - cooling.electric_propulsion_w - 20_000.0).abs() < 1e-9
        );
    }

    #[test]
    fn eclipse_throttles_the_cooler_rather_than_inventing_power() {
        let bus = BusSpec {
            array_w_at_1au: 1_000.0,
            avionics_w: 300.0,
            electric_propulsion_w: 0.0,
        };
        let dark = bus.resolve(0.0, 20_000.0);
        assert_eq!(dark.generated_w, 0.0);
        assert_eq!(dark.discretionary_w, 0.0);
        assert!(dark.cooling_throttled);
    }
}
