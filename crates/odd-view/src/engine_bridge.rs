//! Scalar boundary for the Engine CAD & Schlieren lab.
//!
//! The browser never computes engine performance. It sends six design
//! numbers plus an ambient pressure, and reads back the solved field and the
//! axial flow stations Rust produced. Applying a design to the mission goes
//! through `odd_contracts::Contract::vehicle`, so the stage mass, thrust and
//! specific impulse used by the propagator are the same ones the recorder
//! will sign and a verifier will recompute.
use super::mission_bridge;
use odd_propulsion::plume as nozzle_plume;
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
    plume: nozzle_plume::Plume,
}

/// Schlieren raster, regenerated only when the design or the view changes.
struct Raster {
    width: usize,
    height: usize,
    values: Vec<f64>,
}

static LAB: Mutex<Option<Lab>> = Mutex::new(None);
static RASTER: Mutex<Option<Raster>> = Mutex::new(None);

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
    *RASTER.lock().unwrap() = None;
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
    let thermo =
        odd_propulsion::thermochem::chamber(spec.propellant, spec.of_ratio, spec.p_c_pa).unwrap();
    let plume = nozzle_plume::analyze(&spec, &solution, &thermo, ambient);
    let over = solution.thermal_limit_exceeded;
    *LAB.lock().unwrap() = Some(Lab {
        spec,
        solution,
        stations,
        ambient,
        plume,
    });
    *RASTER.lock().unwrap() = None;
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

/// Plume summary, read with `mission_engine_plume`.
/// 0 regime (0 adapted, 1 underexpanded, 2 overexpanded, 3 separated),
/// 1 pressure ratio Pe/Pamb, 2 Prandtl-Meyer lip half-angle rad,
/// 3 lip oblique-shock wave angle rad (NaN without one), 4 inward deflection
/// rad, 5 fully expanded jet Mach, 6 first shock-cell length m,
/// 7 Mach-disk standoff m (NaN unless strongly underexpanded),
/// 8 free-jet origin x from the throat m, 9 origin radius m,
/// 10 origin pressure Pa, 11 jet density at the origin, 12 ambient density.
#[no_mangle]
pub extern "C" fn mission_engine_plume(index: u32) -> f64 {
    let lab = LAB.lock().unwrap();
    let Some(lab) = lab.as_ref() else {
        return f64::NAN;
    };
    let p = &lab.plume;
    match index {
        0 => match p.regime {
            nozzle_plume::PlumeRegime::Adapted => 0.0,
            nozzle_plume::PlumeRegime::Underexpanded => 1.0,
            nozzle_plume::PlumeRegime::Overexpanded => 2.0,
            nozzle_plume::PlumeRegime::Separated => 3.0,
        },
        1 => p.pressure_ratio,
        2 => p.lip_turn_rad,
        3 => p.lip_shock_rad,
        4 => p.lip_deflection_rad,
        5 => p.jet_mach,
        6 => p.cell_length_m,
        7 => p.mach_disk_m,
        8 => p.origin_x_m,
        9 => p.origin_radius_m,
        10 => p.origin_pressure_pa,
        11 => p.origin_density_kg_m3,
        12 => p.ambient_density_kg_m3,
        _ => f64::NAN,
    }
}

/// Free-jet shear-layer radius at `s` metres past the detachment/exit plane.
#[no_mangle]
pub extern "C" fn mission_engine_plume_boundary(s_m: f64) -> f64 {
    LAB.lock()
        .unwrap()
        .as_ref()
        .map_or(f64::NAN, |lab| lab.plume.boundary_radius(s_m))
}

/// Density on the axis plane, for the continuous Mach/density readout.
#[no_mangle]
pub extern "C" fn mission_engine_density(x_m: f64, r_m: f64) -> f64 {
    LAB.lock()
        .unwrap()
        .as_ref()
        .map_or(f64::NAN, |lab| lab.plume.density_at(x_m, r_m))
}

/// Render the Schlieren field into a scalar raster. Returns the pixel count,
/// or 0 when nothing is solved. The browser paints these values directly; it
/// never differentiates a fabricated field of its own.
#[no_mangle]
pub extern "C" fn mission_engine_schlieren(
    width: u32,
    height: u32,
    x_min_m: f64,
    x_max_m: f64,
    r_max_m: f64,
) -> u32 {
    let lab = LAB.lock().unwrap();
    let Some(lab) = lab.as_ref() else {
        return 0;
    };
    if !x_min_m.is_finite()
        || !x_max_m.is_finite()
        || !r_max_m.is_finite()
        || x_max_m <= x_min_m
        || r_max_m <= 0.0
    {
        return 0;
    }
    let width = (width as usize).clamp(2, 512);
    let height = (height as usize).clamp(2, 512);
    let values = lab
        .plume
        .schlieren(width, height, x_min_m, x_max_m, r_max_m);
    let len = values.len() as u32;
    *RASTER.lock().unwrap() = Some(Raster {
        width,
        height,
        values,
    });
    len
}

#[no_mangle]
pub extern "C" fn mission_engine_schlieren_width() -> u32 {
    RASTER
        .lock()
        .unwrap()
        .as_ref()
        .map_or(0, |r| r.width as u32)
}

#[no_mangle]
pub extern "C" fn mission_engine_schlieren_height() -> u32 {
    RASTER
        .lock()
        .unwrap()
        .as_ref()
        .map_or(0, |r| r.height as u32)
}

#[no_mangle]
pub extern "C" fn mission_engine_schlieren_at(index: u32) -> f64 {
    RASTER
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|r| r.values.get(index as usize))
        .copied()
        .unwrap_or(f64::NAN)
}

/// Diagnose the solved design through the same odd-app causal DAG the flight
/// warnings use, appending to that registry. Returns the combined count, so
/// the browser reads every chain back with the existing causal accessors.
#[no_mangle]
pub extern "C" fn mission_engine_diagnose() -> u32 {
    // Build the inputs inside the lock, then release it before touching the
    // flight causal registry: the two statics must never be held together.
    let inputs = {
        let guard = LAB.lock().unwrap();
        let Some(lab) = guard.as_ref() else {
            return mission_bridge::causal_count();
        };
        let s = &lab.solution;
        odd_app::causal::EngineInputs {
            altitude_m: 0.0,
            ambient_pa: lab.ambient,
            exit_pressure_pa: s.exit_pressure_pa,
            separation_x_norm: s.separation.map_or(f64::NAN, |sep| sep.x_norm),
            side_load_kn: s.separation.map_or(0.0, |sep| sep.side_load_kn),
            wall_temperature_k: s.throat_wall_temperature_k,
            wall_limit_k: odd_propulsion::thermal_mass::MAX_WALL_TEMPERATURE_K,
            throat_heat_flux_w_m2: s.throat_heat_flux_w_m2,
            chamber_temperature_k: s.chamber_temperature_k,
            of_ratio: lab.spec.of_ratio,
            stoichiometric_of: lab.spec.propellant.stoichiometric_of(),
            peak_temperature_of: lab.spec.propellant.peak_temperature_of(),
        }
    };
    mission_bridge::append_causal(odd_app::causal::diagnose_engine(&inputs))
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
        assert!(mission_engine_plume(0).is_nan());
    }

    #[test]
    fn the_schlieren_raster_shows_the_detachment_and_the_cell_train() {
        let _guard = mission_bridge::test_lock();
        // Small bell at sea level: strongly underexpanded, so cells and a
        // Mach disk. The raster must be bounded and carry real structure.
        assert_eq!(
            mission_engine_solve(1, 2.36, 5.0e6, 0.05, 0.10, 0.8, 101_325.0),
            1
        );
        assert_eq!(mission_engine_plume(0), 1.0, "underexpanded");
        assert!(mission_engine_plume(6) > 0.0, "shock-cell spacing");
        assert!(mission_engine_plume(7) > 0.0, "Mach disk");
        let count = mission_engine_schlieren(64, 32, -0.05, 0.9, 0.35);
        assert_eq!(count, 64 * 32);
        assert_eq!(mission_engine_schlieren_width(), 64);
        assert_eq!(mission_engine_schlieren_height(), 32);
        let mut peak = 0.0f64;
        for i in 0..count {
            let v = mission_engine_schlieren_at(i);
            assert!(v.is_finite() && (0.0..=1.0).contains(&v));
            peak = peak.max(v);
        }
        assert!(peak > 0.5, "the field must reach full scale somewhere");
        assert!(mission_engine_schlieren_at(count).is_nan());

        // A big bell at the same altitude detaches inside the nozzle, and the
        // free jet starts exactly at the computed separation line.
        assert_eq!(
            mission_engine_solve(1, 2.36, 5.0e6, 0.05, 0.05 * 8.944_271_91, 0.8, 101_325.0),
            1
        );
        assert_eq!(mission_engine_plume(0), 3.0, "separated");
        let length = mission_engine_field(13);
        assert!((mission_engine_plume(8) - mission_engine_field(21) * length).abs() < 1e-12);
        assert!(mission_engine_plume(3) > 0.0, "lip shock, not a fan");
        // Changing the design invalidates the previous raster.
        assert_eq!(mission_engine_schlieren_width(), 0);
    }
}
