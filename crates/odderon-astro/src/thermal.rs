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
//! They are separate because they are separate in the bible, and because one of them is a *measured reference*
//! for the other: [`VapourCompression::nameplate_cop`] is a real unit's cooling divided by its real draw, and
//! [`VapourCompression::cop`] is the correlation that has to agree with it.
//!
//! **What the coupling is not.** An earlier draft of this paragraph claimed the evaporator temperature "changes
//! with the load, which changes its COP". It does not: `cop()` reads fixed stored temperatures, and the only
//! load-dependence in the draw is the duty cycle and the cyclic degradation term. A sunk evaporator temperature is
//! a real modelling choice — it is what makes the correlation checkable against a nameplate, which is rated at one
//! fixed condition — but a choice written up as a mechanism is a false claim, and this one was. Sinking the
//! evaporator properly needs refrigerant properties, which §3.2 of `docs/CREDIBILITY.md` keeps out of scope.
//!
//! ## What is modelled, and what is not
//!
//! Modelled: sensible heat only; a linear conductance `UA`; solar gain as a single irradiance × area × SHGC
//! product; a heat pump whose COP falls as the lift across it grows; a compressor that cycles on and off rather
//! than modulating, which is what a window unit does; and the cyclic degradation that cycling costs.
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
//!
//! ## What is measured, and what is calibrated
//!
//! The distinction the module turns on, because it is the one that was blurred first time round:
//!
//! * **Measured** — the unit's anchor: 12 000 BTU/h for 1 091 W, i.e. COP 3.224. That is a nameplate.
//! * **Calibrated** — `second_law_efficiency = 0.35`, chosen because it makes the Copeland map reproduce that
//!   nameplate at this unit's lift. A calibration agrees with its own anchor by construction, so agreement is
//!   **not** evidence that the correlation generalises; it is evidence that the arithmetic is consistent.
//! * **Predicted** — everything the correlation says away from that operating point, such as
//!   [`cop_ratio_between_lifts`]. Unanchored, and labelled as such.

use crate::is_positive_finite;
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
    /// The air alone is ~75 kJ/K for a 25 m² room at 2.5 m (75 kg × 1005 J/kg·K), which would swing several
    /// kelvin per minute. The effective figure for a furnished room is more than that, and the difference is
    /// exactly what `Assumed` is for: the air number is measurable, the effective number is a modelling choice.
    ///
    /// The number here was once given as "~30 kJ/K", with `25 × 2.5 × 1.2 × 1005` written out beside it in
    /// justification — a product that is 75 375, not 30 000. The stated ratio "~4x the air" was wrong for the
    /// same reason; the real ratio is 1.59x. The *choice* of 120 kJ/K survives (it is still what puts the
    /// ten-minute drop at 5.9 K), but the argument attached to it did not, which is the failure mode this
    /// project keeps having: a number asserted next to its own disproof.
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
                    basis: "furnished 25 m2 room, 1.59x its 75 kJ/K air mass",
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
                // `Assumed`, not `Defined`: 3 m² is the bible's number for a room the bible invented. It is exact
                // *in the bible*, which is a different kind of fact from exact in the SI, and the first version of
                // this struct marked it `Defined` — the exactness of a fiction.
                value: 3.0,
                prov: Prov::Assumed {
                    basis: "GAME-DESIGN-BIBLE 6.7, the room this module exists to model",
                },
                sigma: 0.5,
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
        if !is_positive_finite(self.loss_conductance.value) {
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
    /// `2C/UA` is the stability limit of forward Euler on `C dT/dt = −UA·T + …`, and this returns **half** of it,
    /// because the limit is where the scheme stops being *stable*, not where it stops being *accurate*: a step at
    /// the limit conserves energy but rings.
    ///
    /// The halving is spelled out because the earlier version of this doc called `C/UA` the limit and then
    /// justified the halving by a distinction between stability and accuracy — comment, constant and test each
    /// said something different, and all three passed. The test now pins all of it: `C/UA` converges, `3·C/UA`
    /// diverges, and the returned bound sits between them.
    pub fn max_stable_step(&self) -> Option<f64> {
        if !is_positive_finite(self.loss_conductance.value) {
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
/// motor heat.
///
/// **`η₂` is a calibration here, not a lookup.** A quoted range of 0.4–0.5 for small units was the first version
/// of this comment, and it was wrong in a way that mattered: at a 31 K lift, `η₂ = 0.45` implies EER 16.4, which
/// is an inverter machine, not the constant-speed unit this class describes. The range that spans real window
/// units at 0.30–0.35 was derived backwards from EER 9–12, and [`VapourCompression::window_unit`] carries 0.35
/// because that is what reproduces a *measured* nameplate.
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
    /// Cooling the nameplate claims, W.
    ///
    /// Carried so that the correlation can be **checked against a real unit** rather than only against itself.
    /// `cop()` is a prediction; `nameplate_cop()` is a measurement; the test that compares them is the only thing
    /// in this file that says the correlation is anchored in anything.
    pub nameplate_cooling_w: Quantity,
    /// Cyclic degradation coefficient `C_d`, dimensionless.
    ///
    /// A cycling unit is *worse* at part load than the linear duty-cycle model says: each start loses capacity
    /// and efficiency to the pressure equalisation that happens while it is off. ASHRAE's seasonal procedure
    /// carries this as `EER_part = (1 − C_d(1 − PLF))·EER`, with `C_d` between 0.04 and 0.25 for residential
    /// equipment and 0.10 the usual default. Omitting it (as this module did at first) makes the predicted draw
    /// *too low* at part load — and the room's part load is where the curtain claim lives.
    pub cycling_degradation: Quantity,
}

impl VapourCompression {
    /// An ordinary 12 000 BTU/h window unit, at its measured nameplate.
    ///
    /// The anchor is a real appliance: **12 000 BTU/h of cooling for about 1 091 W of electricity**, which is
    /// EER 11 — the middle of the measured band for window units (9–12, so COP 2.6–3.5). Everything else is
    /// derived from, or checked against, that pair of numbers.
    ///
    /// `second_law_efficiency` is the one value chosen rather than looked up, and it is chosen the honest way:
    /// 0.35 is what makes the Copeland map reproduce this unit's measured COP at this unit's lift. That makes it
    /// a calibration, not a prediction, and [`VapourCompression::nameplate_cop`] exists so the difference between
    /// the two stays visible.
    pub fn window_unit() -> VapourCompression {
        VapourCompression {
            second_law_efficiency: Quantity {
                value: 0.35,
                prov: Prov::Assumed {
                    basis: "the eta_2 that reproduces a measured EER 11 at a 31 K lift; 0.30-0.35 spans the class",
                },
                sigma: 0.05,
            },
            evaporating_k: Quantity {
                value: ZERO_CELSIUS + 15.0,
                prov: Prov::Assumed {
                    basis: "24 C setpoint less a 9 K air-side approach and 3 K superheat",
                },
                sigma: 2.0,
            },
            condensing_k: Quantity {
                value: ZERO_CELSIUS + 46.0,
                prov: Prov::Assumed {
                    basis: "35 C summer day plus a 6 K approach and 5 K subcool",
                },
                sigma: 2.0,
            },
            setpoint_k: ZERO_CELSIUS + 24.0,
            rated_draw_w: Quantity {
                value: 1_091.0,
                prov: Prov::Measured { sigma: 170.0 },
                sigma: 170.0,
            },
            nameplate_cooling_w: Quantity {
                value: 3_517.0,
                prov: Prov::Measured { sigma: 60.0 },
                sigma: 60.0,
            },
            cycling_degradation: Quantity {
                value: 0.10,
                prov: Prov::Assumed {
                    basis: "ASHRAE cyclic degradation coefficient, 0.04-0.25 for residential, 0.10 default",
                },
                sigma: 0.05,
            },
        }
    }

    /// The measured COP: nameplate cooling divided by nameplate draw.
    ///
    /// **3.224** for [`VapourCompression::window_unit`], against the correlation's **3.253** — agreement to 0.9 %.
    /// This is the reference the correlation is checked against, and it is a measurement rather than a
    /// rearrangement of the correlation.
    pub fn nameplate_cop(&self) -> Option<f64> {
        let cooling = self.nameplate_cooling_w.value;
        let draw = self.rated_draw_w.value;
        if !is_positive_finite(cooling) || !is_positive_finite(draw) {
            return None;
        }
        Some(cooling / draw)
    }

    /// Coefficient of performance at the stored temperatures. `None` when there is no lift to work across.
    pub fn cop(&self) -> Option<f64> {
        let t_evap = self.evaporating_k.value;
        let t_cond = self.condensing_k.value;
        if !is_finite(t_evap) || !is_finite(t_cond) || !is_positive_finite(t_cond - t_evap) {
            return None;
        }
        let eta = self.second_law_efficiency.value;
        if !is_positive_finite(eta) {
            return None;
        }
        Some(eta * t_evap / (t_cond - t_evap))
    }

    /// Cooling delivered at full speed, W.
    pub fn capacity_w(&self) -> Option<f64> {
        Some(self.rated_draw_w.value * self.cop()?)
    }

    /// Electrical draw while removing `cooling_w`, W — including the cycling and its penalty.
    ///
    /// Below capacity the unit runs a fraction `PLF = cooling/capacity` of the time, so the *average* draw is
    /// `rated · PLF / COP_part`. The COP at part load is degraded from the full-load one by
    /// `1 − C_d(1 − PLF)`, which is ASHRAE's cyclic degradation term: a cycling unit loses efficiency at every
    /// start, and at a 26 % runtime fraction it loses about 7 % of its COP.
    ///
    /// **The direction of that correction matters, and the first version of this function had it backwards.** It
    /// claimed a real unit's COP *improves* at part load, and that omitting the term therefore made the predicted
    /// draw conservative. Both halves were wrong: for constant-speed equipment the measurements say part-load
    /// efficiency is *lower* (that is what `C_d` exists to describe), so the omission understated the draw. The
    /// curtain claim survived because closing the curtain drops the load in the numerator by more than the
    /// degradation eats, but the *sizes* were wrong: 281 W instead of 304 W, an 8 % error in the number the
    /// player reads off the meter.
    pub fn draw_for(&self, cooling_w: f64) -> Option<f64> {
        if !is_finite(cooling_w) || cooling_w < 0.0 {
            return None;
        }
        let cop = self.cop()?;
        let capacity = self.capacity_w()?;
        if capacity <= 0.0 {
            return None;
        }
        if cooling_w > capacity {
            // Not a small error: the unit cannot do this, and pretending otherwise would let the zone hold a
            // setpoint it physically could not reach.
            return None;
        }
        if cooling_w == 0.0 {
            return Some(0.0);
        }
        let part_load_fraction = cooling_w / capacity;
        let cd = self.cycling_degradation.value;
        if !is_finite(cd) || !(0.0..=1.0).contains(&cd) {
            return None;
        }
        let degraded_cop = cop * (1.0 - cd * (1.0 - part_load_fraction));
        if !is_positive_finite(degraded_cop) {
            return None;
        }
        Some(self.rated_draw_w.value * part_load_fraction * (cop / degraded_cop))
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
/// `ε σ A (T⁴ − T_surround⁴)`. Both temperatures absolute.
///
/// Passing Celsius makes the answer **smaller**, not larger, and by a factor that grows as the temperature falls:
/// a 300 K surface rejecting against 273 K gives 143.6 W, while the same surface passed as Celsius (26.85 against
/// 0) gives 0.0295 W — a factor of 4 874. The earlier version of this doc said "about a factor of 300", which was
/// an unmeasured guess in a comment whose whole subject was the danger of unmeasured numbers. `tests/thermal.rs`
/// pins the measured ratio.
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
    if area_m2 < 0.0
        || !(0.0..=1.0).contains(&emissivity)
        || surface_k <= 0.0
        || surroundings_k < 0.0
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
    if heat_load_w < 0.0
        || area_m2 <= 0.0
        || !(0.0..=1.0).contains(&emissivity)
        || surroundings_k < 0.0
    {
        return None;
    }
    let base = heat_load_w / (emissivity * STEFAN_BOLTZMANN * area_m2) + pow(surroundings_k, 4.0);
    if base <= 0.0 {
        return None;
    }
    Some(pow(base, 0.25))
}

/// Performance of a vapour-compression cycle as a function of lift, dimensionless.
///
/// A convenience for plotting and for tests: the closed form for the COP ratio between two operating points.
/// Returns `lift_2/lift_1` — the **COP at `lift_1` divided by the COP at `lift_2`**. A ratio above 1 therefore
/// means the second operating point is the worse one. The earlier version of this doc stated the inverse
/// quotient while the code returned this one, and nothing caught it, because the only test asserted `ratio > 1.0`,
/// which both spellings satisfy. `None` for a non-positive lift, where the question has no meaning.
pub fn cop_ratio_between_lifts(lift_1_k: f64, lift_2_k: f64) -> Option<f64> {
    if !is_positive_finite(lift_1_k) || !is_positive_finite(lift_2_k) {
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
    if !is_positive_finite(tau_s) || !is_finite(elapsed_s) || elapsed_s < 0.0 {
        return None;
    }
    Some(exp(-elapsed_s / tau_s))
}
