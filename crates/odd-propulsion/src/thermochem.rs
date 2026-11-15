//! CEA-calibrated bulk combustion properties.
//!
//! This is **not** a chemical-equilibrium solver. Running NASA CEA inside a
//! deterministic wasm kernel is neither practical nor reproducible across
//! platforms. Instead the three quantities a quasi-1D nozzle needs — flame
//! temperature `T_c`, effective expansion molar mass `M̄` and specific-heat
//! ratio `γ` — are smooth closed-form functions of mixture ratio and chamber
//! pressure, anchored to published shifting-equilibrium results and to the
//! measured performance of flying engines.
//!
//! Calibration anchors (see `docs/propulsion.md` for the full table):
//!
//! | pair | anchor | `T_c` | `M̄` | `γ` |
//! |---|---|---|---|---|
//! | LOX/LH2 | O/F 6.0, 20.64 MPa (RS-25) | 3588 K | 13.5 g/mol | 1.195 |
//! | LOX/RP-1 | O/F 2.36, 9.7 MPa (Merlin 1D) | 3664 K | 22.9 g/mol | 1.2215 |
//! | LOX/LCH4 | O/F 3.5, 20 MPa (Raptor class) | 3602 K | 21.2 g/mol | 1.205 |
//! | NTO/MMH | O/F 2.0, 1.5 MPa (SPS class) | 3278 K | 22.5 g/mol | 1.235 |
//!
//! `M̄` is an **effective expansion** molar mass, not the chamber equilibrium
//! value. In a real hydrolox nozzle, dissociated hydrogen recombines as the
//! flow cools, so the mean molar mass of the expanding gas is higher than the
//! chamber value, and increasingly so at fuel-rich mixtures. Folding that
//! into `M̄` is what lets a frozen-`γ` solution reproduce shifting-equilibrium
//! specific impulse, and it is why peak `Isp` lands fuel-rich of the
//! temperature peak rather than running away toward pure hydrogen.
use crate::PropulsionError;

/// Universal gas constant, CODATA 2018.
pub const RU: f64 = 8.314_462_618;
/// Standard gravity, CGPM 1901.
pub const G0: f64 = 9.80665;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Propellant {
    LoxLh2 = 0,
    LoxRp1 = 1,
    LoxLch4 = 2,
    NtoMmh = 3,
}

impl Propellant {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::LoxLh2),
            1 => Some(Self::LoxRp1),
            2 => Some(Self::LoxLch4),
            3 => Some(Self::NtoMmh),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::LoxLh2 => "LOX / LH2",
            Self::LoxRp1 => "LOX / RP-1",
            Self::LoxLch4 => "LOX / LCH4",
            Self::NtoMmh => "NTO / MMH",
        }
    }
    pub fn all() -> [Self; 4] {
        [Self::LoxLh2, Self::LoxRp1, Self::LoxLch4, Self::NtoMmh]
    }
    /// Stoichiometric mass ratio. Peak temperature sits near it; peak
    /// specific impulse does not.
    pub fn stoichiometric_of(self) -> f64 {
        match self {
            Self::LoxLh2 => 7.94,
            Self::LoxRp1 => 3.40,
            Self::LoxLch4 => 4.00,
            Self::NtoMmh => 2.57,
        }
    }
    /// Mixture ratio at which chamber temperature peaks. Distinct from
    /// stoichiometric because dissociation shifts the maximum slightly rich.
    pub fn peak_temperature_of(self) -> f64 {
        self.constants().tc_peak_of
    }
    /// Mixture ratios the calibration covers. Outside them the closed forms
    /// are extrapolation, so the solver refuses rather than inventing data.
    pub fn of_band(self) -> (f64, f64) {
        match self {
            Self::LoxLh2 => (2.5, 10.0),
            Self::LoxRp1 => (1.4, 3.6),
            Self::LoxLch4 => (2.0, 4.6),
            Self::NtoMmh => (1.0, 3.0),
        }
    }
    fn constants(self) -> Constants {
        match self {
            Self::LoxLh2 => Constants {
                rho_ox: 1141.0,
                rho_fuel: 70.8,
                of_ref: 6.0,
                pc_ref: 20.64e6,
                tc_max: 3760.0,
                tc_peak_of: 8.0,
                tc_curvature: 43.0,
                tc_pc_exponent: 0.02,
                molar_ref: 0.0135,
                molar_exponent: 0.33,
                gamma_ref: 1.195,
                gamma_slope: 0.006,
                c_star_efficiency: 0.985,
                nozzle_efficiency: 0.997,
                characteristic_length_m: 1.10,
                gas_viscosity_pa_s: 1.2e-4,
                gas_cp_j_kg_k: 6500.0,
                gas_prandtl: 0.55,
                coolant: Coolant {
                    conductivity_w_m_k: 0.18,
                    viscosity_pa_s: 1.2e-5,
                    cp_j_kg_k: 14_300.0,
                    prandtl: 1.6,
                    inlet_temperature_k: 45.0,
                    density_kg_m3: 70.8,
                },
            },
            Self::LoxRp1 => Constants {
                rho_ox: 1141.0,
                rho_fuel: 815.0,
                of_ref: 2.36,
                pc_ref: 9.7e6,
                tc_max: 3690.0,
                tc_peak_of: 2.75,
                tc_curvature: 170.0,
                tc_pc_exponent: 0.02,
                molar_ref: 0.0229,
                molar_exponent: 0.30,
                gamma_ref: 1.2215,
                gamma_slope: 0.008,
                c_star_efficiency: 0.975,
                nozzle_efficiency: 0.985,
                characteristic_length_m: 0.90,
                gas_viscosity_pa_s: 1.1e-4,
                gas_cp_j_kg_k: 3200.0,
                gas_prandtl: 0.65,
                coolant: Coolant {
                    conductivity_w_m_k: 0.12,
                    viscosity_pa_s: 8.0e-4,
                    cp_j_kg_k: 2400.0,
                    prandtl: 9.0,
                    inlet_temperature_k: 300.0,
                    density_kg_m3: 815.0,
                },
            },
            Self::LoxLch4 => Constants {
                rho_ox: 1141.0,
                rho_fuel: 422.6,
                of_ref: 3.5,
                pc_ref: 20.0e6,
                tc_max: 3620.0,
                tc_peak_of: 3.9,
                tc_curvature: 90.0,
                tc_pc_exponent: 0.02,
                molar_ref: 0.0212,
                molar_exponent: 0.30,
                gamma_ref: 1.205,
                gamma_slope: 0.007,
                c_star_efficiency: 0.980,
                nozzle_efficiency: 0.990,
                characteristic_length_m: 0.95,
                gas_viscosity_pa_s: 1.15e-4,
                gas_cp_j_kg_k: 4200.0,
                gas_prandtl: 0.60,
                coolant: Coolant {
                    conductivity_w_m_k: 0.20,
                    viscosity_pa_s: 1.1e-4,
                    cp_j_kg_k: 3500.0,
                    prandtl: 2.5,
                    inlet_temperature_k: 111.0,
                    density_kg_m3: 422.6,
                },
            },
            Self::NtoMmh => Constants {
                rho_ox: 1443.0,
                rho_fuel: 878.0,
                of_ref: 2.0,
                pc_ref: 1.5e6,
                tc_max: 3320.0,
                tc_peak_of: 2.4,
                tc_curvature: 260.0,
                tc_pc_exponent: 0.015,
                molar_ref: 0.0225,
                molar_exponent: 0.28,
                gamma_ref: 1.235,
                gamma_slope: 0.010,
                c_star_efficiency: 0.965,
                nozzle_efficiency: 0.985,
                characteristic_length_m: 0.85,
                gas_viscosity_pa_s: 1.05e-4,
                gas_cp_j_kg_k: 2900.0,
                gas_prandtl: 0.70,
                coolant: Coolant {
                    conductivity_w_m_k: 0.20,
                    viscosity_pa_s: 5.0e-4,
                    cp_j_kg_k: 2900.0,
                    prandtl: 6.5,
                    inlet_temperature_k: 300.0,
                    density_kg_m3: 878.0,
                },
            },
        }
    }
}

/// Regenerative coolant properties for the fuel side of the jacket.
#[derive(Clone, Copy, Debug)]
pub struct Coolant {
    pub conductivity_w_m_k: f64,
    pub viscosity_pa_s: f64,
    pub cp_j_kg_k: f64,
    pub prandtl: f64,
    pub inlet_temperature_k: f64,
    pub density_kg_m3: f64,
}

#[derive(Clone, Copy, Debug)]
struct Constants {
    rho_ox: f64,
    rho_fuel: f64,
    of_ref: f64,
    pc_ref: f64,
    tc_max: f64,
    tc_peak_of: f64,
    tc_curvature: f64,
    tc_pc_exponent: f64,
    molar_ref: f64,
    molar_exponent: f64,
    gamma_ref: f64,
    gamma_slope: f64,
    c_star_efficiency: f64,
    nozzle_efficiency: f64,
    characteristic_length_m: f64,
    gas_viscosity_pa_s: f64,
    gas_cp_j_kg_k: f64,
    gas_prandtl: f64,
    coolant: Coolant,
}

/// Bulk properties of the chamber and its propellant feed.
#[derive(Clone, Copy, Debug)]
pub struct ChamberThermo {
    pub propellant: Propellant,
    pub of_ratio: f64,
    pub p_c_pa: f64,
    pub temperature_k: f64,
    pub molar_mass_kg_mol: f64,
    pub gamma: f64,
    pub bulk_density_kg_m3: f64,
    /// Specific gas constant of the combustion products, J/(kg K).
    pub gas_constant_j_kg_k: f64,
    pub c_star_efficiency: f64,
    pub nozzle_efficiency: f64,
    pub characteristic_length_m: f64,
    pub gas_viscosity_pa_s: f64,
    pub gas_cp_j_kg_k: f64,
    pub gas_prandtl: f64,
    pub coolant: Coolant,
}

/// Bulk density of the combined load: `(1+r) / (r/rho_ox + 1/rho_fuel)`.
/// This is the number that makes hydrolox tanks enormous for their mass.
pub fn bulk_density(propellant: Propellant, of_ratio: f64) -> f64 {
    let c = propellant.constants();
    (1.0 + of_ratio) / (of_ratio / c.rho_ox + 1.0 / c.rho_fuel)
}

pub fn chamber(
    propellant: Propellant,
    of_ratio: f64,
    p_c_pa: f64,
) -> Result<ChamberThermo, PropulsionError> {
    let (lo, hi) = propellant.of_band();
    if !of_ratio.is_finite() || of_ratio < lo || of_ratio > hi {
        return Err(PropulsionError::MixtureRatioOutOfRange);
    }
    if !p_c_pa.is_finite() || !(0.2e6..=40.0e6).contains(&p_c_pa) {
        return Err(PropulsionError::ChamberPressureOutOfRange);
    }
    let c = propellant.constants();
    let offset = of_ratio - c.tc_peak_of;
    let pressure_scale = libm::pow(p_c_pa / c.pc_ref, c.tc_pc_exponent);
    let temperature_k = (c.tc_max - c.tc_curvature * offset * offset) * pressure_scale;
    let molar_mass_kg_mol = c.molar_ref * libm::pow(of_ratio / c.of_ref, c.molar_exponent);
    let gamma = c.gamma_ref + c.gamma_slope * (of_ratio - c.of_ref);
    if !(temperature_k > 0.0 && molar_mass_kg_mol > 0.0 && gamma > 1.0) {
        return Err(PropulsionError::NumericalRange);
    }
    Ok(ChamberThermo {
        propellant,
        of_ratio,
        p_c_pa,
        temperature_k,
        molar_mass_kg_mol,
        gamma,
        bulk_density_kg_m3: bulk_density(propellant, of_ratio),
        gas_constant_j_kg_k: RU / molar_mass_kg_mol,
        c_star_efficiency: c.c_star_efficiency,
        nozzle_efficiency: c.nozzle_efficiency,
        characteristic_length_m: c.characteristic_length_m,
        gas_viscosity_pa_s: c.gas_viscosity_pa_s,
        gas_cp_j_kg_k: c.gas_cp_j_kg_k,
        gas_prandtl: c.gas_prandtl,
        coolant: c.coolant,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hydrolox_anchor_matches_the_published_cea_point() {
        let c = chamber(Propellant::LoxLh2, 6.0, 20.64e6).unwrap();
        assert!((c.temperature_k - 3588.0).abs() < 1e-9);
        assert!((c.molar_mass_kg_mol - 0.0135).abs() < 1e-12);
        assert!((c.gamma - 1.195).abs() < 1e-12);
    }

    #[test]
    fn bulk_density_is_the_harmonic_combination() {
        // Hydrolox is dominated by the hydrogen volume, kerolox is not.
        let h2 = bulk_density(Propellant::LoxLh2, 6.0);
        let rp1 = bulk_density(Propellant::LoxRp1, 2.36);
        assert!((350.0..370.0).contains(&h2), "{h2}");
        assert!((1000.0..1040.0).contains(&rp1), "{rp1}");
        assert!(rp1 > 2.7 * h2);
    }

    #[test]
    fn temperature_peaks_richer_than_the_calibration_anchor() {
        let peak = chamber(Propellant::LoxLh2, 8.0, 20.64e6)
            .unwrap()
            .temperature_k;
        for of in [3.0, 4.5, 6.0, 7.0, 9.0] {
            assert!(
                chamber(Propellant::LoxLh2, of, 20.64e6)
                    .unwrap()
                    .temperature_k
                    < peak
            );
        }
    }

    #[test]
    fn extrapolation_and_bad_pressure_are_refused() {
        assert_eq!(
            chamber(Propellant::LoxLh2, 20.0, 20e6).err(),
            Some(PropulsionError::MixtureRatioOutOfRange)
        );
        assert_eq!(
            chamber(Propellant::NtoMmh, 2.0, f64::NAN).err(),
            Some(PropulsionError::ChamberPressureOutOfRange)
        );
        assert_eq!(
            chamber(Propellant::LoxRp1, 2.36, 100e6).err(),
            Some(PropulsionError::ChamberPressureOutOfRange)
        );
    }
}
