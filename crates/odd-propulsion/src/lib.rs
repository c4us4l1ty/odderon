//! Deterministic liquid-rocket engine performance from physical inputs.
//!
//! A player designs an engine — propellant pair, mixture ratio, chamber
//! pressure, throat and exit radius, bell length — and this crate derives
//! everything the flight simulator needs: thrust, specific impulse, mass
//! flow and engine dry mass. Nothing is a lookup of a famous engine's
//! numbers, so a signed flight record can carry the *design* and a verifier
//! can recompute the performance from first principles.
//!
//! Determinism: `f64` throughout, every transcendental through `libm`, fixed
//! iteration counts, no I/O, no clock, no randomness. Native and wasm32
//! produce bit-identical solutions (pinned by the core oracle grid).
//!
//! ```
//! use odd_propulsion::{solve_engine, EngineSpec, Propellant};
//! let rs25 = EngineSpec {
//!     propellant: Propellant::LoxLh2,
//!     of_ratio: 6.0,
//!     p_c_pa: 20.64e6,
//!     r_throat_m: 0.1307,
//!     r_exit_m: 0.1307 * 8.3066,
//!     length_fraction: 0.8,
//! };
//! let vacuum = solve_engine(&rs25, 0.0).unwrap();
//! assert!((vacuum.specific_impulse_s - 452.0).abs() < 8.0);
//! ```
pub mod nozzle;
pub mod plume;
pub mod thermal_mass;
pub mod thermochem;

pub use nozzle::{FlowSeparation, NozzleGeometry, Station};
pub use thermochem::{ChamberThermo, Propellant, G0, RU};

/// A complete physical engine design. These six numbers are what the flight
/// recorder signs; thrust and Isp are deliberately *not* inputs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EngineSpec {
    pub propellant: Propellant,
    /// Oxidizer-to-fuel mass ratio.
    pub of_ratio: f64,
    pub p_c_pa: f64,
    pub r_throat_m: f64,
    pub r_exit_m: f64,
    /// Bell length as a fraction of the equivalent 15° cone.
    pub length_fraction: f64,
}

impl EngineSpec {
    pub fn geometry(&self) -> NozzleGeometry {
        NozzleGeometry {
            r_throat_m: self.r_throat_m,
            r_exit_m: self.r_exit_m,
            length_fraction: self.length_fraction,
        }
    }
    /// Structural validation without solving the flow.
    pub fn validate(&self) -> Result<(), PropulsionError> {
        self.geometry().validate()?;
        thermochem::chamber(self.propellant, self.of_ratio, self.p_c_pa)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropulsionError {
    MixtureRatioOutOfRange,
    ChamberPressureOutOfRange,
    GeometryOutOfRange,
    NumericalRange,
}

impl core::fmt::Display for PropulsionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::MixtureRatioOutOfRange => write!(f, "mixture ratio outside the calibrated band"),
            Self::ChamberPressureOutOfRange => write!(f, "chamber pressure outside 0.2–40 MPa"),
            Self::GeometryOutOfRange => {
                write!(f, "throat, exit radius or bell length out of range")
            }
            Self::NumericalRange => write!(f, "no converged isentropic solution"),
        }
    }
}

/// Everything the simulator, the dossier and the test stand need.
#[derive(Clone, Copy, Debug)]
pub struct EngineSolution {
    pub ambient_pressure_pa: f64,
    pub thrust_n: f64,
    pub specific_impulse_s: f64,
    pub mass_flow_kg_s: f64,
    pub exhaust_velocity_m_s: f64,
    pub c_star_m_s: f64,
    pub chamber_temperature_k: f64,
    pub molar_mass_kg_mol: f64,
    pub gamma: f64,
    pub bulk_density_kg_m3: f64,
    pub expansion_ratio: f64,
    pub exit_mach: f64,
    pub exit_pressure_pa: f64,
    pub exit_temperature_k: f64,
    pub divergence_factor: f64,
    pub nozzle_length_m: f64,
    pub throat_heat_flux_w_m2: f64,
    pub throat_wall_temperature_k: f64,
    pub film_coefficient_w_m2_k: f64,
    pub dry_mass_kg: f64,
    pub pump_power_w: f64,
    pub chamber_radius_m: f64,
    pub chamber_length_m: f64,
    pub separation: Option<FlowSeparation>,
    pub thermal_limit_exceeded: bool,
}

impl EngineSolution {
    /// A flight may only use an engine inside its thermal limit. Separation
    /// is an ambient condition, not a defect, so it does not block a vacuum
    /// flight.
    pub fn flight_ready(&self) -> bool {
        !self.thermal_limit_exceeded
            && self.thrust_n.is_finite()
            && self.thrust_n > 0.0
            && self.specific_impulse_s > 0.0
            && self.dry_mass_kg > 0.0
    }
}

/// Solve an engine at a given ambient pressure. Pass `0.0` for vacuum, which
/// is what an orbital flight uses.
pub fn solve_engine(
    spec: &EngineSpec,
    ambient_pressure_pa: f64,
) -> Result<EngineSolution, PropulsionError> {
    let geometry = spec.geometry();
    geometry.validate()?;
    let thermo = thermochem::chamber(spec.propellant, spec.of_ratio, spec.p_c_pa)?;
    if !ambient_pressure_pa.is_finite() || ambient_pressure_pa < 0.0 {
        return Err(PropulsionError::NumericalRange);
    }
    let c_star = thermo.c_star_efficiency * nozzle::ideal_c_star(&thermo);
    let mass_flow_kg_s = thermo.p_c_pa * geometry.throat_area_m2() / c_star;
    let expansion_ratio = geometry.expansion_ratio();
    let exit_mach = nozzle::mach_at_area(expansion_ratio, thermo.gamma)?;
    let (exit_pressure_pa, exit_temperature_k, _density, exhaust_velocity_m_s) =
        nozzle::state_at_mach(exit_mach, &thermo);
    let exit = Station {
        x_norm: 1.0,
        radius_m: geometry.r_exit_m,
        area_ratio: expansion_ratio,
        mach: exit_mach,
        pressure_pa: exit_pressure_pa,
        temperature_k: exit_temperature_k,
        density_kg_m3: _density,
        velocity_m_s: exhaust_velocity_m_s,
    };
    let separation = nozzle::separation(
        geometry,
        &thermo,
        ambient_pressure_pa,
        exit_mach,
        exit_pressure_pa,
    );
    let (thrust_n, specific_impulse_s) = nozzle::performance(
        geometry,
        &thermo,
        mass_flow_kg_s,
        &exit,
        ambient_pressure_pa,
        separation,
    );
    let thermal = thermal_mass::thermal(&thermo, geometry, mass_flow_kg_s, c_star);
    let masses = thermal_mass::mass(&thermo, geometry, mass_flow_kg_s);
    let solution = EngineSolution {
        ambient_pressure_pa,
        thrust_n,
        specific_impulse_s,
        mass_flow_kg_s,
        exhaust_velocity_m_s,
        c_star_m_s: c_star,
        chamber_temperature_k: thermo.temperature_k,
        molar_mass_kg_mol: thermo.molar_mass_kg_mol,
        gamma: thermo.gamma,
        bulk_density_kg_m3: thermo.bulk_density_kg_m3,
        expansion_ratio,
        exit_mach,
        exit_pressure_pa,
        exit_temperature_k,
        divergence_factor: geometry.divergence_factor(),
        nozzle_length_m: geometry.length_m(),
        throat_heat_flux_w_m2: thermal.heat_flux_w_m2,
        throat_wall_temperature_k: thermal.wall_temperature_k,
        film_coefficient_w_m2_k: thermal.film_coefficient_w_m2_k,
        dry_mass_kg: masses.total_mass_kg,
        pump_power_w: masses.pump_power_w,
        chamber_radius_m: masses.chamber_radius_m,
        chamber_length_m: masses.chamber_length_m,
        separation,
        thermal_limit_exceeded: thermal.limit_exceeded,
    };
    if !solution.thrust_n.is_finite()
        || !solution.specific_impulse_s.is_finite()
        || !solution.dry_mass_kg.is_finite()
        || !solution.throat_wall_temperature_k.is_finite()
    {
        return Err(PropulsionError::NumericalRange);
    }
    Ok(solution)
}

/// Axial flow field for the Schlieren view: the divergent stations plus the
/// separation line when the flow has detached.
pub fn flow_profile(
    spec: &EngineSpec,
    ambient_pressure_pa: f64,
    count: usize,
) -> Result<Vec<Station>, PropulsionError> {
    let thermo = thermochem::chamber(spec.propellant, spec.of_ratio, spec.p_c_pa)?;
    nozzle::stations(spec.geometry(), &thermo, count).map(|mut stations| {
        // Downstream of separation the quasi-1D solution no longer describes
        // the wall flow; the view marks those stations by their pressure.
        let _ = ambient_pressure_pa;
        stations.truncate(count.clamp(2, 512));
        stations
    })
}
