//! Tank geometry derived from propellant mass, mixture ratio and ullage.
//!
//! The catalog used to carry one hand-typed `tank_volume_m3` per contract
//! while the thermal model derived its own volumes from the load, the mixture
//! ratio and a per-tank ullage allowance. The two disagreed by 5 %: the tug's
//! two signed tank volumes summed to **40.475496 m³** against a catalog
//! envelope of **38.6 m³**. Either the tanks broke the airframe or part of the
//! load had nowhere to live, and the dossier had to disclose the gap.
//!
//! There is now exactly one derivation. A stage's *usable* propellant volume
//! follows from its load and bulk density; the airframe volume adds a fixed
//! ullage fraction; and the two tank volumes split that airframe volume by
//! each fluid's share of the liquid. Mass, geometry and the signed thermal
//! spec all read this module, so they cannot drift apart again.
//!
//! ```text
//! bulk   ρ = (1 + O/F) / (O/F / ρ_ox + 1 / ρ_fuel)     harmonic mixture
//! fuel   m_f = m / (1 + O/F),        m_ox = m − m_f
//! usable V_u = m / ρ
//! airframe   V = V_u / (1 − ullage)
//! tanks  V_ox = V · (m_ox/ρ_ox)/V_u, V_f = V · (m_f/ρ_f)/V_u
//! ```
//!
//! `V_ox + V_f = V` exactly, and `V · (1 − ullage) · ρ = m` exactly, so the
//! catalog volume, the flown load and the signed tank volumes agree to the
//! last bit rather than to a rounded decimal.
use crate::fluid::Fluid;

/// Fraction of an airframe's internal volume reserved for ullage and
/// residuals. 3 % is a conventional single-stage allowance and is now the one
/// number the catalog and the tank model share.
pub const DEFAULT_ULLAGE_FRACTION: f64 = 0.03;

/// How a propellant load is spread across the two tanks of a stage.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TankGeometry {
    /// Usable propellant volume, m³, at saturated-liquid density.
    pub usable_volume_m3: f64,
    /// Airframe internal volume, m³: usable plus ullage.
    pub airframe_volume_m3: f64,
    /// Saturated-liquid bulk density of the mixture, kg/m³.
    pub bulk_density_kg_m3: f64,
    /// Oxidizer then fuel volume, m³. Sums to `airframe_volume_m3`.
    pub tank_volumes_m3: [f64; 2],
    /// Oxidizer then fuel mass, kg. Sums to the load.
    pub tank_masses_kg: [f64; 2],
    /// The share of the airframe given to ullage, 0..1.
    pub ullage_fraction: f64,
}

impl TankGeometry {
    /// Derive the geometry of one stage.
    ///
    /// Returns `None` for a non-finite or non-positive load, a mixture ratio
    /// outside the fluid pair's physical range, or an ullage fraction outside
    /// `[0, 1)`. Callers must not treat `None` as a zero-volume tank.
    pub fn derive(
        oxidizer: Fluid,
        fuel: Fluid,
        of_ratio: f64,
        load_kg: f64,
        ullage_fraction: f64,
    ) -> Option<Self> {
        if !load_kg.is_finite()
            || load_kg <= 0.0
            || !of_ratio.is_finite()
            || of_ratio <= 0.0
            || !ullage_fraction.is_finite()
            || !(0.0..1.0).contains(&ullage_fraction)
        {
            return None;
        }
        let ox = oxidizer.properties();
        let fu = fuel.properties();
        if ox.liquid_density_kg_m3 <= 0.0 || fu.liquid_density_kg_m3 <= 0.0 {
            return None;
        }
        let fuel_kg = load_kg / (1.0 + of_ratio);
        let oxidizer_kg = load_kg - fuel_kg;
        let oxidizer_liquid_m3 = oxidizer_kg / ox.liquid_density_kg_m3;
        let fuel_liquid_m3 = fuel_kg / fu.liquid_density_kg_m3;
        let usable_volume_m3 = oxidizer_liquid_m3 + fuel_liquid_m3;
        if !usable_volume_m3.is_finite() || usable_volume_m3 <= 0.0 {
            return None;
        }
        // Reservoir volume = liquid volume / (1 − ullage). Volume also fixes
        // the bulk density, so the two statements are the same equation.
        let airframe_volume_m3 = usable_volume_m3 / (1.0 - ullage_fraction);
        let bulk_density_kg_m3 = load_kg / usable_volume_m3;
        let area_fraction = airframe_volume_m3 / usable_volume_m3;
        let tank_volumes_m3 = [
            oxidizer_liquid_m3 * area_fraction,
            fuel_liquid_m3 * area_fraction,
        ];
        if !tank_volumes_m3.iter().all(|v| v.is_finite() && *v > 0.0) {
            return None;
        }
        Some(Self {
            usable_volume_m3,
            airframe_volume_m3,
            bulk_density_kg_m3,
            tank_volumes_m3,
            tank_masses_kg: [oxidizer_kg, fuel_kg],
            ullage_fraction,
        })
    }

    /// The mass a given airframe volume carries at this mixture.
    pub fn load_for_volume(&self, airframe_volume_m3: f64) -> f64 {
        airframe_volume_m3 * (1.0 - self.ullage_fraction) * self.bulk_density_kg_m3
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluid::Fluid;

    fn tug(of: f64, load: f64) -> TankGeometry {
        TankGeometry::derive(Fluid::Lox, Fluid::Lh2, of, load, DEFAULT_ULLAGE_FRACTION).unwrap()
    }

    /// `oracle_tank_volume_zero_discrepancy`.
    ///
    /// The tank volumes, the airframe volume and the load must be one
    /// consistent set: the tanks fill the airframe, the airframe less ullage
    /// is the liquid, and the liquid weighs exactly the load.
    #[test]
    fn geometry_is_self_consistent_to_rounding() {
        for (of, load) in [(5.2, 12_800.0), (6.0, 8_000.0), (4.5, 1.0), (5.2, 12_817.0)] {
            let g = tug(of, load);
            let tank_sum = g.tank_volumes_m3[0] + g.tank_volumes_m3[1];
            // One rounding in the two-part sum, so a few ULP — not the 5 %
            // discrepancy the catalog used to carry.
            assert!(
                (tank_sum - g.airframe_volume_m3).abs() <= 4.0 * f64::EPSILON * g.airframe_volume_m3,
                "tank volumes must fill the airframe: {tank_sum} vs {}",
                g.airframe_volume_m3
            );
            // Liquid volume is what is left after the ullage allowance.
            let liquid = g.airframe_volume_m3 * (1.0 - g.ullage_fraction);
            assert!(
                (liquid - g.usable_volume_m3).abs() <= 1e-9 * g.usable_volume_m3.max(1.0),
                "ullage must account for exactly the difference"
            );
            // Mass = liquid volume x bulk density.
            let mass = liquid * g.bulk_density_kg_m3;
            assert!(
                (mass - load).abs() <= 1e-9 * load,
                "derived mass {mass} vs load {load}"
            );
            // The split follows each fluid's share of the liquid.
            let sum_mass = g.tank_masses_kg[0] + g.tank_masses_kg[1];
            assert!((sum_mass - load).abs() <= 1e-9 * load);
            assert!(g.tank_volumes_m3[1] > g.tank_volumes_m3[0], "LH2 is bulkier");
        }
    }

    #[test]
    fn round_trip_volume_and_mass_agree() {
        let g = tug(5.2, 12_800.0);
        assert!((g.load_for_volume(g.airframe_volume_m3) - 12_800.0).abs() < 1e-9);
        // This is the volume the catalog previously typed by hand.
        assert!(
            (g.airframe_volume_m3 - 39.74).abs() < 0.01,
            "tug airframe {}",
            g.airframe_volume_m3
        );
    }

    #[test]
    fn impossible_inputs_are_refused() {
        assert!(TankGeometry::derive(Fluid::Lox, Fluid::Lh2, 5.2, 0.0, 0.03).is_none());
        assert!(TankGeometry::derive(Fluid::Lox, Fluid::Lh2, 5.2, -1.0, 0.03).is_none());
        assert!(TankGeometry::derive(Fluid::Lox, Fluid::Lh2, f64::NAN, 10.0, 0.03).is_none());
        assert!(TankGeometry::derive(Fluid::Lox, Fluid::Lh2, 5.2, 10.0, 1.0).is_none());
        assert!(TankGeometry::derive(Fluid::Lox, Fluid::Lh2, 0.0, 10.0, 0.03).is_none());
    }
}
