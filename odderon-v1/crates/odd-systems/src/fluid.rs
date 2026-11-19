//! Cryogen and storable fluid properties, in SI base units.
//!
//! Values are the normal-boiling-point (NBP, 101 325 Pa) saturation point and
//! latent heat, the saturated-liquid specific heat and density near NBP, and
//! the molar mass. Sources: NIST Chemistry WebBook / Cryogenic Fluids databook
//! (McCarty) for H2, O2 and CH4; propellant handbooks for NTO, MMH and RP-1.
//!
//! These are **constants with provenance**, not performance lookups: every
//! heat flow, boil-off rate and pressure in `thermal` is derived algebraically
//! from them and the tank geometry, satisfying the No-Magic-Lookup Law.
//!
//! The fluid list is deliberately *not* a second copy of the engine's
//! `Propellant` ordering — a pair and a fluid are different concepts. The
//! pair-to-fluid mapping lives in [`Fluid::pair`], and the wire ids here are
//! frozen by a test for the same reason the engine's are.

use odd_propulsion::Propellant;

/// A single stored fluid. A bipropellant stage has two tanks, one per fluid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fluid {
    /// Liquid hydrogen.
    Lh2 = 0,
    /// Liquid oxygen.
    Lox = 1,
    /// Liquid methane.
    Lch4 = 2,
    /// Kerosene, storable.
    Rp1 = 3,
    /// Nitrogen tetroxide, storable.
    Nto = 4,
    /// Monomethylhydrazine, storable.
    Mmh = 5,
}

/// Saturation and bulk properties at the reference pressure.
#[derive(Clone, Copy, Debug)]
pub struct FluidProperties {
    /// Normal boiling point, K.
    pub saturation_temperature_k: f64,
    /// Latent heat of vaporization at NBP, J/kg.
    pub heat_of_vaporization_j_kg: f64,
    /// Saturated-liquid specific heat near NBP, J/(kg·K).
    pub liquid_specific_heat_j_kg_k: f64,
    /// Saturated-liquid density near NBP, kg/m³.
    pub liquid_density_kg_m3: f64,
    /// Molar mass, kg/mol.
    pub molar_mass_kg_mol: f64,
    /// Critical temperature, K. Above it no liquid phase exists.
    pub critical_temperature_k: f64,
    /// True when the fluid is held far below ambient and therefore boils off.
    /// Storables sit near room temperature and lose nothing by boiling.
    pub cryogenic: bool,
}

impl Fluid {
    /// The oxidizer and fuel of an engine pair, in that order.
    pub fn pair(propellant: Propellant) -> (Self, Self) {
        match propellant {
            Propellant::LoxLh2 => (Self::Lox, Self::Lh2),
            Propellant::LoxRp1 => (Self::Lox, Self::Rp1),
            Propellant::LoxLch4 => (Self::Lox, Self::Lch4),
            Propellant::NtoMmh => (Self::Nto, Self::Mmh),
        }
    }

    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Lh2),
            1 => Some(Self::Lox),
            2 => Some(Self::Lch4),
            3 => Some(Self::Rp1),
            4 => Some(Self::Nto),
            5 => Some(Self::Mmh),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Lh2 => "LH2",
            Self::Lox => "LOX",
            Self::Lch4 => "LCH4",
            Self::Rp1 => "RP-1",
            Self::Nto => "NTO",
            Self::Mmh => "MMH",
        }
    }

    pub fn properties(self) -> FluidProperties {
        match self {
            // NIST/McCarty: parahydrogen NBP 20.27 K, 445.6 kJ/kg, 70.85 kg/m³.
            Self::Lh2 => FluidProperties {
                saturation_temperature_k: 20.27,
                heat_of_vaporization_j_kg: 445_600.0,
                liquid_specific_heat_j_kg_k: 9_690.0,
                liquid_density_kg_m3: 70.85,
                molar_mass_kg_mol: 0.002_015_88,
                critical_temperature_k: 33.145,
                cryogenic: true,
            },
            // NIST: oxygen NBP 90.19 K, 213.0 kJ/kg, 1141 kg/m³.
            Self::Lox => FluidProperties {
                saturation_temperature_k: 90.19,
                heat_of_vaporization_j_kg: 213_000.0,
                liquid_specific_heat_j_kg_k: 1_700.0,
                liquid_density_kg_m3: 1_141.0,
                molar_mass_kg_mol: 0.031_998_8,
                critical_temperature_k: 154.58,
                cryogenic: true,
            },
            // NIST: methane NBP 111.7 K, 510.8 kJ/kg, 422.6 kg/m³.
            Self::Lch4 => FluidProperties {
                saturation_temperature_k: 111.7,
                heat_of_vaporization_j_kg: 510_800.0,
                liquid_specific_heat_j_kg_k: 3_480.0,
                liquid_density_kg_m3: 422.6,
                molar_mass_kg_mol: 0.016_042_5,
                critical_temperature_k: 190.56,
                cryogenic: true,
            },
            // RP-1 is a wide-cut kerosene: storable, no cryogenic boil-off.
            Self::Rp1 => FluidProperties {
                saturation_temperature_k: 489.0,
                heat_of_vaporization_j_kg: 250_000.0,
                liquid_specific_heat_j_kg_k: 2_010.0,
                liquid_density_kg_m3: 810.0,
                molar_mass_kg_mol: 0.170,
                critical_temperature_k: 679.0,
                cryogenic: false,
            },
            // NTO (N2O4) is stored near ambient; its NBP is 294.3 K.
            Self::Nto => FluidProperties {
                saturation_temperature_k: 294.3,
                heat_of_vaporization_j_kg: 414_000.0,
                liquid_specific_heat_j_kg_k: 1_460.0,
                liquid_density_kg_m3: 1_442.5,
                molar_mass_kg_mol: 0.092_011,
                critical_temperature_k: 431.0,
                cryogenic: false,
            },
            // MMH NBP 360.5 K, comfortably above ambient.
            Self::Mmh => FluidProperties {
                saturation_temperature_k: 360.5,
                heat_of_vaporization_j_kg: 875_000.0,
                liquid_specific_heat_j_kg_k: 2_930.0,
                liquid_density_kg_m3: 874.0,
                molar_mass_kg_mol: 0.046_072,
                critical_temperature_k: 585.0,
                cryogenic: false,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The signed thermal spec stores this id, so reordering silently
    /// re-labels historical records. Same law as the engine's wire ids.
    #[test]
    fn fluid_wire_ids_are_frozen() {
        for (id, fluid) in [
            Fluid::Lh2,
            Fluid::Lox,
            Fluid::Lch4,
            Fluid::Rp1,
            Fluid::Nto,
            Fluid::Mmh,
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(fluid as u8, id as u8);
            assert_eq!(Fluid::from_u8(id as u8), Some(fluid));
        }
        assert_eq!(Fluid::from_u8(6), None);
    }

    #[test]
    fn every_engine_pair_maps_to_two_tanks() {
        assert_eq!(Fluid::pair(Propellant::LoxLh2), (Fluid::Lox, Fluid::Lh2));
        assert_eq!(Fluid::pair(Propellant::LoxRp1), (Fluid::Lox, Fluid::Rp1));
        assert_eq!(Fluid::pair(Propellant::LoxLch4), (Fluid::Lox, Fluid::Lch4));
        assert_eq!(Fluid::pair(Propellant::NtoMmh), (Fluid::Nto, Fluid::Mmh));
    }

    #[test]
    fn only_the_three_cryogens_boil_off() {
        for fluid in [Fluid::Lh2, Fluid::Lox, Fluid::Lch4] {
            let p = fluid.properties();
            assert!(p.cryogenic);
            assert!(p.saturation_temperature_k < 120.0);
            assert!(p.saturation_temperature_k < p.critical_temperature_k);
        }
        for fluid in [Fluid::Rp1, Fluid::Nto, Fluid::Mmh] {
            assert!(!fluid.properties().cryogenic);
        }
    }
}
