//! Coupled two-tank thermal integration with **instantaneous** bus sharing.
//!
//! The previous implementation integrated the oxidizer to the end of the
//! flight, took its *final* compressor draw, and gave the fuel tank a fixed
//! allowance of `capacity − ox_peak` for the whole mission. That is wrong in
//! both directions: the oxidizer's draw varies with sunlight and tank
//! temperature, so the fuel was starved in eclipse (when the bus itself had
//! collapsed) and over-fed in full sun.
//!
//! Here both tanks advance in one adaptive state vector. At every derivative
//! evaluation the bus is resolved in strict priority order:
//!
//! 1. generation `P_gen(t) = array_1au · (flux(t)/flux_1au) · illumination(t)`;
//! 2. the fixed avionics load comes off the top;
//! 3. the oxidizer's cryocooler draws what it needs, bounded by the bus;
//! 4. the fuel gets exactly `max(0, P_bus(t) − P_ox(t))`;
//! 5. only the surplus after both coolers is left for electric propulsion.
//!
//! The allocation is recorded at every accepted step so the priority rule is
//! checkable rather than asserted.

use crate::bus::BusSpec;
use crate::environment::{ThermalEnvironment, SOLAR_CONSTANT_W_M2};
use crate::thermal::{
    heat_balance, TankHistory, TankSpec, TankState, ThermalClock, ThermalContext, VentEvent,
};
use odd_astro::dp45;
use odd_astro::integrator::IntegrationError;

/// Four ODE states per tank: temperature, liquid, vented, cooler joules.
const STRIDE: usize = 4;
const STATES: usize = 2 * STRIDE;

/// The bus split at one instant, as the integrator actually applied it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BusAllocation {
    pub time_s: f64,
    /// Solar generation after distance and shadow, W.
    pub generation_w: f64,
    /// Generation minus the fixed avionics load, W.
    pub bus_w: f64,
    /// What the oxidizer's compressor drew, W.
    pub oxidizer_w: f64,
    /// Exactly `max(0, bus_w − oxidizer_w)`: the fuel's allowance, W.
    pub fuel_available_w: f64,
    /// What the fuel's compressor drew, W.
    pub fuel_w: f64,
    /// Surplus after both coolers, available to electric propulsion, W.
    pub electric_propulsion_w: f64,
    /// Fraction of the solar disc visible.
    pub illumination: f64,
}

/// Split the bus at one instant. Pure, so the oracle can check the rule
/// independently of the integrator that calls it.
pub fn allocate_bus(
    bus: &BusSpec,
    env: &ThermalEnvironment,
    oxidizer_draw_w: f64,
    fuel_draw_w: f64,
) -> BusAllocation {
    let illumination = env.illumination();
    let generation_w =
        bus.array_w_at_1au * (env.solar_flux_w_m2() / SOLAR_CONSTANT_W_M2) * illumination;
    let bus_w = libm::fmax(0.0, generation_w - bus.avionics_w);
    let oxidizer_w = libm::fmin(libm::fmax(0.0, oxidizer_draw_w), bus_w);
    let fuel_available_w = libm::fmax(0.0, bus_w - oxidizer_w);
    let fuel_w = libm::fmin(libm::fmax(0.0, fuel_draw_w), fuel_available_w);
    BusAllocation {
        time_s: f64::NAN,
        generation_w,
        bus_w,
        oxidizer_w,
        fuel_available_w,
        fuel_w,
        electric_propulsion_w: libm::fmax(0.0, fuel_available_w - fuel_w),
        illumination,
    }
}

/// One tank's contribution to the coupled right-hand side.
struct TankRate {
    temperature: f64,
    liquid: f64,
    vented: f64,
    joules: f64,
    draw_w: f64,
    net_w: f64,
}

fn tank_rate(
    spec: &TankSpec,
    temperature_k: f64,
    liquid_kg: f64,
    venting: bool,
    env: &ThermalEnvironment,
    context: &ThermalContext,
    available_w: f64,
) -> TankRate {
    if liquid_kg <= 0.0 {
        // A dry tank has nothing to cool and nothing to boil.
        return TankRate {
            temperature: 0.0,
            liquid: 0.0,
            vented: 0.0,
            joules: 0.0,
            draw_w: 0.0,
            net_w: 0.0,
        };
    }
    let balance = heat_balance(
        spec,
        temperature_k,
        env,
        context.structure_temperature_k,
        context.sink_temperature_k,
        available_w,
    );
    let props = spec.properties();
    let (temperature, liquid, vented) = if venting {
        let rate = libm::fmax(0.0, balance.net_w) / props.heat_of_vaporization_j_kg;
        let rate = libm::fmin(rate, liquid_kg);
        (0.0, -rate, rate)
    } else {
        let capacity = liquid_kg * props.liquid_specific_heat_j_kg_k;
        (balance.net_w / capacity, 0.0, 0.0)
    };
    TankRate {
        temperature,
        liquid,
        vented,
        joules: balance.cryocooler_bus_w,
        draw_w: balance.cryocooler_bus_w,
        net_w: balance.net_w,
    }
}

/// The two tanks and their starting states, grouped so the integrator's
/// signature stays inside the argument budget.
#[derive(Clone, Copy, Debug)]
pub struct TankPair {
    pub oxidizer: TankSpec,
    pub fuel: TankSpec,
    pub oxidizer_initial: TankState,
    pub fuel_initial: TankState,
}

/// Integrate both tanks together, sharing the bus at every substep.
///
/// `context.bus_w` is ignored: the available power is derived from the
/// trajectory's own sunlight and the array's rating, not from a constant.
pub fn integrate_pair<F>(
    pair: &TankPair,
    bus: &BusSpec,
    clock: ThermalClock,
    context: ThermalContext,
    mut environment: F,
) -> Result<(TankHistory, TankHistory, Vec<BusAllocation>), IntegrationError>
where
    F: FnMut(f64) -> Result<ThermalEnvironment, IntegrationError>,
{
    pair.oxidizer.validate()?;
    pair.fuel.validate()?;
    let specs = [&pair.oxidizer, &pair.fuel];
    let ThermalClock {
        start_s,
        end_s,
        options,
    } = clock;
    if !end_s.is_finite()
        || end_s < start_s
        || options.max_steps == 0
        || !bus.array_w_at_1au.is_finite()
        || bus.array_w_at_1au < 0.0
        || !bus.avionics_w.is_finite()
        || bus.avionics_w < 0.0
    {
        return Err(IntegrationError::InvalidInput);
    }
    let initials = [pair.oxidizer_initial, pair.fuel_initial];
    let mut ventable = [false; 2];
    let mut vent_temperature = [0.0_f64; 2];
    let mut y = [0.0_f64; STATES];
    for (i, spec) in specs.iter().enumerate() {
        let initial = initials[i];
        if !initial.temperature_k.is_finite()
            || initial.temperature_k <= 0.0
            || !initial.liquid_kg.is_finite()
            || initial.liquid_kg < 0.0
        {
            return Err(IntegrationError::InvalidInput);
        }
        vent_temperature[i] = spec.saturation_temperature_k(spec.relief_pressure_pa);
        if !vent_temperature[i].is_finite() || vent_temperature[i] <= 0.0 {
            return Err(IntegrationError::NumericalRange);
        }
        ventable[i] = spec.properties().cryogenic;
        y[i * STRIDE] = initial.temperature_k;
        y[i * STRIDE + 1] = initial.liquid_kg;
        y[i * STRIDE + 2] = initial.vented_kg;
        y[i * STRIDE + 3] = 0.0;
    }

    let mut t = start_s;
    let mut venting = [false; 2];
    let mut vents: Vec<VentEvent> = Vec::new();
    let mut trends: [Vec<(f64, f64, f64)>; 2] = [vec![], vec![]];
    let mut peak_pressure = [0.0_f64; 2];
    for (i, spec) in specs.iter().enumerate() {
        venting[i] = ventable[i] && y[i * STRIDE] >= vent_temperature[i];
        if venting[i] {
            vents.push(VentEvent {
                time_us: libm::round(t * 1e6) as i64,
                tank: i as u8,
                pressure_pa: spec.relief_pressure_pa,
                temperature_k: vent_temperature[i],
            });
        }
        trends[i].push((t, y[i * STRIDE], y[i * STRIDE + 1]));
        peak_pressure[i] = spec.saturation_pressure_pa(y[i * STRIDE]);
    }

    // The coupled right-hand side. Priority is resolved inside it, so the
    // fuel's allowance is exact at every trial stage, not interpolated from
    // the oxidizer's history.
    let derivative = |time: f64,
                      z: [f64; STATES],
                      flags: [bool; 2],
                      env_fn: &mut F|
     -> Result<[f64; STATES], IntegrationError> {
        let env = env_fn(time)?;
        let illumination = env.illumination();
        let generation =
            bus.array_w_at_1au * (env.solar_flux_w_m2() / SOLAR_CONSTANT_W_M2) * illumination;
        let bus_w = libm::fmax(0.0, generation - bus.avionics_w);
        let ox = tank_rate(specs[0], z[0], z[1], flags[0], &env, &context, bus_w);
        let fuel_available = libm::fmax(0.0, bus_w - ox.draw_w);
        let fu = tank_rate(
            specs[1],
            z[STRIDE],
            z[STRIDE + 1],
            flags[1],
            &env,
            &context,
            fuel_available,
        );
        let mut out = [0.0; STATES];
        out[0] = ox.temperature;
        out[1] = ox.liquid;
        out[2] = ox.vented;
        out[3] = ox.joules;
        out[STRIDE] = fu.temperature;
        out[STRIDE + 1] = fu.liquid;
        out[STRIDE + 2] = fu.vented;
        out[STRIDE + 3] = fu.joules;
        Ok(out)
    };

    let mut allocations: Vec<BusAllocation> = Vec::new();
    let record = |time: f64,
                  z: &[f64; STATES],
                  flags: [bool; 2],
                  env_fn: &mut F,
                  out: &mut Vec<BusAllocation>|
     -> Result<(), IntegrationError> {
        let env = env_fn(time)?;
        let illumination = env.illumination();
        let generation =
            bus.array_w_at_1au * (env.solar_flux_w_m2() / SOLAR_CONSTANT_W_M2) * illumination;
        let bus_w = libm::fmax(0.0, generation - bus.avionics_w);
        let ox = tank_rate(specs[0], z[0], z[1], flags[0], &env, &context, bus_w);
        let split = allocate_bus(bus, &env, ox.draw_w, 0.0);
        let fu = tank_rate(
            specs[1],
            z[STRIDE],
            z[STRIDE + 1],
            flags[1],
            &env,
            &context,
            split.fuel_available_w,
        );
        let mut split = allocate_bus(bus, &env, ox.draw_w, fu.draw_w);
        split.time_s = time;
        out.push(split);
        Ok(())
    };
    record(t, &y, venting, &mut environment, &mut allocations)?;

    let mut h = options.initial_step.min(options.max_step);
    let mut attempts = 0_usize;
    while t < end_s {
        attempts += 1;
        if attempts > options.max_steps {
            return Err(IntegrationError::StepLimit);
        }
        let remaining = end_s - t;
        h = h.min(options.max_step).min(remaining);
        let next_t = if h == remaining { end_s } else { t + h };
        if next_t <= t {
            return Err(IntegrationError::StepUnderflow);
        }
        h = next_t - t;
        let (next, err) = dp45::trial(
            t,
            y,
            h,
            options.relative_tolerance,
            options.absolute_tolerance,
            |time, z| derivative(time, z, venting, &mut environment),
        )?;
        if err <= 1.0 {
            // Find the earliest relief onset across both tanks on this step
            // and bisect to it, so the recorded microsecond does not depend
            // on the step size the controller happened to choose.
            let mut onset: Option<(usize, f64)> = None;
            for (i, spec) in specs.iter().enumerate() {
                let _ = spec;
                if ventable[i] && !venting[i] && next[i * STRIDE] >= vent_temperature[i] {
                    let (mut lo, mut hi) = (t, next_t);
                    for _ in 0..48 {
                        let mid = lo + (hi - lo) * 0.5;
                        if mid <= lo || mid >= hi {
                            break;
                        }
                        let (z, _) = dp45::trial(
                            t,
                            y,
                            mid - t,
                            options.relative_tolerance,
                            options.absolute_tolerance,
                            |time, z| derivative(time, z, venting, &mut environment),
                        )?;
                        if z[i * STRIDE] >= vent_temperature[i] {
                            hi = mid;
                        } else {
                            lo = mid;
                        }
                    }
                    if onset.is_none_or(|(_, earlier)| hi < earlier) {
                        onset = Some((i, hi));
                    }
                }
            }
            if let Some((i, hi)) = onset {
                let (z, _) = dp45::trial(
                    t,
                    y,
                    hi - t,
                    options.relative_tolerance,
                    options.absolute_tolerance,
                    |time, z| derivative(time, z, venting, &mut environment),
                )?;
                t = hi;
                y = z;
                y[i * STRIDE] = vent_temperature[i];
                venting[i] = true;
                vents.push(VentEvent {
                    time_us: libm::round(t * 1e6) as i64,
                    tank: i as u8,
                    pressure_pa: specs[i].relief_pressure_pa,
                    temperature_k: vent_temperature[i],
                });
                for (k, trend) in trends.iter_mut().enumerate() {
                    trend.push((t, y[k * STRIDE], y[k * STRIDE + 1]));
                }
                record(t, &y, venting, &mut environment, &mut allocations)?;
                h = options.initial_step.min(options.max_step);
                continue;
            }
            t = next_t;
            y = next;
            for (i, spec) in specs.iter().enumerate() {
                if y[i * STRIDE + 1] < 0.0 {
                    y[i * STRIDE + 1] = 0.0;
                }
                peak_pressure[i] =
                    libm::fmax(peak_pressure[i], spec.saturation_pressure_pa(y[i * STRIDE]));
                trends[i].push((t, y[i * STRIDE], y[i * STRIDE + 1]));
            }
            record(t, &y, venting, &mut environment, &mut allocations)?;
            // While venting, a net-negative balance closes the valve again.
            let env = environment(t)?;
            let illumination = env.illumination();
            let generation =
                bus.array_w_at_1au * (env.solar_flux_w_m2() / SOLAR_CONSTANT_W_M2) * illumination;
            let bus_w = libm::fmax(0.0, generation - bus.avionics_w);
            let ox = tank_rate(specs[0], y[0], y[1], venting[0], &env, &context, bus_w);
            let fuel_available = libm::fmax(0.0, bus_w - ox.draw_w);
            let fu = tank_rate(
                specs[1],
                y[STRIDE],
                y[STRIDE + 1],
                venting[1],
                &env,
                &context,
                fuel_available,
            );
            if venting[0] && ox.net_w < 0.0 {
                venting[0] = false;
            }
            if venting[1] && fu.net_w < 0.0 {
                venting[1] = false;
            }
        }
        h *= dp45::controller(err);
    }

    vents.sort_by(|a, b| a.time_us.cmp(&b.time_us).then(a.tank.cmp(&b.tank)));
    let elapsed = end_s - start_s;
    let history = |i: usize| TankHistory {
        state: TankState {
            temperature_k: y[i * STRIDE],
            liquid_kg: y[i * STRIDE + 1],
            vented_kg: y[i * STRIDE + 2],
        },
        vents: vents
            .iter()
            .filter(|v| v.tank == i as u8)
            .copied()
            .collect(),
        mean_boil_off_kg_s: if elapsed > 0.0 {
            y[i * STRIDE + 2] / elapsed
        } else {
            0.0
        },
        cryocooler_energy_j: y[i * STRIDE + 3],
        peak_pressure_pa: peak_pressure[i],
        trend: trends[i].clone(),
    };
    Ok((history(0), history(1), allocations))
}
