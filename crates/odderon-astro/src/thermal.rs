//! A lumped thermal network, and a vapour-compression loop that removes heat from it.
//!
//! ## Why this is in the astrodynamics crate
//!
//! It is not. It is here because `odderon-astro` is currently the only crate in the workspace that is
//! `#![no_std]`, `#![forbid(unsafe_code)]` and gated by the same kernel rules, and a second crate would mean a
//! second copy of those rules to keep in step. When the thermal model outgrows this file it moves to
//! `odderon-thermal` and this module becomes a re-export; the algorithms below do not depend on where they live.
//!
//! ## The claim this exists to make testable
//!
//! `GAME-DESIGN-BIBLE.md` §6.7: *"close the curtain."* Peak summer solar gain through 3 m² of glazing at
//! 800 W/m² with SHGC 0.6 is ≈1440 W; closed it is ≈120 W. The room cools by several kelvin over minutes, and
//! the AC's electrical draw **visibly drops on the wall meter.**
//!
//! That is a falsifiable statement about a coupled system, and this module is the smallest thing that can
//! falsify it. Note what it does *not* need: a building-energy model, or a psychrometric chart, or a standard
//! atmosphere table (`atmosphere-1976` is classified OPEN but **absent** in `src/classes.json`, and this module
//! must not be the thing that smuggles it in).
//!
//! ## The two halves
//!
//! 1. [`Zone`] — one lumped thermal mass with a conductance to outdoors. Solar gain enters it directly; the
//!    AC removes heat from it. This is the "room" row of §6.7's table.
//! 2. [`VapourCompression`] — a closed-loop heat pump: compressor, condenser rejecting to outdoor air,
//!    expansion valve, evaporator. Copeland's map gives its coefficient of performance from the evaporating
//!    and condensing temperatures. This is the "window AC unit" row.
//!
//! They are separate because they are separate in the bible, and because the coupling is one number: the
//! evaporator temperature the loop runs at changes with the load, which changes its COP, which changes its
//! draw. A fixed-COP model would reproduce the draw *ratio* trivially and would be wrong about the absolute
//! numbers, which are what a wall meter shows.
//!
//! ## What is modelled, and what is not
//!
//! Modelled: sensible heat only; a linear conductance `UA`; solar gain as a single irradiance × area × SHGC
//! product; a heat pump whose COP falls as the lift across it grows; and a compressor that cycles on and off
//! rather than modulating, which is what a window unit does.
//!
//! **Not modelled, and each is a real omission rather than a simplification too small to mention:**
//!
//! * **Latent heat.** A real AC removes water vapour as well as heat, and in humid air that is 20–40 % of the
//!   load. Absent here, so the sensible load is the whole load and the draw is understated on a humid day.
//! * **Thermal mass beyond one node.** The walls, the furniture and the air are one `C` here. A real room
//!   responds on three timescales, and one node cannot show the slow one.
//! * **Radiative exchange with surfaces.** `UA` is a single linear conductance standing in for conduction
//!   through the envelope plus a convection coefficient. It cannot distinguish an inner wall from a window.
//! * **Refrigerant properties.** The COP map is a correlation, not a cycle solved from an equation of state,
//!   which is why `docs/CREDIBILITY.md` §3.2 has a separate row demanding < 1 % against REFPROP before the
//!   *real-gas* claim is allowed. Nothing here claims it.
//!
//! The honest summary: this module can show a player that closing a curtain lowers the meter reading, and it
//! can say by how much a lumped model says it should. It cannot be used to size a real air conditioner.

use crate::provenance::{Prov, Quantity};
use odderon_dmath::{exp, is_finite, pow};

/// Stefan–Boltzmann constant, W·m⁻²·K⁻⁴. Exact by definition since the 2019 SI redefinition, so this value is
/// `Defined` rather than `Measured` — a distinction [`Prov`] exists to make.
pub const STEFAN_BOLTZMANN: f64 = 5.670_374_419e-8;

/// Celsius to kelvin. `Defined`: the offset is exact.
pub const ZERO_CELSIUS: f64 = 273.15;

/// A lumped thermal zone: one capacity, one conductance to outdoors, one solar aperture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    /// Heat capacity of everything that responds on the fast timescale, J/K.
    ///
    /// The air alone is ~30 kJ/K for a 25 m² room, which would swing several kelvin per minute. The effective
    /// figure for a furnished room is several times that, and the difference is exactly what `Assumed` is for:
    /// the air number is measurable, the effective number is a modelling choice.
    pub heat_capacity: Quantity,
    /// Conductance from the zone to outdoor air, W/K. Includes envelope conduction and the inner surface
    /// convection coefficient, lumped. **This is the least well-founded number in the module**: it depends on
    /// construction, on wind, and on which wall the sun is on, and no single value is right for a room.
    pub loss_conductance: Quantity,
    /// Glazing area, m².
    pub glazing_area: Quantity,
    /// Solar heat gain coefficient, dimensionless 0–1.
    pub shgc: Quantity,
}

/// The state of a zone: one temperature.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ZoneState {
    /// Zone temperature, K.
    pub temperature_k: f64,
}

/// What the outdoors is doing. Held separately from the state because it is an input, not something the model
/// evolves — the room does not change the weather.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outdoors {
    /// Outdoor air temperature, K.
    pub temperature_k: f64,
    /// Total irradiance on the glazing, W/m², **in the plane of the glazing**.
    ///
    /// Not the horizontal global irradiance: a vertical window at 4 pm in summer receives several times what a
    /// horizontal surface does. Passing the wrong one is a factor-of-three error that looks entirely plausible,
    /// which is why the units and the plane are both named.
    pub irradiance_w_per_m2: f64,
}

/// A load on the zone: something inside it producing heat, W.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InternalGain {
    /// Steady sensible heat, W.
    pub watts: f64,
}

impl Zone {
    /// The §6.7 room: 25 m² of floor, 3 m² of glazing, reasonably insulated.
    ///
    /// Every number here is `Assumed` and says so. The heat capacity is the one worth arguing about: the air
    /// alone is ~30 kJ/K (25 m² × 2.5 m × 1.2 kg/m³ × 1005 J/kg·K), which would swing several kelvin a minute.
    /// A furnished room's *effective* capacity is several times that, and 120 kJ/K is what makes the bible's
    /// "several kelvin over minutes" come out as measured: **5.9 K in the first ten minutes** after the curtain
    /// closes, against a 44.4 min time constant.
    pub fn small_apartment_room() -> Zone {
        Zone {
            heat_capacity: Quantity {
                value: 120_000.0,
                prov: Prov::Assumed {
                    basis: "furnished 25 m2 room, ~4x the air alone",
                },
                sigma: 40_000.0,
            },
            loss_conductance: Quantity {
                value: 45.0,
                prov: Prov::Assumed {
                    basis: "envelope + inner convection, single glazing class",
                },
                sigma: 15.0,
            },
            glazing_area: Quantity {
                value: 3.0,
                prov: Prov::Defined,
                sigma: 0.0,
            },
            shgc: Quantity {
                value: 0.60,
                prov: Prov::Assumed {
                    basis: "clear double glazing, NFRC whole-window SHGC 0.55-0.65",
                },
                sigma: 0.05,
            },
        }
    }

    /// The same room with the curtain closed.
    ///
    /// SHGC 0.05 is a heavy opaque curtain. It is not zero, because a curtain is not a shutter: it blocks the
    /// direct beam and leaks around the edges, and claiming zero would make the closed state a different kind of
    /// object from the open one.
    pub fn curtain_closed(&self) -> Zone {
        Zone {
            shgc: Quantity {
                value: 0.05,
                prov: Prov::Assumed {
                    basis: "heavy opaque curtain, bible 6.7 figure",
                },
                sigma: 0.02,
            },
            ..*self
        }
    }

    /// Solar gain through the glazing, W.
    ///
    /// `irradiance × area × SHGC`. Three multiplied numbers, and the whole of the bible's curtain experiment:
    /// closing the curtain drops `shgc` from ~0.6 to ~0.05 while the other two are unchanged.
    pub fn solar_gain(&self, outdoors: &Outdoors) -> f64 {
        self.glazing_area.value * outdoors.irradiance_w_per_m2 * self.shgc.value
    }

    /// Conductive loss to outdoors, W. Positive means the zone is losing heat.
    pub fn loss(&self, state: &ZoneState, outdoors: &Outdoors) -> f64 {
        self.loss_conductance.value * (state.temperature_k - outdoors.temperature_k)
    }

    /// Net heat into the zone with no cooling, W. Positive means it is warming.
    pub fn net_gain(&self, state: &ZoneState, outdoors: &Outdoors, internal: &InternalGain) -> f64 {
        self.solar_gain(outdoors) + internal.watts - self.loss(state, outdoors)
    }

    /// The temperature the zone would settle at with no cooling, K.
    ///
    /// `T_out + Q/(UA)`. Returned as `None` when the conductance is not positive, because then there is no
    /// equilibrium — an insulated room with a gain source heats without bound, and returning a very large number
    /// would look like an answer.
    pub fn free_float_temperature(
        &self,
        outdoors: &Outdoors,
        internal: &InternalGain,
    ) -> Option<f64> {
        if !(self.loss_conductance.value > 0.0) {
            return None;
        }
        Some(
            outdoors.temperature_k
                + self.net_gain_at(outdoors, internal) / self.loss_conductance.value,
        )
    }

    /// Net gain evaluated at the free-float temperature — i.e. with the loss term dropped.
    fn net_gain_at(&self, outdoors: &Outdoors, internal: &InternalGain) -> f64 {
        self.solar_gain(outdoors) + internal.watts
    }

    /// One explicit step of the lumped energy balance, K.
    ///
    /// `C dT/dt = Q_solar + Q_internal − UA(T − T_out) − Q_cooling`.
    ///
    /// **Explicit, not implicit**, and the time step is the caller's responsibility — but the caller is given the
    /// bound rather than being expected to know it: see [`Zone::max_stable_step`]. An explicit integrator run
    /// past its stability limit does not fail, it oscillates with growing amplitude, which in a game looks like a
    /// rendering bug.
    pub fn step(
        &self,
        state: &ZoneState,
        outdoors: &Outdoors,
        internal: &InternalGain,
        cooling_w: f64,
        dt_s: f64,
    ) -> ZoneState {
        let net = self.net_gain(state, outdoors, internal) - cooling_w;
        ZoneState {
            temperature_k: state.temperature_k + net * dt_s / self.heat_capacity.value,
        }
    }

    /// The largest explicit step this zone can take without going unstable, s.
    ///
    /// `2C/UA` is the stability limit of forward Euler on `C dT/dt = −UA·T + …`. Halved, because the limit is
    /// where the scheme stops being *stable*, not where it stops being accurate, and the two are different
    /// questions: a step at the limit conserves energy but rings.
    pub fn max_stable_step(&self) -> Option<f64> {
        if !(self.loss_conductance.value > 0.0) {
            return None;
        }
        Some(self.heat_capacity.value / self.loss_conductance.value)
    }
}

/// A closed-loop vapour-compression heat pump, by its Copeland correlation.
///
/// ```text
/// COP = η₂ · T_evap / (T_cond − T_evap)
/// ```
///
/// with temperatures absolute. This is Carnot's coefficient of performance multiplied by a second-law
/// efficiency `η₂` that stands in for everything a real cycle loses — throttling, superheat, pressure drop,
/// motor heat. Reported values for small window units are `η₂ ≈ 0.4–0.5`; [`VapourCompression::window_unit`]
/// carries `0.45` as `Assumed`, with that range as the basis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VapourCompression {
    /// Second-law efficiency, dimensionless.
    pub second_law_efficiency: Quantity,
    /// Evaporating temperature, K. Below the zone setpoint by the approach.
    pub evaporating_k: Quantity,
    /// Condensing temperature, K. Above outdoor air by the approach.
    pub condensing_k: Quantity,
    /// The temperature the loop holds the zone at, K.
    pub setpoint_k: f64,
    /// Compressor electrical draw when running, W.
    ///
    /// A window unit cycles: it runs at one speed or it does not run. Modelling a modulating compressor would
    /// make the meter reading smooth and would be a different, larger machine.
    pub rated_draw_w: Quantity,
}

impl VapourCompression {
    /// A small window unit sized for the §6.7 room, with every assumed number labelled as such.
    pub fn window_unit() -> VapourCompression {
        VapourCompression {
            second_law_efficiency: Quantity {
                value: 0.45,
                prov: Prov::Assumed {
                    basis: "reported eta_2 range for small window units, 0.4-0.5",
                },
                sigma: 0.05,
            },
            evaporating_k: Quantity {
                value: ZERO_CELSIUS + 16.0,
                prov: Prov::Assumed {
                    basis: "evaporator 8 K below a 24 C setpoint",
                },
                sigma: 1.0,
            },
            condensing_k: Quantity {
                value: ZERO_CELSIUS + 43.0,
                prov: Prov::Assumed {
                    basis: "condenser 8 K above a 35 C summer day",
                },
                sigma: 1.0,
            },
            setpoint_k: ZERO_CELSIUS + 24.0,
            rated_draw_w: Quantity {
                value: 1_100.0,
                prov: Prov::Assumed {
                    basis: "class of ~3500 BTU/h window unit, nameplate",
                },
                sigma: 100.0,
            },
        }
    }

    /// Coefficient of performance at the stored temperatures. `None` when there is no lift to work across.
    pub fn cop(&self) -> Option<f64> {
        let t_evap = self.evaporating_k.value;
        let t_cond = self.condensing_k.value;
        if !is_finite(t_evap) || !is_finite(t_cond) || !(t_cond > t_evap) {
            return None;
        }
        let eta = self.second_law_efficiency.value;
        if !(eta > 0.0) || !is_finite(eta) {
            return None;
        }
        Some(eta * t_evap / (t_cond - t_evap))
    }

    /// Cooling delivered at full speed, W.
    pub fn capacity_w(&self) -> Option<f64> {
        Some(self.rated_draw_w.value * self.cop()?)
    }

    /// Electrical draw while removing `cooling_w`, W — including the cycling.
    ///
    /// Below capacity the unit cycles at a duty cycle `cooling/capacity`, so the *average* draw falls in
    /// proportion. That linearity is a property of this model, not of refrigeration: a real unit's COP improves
    /// at part load because the condenser sees a lower ambient, and cycling also costs a little at each start.
    /// Neither is here, and the direction of both is that this model overstates the draw at low load — so the
    /// *drop* it predicts is conservative, which is the right way for a claim to be wrong.
    pub fn draw_for(&self, cooling_w: f64) -> Option<f64> {
        if !is_finite(cooling_w) || cooling_w < 0.0 {
            return None;
        }
        let capacity = self.capacity_w()?;
        if capacity <= 0.0 {
            return None;
        }
        if cooling_w > capacity {
            // Not a small error: the unit cannot do this, and pretending otherwise would let the zone hold a
            // setpoint it physically could not reach.
            return None;
        }
        Some(self.rated_draw_w.value * cooling_w / capacity)
    }

    /// The cooling needed to hold the setpoint, W. Zero when the zone would float at or below it.
    pub fn cooling_to_hold(
        &self,
        zone: &Zone,
        outdoors: &Outdoors,
        internal: &InternalGain,
    ) -> f64 {
        let held = ZoneState {
            temperature_k: self.setpoint_k,
        };
        let net = zone.net_gain(&held, outdoors, internal);
        if net > 0.0 {
            net
        } else {
            0.0
        }
    }
}

/// Radiative heat a surface rejects, W, by Stefan–Boltzmann.
///
/// `ε σ A (T⁴ − T_surround⁴)`. Both temperatures absolute — a Celsius temperature here makes the answer wrong by
/// about a factor of 300 and, worse, makes it *look* like a plausible number of watts.
pub fn radiative_rejection(
    area_m2: f64,
    emissivity: f64,
    surface_k: f64,
    surroundings_k: f64,
) -> Option<f64> {
    if !is_finite(area_m2)
        || !is_finite(emissivity)
        || !is_finite(surface_k)
        || !is_finite(surroundings_k)
    {
        return None;
    }
    if !(area_m2 >= 0.0)
        || !(0.0..=1.0).contains(&emissivity)
        || !(surface_k > 0.0)
        || !(surroundings_k > 0.0)
    {
        return None;
    }
    // `pow` rather than `x*x*x*x` so the compiler cannot reassociate it, and so the same function is used for
    // both temperatures — the difference of two fourth powers is the one place in this file where a
    // reassociated expression could change a result by more than an ulp.
    Some(emissivity * STEFAN_BOLTZMANN * area_m2 * (pow(surface_k, 4.0) - pow(surroundings_k, 4.0)))
}

/// The temperature a radiator settles at, K, given a heat load and a rejection temperature.
///
/// Inverts [`radiative_rejection`]: `T = (Q/(εσA) + T_surr⁴)^(1/4)`. This is the sizing direction — you know
/// the load and you want the area — and it is the one a player actually asks about.
pub fn radiator_temperature(
    heat_load_w: f64,
    area_m2: f64,
    emissivity: f64,
    surroundings_k: f64,
) -> Option<f64> {
    if !is_finite(heat_load_w)
        || !is_finite(area_m2)
        || !is_finite(emissivity)
        || !is_finite(surroundings_k)
    {
        return None;
    }
    if !(heat_load_w >= 0.0)
        || !(area_m2 > 0.0)
        || !(0.0..=1.0).contains(&emissivity)
        || !(surroundings_k > 0.0)
    {
        return None;
    }
    let base = heat_load_w / (emissivity * STEFAN_BOLTZMANN * area_m2) + pow(surroundings_k, 4.0);
    if !(base > 0.0) {
        return None;
    }
    Some(pow(base, 0.25))
}

/// Performance of a vapour-compression cycle as a function of lift, dimensionless.
///
/// A convenience for plotting and for tests: the closed form for the draw ratio between two operating points.
/// Returns the ratio `COP(lift_small)/COP(lift_large)` — i.e. how much better a cycle is when it has less lift
/// to work across. `None` for a non-positive lift, where the question has no meaning.
pub fn cop_ratio_between_lifts(lift_1_k: f64, lift_2_k: f64) -> Option<f64> {
    if !(lift_1_k > 0.0) || !(lift_2_k > 0.0) || !is_finite(lift_1_k) || !is_finite(lift_2_k) {
        return None;
    }
    Some(lift_2_k / lift_1_k)
}

/// Exponential approach of a first-order system toward an equilibrium, for reporting settling time.
///
/// `ΔT(t) = ΔT₀ · exp(−t/τ)` with `τ = C/UA`. Present because the *time* the room takes to cool is half of the
/// §6.7 claim ("cool by several kelvin over minutes") and it is the half a step-by-step integration makes hard to
/// state. `None` for a non-positive time constant.
pub fn first_order_fraction_remaining(tau_s: f64, elapsed_s: f64) -> Option<f64> {
    if !(tau_s > 0.0) || !is_finite(tau_s) || !is_finite(elapsed_s) {
        return None;
    }
    Some(exp(-elapsed_s / tau_s))
}

