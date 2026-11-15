//! Bartz throat heating, regenerative wall equilibrium and physics-derived
//! engine dry mass.
//!
//! The mass model is a pressure vessel plus a power-dense turbopump, not a
//! lookup table: chamber wall thickness follows hoop stress, bell mass
//! follows its actual surface area, and pump mass follows shaft power. That
//! is what makes a 30 MPa methalox engine heavier than a 1.5 MPa storable one
//! of the same thrust, and what stops a player from designing a free engine.
use crate::nozzle::{ideal_c_star, NozzleGeometry};
use crate::thermochem::ChamberThermo;

/// Hot-wall limit for a copper-chrome-niobium liner in a nickel jacket.
pub const MAX_WALL_TEMPERATURE_K: f64 = 1350.0;
/// Bartz over-predicts throat film coefficient because it ignores boundary
/// layer property variation. 0.7 is the usual preliminary-design correction.
const BARTZ_CORRECTION: f64 = 0.70;
/// Regenerative jacket design velocity, m/s. Fixed so the wall solution is a
/// property of the engine, not of a hidden operating point.
const COOLANT_VELOCITY_M_S: f64 = 200.0;
const CHANNEL_PITCH_M: f64 = 0.003;
const LINER_THICKNESS_M: f64 = 0.001;
const LINER_CONDUCTIVITY_W_M_K: f64 = 380.0;
/// Chamber contraction ratio and structural allowables.
const CONTRACTION_RATIO: f64 = 3.0;
const SAFETY_FACTOR: f64 = 1.5;
const ALLOWABLE_STRESS_PA: f64 = 400.0e6;
const SUPERALLOY_DENSITY_KG_M3: f64 = 8200.0;
const LINER_DENSITY_KG_M3: f64 = 8900.0;
/// Turbopump specific power. Flying pumps run 40–150 kW/kg; 40 kW/kg is a
/// conservative assembly figure including the turbine and manifolds.
const PUMP_SPECIFIC_POWER_W_KG: f64 = 40_000.0;
const PUMP_EFFICIENCY: f64 = 0.70;
/// Injector, valves, ducting and gimbal allowance on the static structure.
const ACCESSORY_FACTOR: f64 = 1.35;
/// Pump head above chamber pressure: injector drop plus line losses.
const PUMP_HEAD_FACTOR: f64 = 1.35;

#[derive(Clone, Copy, Debug)]
pub struct ThermalSolution {
    pub film_coefficient_w_m2_k: f64,
    pub adiabatic_wall_temperature_k: f64,
    pub wall_temperature_k: f64,
    pub heat_flux_w_m2: f64,
    pub coolant_coefficient_w_m2_k: f64,
    pub limit_exceeded: bool,
}

/// Bartz film coefficient at the throat, evaluated at a trial wall
/// temperature. `sigma` carries the property-variation correction.
fn bartz(
    thermo: &ChamberThermo,
    geometry: NozzleGeometry,
    c_star: f64,
    wall_temperature_k: f64,
) -> f64 {
    let g = thermo.gamma;
    let diameter = 2.0 * geometry.r_throat_m;
    let chamber_radius = geometry.r_throat_m * libm::sqrt(CONTRACTION_RATIO);
    // Radius of curvature of the convergent throat arc.
    let curvature_radius = 0.382 * chamber_radius;
    let mach_term = 1.0 + 0.5 * (g - 1.0);
    let sigma = libm::pow(
        0.5 * (wall_temperature_k / thermo.temperature_k) * mach_term + 0.5,
        -0.68,
    ) * libm::pow(mach_term, -0.12);
    let transport = libm::pow(thermo.gas_viscosity_pa_s, 0.2) * thermo.gas_cp_j_kg_k
        / libm::pow(thermo.gas_prandtl, 0.6);
    BARTZ_CORRECTION
        * (0.026 / libm::pow(diameter, 0.2))
        * transport
        * libm::pow(thermo.p_c_pa / c_star, 0.8)
        * libm::pow(diameter / curvature_radius, 0.1)
        * sigma
}

/// Coolant-side coefficient from a Dittus–Boelter jacket sized by continuity.
fn coolant_coefficient(thermo: &ChamberThermo, geometry: NozzleGeometry, mass_flow: f64) -> f64 {
    let coolant = thermo.coolant;
    let fuel_flow = mass_flow / (1.0 + thermo.of_ratio);
    let chamber_radius = geometry.r_throat_m * libm::sqrt(CONTRACTION_RATIO);
    let channels = libm::fmax(
        12.0,
        libm::floor(2.0 * core::f64::consts::PI * chamber_radius / CHANNEL_PITCH_M),
    );
    let total_area = fuel_flow / (coolant.density_kg_m3 * COOLANT_VELOCITY_M_S);
    let channel_area = total_area / channels;
    // 2:1 channel aspect ratio sets the hydraulic diameter.
    let width = libm::sqrt(channel_area * 0.5);
    let hydraulic = 2.0 * channel_area / (3.0 * width);
    let mass_flux = coolant.density_kg_m3 * COOLANT_VELOCITY_M_S;
    let reynolds = mass_flux * hydraulic / coolant.viscosity_pa_s;
    let nusselt = 0.023 * libm::pow(reynolds, 0.8) * libm::pow(coolant.prandtl, 0.4);
    nusselt * coolant.conductivity_w_m_k / hydraulic
}

pub fn thermal(
    thermo: &ChamberThermo,
    geometry: NozzleGeometry,
    mass_flow_kg_s: f64,
    c_star: f64,
) -> ThermalSolution {
    let g = thermo.gamma;
    let recovery = libm::pow(thermo.gas_prandtl, 1.0 / 3.0);
    let adiabatic =
        thermo.temperature_k * (1.0 + recovery * 0.5 * (g - 1.0)) / (1.0 + 0.5 * (g - 1.0));
    let h_c = coolant_coefficient(thermo, geometry, mass_flow_kg_s);
    let resistance = 1.0 / h_c + LINER_THICKNESS_M / LINER_CONDUCTIVITY_W_M_K;
    // Fixed-point iteration with a fixed count: the wall temperature is a
    // deterministic function of the spec, never of a convergence history.
    let mut wall = 900.0;
    let mut film = bartz(thermo, geometry, c_star, wall);
    for _ in 0..40 {
        film = bartz(thermo, geometry, c_star, wall);
        let product = film * resistance;
        wall = (thermo.coolant.inlet_temperature_k + product * adiabatic) / (1.0 + product);
    }
    let heat_flux = film * (adiabatic - wall);
    ThermalSolution {
        film_coefficient_w_m2_k: film,
        adiabatic_wall_temperature_k: adiabatic,
        wall_temperature_k: wall,
        heat_flux_w_m2: heat_flux,
        coolant_coefficient_w_m2_k: h_c,
        limit_exceeded: wall > MAX_WALL_TEMPERATURE_K,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MassSolution {
    pub chamber_mass_kg: f64,
    pub bell_mass_kg: f64,
    pub injector_mass_kg: f64,
    pub pump_mass_kg: f64,
    pub total_mass_kg: f64,
    pub pump_power_w: f64,
    pub chamber_radius_m: f64,
    pub chamber_length_m: f64,
    pub chamber_wall_m: f64,
    pub bell_wall_m: f64,
}

pub fn mass(thermo: &ChamberThermo, geometry: NozzleGeometry, mass_flow_kg_s: f64) -> MassSolution {
    let pi = core::f64::consts::PI;
    let rt = geometry.r_throat_m;
    let chamber_radius_m = rt * libm::sqrt(CONTRACTION_RATIO);
    let chamber_area = CONTRACTION_RATIO * geometry.throat_area_m2();
    let chamber_length_m = thermo.characteristic_length_m / CONTRACTION_RATIO;
    let chamber_wall_m = libm::fmax(
        0.0015,
        SAFETY_FACTOR * thermo.p_c_pa * chamber_radius_m / ALLOWABLE_STRESS_PA,
    );
    let bell_wall_m = libm::fmax(0.0012, 0.35 * chamber_wall_m);
    // Cylindrical barrel plus the convergent frustum down to the throat.
    let barrel = 2.0 * pi * chamber_radius_m * chamber_length_m;
    let convergent_length = (chamber_radius_m - rt) / libm::tan(0.523_598_775_598_298_8);
    let convergent = pi
        * (chamber_radius_m + rt)
        * libm::sqrt(
            convergent_length * convergent_length
                + (chamber_radius_m - rt) * (chamber_radius_m - rt),
        );
    let chamber_mass_kg = (barrel + convergent)
        * chamber_wall_m
        * 0.5
        * (SUPERALLOY_DENSITY_KG_M3 + LINER_DENSITY_KG_M3);
    let injector_thickness = libm::fmax(0.008, 3.0 * chamber_wall_m);
    let injector_mass_kg = chamber_area * injector_thickness * SUPERALLOY_DENSITY_KG_M3;
    let rise = geometry.r_exit_m - rt;
    let length = geometry.length_m();
    let bell_area = pi * (rt + geometry.r_exit_m) * libm::sqrt(length * length + rise * rise);
    let bell_mass_kg = bell_area * bell_wall_m * SUPERALLOY_DENSITY_KG_M3;
    let delta_p = PUMP_HEAD_FACTOR * thermo.p_c_pa;
    let pump_power_w = mass_flow_kg_s * delta_p / (thermo.bulk_density_kg_m3 * PUMP_EFFICIENCY);
    let pump_mass_kg = pump_power_w / PUMP_SPECIFIC_POWER_W_KG;
    let total_mass_kg =
        ACCESSORY_FACTOR * (chamber_mass_kg + injector_mass_kg + bell_mass_kg) + pump_mass_kg;
    MassSolution {
        chamber_mass_kg,
        bell_mass_kg,
        injector_mass_kg,
        pump_mass_kg,
        total_mass_kg,
        pump_power_w,
        chamber_radius_m,
        chamber_length_m,
        chamber_wall_m,
        bell_wall_m,
    }
}

/// Convenience for callers that only have the chamber state.
pub fn c_star_actual(thermo: &ChamberThermo) -> f64 {
    thermo.c_star_efficiency * ideal_c_star(thermo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermochem::{chamber, Propellant};

    #[test]
    fn rs25_throat_flux_and_wall_are_in_the_published_range() {
        let t = chamber(Propellant::LoxLh2, 6.0, 20.64e6).unwrap();
        let g = NozzleGeometry {
            r_throat_m: 0.1307,
            r_exit_m: 0.1307 * libm::sqrt(69.0),
            length_fraction: 0.8,
        };
        let c_star = c_star_actual(&t);
        let flow = t.p_c_pa * g.throat_area_m2() / c_star;
        let solution = thermal(&t, g, flow, c_star);
        // SSME throat flux is quoted near 1.6e8 W/m^2 with a hot wall well
        // below the copper-alloy limit.
        assert!(
            (0.8e8..2.6e8).contains(&solution.heat_flux_w_m2),
            "{:?}",
            solution
        );
        assert!(solution.wall_temperature_k < MAX_WALL_TEMPERATURE_K);
        assert!(solution.wall_temperature_k > 600.0);
    }

    #[test]
    fn a_tiny_high_pressure_throat_overheats() {
        let t = chamber(Propellant::LoxRp1, 2.36, 30.0e6).unwrap();
        let g = NozzleGeometry {
            r_throat_m: 0.006,
            r_exit_m: 0.03,
            length_fraction: 0.8,
        };
        let c_star = c_star_actual(&t);
        let flow = t.p_c_pa * g.throat_area_m2() / c_star;
        assert!(thermal(&t, g, flow, c_star).limit_exceeded);
    }

    #[test]
    fn engine_mass_is_the_right_order_for_flying_engines() {
        let t = chamber(Propellant::LoxLh2, 6.0, 20.64e6).unwrap();
        let g = NozzleGeometry {
            r_throat_m: 0.1307,
            r_exit_m: 0.1307 * libm::sqrt(69.0),
            length_fraction: 0.8,
        };
        let flow = t.p_c_pa * g.throat_area_m2() / c_star_actual(&t);
        let rs25 = mass(&t, g, flow).total_mass_kg;
        // RS-25 dry mass is 3,527 kg.
        assert!((2200.0..5000.0).contains(&rs25), "{rs25}");
        let t = chamber(Propellant::LoxRp1, 2.36, 9.7e6).unwrap();
        let g = NozzleGeometry {
            r_throat_m: 0.1325,
            r_exit_m: 0.53,
            length_fraction: 0.8,
        };
        let flow = t.p_c_pa * g.throat_area_m2() / c_star_actual(&t);
        let merlin = mass(&t, g, flow).total_mass_kg;
        // Merlin 1D is about 470 kg.
        assert!((250.0..900.0).contains(&merlin), "{merlin}");
        assert!(rs25 > 4.0 * merlin);
    }
}
