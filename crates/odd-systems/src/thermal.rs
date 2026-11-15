//! Lumped single-node tank thermal model, integrated with the shared DP5(4).
//!
//! ```text
//! C_liq(T) dT/dt = Q_in(t) − Q_cryo(t) − Q_vent(t)
//! ```
//!
//! `Q_in` is the sum of MLI radiation, absorbed sunlight (shadowed by the real
//! conical eclipse), planet IR over the visible solid angle, and support-strut
//! conduction. Nothing is orbit-averaged.
//!
//! Two regimes, separated by an exactly localized event:
//!
//! * **Self-pressurization** (valve shut): net heat raises the liquid
//!   temperature and therefore its saturation pressure. No propellant leaves.
//! * **Venting** (relief reached): temperature is pinned at `T_sat(P_relief)`
//!   and *all* net heat vaporizes liquid, `ṁ = Q_net/h_vap`. That mass is lost.
//!
//! The onset is bracketed on accepted steps and localized by bisection with
//! fresh Runge–Kutta trials — the same convention the orbital integrator uses
//! for state events — so the vent timestamp is reproducible bit-for-bit and
//! can be signed.

use crate::environment::{ThermalEnvironment, SPACE_TEMPERATURE_K};
use crate::fluid::{Fluid, FluidProperties};
use odd_astro::dp45;
use odd_astro::integrator::{IntegrationError, Options};
use odd_astro::spline::TrajectorySpline;
use odd_core::consts::{MOLAR_GAS_CONSTANT_J_MOL_K, STEFAN_BOLTZMANN_W_M2_K4};

/// Reference pressure for the fluid's normal boiling point, Pa.
pub const NBP_PRESSURE_PA: f64 = 101_325.0;
/// Emittance of one aluminized-mylar radiation shield.
const SHIELD_EMISSIVITY: f64 = 0.05;

/// Tank shell material. Wire ids are part of the signed thermal spec.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellMaterial {
    Aluminium2219 = 0,
    Stainless304 = 1,
    CompositeOverwrap = 2,
}

impl ShellMaterial {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Aluminium2219),
            1 => Some(Self::Stainless304),
            2 => Some(Self::CompositeOverwrap),
            _ => None,
        }
    }
    pub fn density_kg_m3(self) -> f64 {
        match self {
            Self::Aluminium2219 => 2_840.0,
            Self::Stainless304 => 8_030.0,
            Self::CompositeOverwrap => 1_600.0,
        }
    }
    /// Allowable hoop stress, Pa.
    pub fn allowable_stress_pa(self) -> f64 {
        match self {
            Self::Aluminium2219 => 290e6,
            Self::Stainless304 => 205e6,
            Self::CompositeOverwrap => 400e6,
        }
    }
    /// Through-thickness conductivity, W/(m·K), near the cold end.
    pub fn conductivity_w_m_k(self) -> f64 {
        match self {
            Self::Aluminium2219 => 120.0,
            Self::Stainless304 => 14.0,
            // A composite overwrap carries load but its fibre path is a poor
            // heat short compared with metal.
            Self::CompositeOverwrap => 5.0,
        }
    }
}

/// Ideal radiation-only effective emittance of an `n`-layer blanket.
///
/// `n` floating shields between two surfaces divide the radiative exchange by
/// `n+1`. Real installations always do worse because of seams, penetrations
/// and spacer conduction, so this is a **lower bound** the verifier checks a
/// claimed `epsilon_eff` against — a record cannot claim fewer layers than its
/// own emittance implies.
pub fn ideal_epsilon_for_layers(layers: u32) -> f64 {
    SHIELD_EMISSIVITY / (f64::from(layers) + 1.0)
}

/// Everything about a tank that a designer chooses. This is the physical
/// provenance the signed record carries; performance is never stored.
#[derive(Clone, Copy, Debug)]
pub struct TankSpec {
    pub fluid: Fluid,
    /// Internal volume, m³.
    pub volume_m3: f64,
    /// External radiating area, m².
    pub area_m2: f64,
    /// Blanket shield count.
    pub mli_layers: u32,
    /// Installed effective emittance (≥ the ideal for `mli_layers`).
    pub epsilon_eff: f64,
    pub shell: ShellMaterial,
    /// Solar (short-wave) absorptivity of the blanket's outer surface.
    pub absorptivity: f64,
    /// Thermal (long-wave) emittance of that same outer surface. Independent
    /// of `absorptivity`: a silvered-tefllon skin has low alpha and high
    /// epsilon, which is exactly what keeps a sunlit tank cold.
    pub outer_epsilon: f64,
    /// Lumped support-strut and penetration conductance, W/K.
    pub strut_conductance_w_k: f64,
    /// Relief-valve set pressure, Pa.
    pub relief_pressure_pa: f64,
    /// Cryocooler hot-side radiator area, m².
    pub radiator_area_m2: f64,
    pub radiator_epsilon: f64,
    /// Commanded heat lift at the tank, W. Zero disables active cooling.
    pub cryocooler_lift_w: f64,
    /// Achieved fraction of Carnot COP for the machine.
    pub cryocooler_carnot_fraction: f64,
}

impl TankSpec {
    pub fn validate(&self) -> Result<(), IntegrationError> {
        let ok = self.volume_m3.is_finite()
            && self.volume_m3 > 0.0
            && self.area_m2.is_finite()
            && self.area_m2 > 0.0
            && self.epsilon_eff.is_finite()
            && self.epsilon_eff > 0.0
            && self.epsilon_eff <= 1.0
            && self.absorptivity.is_finite()
            && (0.0..=1.0).contains(&self.absorptivity)
            && self.outer_epsilon.is_finite()
            && self.outer_epsilon > 0.0
            && self.outer_epsilon <= 1.0
            && self.strut_conductance_w_k.is_finite()
            && self.strut_conductance_w_k >= 0.0
            && self.relief_pressure_pa.is_finite()
            && self.relief_pressure_pa >= NBP_PRESSURE_PA
            && self.radiator_area_m2.is_finite()
            && self.radiator_area_m2 >= 0.0
            && self.radiator_epsilon.is_finite()
            && (0.0..=1.0).contains(&self.radiator_epsilon)
            && self.cryocooler_lift_w.is_finite()
            && self.cryocooler_lift_w >= 0.0
            && self.cryocooler_carnot_fraction.is_finite()
            && self.cryocooler_carnot_fraction > 0.0
            && self.cryocooler_carnot_fraction <= 1.0
            && (self.cryocooler_lift_w == 0.0 || self.radiator_area_m2 > 0.0);
        if !ok {
            return Err(IntegrationError::InvalidInput);
        }
        // A blanket cannot beat its own shield count.
        if self.epsilon_eff < ideal_epsilon_for_layers(self.mli_layers) * (1.0 - 1e-12) {
            return Err(IntegrationError::InvalidInput);
        }
        Ok(())
    }

    pub fn properties(&self) -> FluidProperties {
        self.fluid.properties()
    }

    /// Saturation pressure at `temperature_k`, Pa, by Clausius–Clapeyron
    /// integrated from the normal boiling point with constant latent heat.
    pub fn saturation_pressure_pa(&self, temperature_k: f64) -> f64 {
        let p = self.properties();
        let scale = p.heat_of_vaporization_j_kg * p.molar_mass_kg_mol / MOLAR_GAS_CONSTANT_J_MOL_K;
        NBP_PRESSURE_PA
            * libm::exp(-scale * (1.0 / temperature_k - 1.0 / p.saturation_temperature_k))
    }

    /// Liquid temperature whose saturation pressure is `pressure_pa`, K.
    pub fn saturation_temperature_k(&self, pressure_pa: f64) -> f64 {
        let p = self.properties();
        let scale = p.heat_of_vaporization_j_kg * p.molar_mass_kg_mol / MOLAR_GAS_CONSTANT_J_MOL_K;
        1.0 / (1.0 / p.saturation_temperature_k - libm::log(pressure_pa / NBP_PRESSURE_PA) / scale)
    }

    /// Pressure-vessel shell mass, kg, from hoop stress at the relief setting.
    pub fn shell_mass_kg(&self) -> f64 {
        // Thin-wall sphere: t = P·r/(2σ), m = 4πr²·t·ρ, with r from volume.
        let r = libm::cbrt(3.0 * self.volume_m3 / (4.0 * std::f64::consts::PI));
        let t = self.relief_pressure_pa * r / (2.0 * self.shell.allowable_stress_pa());
        4.0 * std::f64::consts::PI * r * r * t * self.shell.density_kg_m3() * 1.35
    }
}

/// Cryocooler operating point: electrical draw, achieved COP and the hot-side
/// rejection temperature. Solved by a fixed 40-pass iteration so the answer
/// never depends on a convergence history.
#[derive(Clone, Copy, Debug)]
pub struct CryocoolerPoint {
    pub lift_w: f64,
    pub power_w: f64,
    pub cop: f64,
    pub hot_side_k: f64,
}

/// Solve the coupled radiator/COP balance for a commanded lift.
///
/// `P = Q/COP`, `COP = f·T_c/(T_h−T_c)`, and `T_h` follows from rejecting
/// `Q+P` through the radiator. The three are implicit; iterate deterministically.
pub fn cryocooler_point(
    spec: &TankSpec,
    cold_k: f64,
    commanded_lift_w: f64,
    available_w: f64,
) -> CryocoolerPoint {
    cryocooler_point_with_load(spec, cold_k, commanded_lift_w, available_w, 0.0)
}

/// Minimum approach temperature across the cooler's internal heat
/// exchangers, K. Without a floor the hot-side iteration can collapse onto
/// the cold tip when the radiator is oversized for the load, which drives
/// `COP = f·T_c/(T_h−T_c)` to infinity and makes active cooling free.
pub const MIN_COOLER_APPROACH_K: f64 = 5.0;

/// As `cryocooler_point`, but the radiator also absorbs its environment.
///
/// A radiator in sunlight or above a planet cannot sit at the tank
/// temperature: it absorbs solar and planet-IR flux, which sets a floor on
/// its rejection temperature and therefore a ceiling on the achievable COP.
/// Ignoring that load is what let a big radiator produce free cooling.
pub fn cryocooler_point_with_load(
    spec: &TankSpec,
    cold_k: f64,
    commanded_lift_w: f64,
    available_w: f64,
    absorbed_w: f64,
) -> CryocoolerPoint {
    if commanded_lift_w <= 0.0 || spec.radiator_area_m2 <= 0.0 || available_w <= 0.0 {
        return CryocoolerPoint {
            lift_w: 0.0,
            power_w: 0.0,
            cop: 0.0,
            hot_side_k: SPACE_TEMPERATURE_K,
        };
    }
    let rejection = spec.radiator_epsilon * STEFAN_BOLTZMANN_W_M2_K4 * spec.radiator_area_m2;
    // Start from the Carnot ceiling against a 300 K sink, then relax.
    let mut hot = 300.0_f64;
    let mut lift = commanded_lift_w;
    let mut power = 0.0;
    let mut cop = 0.0;
    for _ in 0..40 {
        let span = (hot - cold_k).max(1e-6);
        cop = spec.cryocooler_carnot_fraction * cold_k / span;
        power = lift / cop;
        // The bus is a hard limit: throttle the lift to what it can supply.
        if power > available_w {
            power = available_w;
            lift = power * cop;
        }
        let rejected = lift + power + libm::fmax(0.0, absorbed_w);
        let next = libm::pow(
            rejected / rejection + libm::pow(SPACE_TEMPERATURE_K, 4.0),
            0.25,
        );
        hot = next.max(cold_k + MIN_COOLER_APPROACH_K);
    }
    CryocoolerPoint {
        lift_w: lift,
        power_w: power,
        cop,
        hot_side_k: hot,
    }
}

/// Instantaneous heat flows into a tank, W.
#[derive(Clone, Copy, Debug)]
pub struct HeatBalance {
    /// Heat actually reaching the liquid through the blanket, W.
    pub mli_w: f64,
    /// Sunlight absorbed by the outer skin, W (diagnostic).
    pub solar_w: f64,
    /// Planet IR absorbed by the outer skin, W (diagnostic).
    pub earth_ir_w: f64,
    /// Quasi-steady outer-skin temperature, K.
    pub skin_temperature_k: f64,
    pub strut_w: f64,
    /// Heat lifted out of the propellant, W (thermal, at the cold tip).
    pub cryocooler_w: f64,
    /// Electricity the cooler draws from the bus, W. Always >= the lift, and
    /// at 20 K against a ~300 K sink it is one to two orders of magnitude
    /// larger. This is the number that steals from electric propulsion.
    pub cryocooler_bus_w: f64,
    /// Total in minus active cooling. Positive means the tank gains energy.
    pub net_w: f64,
    pub illumination: f64,
}

/// Net heat into the tank at `temperature_k`. `structure_k` is the warm end of
/// the support struts. `available_w` bounds the cryocooler's electrical draw.
pub fn heat_balance(
    spec: &TankSpec,
    temperature_k: f64,
    env: &ThermalEnvironment,
    structure_k: f64,
    sink_k: f64,
    available_w: f64,
) -> HeatBalance {
    let sigma = STEFAN_BOLTZMANN_W_M2_K4;
    let illumination = env.illumination();
    // A sphere presents a quarter of its surface to a collimated beam.
    let solar = spec.absorptivity * (spec.area_m2 * 0.25) * env.solar_flux_w_m2() * illumination;
    // Planet IR arrives over the visible disc; the same quarter-area factor
    // applies to a convex tank whose elements face many directions.
    let earth_ir = spec.outer_epsilon * spec.area_m2 * 0.25 * env.earth_ir_flux_w_m2();
    // Quasi-steady blanket skin: absorbed external flux = re-radiation to the
    // sink + leakage through the blanket to the liquid. Solving for T_skin^4
    // is closed-form, so no iteration and no convergence history.
    //
    //   a = eps_out (T_skin^4 - T_sink^4) + eps_eff (T_skin^4 - T_liq^4)
    //
    // This is why a sunlit MLI tank boils off a few percent a day rather than
    // flashing off: the skin runs near 200 K and re-radiates almost everything
    // it absorbs, and only eps_eff of the imbalance reaches the propellant.
    let absorbed = (solar + earth_ir) / (sigma * spec.area_m2);
    let skin4 = (absorbed
        + spec.outer_epsilon * libm::pow(sink_k, 4.0)
        + spec.epsilon_eff * libm::pow(temperature_k, 4.0))
        / (spec.outer_epsilon + spec.epsilon_eff);
    let skin_temperature_k = libm::pow(libm::fmax(skin4, 0.0), 0.25);
    let mli = spec.epsilon_eff * sigma * spec.area_m2 * (skin4 - libm::pow(temperature_k, 4.0));
    // Struts and penetrations bypass the blanket entirely.
    let strut = spec.strut_conductance_w_k * (structure_k - temperature_k);
    // A real cooler is thermostatically modulated: it lifts only what it must
    // to hold the setpoint, never enough to drive the liquid below its own
    // cold-tip temperature. Without this clamp a 2 kW cooler on a 20 W load
    // would refrigerate the tank towards absolute zero and then report the
    // full nameplate draw forever.
    let passive_w = mli + strut;
    let commanded = if spec.cryocooler_lift_w > 0.0 && passive_w > 0.0 {
        libm::fmin(spec.cryocooler_lift_w, passive_w)
    } else {
        0.0
    };
    // The radiator sees the same sky as the tank. A panel is normally
    // mounted edge-on to the Sun, so it takes a quarter of the direct beam
    // and half of the planet's IR disc.
    let radiator_absorbed_w = spec.absorptivity
        * spec.radiator_area_m2
        * (0.25 * env.solar_flux_w_m2() * illumination + 0.5 * env.earth_ir_flux_w_m2());
    let cooler = cryocooler_point_with_load(
        spec,
        temperature_k,
        commanded,
        available_w,
        radiator_absorbed_w,
    );
    HeatBalance {
        mli_w: mli,
        solar_w: solar,
        earth_ir_w: earth_ir,
        skin_temperature_k,
        strut_w: strut,
        cryocooler_w: cooler.lift_w,
        cryocooler_bus_w: cooler.power_w,
        // Solar and planet IR enter *through* the skin node, so they are
        // already inside `mli_w` and must not be added a second time.
        net_w: mli + strut - cooler.lift_w,
        illumination,
    }
}

/// A vent opening, localized by bisection. `time_us` is the exact microsecond
/// the relief setting was reached, so a verifier reproduces it identically.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VentEvent {
    pub time_us: i64,
    pub tank: u8,
    pub pressure_pa: f64,
    pub temperature_k: f64,
}

/// Integrated tank state.
#[derive(Clone, Copy, Debug)]
pub struct TankState {
    pub temperature_k: f64,
    pub liquid_kg: f64,
    pub vented_kg: f64,
}

/// Outcome of integrating one tank along a trajectory.
#[derive(Clone, Debug)]
pub struct TankHistory {
    pub state: TankState,
    pub vents: Vec<VentEvent>,
    /// Time-averaged boil-off rate, kg/s.
    pub mean_boil_off_kg_s: f64,
    /// Electricity drawn from the bus over the window, J. This is `∫P dt`
    /// with `P = Q_lift/COP`, NOT the heat lifted.
    pub cryocooler_energy_j: f64,
    pub peak_pressure_pa: f64,
    /// Samples along the flight for the UI trend, `(t, T, liquid_kg)`.
    pub trend: Vec<(f64, f64, f64)>,
}

/// The flight time window and the tolerances the tank ODE is stepped with.
/// `start_s`/`end_s` are flight seconds, never wall time.
#[derive(Clone, Copy, Debug)]
pub struct ThermalClock {
    pub start_s: f64,
    pub end_s: f64,
    pub options: Options,
}

/// The boundary conditions a tank is integrated against.
#[derive(Clone, Copy, Debug)]
pub struct ThermalContext {
    /// Warm end of the support struts, K.
    pub structure_temperature_k: f64,
    /// Radiative sink the blanket's outer skin sees, K.
    pub sink_temperature_k: f64,
    /// Electrical power the cooler may draw, W.
    pub bus_w: f64,
}

/// Integrate one tank across `clock`.
///
/// `position` is sampled from the trajectory spline by the caller's
/// `environment` closure, so the thermal state advances on the **flight's**
/// clock and never on wall time. `bus_w` is the power available to the cooler.
pub fn integrate_tank<F>(
    spec: &TankSpec,
    tank_id: u8,
    initial: TankState,
    clock: ThermalClock,
    context: ThermalContext,
    mut environment: F,
) -> Result<TankHistory, IntegrationError>
where
    F: FnMut(f64) -> Result<ThermalEnvironment, IntegrationError>,
{
    spec.validate()?;
    let ThermalClock {
        start_s,
        end_s,
        options,
    } = clock;
    let ThermalContext {
        structure_temperature_k: structure_k,
        sink_temperature_k: sink_k,
        bus_w,
    } = context;
    if !end_s.is_finite()
        || end_s < start_s
        || !initial.temperature_k.is_finite()
        || initial.temperature_k <= 0.0
        || !initial.liquid_kg.is_finite()
        || initial.liquid_kg < 0.0
        || options.max_steps == 0
    {
        return Err(IntegrationError::InvalidInput);
    }
    let props = spec.properties();
    // Storables never reach their relief setting in this model: they are
    // ambient fluids. Integrate their (tiny) sensible drift but never vent.
    let ventable = props.cryogenic;
    let vent_temperature = spec.saturation_temperature_k(spec.relief_pressure_pa);
    if !vent_temperature.is_finite() || vent_temperature <= 0.0 {
        return Err(IntegrationError::NumericalRange);
    }

    let mut t = start_s;
    let mut y = [initial.temperature_k, initial.liquid_kg, initial.vented_kg];
    let mut venting = ventable && y[0] >= vent_temperature;
    let mut vents: Vec<VentEvent> = Vec::new();
    if venting {
        // Already at the relief isotherm when integration starts: the valve is
        // open from the first instant, and that belongs in the signed history.
        vents.push(VentEvent {
            time_us: libm::round(t * 1e6) as i64,
            tank: tank_id,
            pressure_pa: spec.relief_pressure_pa,
            temperature_k: vent_temperature,
        });
    }
    let mut trend = vec![(t, y[0], y[1])];
    let mut cooler_energy = 0.0;
    let mut peak_pressure = spec.saturation_pressure_pa(y[0]);
    let mut h = options.initial_step.min(options.max_step);
    let mut attempts = 0_usize;

    // Regime-dependent right-hand side. It takes the environment closure by
    // reborrow so each DP trial borrows it only for the duration of the trial.
    // In the venting regime temperature is pinned and every net watt
    // vaporizes liquid.
    let derivative = |time: f64,
                      z: [f64; 3],
                      vent: bool,
                      env_fn: &mut F|
     -> Result<[f64; 3], IntegrationError> {
        if z[1] <= 0.0 {
            // Dry tank: no liquid left to heat or boil.
            return Ok([0.0, 0.0, 0.0]);
        }
        let env = env_fn(time)?;
        let balance = heat_balance(spec, z[0], &env, structure_k, sink_k, bus_w);
        if vent {
            let rate = libm::fmax(0.0, balance.net_w) / props.heat_of_vaporization_j_kg;
            let rate = libm::fmin(rate, z[1]);
            Ok([0.0, -rate, rate])
        } else {
            let capacity = z[1] * props.liquid_specific_heat_j_kg_k;
            Ok([balance.net_w / capacity, 0.0, 0.0])
        }
    };

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
            // Detect relief onset on the accepted step, then bisect with fresh
            // trials so the recorded timestamp does not depend on step size.
            if ventable && !venting && next[0] >= vent_temperature {
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
                        |time, z| derivative(time, z, false, &mut environment),
                    )?;
                    if z[0] >= vent_temperature {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                let width = hi - t;
                let (z, _) = dp45::trial(
                    t,
                    y,
                    width,
                    options.relative_tolerance,
                    options.absolute_tolerance,
                    |time, z| derivative(time, z, false, &mut environment),
                )?;
                t = hi;
                y = z;
                // Pin exactly at the relief isotherm and open the valve.
                y[0] = vent_temperature;
                venting = true;
                vents.push(VentEvent {
                    time_us: libm::round(t * 1e6) as i64,
                    tank: tank_id,
                    pressure_pa: spec.relief_pressure_pa,
                    temperature_k: vent_temperature,
                });
                trend.push((t, y[0], y[1]));
                h = options.initial_step.min(options.max_step);
                continue;
            }
            let env = environment(next_t)?;
            let balance = heat_balance(spec, next[0], &env, structure_k, sink_k, bus_w);
            cooler_energy += balance.cryocooler_bus_w * h;
            // While venting, a net-negative balance closes the valve again.
            if venting && balance.net_w < 0.0 {
                venting = false;
            }
            t = next_t;
            y = next;
            if y[1] < 0.0 {
                y[1] = 0.0;
            }
            peak_pressure = libm::fmax(peak_pressure, spec.saturation_pressure_pa(y[0]));
            trend.push((t, y[0], y[1]));
        }
        h *= dp45::controller(err);
    }

    let elapsed = end_s - start_s;
    Ok(TankHistory {
        state: TankState {
            temperature_k: y[0],
            liquid_kg: y[1],
            vented_kg: y[2],
        },
        vents,
        mean_boil_off_kg_s: if elapsed > 0.0 { y[2] / elapsed } else { 0.0 },
        cryocooler_energy_j: cooler_energy,
        peak_pressure_pa: peak_pressure,
        trend,
    })
}

/// Convenience: integrate a tank along a cached trajectory, sampling position
/// from the spline. The caller supplies the body vectors for that position.
pub fn integrate_along<F>(
    spec: &TankSpec,
    tank_id: u8,
    initial: TankState,
    trajectory: &TrajectorySpline,
    clock: ThermalClock,
    context: ThermalContext,
    mut body_vectors: F,
) -> Result<TankHistory, IntegrationError>
where
    F: FnMut(f64, [f64; 3]) -> Result<ThermalEnvironment, IntegrationError>,
{
    let start = trajectory
        .keyframe(0)
        .ok_or(IntegrationError::InvalidInput)?
        .time;
    let clock = ThermalClock {
        start_s: start,
        ..clock
    };
    integrate_tank(spec, tank_id, initial, clock, context, |t| {
        let frame = trajectory
            .sample(t)
            .map_err(|_| IntegrationError::ForceFailure)?;
        body_vectors(t, frame.position)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::ThermalEnvironment;
    use odd_core::consts::ASTRONOMICAL_UNIT_M;

    fn lh2_tank() -> TankSpec {
        TankSpec {
            fluid: Fluid::Lh2,
            volume_m3: 100.0 / 70.85,
            area_m2: 4.0,
            mli_layers: 20,
            epsilon_eff: 0.03,
            shell: ShellMaterial::Aluminium2219,
            absorptivity: 0.2,
            outer_epsilon: 0.8,
            strut_conductance_w_k: 0.0,
            relief_pressure_pa: NBP_PRESSURE_PA,
            radiator_area_m2: 0.0,
            radiator_epsilon: 0.8,
            cryocooler_lift_w: 0.0,
            cryocooler_carnot_fraction: 0.25,
        }
    }

    fn sun_only() -> ThermalEnvironment {
        ThermalEnvironment {
            sun: [ASTRONOMICAL_UNIT_M, 0.0, 0.0],
            earth: [0.0; 3],
            moon: [0.0; 3],
        }
    }

    fn options() -> Options {
        Options {
            relative_tolerance: 1e-10,
            absolute_tolerance: 1e-9,
            initial_step: 30.0,
            max_step: 120.0,
            max_steps: 400_000,
        }
    }

    #[test]
    fn saturation_round_trips_through_clausius_clapeyron() {
        let spec = lh2_tank();
        let p = spec.saturation_pressure_pa(20.27);
        assert!((p - NBP_PRESSURE_PA).abs() < 1e-6, "{p}");
        for pressure in [101_325.0, 200_000.0, 350_000.0] {
            let t = spec.saturation_temperature_k(pressure);
            assert!((spec.saturation_pressure_pa(t) - pressure).abs() / pressure < 1e-12);
        }
        // LH2 at 2 bar boils near 23 K.
        let t = spec.saturation_temperature_k(200_000.0);
        assert!((22.0..25.0).contains(&t), "{t}");
    }

    /// The analytic steady state is `ṁ = Q_net/h_vap`. With the tank already at
    /// relief, an integration must reproduce that quotient, not approximate it.
    #[test]
    fn steady_vent_rate_is_the_net_heat_over_latent_heat() {
        let spec = lh2_tank();
        let sink = 300.0;
        let balance = heat_balance(&spec, 20.27, &sun_only(), sink, sink, 0.0);
        let expected = balance.net_w / 445_600.0;
        let history = integrate_tank(
            &spec,
            0,
            TankState {
                temperature_k: 20.27,
                liquid_kg: 100.0,
                vented_kg: 0.0,
            },
            ThermalClock {
                start_s: 0.0,
                end_s: 86_400.0,
                options: options(),
            },
            ThermalContext {
                structure_temperature_k: sink,
                sink_temperature_k: sink,
                bus_w: 0.0,
            },
            |_| Ok(sun_only()),
        )
        .unwrap();
        let rate = history.state.vented_kg / 86_400.0;
        assert!(
            (rate - expected).abs() / expected < 1e-9,
            "{rate} vs {expected}"
        );
        assert_eq!(history.vents.len(), 1);
        assert_eq!(history.vents[0].time_us, 0);
    }

    /// A closed tank must pressurize instead of losing mass, and the valve must
    /// open at a localized, step-size-independent microsecond.
    #[test]
    fn a_closed_tank_pressurizes_then_vents_at_a_reproducible_instant() {
        let mut spec = lh2_tank();
        spec.relief_pressure_pa = 120_000.0;
        let initial = TankState {
            temperature_k: 20.27,
            liquid_kg: 100.0,
            vented_kg: 0.0,
        };
        let mut times = Vec::new();
        for max_step in [60.0, 120.0, 240.0] {
            let mut opts = options();
            opts.max_step = max_step;
            let h = integrate_tank(
                &spec,
                0,
                initial,
                ThermalClock {
                    start_s: 0.0,
                    end_s: 20_000.0,
                    options: opts,
                },
                ThermalContext {
                    structure_temperature_k: 300.0,
                    sink_temperature_k: 300.0,
                    bus_w: 0.0,
                },
                |_| Ok(sun_only()),
            )
            .unwrap();
            assert_eq!(h.vents.len(), 1, "one opening per run");
            assert!(h.peak_pressure_pa >= spec.relief_pressure_pa * (1.0 - 1e-9));
            times.push(h.vents[0].time_us);
        }
        assert!(
            times.windows(2).all(|w| w[0] == w[1]),
            "vent instant moved with step size: {times:?}"
        );
    }

    #[test]
    fn eclipse_cuts_the_absorbed_sun_to_zero() {
        let spec = lh2_tank();
        let lit = heat_balance(&spec, 20.27, &sun_only(), 300.0, 300.0, 0.0);
        let shadow = ThermalEnvironment {
            sun: [ASTRONOMICAL_UNIT_M, 0.0, 0.0],
            earth: [crate::environment::EARTH_RADIUS_M + 400_000.0, 0.0, 0.0],
            moon: [0.0; 3],
        };
        let dark = heat_balance(&spec, 20.27, &shadow, 300.0, 300.0, 0.0);
        assert!(lit.solar_w > 0.0);
        assert!(dark.solar_w < 1e-12);
        assert!(dark.earth_ir_w > 0.0, "planet IR still arrives in eclipse");
    }

    #[test]
    fn a_blanket_cannot_claim_more_layers_than_its_emittance_allows() {
        let mut spec = lh2_tank();
        // One shield cannot deliver a 100-layer emittance.
        spec.mli_layers = 1;
        spec.epsilon_eff = ideal_epsilon_for_layers(100);
        assert_eq!(spec.validate().err(), Some(IntegrationError::InvalidInput));
        // A real installation is always worse than ideal, which is allowed.
        spec.epsilon_eff = 0.03;
        assert!(spec.validate().is_ok());
        // Exactly ideal for its own count is the boundary case.
        spec.mli_layers = 200;
        spec.epsilon_eff = ideal_epsilon_for_layers(200);
        assert!(spec.validate().is_ok());
    }

    #[test]
    fn storables_do_not_boil_off() {
        let mut spec = lh2_tank();
        spec.fluid = Fluid::Mmh;
        spec.epsilon_eff = 0.03;
        let h = integrate_tank(
            &spec,
            0,
            TankState {
                temperature_k: 293.0,
                liquid_kg: 100.0,
                vented_kg: 0.0,
            },
            ThermalClock {
                start_s: 0.0,
                end_s: 86_400.0,
                options: options(),
            },
            ThermalContext {
                structure_temperature_k: 293.0,
                sink_temperature_k: 293.0,
                bus_w: 0.0,
            },
            |_| Ok(sun_only()),
        )
        .unwrap();
        assert_eq!(h.state.vented_kg, 0.0);
        assert!(h.vents.is_empty());
    }
}
