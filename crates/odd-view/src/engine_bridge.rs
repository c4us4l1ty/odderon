//! Scalar boundary for the Engine CAD & Schlieren lab.
//!
//! The browser never computes engine performance. It sends six design
//! numbers plus an ambient pressure, and reads back the solved field and the
//! axial flow stations Rust produced. Applying a design to the mission goes
//! through `odd_contracts::Contract::vehicle`, so the stage mass, thrust and
//! specific impulse used by the propagator are the same ones the recorder
//! will sign and a verifier will recompute.
use super::mission_bridge;
use odd_propulsion::{
    nozzle, solve_engine, EngineSolution, EngineSpec, Propellant, PropulsionError, Station,
};
use std::sync::Mutex;

/// Solution field layout, read with `mission_engine_field`.
/// 0 thrust N, 1 Isp s, 2 mass flow kg/s, 3 c* m/s, 4 Tc K, 5 molar mass kg/mol,
/// 6 gamma, 7 bulk density, 8 expansion ratio, 9 exit Mach, 10 exit pressure,
/// 11 exit temperature, 12 divergence factor, 13 nozzle length,
/// 14 throat flux W/m2, 15 wall temperature K, 16 film coefficient,
/// 17 engine dry mass kg, 18 pump power W, 19 chamber radius, 20 chamber length,
/// 21 separation x/L (NaN attached), 22 separation area ratio, 23 side load kN,
/// 24 thermal limit flag, 25 exhaust velocity, 26 ambient pressure,
/// 27 flight ready, 28 O/F, 29 chamber pressure, 30 throat radius,
/// 31 exit radius, 32 length fraction, 33 propellant id, 34 stage dry mass,
/// 35 stage propellant, 36 stage wet mass, 37 volume limited.
pub const SOLUTION_FIELDS: usize = 38;
const STATIONS: usize = 96;

struct Lab {
    spec: EngineSpec,
    solution: EngineSolution,
    stations: Vec<Station>,
    ambient: f64,
}

static LAB: Mutex<Option<Lab>> = Mutex::new(None);

fn spec_from(
    propellant: u32,
    of_ratio: f64,
    p_c_pa: f64,
    r_throat_m: f64,
    r_exit_m: f64,
    length_fraction: f64,
) -> Option<EngineSpec> {
    Some(EngineSpec {
        propellant: Propellant::from_u8(u8::try_from(propellant).ok()?)?,
        of_ratio,
        p_c_pa,
        r_throat_m,
        r_exit_m,
        length_fraction,
    })
}

/// Solve a design for the test stand. Returns 1 on success, 0 for a bad
/// propellant id, or a negative `PropulsionError` code:
/// -1 mixture ratio, -2 chamber pressure, -3 geometry, -4 numerical,
/// -5 over the throat thermal limit (still solved and readable).
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn mission_engine_solve(
    propellant: u32,
    of_ratio: f64,
    p_c_pa: f64,
    r_throat_m: f64,
    r_exit_m: f64,
    length_fraction: f64,
    ambient_pa: f64,
) -> i32 {
    *LAB.lock().unwrap() = None;
    let Some(spec) = spec_from(
        propellant,
        of_ratio,
        p_c_pa,
        r_throat_m,
        r_exit_m,
        length_fraction,
    ) else {
        return 0;
    };
    let ambient = if ambient_pa.is_finite() && ambient_pa >= 0.0 {
        ambient_pa
    } else {
        return -4;
    };
    let solution = match solve_engine(&spec, ambient) {
        Ok(solution) => solution,
        Err(PropulsionError::MixtureRatioOutOfRange) => return -1,
        Err(PropulsionError::ChamberPressureOutOfRange) => return -2,
        Err(PropulsionError::GeometryOutOfRange) => return -3,
        Err(PropulsionError::NumericalRange) => return -4,
    };
    let stations = nozzle::stations(
        spec.geometry(),
        &odd_propulsion::thermochem::chamber(spec.propellant, spec.of_ratio, spec.p_c_pa).unwrap(),
        STATIONS,
    )
    .unwrap_or_default();
    let over = solution.thermal_limit_exceeded;
    *LAB.lock().unwrap() = Some(Lab {
        spec,
        solution,
        stations,
        ambient,
    });
    if over {
        -5
    } else {
        1
    }
}

/// Ambient pressure of the test stand at an altitude, so the slider and the
/// flight drag model agree on the atmosphere.
#[no_mangle]
pub extern "C" fn mission_engine_ambient(altitude_m: f64) -> f64 {
    nozzle::ambient_pressure(altitude_m)
}

#[no_mangle]
pub extern "C" fn mission_engine_field_count() -> u32 {
    SOLUTION_FIELDS as u32
}

#[no_mangle]
pub extern "C" fn mission_engine_field(index: u32) -> f64 {
    let lab = LAB.lock().unwrap();
    let Some(lab) = lab.as_ref() else {
        return f64::NAN;
    };
    let s = &lab.solution;
    let separated = s.separation;
    match index {
        0 => s.thrust_n,
        1 => s.specific_impulse_s,
        2 => s.mass_flow_kg_s,
        3 => s.c_star_m_s,
        4 => s.chamber_temperature_k,
        5 => s.molar_mass_kg_mol,
        6 => s.gamma,
        7 => s.bulk_density_kg_m3,
        8 => s.expansion_ratio,
        9 => s.exit_mach,
        10 => s.exit_pressure_pa,
        11 => s.exit_temperature_k,
        12 => s.divergence_factor,
        13 => s.nozzle_length_m,
        14 => s.throat_heat_flux_w_m2,
        15 => s.throat_wall_temperature_k,
        16 => s.film_coefficient_w_m2_k,
        17 => s.dry_mass_kg,
        18 => s.pump_power_w,
        19 => s.chamber_radius_m,
        20 => s.chamber_length_m,
        21 => separated.map_or(f64::NAN, |sep| sep.x_norm),
        22 => separated.map_or(f64::NAN, |sep| sep.area_ratio_sep),
        23 => separated.map_or(f64::NAN, |sep| sep.side_load_kn),
        24 => f64::from(u8::from(s.thermal_limit_exceeded)),
        25 => s.exhaust_velocity_m_s,
        26 => lab.ambient,
        27 => f64::from(u8::from(s.flight_ready())),
        28 => lab.spec.of_ratio,
        29 => lab.spec.p_c_pa,
        30 => lab.spec.r_throat_m,
        31 => lab.spec.r_exit_m,
        32 => lab.spec.length_fraction,
        33 => lab.spec.propellant as u8 as f64,
        _ => stage_field(lab, index),
    }
}

/// Stage consequences of the design for the contract currently loaded.
fn stage_field(lab: &Lab, index: u32) -> f64 {
    let Some(contract) = mission_bridge::current_contract() else {
        return f64::NAN;
    };
    let Ok(vehicle) = contract.vehicle(lab.spec) else {
        return f64::NAN;
    };
    match index {
        34 => vehicle.dry_mass_kg,
        35 => vehicle.propellant_kg,
        36 => vehicle.initial_mass_kg,
        37 => f64::from(u8::from(vehicle.volume_limited)),
        _ => f64::NAN,
    }
}

#[no_mangle]
pub extern "C" fn mission_engine_station_count() -> u32 {
    LAB.lock()
        .unwrap()
        .as_ref()
        .map_or(0, |lab| lab.stations.len() as u32)
}

/// 0 x/L, 1 radius, 2 area ratio, 3 Mach, 4 pressure, 5 temperature,
/// 6 density, 7 velocity.
#[no_mangle]
pub extern "C" fn mission_engine_station(index: u32, field: u32) -> f64 {
    let lab = LAB.lock().unwrap();
    let Some(station) = lab
        .as_ref()
        .and_then(|lab| lab.stations.get(index as usize))
    else {
        return f64::NAN;
    };
    match field {
        0 => station.x_norm,
        1 => station.radius_m,
        2 => station.area_ratio,
        3 => station.mach,
        4 => station.pressure_pa,
        5 => station.temperature_k,
        6 => station.density_kg_m3,
        7 => station.velocity_m_s,
        _ => f64::NAN,
    }
}

/// Fit the solved design to the loaded contract and clear any compiled plan:
/// a new engine changes mass, thrust and Isp, so old nodes are meaningless.
/// Returns 1 on success, 0 when nothing is solved or no contract is loaded,
/// -1 when the design cannot fly (thermal limit or no thrust).
#[no_mangle]
pub extern "C" fn mission_engine_apply() -> i32 {
    let spec = {
        let lab = LAB.lock().unwrap();
        let Some(lab) = lab.as_ref() else { return 0 };
        lab.spec
    };
    mission_bridge::apply_engine_design(spec)
}

/// Load a contract's baseline design into the lab so the panel opens showing
/// the engine the stage actually carries.
#[no_mangle]
pub extern "C" fn mission_engine_baseline() -> i32 {
    let Some(spec) = mission_bridge::current_engine_spec() else {
        return 0;
    };
    mission_engine_solve(
        spec.propellant as u8 as u32,
        spec.of_ratio,
        spec.p_c_pa,
        spec.r_throat_m,
        spec.r_exit_m,
        spec.length_fraction,
        0.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lab_solves_reads_back_and_applies() {
        let _guard = mission_bridge::test_lock();
        assert_eq!(mission_bridge::mission_contract(101), 1);
        assert_eq!(mission_engine_baseline(), 1);
        assert_eq!(mission_engine_field_count(), SOLUTION_FIELDS as u32);
        let baseline_thrust = mission_engine_field(0);
        assert!(
            (baseline_thrust - 2396.776).abs() < 0.5,
            "{baseline_thrust}"
        );
        assert!(
            mission_engine_field(21).is_nan(),
            "vacuum must not separate"
        );
        assert_eq!(mission_engine_station_count(), STATIONS as u32);
        assert_eq!(mission_engine_station(0, 3), 1.0, "throat is sonic");
        assert!(mission_engine_station(STATIONS as u32 - 1, 3) > 4.0);
        assert!(mission_engine_field(35) > 0.0, "stage propellant");
        assert_eq!(mission_engine_apply(), 1);

        // Sea level on a big bell separates; the field says where.
        let code = mission_engine_solve(1, 2.36, 5.0e6, 0.05, 0.05 * 8.944, 0.8, 101_325.0);
        assert_eq!(code, 1);
        assert!((0.1..0.9).contains(&mission_engine_field(21)));
        assert!(mission_engine_field(23) > 0.0);

        // A design outside the envelope is refused by name, not clamped.
        assert_eq!(mission_engine_solve(0, 30.0, 20e6, 0.1, 0.5, 0.8, 0.0), -1);
        assert!(mission_engine_field(0).is_nan());
        assert_eq!(mission_engine_station_count(), 0);
    }
}
