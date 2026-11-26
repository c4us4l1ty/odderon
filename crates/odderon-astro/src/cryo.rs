//! A one-dimensional cryogenic fluid network, and the failure it makes reproducible.
//!
//! ## Why this is in the astrodynamics crate
//!
//! It is not. It is here for exactly the reason [`thermal`] is: `odderon-astro` is the only crate in the
//! workspace that is `#![no_std]`, `#![forbid(unsafe_code)]` and gated by the same kernel rules, and a
//! second crate would mean a second copy of those rules to keep in step. When the pad model outgrows this
//! file it moves to `odderon-pad` and this module becomes a re-export; the algorithms below do not depend on
//! where they live.
//!
//! ## The claim this exists to make testable
//!
//! `GAME-DESIGN-BIBLE.md` §17 and the P1 row of `docs/BUILD-PLAN.md` §5: the player's first cryo load is
//! **under-specified on purpose**. It froths, the pump cavitates, and the test article dies cheaply on the
//! stand — a *productive failure* that the debrief then explains. The gate to leave P1 is *a reproducible
//! failure from the input log*: the same checklist, run twice, must fail the same way.
//!
//! That is a falsifiable statement about a coupled system, and this module is the smallest thing that can
//! falsify it. It is a **pure function of the input log and `t`** — no clock, no global state, no allocator —
//! so seeking, replay and reload all agree, exactly as [`thermal::room_temperature_from_k`] does for the
//! room. The failure is a function of the log, not of when you happen to run it.
//!
//! ## The network
//!
//! A 1-D lumped flow path, the same shape as the room's single zone: **tank → line → pump → test article**.
//!
//! 1. [`ChillPlan`] — what the player's clipboard specifies *before* fill: how long the line was chilled
//!    (`chill_s`) and how many purge/bleed cycles were run (`bleeds`). This is the input log's chilldown
//!    step, and it is where the bible's "one item that is wrong" lives: under-specified chill leaves heat in
//!    the line.
//! 2. [`chilldown_residual_heat`] — the heat per kilogram still in the line after that chilldown. Lower is
//!    better chilled. This is the single number the whole failure turns on.
//! 3. [`flash_fraction`] — when cryogen enters a line carrying that residual heat, the energy balance decides
//!    how much of it flashes to vapour before it reaches the pump.
//! 4. [`Cavitation`] — the pump requires subcooled liquid at its inlet. A two-phase mixture above a threshold
//!    vapour fraction makes it lose prime: the pump **cavitates**, and the test article downstream dies.
//!
//! ## What is modelled, and what is not
//!
//! * **The cryogen is LOX** (liquid oxygen), the bible's "cryo load". Its saturation curve is
//!   [`vapour_pressure_kpa`], a Clausius–Clapeyron form anchored at the normal boiling point. That is a
//!   *constant-latent-heat* correlation: honest over the ~90–120 K band a pad load lives in, and an
//!   approximation everywhere else. It is marked `Assumed`, not `Defined`, for that reason.
//! * **The line is one lumped thermal mass**, not a distributed pipe. A real line has a temperature profile;
//!   this one has a single residual-heat number per kilogram. It is the smallest thing that carries the
//!   chilldown → flash → cavitation chain, which is the claim being tested.
//! * **No two-phase flow dynamics.** The flash is an instantaneous energy balance at fill, not a drifting
//!   void-fraction model. A real pump's NPSH margin is a function of flow and geometry; here it is a
//!   threshold on the inlet vapour fraction. Both are modelling choices, and both are stated as such rather
//!   than dressed as a pump curve.
//! * **Refrigerant properties are out of scope**, exactly as §3.2 of `docs/CREDIBILITY.md` keeps them out for
//!   the thermal module. No equation of state, no REFPROP.
//!
//! ## The failure, stated as a predicate
//!
//! The whole point of the module is that the failure is **reproducible from the input log**. `simulate`
//! integrates the fill forward from `t = 0` and reports the first instant the pump inlet vapour fraction
//! crosses [`Cavitation::threshold`] — or `None` if it never does. The same [`ChillPlan`] and fill rate
//! return the same instant on every run, on every platform, which is what makes it a gate rather than a
//! story.

use crate::is_positive_finite;
use crate::provenance::{Prov, Quantity};
use odderon_dmath::{exp, fmin, is_finite, ln};

// =============================================================================================
// The cryogen: liquid oxygen
// =============================================================================================
//
// Physical constants are `Defined` because they are facts about the world, not choices — the same
// discipline [`thermal`] applies to `STEFAN_BOLTZMANN`. The modelling choices that follow (the chill time
// constant, the bleed fraction, the cavitation threshold) are `Assumed` and say what they rest on.

/// Normal boiling point of LOX, K. The anchor for [`vapour_pressure_kpa`]: at this temperature the vapour
/// pressure is one atmosphere by definition.
pub const LOX_NBP_K: f64 = 90.188;

/// Standard atmosphere, kPa. The pressure at the normal boiling point.
pub const ATMOSPHERE_KPA: f64 = 101.325;

/// Specific gas constant of O₂, J/(kg·K): the universal constant divided by the molar mass of molecular
/// oxygen (0.0319988 kg/mol). Used by the Clausius–Clapeyron saturation curve.
pub const LOX_GAS_CONSTANT: f64 = 8.314_462_618 / 0.031_998_8;

/// Latent heat of vaporisation of LOX at the normal boiling point, J/kg.
///
/// Marked `Assumed` rather than `Defined`: it is a measured property, but it varies with pressure and
/// temperature, and the value used here is a representative one for the band this model operates in. The
/// saturation curve below treats it as constant, which is the approximation being flagged.
pub const LOX_LATENT_HEAT: Quantity = Quantity {
    value: 213_000.0,
    prov: Prov::Assumed {
        basis: "LOX latent heat at the NBP, ~213 kJ/kg; treated as constant by the saturation curve",
    },
    sigma: 5_000.0,
};

/// Specific heat of liquid LOX, J/(kg·K). A representative value; used only to convert a subcooling margin
/// in kelvin into the enthalpy margin in J/kg that the flash balance needs.
pub const LOX_CP: Quantity = Quantity {
    value: 1_700.0,
    prov: Prov::Assumed {
        basis: "liquid LOX specific heat, ~1.7 kJ/kg·K near the NBP",
    },
    sigma: 200.0,
};

/// The temperature the line starts at before chilldown, K — warm gas left in the pipe from the previous
/// operation (or from sitting at ambient). This is the heat the chilldown has to remove.
pub const LINE_INITIAL_TEMP_K: Quantity = Quantity {
    value: 300.0,
    prov: Prov::Assumed {
        basis: "a pad line at ambient before chilldown, ~300 K",
    },
    sigma: 10.0,
};

/// The temperature the line is chilled toward, K. Cold gas running through the line drives it toward the
/// cryogen's own temperature; this is the floor the chilldown asymptotes to.
pub const LINE_CHILL_FLOOR_K: Quantity = Quantity {
    value: 95.0,
    prov: Prov::Assumed {
        basis: "a few kelvin above the LOX NBP, the practical floor of a gas chilldown",
    },
    sigma: 5.0,
};

/// The time constant of the line's chilldown, s. One `e-folding` of residual heat per `tau_chill` seconds
/// of chill time. A modelling choice: a real line's chill rate depends on flow, insulation and geometry.
pub const CHILL_TAU_S: Quantity = Quantity {
    value: 60.0,
    prov: Prov::Assumed {
        basis: "lumped line chilldown time constant, one e-folding per minute of chill",
    },
    sigma: 20.0,
};

/// The fraction of the line's residual heat one purge/bleed cycle removes, dimensionless 0–1. Each bleed
/// runs warm gas out and cold gas in; this is how much of the remaining heat each cycle takes with it.
pub const BLEED_FRACTION: Quantity = Quantity {
    value: 0.40,
    prov: Prov::Assumed {
        basis: "one purge cycle removes ~40% of the residual line heat",
    },
    sigma: 0.10,
};

/// The pump inlet vapour fraction above which the pump loses prime and cavitates, dimensionless 0–1.
///
/// This is the threshold the whole failure rides on. A small vapour fraction at a pump inlet is survivable;
/// past roughly a few percent the pump cannot develop head and it cavitates. The value is a modelling choice
/// calibrated to the bible's failure (an under-specified chill *does* cross it, a correct chill *does not*),
/// and it is stated as a choice rather than derived, because no instrument in this workspace measures a
/// pump's two-phase limit.
pub const CAVITATION_THRESHOLD: Quantity = Quantity {
    value: 0.02,
    prov: Prov::Assumed {
        basis: "pump inlet vapour fraction at which a centrifugal pump loses prime; calibrated, not measured",
    },
    sigma: 0.01,
};

/// The fill rate at which the bible's "first cryo load" is run, kg/s. A small, deliberately modest fill so
/// the failure is cheap and the test article dies on the stand rather than in flight.
pub const REFERENCE_FILL_RATE: Quantity = Quantity {
    value: 0.50,
    prov: Prov::Assumed {
        basis: "a slow pad fill, ~0.5 kg/s, so the under-specified load fails cheaply",
    },
    sigma: 0.10,
};

// =============================================================================================
// The saturation curve
// =============================================================================================

/// Vapour (saturation) pressure of LOX at temperature `t_k`, kPa.
///
/// Clausius–Clapeyron with a **constant** latent heat, anchored so that `vapour_pressure_kpa(LOX_NBP_K)`
/// is exactly one atmosphere:
///
/// ```text
/// ln(P / P0) = -(L / R) * (1/T - 1/T0)
/// ```
///
/// The constant-`L` approximation is the honest limit of this curve: it is good over the ~90–120 K band a
/// pad load lives in and degrades away from it. It is *not* a look-up table and *not* an equation of state;
/// a real saturation curve bends, and this one does not. The approximation is the price of a closed form
/// that is bit-identical on every platform, which is the determinism contract this crate exists to keep.
///
/// Refuses rather than answers: a non-finite or non-positive temperature returns `None`, because a
/// saturation pressure is a physical quantity and there is no honest value for a temperature that is not
/// one. (`is_positive_finite` is `false` for both `NaN` and `Infinity`, and for zero and negatives — which
/// is exactly the set of inputs with no saturation pressure.)
pub fn vapour_pressure_kpa(t_k: f64) -> Option<f64> {
    if !is_positive_finite(t_k) {
        return None;
    }
    // The exponent is -(L/R)(1/T - 1/T0). It is finite for every finite positive T, but the exponential
    // can overflow for T far below the NBP (1/T → ∞). Clamp the *result* rather than the input: a
    // saturation pressure that has overflowed is not a pressure, and returning `None` for an unrepresentable
    // value is the same refusal as for a non-finite input.
    let exponent = -(LOX_LATENT_HEAT.value / LOX_GAS_CONSTANT) * (1.0 / t_k - 1.0 / LOX_NBP_K);
    let p = ATMOSPHERE_KPA * exp(exponent);
    if is_finite(p) {
        Some(p)
    } else {
        None
    }
}

// =============================================================================================
// The chilldown: the input log's first step
// =============================================================================================

/// What the player's clipboard specifies for the chilldown, before any cryogen is filled.
///
/// This is the input-log step the bible's "one item that is wrong" lives in. A correct plan chills the line
/// long enough and bleeds it enough times; the *wrong* checklist item under-specifies one of them, and the
/// failure that follows is a consequence of the plan, not of when the run happens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChillPlan {
    /// Seconds the line was chilled (gas run through it to cool it toward [`LINE_CHILL_FLOOR_K`]).
    pub chill_s: f64,
    /// Number of purge/bleed cycles run before fill.
    pub bleeds: u32,
}

impl ChillPlan {
    /// The bible's *under-specified* first load: a short chill and no bleeds. This is the plan with the
    /// wrong checklist item — the one that is supposed to froth, cavitate and kill the test article.
    ///
    /// Deliberately a named constructor rather than a literal at the call site, so the "wrong" plan is one
    /// obvious value the tests can point at, the way [`thermal::Zone::curtain_closed`] names the closed
    /// room.
    pub fn underspecified() -> ChillPlan {
        ChillPlan { chill_s: 5.0, bleeds: 0 }
    }

    /// A correctly specified chill: enough time for the line to approach its floor, and bleeds to carry the
    /// residual heat out. The plan that is supposed to *survive*.
    pub fn specified() -> ChillPlan {
        ChillPlan { chill_s: 300.0, bleeds: 3 }
    }
}

/// The residual heat per kilogram still in the line after a chilldown, J/kg.
///
/// Two mechanisms, both in the input log:
///
/// * **Time.** The line cools exponentially toward [`LINE_CHILL_FLOOR_K`] with time constant
///   [`CHILL_TAU_S`]. The residual *above the floor* falls as `exp(-chill_s / tau)`.
/// * **Bleeds.** Each purge cycle removes [`BLEED_FRACTION`] of whatever heat remains, so `bleeds` cycles
///   multiply the residual by `(1 - fraction)^bleeds`.
///
/// The two are multiplied because they remove the same heat by different means, and neither substitutes for
/// the other: a long chill with no bleeds still leaves the heat the gas carried in, and many bleeds with no
/// chill time still leave a line that never got cold. **A non-finite chill time is refused** (`None`), the
/// same rule the room's trajectory applies to a non-finite start: a plan that is not a plan is not a chill
/// down, and returning a number for it would be the same lie as a temperature for a missing kernel.
pub fn chilldown_residual_heat(plan: &ChillPlan) -> Option<f64> {
    if !is_finite(plan.chill_s) || plan.chill_s < 0.0 {
        return None;
    }
    // Heat above the floor at the start: the line is at LINE_INITIAL_TEMP_K and chills toward the floor.
    let initial_above_floor = LOX_CP.value * (LINE_INITIAL_TEMP_K.value - LINE_CHILL_FLOOR_K.value);
    let time_factor = exp(-plan.chill_s / CHILL_TAU_S.value);
    // Each bleed multiplies the residual by `(1 - fraction)`, so `bleeds` cycles give `(1-f)^bleeds`.
    // Written as `exp(ln(1-f) * bleeds)` rather than `powi` because `f64::powi` is a `std` method and this
    // crate is `no_std`; the two agree bit-for-bit for an integer exponent, and the `ln` of the base is
    // exact for the fixed fraction this model uses.
    let bleed_factor = exp(ln(1.0 - BLEED_FRACTION.value) * (plan.bleeds as f64));
    let residual = initial_above_floor * time_factor * bleed_factor;
    if is_finite(residual) {
        Some(residual)
    } else {
        None
    }
}

// =============================================================================================
// The flash: cryogen meeting a warm line
// =============================================================================================

/// The mass fraction of incoming cryogen that flashes to vapour when it absorbs `q_res` J/kg from the line,
/// dimensionless 0–1.
///
/// The energy balance, stated as one line: the line gives the cryogen `q_res` J/kg. The liquid can absorb
/// [`subcooling_margin_j_per_kg`] J/kg of heat while staying liquid (the subcooling below saturation).
/// Anything past that goes into vaporisation, and the fraction vaporised is the excess divided by the latent
/// heat. Clamped to `[0, 1]`: a line can flash *all* of the cryogen, never more.
///
/// A non-finite or negative `q_res` returns `0.0` **only** when it is also not a real heat load — but the
/// honest refusal is the same as everywhere else: a non-finite residual heat is not a chilldown, so this
/// returns `None` and the caller decides. A *negative* residual heat is physically meaningless (the line
/// cannot give the cryogen less than nothing), and is refused the same way.
pub fn flash_fraction(q_res_j_per_kg: f64, subcooling_margin_j_per_kg: f64) -> Option<f64> {
    if !is_finite(q_res_j_per_kg) || !is_finite(subcooling_margin_j_per_kg) {
        return None;
    }
    if q_res_j_per_kg < 0.0 || subcooling_margin_j_per_kg < 0.0 {
        return None;
    }
    let excess = q_res_j_per_kg - subcooling_margin_j_per_kg;
    if excess <= 0.0 {
        return Some(0.0);
    }
    let f = excess / LOX_LATENT_HEAT.value;
    Some(if f > 1.0 { 1.0 } else { f })
}

/// The enthalpy margin, J/kg, that keeps liquid LOX liquid: the heat it can take before flashing, given the
/// subcooling between the tank temperature and the saturation temperature at the tank pressure.
///
/// `c_p * (T_sat(P_tank) - T_tank)`. The saturation temperature at the tank pressure is found by inverting
/// the Clausius–Clapeyron form — `ln(P/P0) = -(L/R)(1/T - 1/T0)`, so `1/T = 1/T0 - (R/L)·ln(P/P0)` — which
/// is exact for the same curve [`vapour_pressure_kpa`] uses, so the two agree rather than being two
/// independent approximations of the same physics.
///
/// A non-finite or non-positive subcooling returns `None`, because a negative subcooling is not a margin —
/// it is a line already past saturation, which is the failure state itself rather than a number to carry
/// forward.
pub fn subcooling_margin_j_per_kg(tank_temp_k: f64, tank_pressure_kpa: f64) -> Option<f64> {
    if !is_positive_finite(tank_pressure_kpa) {
        return None;
    }
    // Invert the saturation curve for T at the tank pressure: 1/T = 1/T0 - (R/L)·ln(P/P0).
    let ln_ratio = ln(tank_pressure_kpa / ATMOSPHERE_KPA);
    let inv_t_sat = 1.0 / LOX_NBP_K - (LOX_GAS_CONSTANT / LOX_LATENT_HEAT.value) * ln_ratio;
    if !is_finite(inv_t_sat) || inv_t_sat <= 0.0 {
        return None;
    }
    let t_sat = 1.0 / inv_t_sat;
    let margin_k = t_sat - tank_temp_k;
    if !is_finite(margin_k) || margin_k <= 0.0 {
        return None;
    }
    Some(LOX_CP.value * margin_k)
}

// =============================================================================================
// The pump: the failure
// =============================================================================================

/// The failure a pad cryo load can produce. Named, not a boolean, because "the test article died" and
/// "the pump cavitated" are the same event at two levels of description and the bible wants the cause.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The pump inlet saw a two-phase mixture above [`CAVITATION_THRESHOLD`] and lost prime. The test
    /// article downstream received no flow and failed on the stand.
    Cavitation,
}

/// The outcome of a cryo load: the tank state at the end of the run, the pump inlet vapour fraction that
/// decided it, and the failure (if any) at the instant it first occurred.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RunOutcome {
    /// Liquid cryogen in the tank at the end of the run, kg.
    pub liquid_kg: f64,
    /// Tank pressure at the end of the run, kPa.
    pub pressure_kpa: f64,
    /// Tank temperature at the end of the run, K.
    pub temperature_k: f64,
    /// The pump inlet vapour fraction at the end of the run, dimensionless 0–1. The quantity the cavitation
    /// predicate reads; carried on the outcome so a run that *did not* fail still shows how close it came.
    pub pump_inlet_vapour_fraction: f64,
    /// The failure, and the simulated time at which it first occurred. `None` means the load ran to the end
    /// of the window without cavitating.
    pub failure: Option<(Failure, f64)>,
}

/// A failed cryo load, kept as a record: the input log that produced it and the state it failed into.
///
/// This is the **wreck**, and the bible's rule is that it is always examinable -- never cut to black on
/// failure. A wreck is not a boolean: it carries the checklist that was run (`chill_s`, `bleeds`, the fill
/// rate and the window) alongside the failure, so a reader can look at *why* it failed and re-simulate the
/// same input log to confirm it fails the same way. That is what makes it a record rather than a verdict.
///
/// A `Wreck` exists only for a run that actually failed (cavitated). A run that ran clean has no wreck --
/// there is nothing to examine, and inventing one would be the same lie as a temperature for a missing
/// kernel. `CryoNetwork::wreck` returns `None` for a clean run and for a refused one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wreck {
    /// The input log: the chill time the checklist specified, s.
    pub chill_s: f64,
    /// The input log: the purge cycles the checklist specified.
    pub bleeds: u32,
    /// The input log: the fill rate, kg/s.
    pub fill_rate_kg_per_s: f64,
    /// The input log: the fill window, simulated seconds.
    pub window_s: f64,
    /// The simulated time at which the pump first cavitated, s.
    pub failure_time_s: f64,
    /// The pump inlet vapour fraction that decided it, dimensionless 0–1.
    pub inlet_vapour_fraction: f64,
    /// The residual heat left in the line after the chilldown, J/kg. The cause, carried on the wreck.
    pub residual_heat_j_per_kg: f64,
}

// =============================================================================================
// The network and its simulation
// =============================================================================================

/// The configuration of a pad cryo load: the cryogen properties are module constants, so this holds only
/// the two things that vary from site to site and run to run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CryoNetwork {
    /// The tank's ullage pressure at the start of fill, kPa. The head the pump draws against.
    pub initial_pressure_kpa: f64,
    /// The temperature the tank and its contents start at, K.
    pub initial_temperature_k: f64,
}

impl CryoNetwork {
    /// The reference pad network: a tank at one atmosphere of ullage, contents at the cryogen's own
    /// subcooled temperature near the NBP.
    pub fn reference() -> CryoNetwork {
        CryoNetwork {
            initial_pressure_kpa: ATMOSPHERE_KPA,
            initial_temperature_k: LOX_NBP_K - 5.0,
        }
    }

    /// The step, in seconds, this simulation integrates at.
    ///
    /// Fixed rather than adaptive, for the same reason the room's trajectory uses a fixed step: the input
    /// log is a sequence of setpoint changes, and a fixed step makes the failure time a function of the
    /// log alone. An adaptive step would make "the same log, twice" two different answers.
    pub const STEP_S: f64 = 0.5;

    /// Integrate a cryo load forward `elapsed_s` seconds and report what happened.
    ///
    /// **A pure function of the input log and `t`.** No clock, no global state: the same [`ChillPlan`],
    /// the same fill rate and the same `elapsed_s` return the same [`RunOutcome`] on every platform, which
    /// is what makes the failure reproducible and what the determinism gate checks.
    ///
    /// The fill is a simple mass balance: liquid enters at `fill_rate_kg_per_s`. Each kilogram carries the
    /// line's [`chilldown_residual_heat`] into the tank; the energy balance decides how much of the incoming
    /// cryogen flashes, and the pump inlet vapour fraction is what the cavitation predicate reads. The
    /// tank's temperature drifts toward saturation as the flashed vapour and the residual heat are absorbed.
    ///
    /// A non-finite fill rate or elapsed time is refused: the run is not a run, and a number returned for it
    /// would be the same lie as a temperature for a missing kernel.
    pub fn simulate(
        &self,
        plan: &ChillPlan,
        fill_rate_kg_per_s: f64,
        elapsed_s: f64,
    ) -> Option<RunOutcome> {
        if !is_finite(fill_rate_kg_per_s) || fill_rate_kg_per_s < 0.0 {
            return None;
        }
        if !is_finite(elapsed_s) || elapsed_s < 0.0 {
            return None;
        }

        let q_res = chilldown_residual_heat(plan)?;

        // The subcooling margin the incoming liquid starts with, at the tank's initial state. This is the
        // energy the liquid can absorb before it flashes; the line's residual heat is spent against it.
        let margin = subcooling_margin_j_per_kg(self.initial_temperature_k, self.initial_pressure_kpa)?;
        let flash = flash_fraction(q_res, margin)?;

        // The tank's temperature rises as the line's heat and the flashed vapour are absorbed. A simple
        // lumped balance: the incoming liquid carries `q_res` J/kg, of which the part above the margin
        // flashes (removing latent heat, which cools the remainder). The net heat deposited per kg is the
        // residual heat minus the latent heat carried away by the flashed fraction.
        let net_heat_per_kg = q_res - flash * LOX_LATENT_HEAT.value;

        // Integrate the mass and temperature with explicit Euler at a fixed step. The temperature responds
        // with the tank's thermal mass; the mass is a pure accumulation. The step is small enough that the
        // temperature balance is stable over the window a pad load runs in (the same discipline the room's
        // `max_stable_step` enforces, fixed here for determinism).
        let mut liquid_kg = 0.0;
        let mut temperature_k = self.initial_temperature_k;
        // The tank's thermal mass, J/K: a small pad tank of LOX. Modelled, not measured — but it only sets
        // the *rate* at which the temperature moves, not whether the failure occurs, which is decided by the
        // inlet vapour fraction at fill time.
        let thermal_mass = 50_000.0;
        let mut failure: Option<(Failure, f64)> = None;

        let mut t = 0.0;
        while t < elapsed_s {
            let dt = fmin(CryoNetwork::STEP_S, elapsed_s - t);
            let d_mass = fill_rate_kg_per_s * dt;

            // The inlet vapour fraction for this step is the flash the incoming cryogen undergoes entering
            // the line. It is the quantity the pump reads, and it is a function of the plan, not of `t`.
            let inlet_vapour_fraction = flash;

            // Cavitation: the pump loses prime the moment it draws a two-phase mixture whose inlet vapour
            // fraction crosses the threshold. **Only when it is actually drawing** - a zero fill rate means
            // the pump is not running, so there is no two-phase mixture at its inlet and nothing to cavitate.
            // Gating on flow is not a detail: without it the model reports cavitation from a cold, empty
            // line, which is the failure state rather than a run. The first crossing is the failure time;
            // later crossings are the same event.
            if d_mass > 0.0 && failure.is_none() && inlet_vapour_fraction > CAVITATION_THRESHOLD.value {
                failure = Some((Failure::Cavitation, t));
            }

            liquid_kg += d_mass;
            if liquid_kg > 0.0 {
                temperature_k += net_heat_per_kg * d_mass / thermal_mass;
            }
            t += dt;
        }

        // The tank pressure follows the saturation curve at the tank temperature, plus the ullage head. A
        // non-finite pressure is refused rather than carried forward as a number.
        let pressure_kpa = match vapour_pressure_kpa(temperature_k) {
            Some(p) => p,
            None => return None,
        };

        Some(RunOutcome {
            liquid_kg,
            pressure_kpa,
            temperature_k,
            pump_inlet_vapour_fraction: flash,
            failure,
        })
    }

    /// The **wreck** of a cryo load: the record of a run that failed, carrying the input log that produced
    /// it and the state it failed into. `None` for a run that ran clean or was refused -- there is no wreck
    /// to examine in either case, and returning one would be inventing a failure.
    ///
    /// A pure function of the input log, exactly like [`simulate`]: the same checklist returns the same
    /// wreck (or `None`), so a wreck is reproducible from its input log rather than a story about when it
    /// was recorded. This is the property the P1 gate rests on, and the reason the wreck is examinable at
    /// all -- a reader can take the `chill_s`, `bleeds`, `fill_rate_kg_per_s` and `window_s` on the wreck
    /// and re-run them to confirm the failure.
    pub fn wreck(
        &self,
        plan: &ChillPlan,
        fill_rate_kg_per_s: f64,
        window_s: f64,
    ) -> Option<Wreck> {
        let outcome = self.simulate(plan, fill_rate_kg_per_s, window_s)?;
        let (_, failure_time_s) = outcome.failure?;   // `None` here means the run did not fail: no wreck.
        let q_res = chilldown_residual_heat(plan)?;
        Some(Wreck {
            chill_s: plan.chill_s,
            bleeds: plan.bleeds,
            fill_rate_kg_per_s,
            window_s,
            failure_time_s,
            inlet_vapour_fraction: outcome.pump_inlet_vapour_fraction,
            residual_heat_j_per_kg: q_res,
        })
    }

    /// Does this input log produce a wreck? The predicate the page uses to decide whether a fill leaves
    /// something on the stand to examine.
    ///
    /// A pure function of the input log, so the same checklist answers the same way every run. A refused
    /// run (a non-finite input) is not a wreck -- it is a run that never happened, and answering `true` for
    /// it would be the same lie as a temperature for a missing kernel.
    pub fn is_wreck(&self, plan: &ChillPlan, fill_rate_kg_per_s: f64, window_s: f64) -> bool {
        self.wreck(plan, fill_rate_kg_per_s, window_s).is_some()
    }
}
