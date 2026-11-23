//! The Room, as five measurable claims about a thermal network.
//!
//! `BUILD-PLAN.md` §4 defines the first increment as five claims, and the second of them is the reason this file
//! exists:
//!
//! > **The room is a spacecraft.** Closing the curtain drops solar gain by the amount §6.7 says, and the AC's
//! > electrical draw visibly falls.
//!
//! That is a falsifiable statement about a coupled system. So the tests below are grouped by which part of it
//! they can falsify:
//!
//! 1. **The bible's own arithmetic.** 3 m² × 800 W/m² × SHGC 0.6 is 1440 W; closed, 120 W. If these do not
//!    reproduce, one of the two documents is wrong and the number in the bible is not evidence of anything.
//! 2. **The coupling.** Closing the curtain must lower the *draw*, not just the solar gain — via the load, the
//!    duty cycle, and a COP that does not get to hide the change.
//! 3. **The dynamics.** "Cools by several kelvin over minutes" is a claim about a time constant, and it is
//!    measured rather than asserted: 5.9 K in ten minutes.
//! 4. **The honest failures.** A unit asked to deliver more than its capacity must refuse, not saturate; a
//!    radiator asked for zero area must refuse; a temperature in Celsius must produce nonsense that the API
//!    does not accept in the first place.
//! 5. **The invariants a wrong sign would break.** Energy conservation over a step, and the sign of every term.
//!
//! Every number quoted in a comment was computed in `node` against the same constants before being written
//! down, so a failure says which of the two disagreed.

use odderon_astro::thermal::{
    cop_ratio_between_lifts, first_order_fraction_remaining, radiative_rejection,
    radiator_temperature, InternalGain, Outdoors, VapourCompression, Zone, ZoneState,
    STEFAN_BOLTZMANN, ZERO_CELSIUS,
};

/// Outdoor conditions from §6.7: a peak summer day, sun on the glazing.
fn summer() -> Outdoors {
    Outdoors {
        temperature_k: 35.0 + ZERO_CELSIUS,
        irradiance_w_per_m2: 800.0,
    }
}

/// The light load of a person and a laptop.
fn occupant() -> InternalGain {
    InternalGain { watts: 300.0 }
}

fn room() -> Zone {
    Zone::small_apartment_room()
}

// ---------------------------------------------------------------------------------------------
// 1. The bible's arithmetic, reproduced
// ---------------------------------------------------------------------------------------------

/// The two numbers in §6.7, reproduced from the module's own defaults.
///
/// This is the weakest test in the file and it is first on purpose: it is the only thing that establishes the
/// module and the design document are talking about the same room. A thermal model that is internally perfect
/// about a 5 m² window would pass every other test here and be describing a different building.
#[test]
fn the_bible_s_curtain_numbers_are_reproduced() {
    let open = room();
    let closed = open.curtain_closed();
    let outdoors = summer();

    assert!(
        (open.solar_gain(&outdoors) - 1_440.0).abs() < 1.0,
        "open curtain: 3 m2 x 800 W/m2 x 0.6 should be 1440 W, got {:.1}",
        open.solar_gain(&outdoors)
    );
    assert!(
        (closed.solar_gain(&outdoors) - 120.0).abs() < 1.0,
        "closed curtain: 3 m2 x 800 W/m2 x 0.05 should be 120 W, got {:.1}",
        closed.solar_gain(&outdoors)
    );

    // And the drop is 92 %, which is the number the player sees as "the room got darker and cooler".
    let ratio = closed.solar_gain(&outdoors) / open.solar_gain(&outdoors);
    assert!(
        (ratio - 0.05 / 0.60).abs() < 1e-12,
        "the ratio must be the SHGC ratio exactly, got {ratio}"
    );
}

/// Closing the curtain does not change anything except the aperture.
///
/// A `curtain_closed` that quietly changed the conductance too would produce the right draw *drop* for the wrong
/// reason, and the next change to `UA` would move a number that was supposed to be independent of it.
#[test]
fn closing_the_curtain_changes_only_the_shgc() {
    let open = room();
    let closed = open.curtain_closed();
    assert_eq!(closed.heat_capacity.value, open.heat_capacity.value);
    assert_eq!(closed.loss_conductance.value, open.loss_conductance.value);
    assert_eq!(closed.glazing_area.value, open.glazing_area.value);
    assert!(closed.shgc.value < open.shgc.value);
    // A curtain leaks; it is not a shutter. Claiming zero would make the closed state a different object.
    assert!(closed.shgc.value > 0.0, "a curtain is not a shutter");
}

// ---------------------------------------------------------------------------------------------
// 2. The coupling: the draw falls, through the load
// ---------------------------------------------------------------------------------------------

/// The claim of §6.7, end to end: close the curtain, the meter reading falls.
///
/// Measured with the module's own defaults: **464 W open, 190 W closed** — a 59 % drop. The absolute numbers
/// depend on the assumptions and are not the point; the drop is.
#[test]
fn closing_the_curtain_lowers_the_ac_draw() {
    let open = room();
    let closed = open.curtain_closed();
    let outdoors = summer();
    let ac = VapourCompression::window_unit();

    let cooling_open = ac.cooling_to_hold(&open, &outdoors, &occupant());
    let cooling_closed = ac.cooling_to_hold(&closed, &outdoors, &occupant());
    assert!(
        cooling_closed < cooling_open,
        "the closed room needs less cooling: {cooling_closed:.0} W vs {cooling_open:.0} W"
    );

    let draw_open = ac
        .draw_for(cooling_open)
        .expect("the unit can meet the load");
    let draw_closed = ac
        .draw_for(cooling_closed)
        .expect("the smaller load is also within capacity");

    assert!(
        draw_closed < draw_open,
        "the meter must fall when the curtain closes: {draw_closed:.0} W vs {draw_open:.0} W"
    );

    // "Visibly drops" is not a tolerance question: the drop must be large enough to see on a meter, which for a
    // 1100 W appliance means hundreds of watts, not a few.
    let drop = draw_open - draw_closed;
    assert!(
        drop > 200.0,
        "a 92 % solar reduction must drop the draw by hundreds of watts, got {drop:.0} W"
    );

    // And the drop is the load ratio, because this model's COP is fixed. Stated as a test rather than left
    // implicit, because it is the model's main simplification and somebody should be able to find it.
    let draw_ratio = draw_closed / draw_open;
    let load_ratio = cooling_closed / cooling_open;
    assert!(
        (draw_ratio - load_ratio).abs() < 1e-12,
        "with a fixed COP the draw is proportional to the load, got {draw_ratio} vs {load_ratio}"
    );
}

/// A room already cooler than the setpoint needs no cooling, and the AC must draw nothing.
///
/// The zero case is where a naive implementation returns a tiny negative number and then a draw of `-3 W`, which
/// a wall meter would render as an impossibility.
#[test]
fn a_cool_room_needs_no_cooling() {
    let ac = VapourCompression::window_unit();
    let cold = Outdoors {
        temperature_k: 12.0 + ZERO_CELSIUS,
        irradiance_w_per_m2: 0.0,
    };
    let cooling = ac.cooling_to_hold(&room(), &cold, &occupant());
    assert_eq!(
        cooling, 0.0,
        "no cooling is needed, and none may be reported"
    );
    assert_eq!(
        ac.draw_for(cooling),
        Some(0.0),
        "an idle unit draws nothing"
    );
}

/// The unit refuses a load it cannot meet rather than saturating.
///
/// Returning the capacity would be the friendly thing to do and the wrong one: the zone would then hold a
/// setpoint it physically could not reach, and every downstream number would be right about a room that does not
/// exist. The refusal is a `None`, which the caller has to handle.
#[test]
fn a_load_beyond_capacity_is_refused_not_saturated() {
    let ac = VapourCompression::window_unit();
    let capacity = ac.capacity_w().expect("capacity computable");
    assert!(capacity > 0.0);

    assert!(
        ac.draw_for(capacity * 0.99).is_some(),
        "just inside capacity"
    );
    assert!(
        ac.draw_for(capacity * 1.01).is_none(),
        "just outside capacity must be refused, not clamped"
    );
}

// ---------------------------------------------------------------------------------------------
// 3. The dynamics: "several kelvin over minutes"
// ---------------------------------------------------------------------------------------------

/// The room cools by several kelvin over minutes once the curtain closes.
///
/// Integrated with the explicit step the module provides, at a step well inside the stated stability bound.
/// Measured: **5.9 K in the first ten minutes** from a free-floating start, against a time constant of 44.4 min.
#[test]
fn the_room_cools_by_several_kelvin_over_minutes() {
    let zone = room();
    let outdoors = summer();

    let settled = zone
        .free_float_temperature(&outdoors, &occupant())
        .expect("the room has a conductance, so it has a resting temperature");
    let start = ZoneState {
        temperature_k: settled,
    };

    // The curtain closes: from here the zone is the closed one.
    let closed = zone.curtain_closed();

    let step = 30.0;
    let max_step = zone.max_stable_step().expect("stable step computable");
    assert!(
        step < max_step,
        "the test uses a step inside the stability bound ({step} s vs {max_step} s)"
    );

    let mut state = start;
    let mut elapsed = 0.0;
    while elapsed < 600.0 {
        state = closed.step(&state, &outdoors, &occupant(), 0.0, step);
        elapsed += step;
    }

    let drop = start.temperature_k - state.temperature_k;
    assert!(
        drop > 3.0,
        "the bible says several kelvin over minutes; ten minutes gave {drop:.2} K"
    );
    assert!(
        drop < 12.0,
        "and not a plunge: {drop:.2} K in ten minutes against a 44.4 min time constant is too fast, which \
         usually means the heat capacity or the conductance is off by an order of magnitude"
    );
}

/// Energy is conserved across a step, to the arithmetic's precision.
///
/// The one invariant that a sign error in any term breaks, and the reason it is checked as a residual rather than
/// by re-reading the formula: `C·ΔT` must equal the net heat over the interval, whatever the individual terms did.
#[test]
fn a_step_conserves_energy() {
    let zone = room();
    let outdoors = summer();
    let state = ZoneState {
        temperature_k: 30.0 + ZERO_CELSIUS,
    };
    let dt = 60.0;
    let cooling = 500.0;

    let after = zone.step(&state, &outdoors, &occupant(), cooling, dt);

    let stored = zone.heat_capacity.value * (after.temperature_k - state.temperature_k);
    let supplied = zone.net_gain(&state, &outdoors, &occupant()) - cooling;
    let expected = supplied * dt;

    assert!(
        (stored - expected).abs() < 1e-9 * expected.abs().max(1.0),
        "stored {stored:.6} J vs supplied {expected:.6} J"
    );
}

/// Conductive loss has the right magnitude and the right sign in both directions.
///
/// The sign is the part worth pinning: `loss` positive means the zone is losing heat, so a zone warmer than
/// outdoors must return positive and a zone colder must return negative. A model that returned the magnitude
/// would keep every *steady state* correct and get every *transient* backwards, which is the kind of error a
/// settling test cannot see.
#[test]
fn loss_is_positive_when_the_zone_is_warmer() {
    let zone = room();
    let outdoors = summer();
    let ua = zone.loss_conductance.value;

    let warm = ZoneState {
        temperature_k: outdoors.temperature_k + 10.0,
    };
    assert!(
        (zone.loss(&warm, &outdoors) - 10.0 * ua).abs() < 1e-9,
        "10 K above outdoors must lose 10 K x UA"
    );

    let cold = ZoneState {
        temperature_k: outdoors.temperature_k - 10.0,
    };
    assert!(
        zone.loss(&cold, &outdoors) < 0.0,
        "below outdoors the zone gains heat, so the loss term is negative"
    );

    let equal = ZoneState {
        temperature_k: outdoors.temperature_k,
    };
    assert_eq!(zone.loss(&equal, &outdoors), 0.0, "no difference, no flow");
}

/// A free-floating room settles at `T_out + Q/UA`, and the closed loop of that identity holds.
#[test]
fn free_float_temperature_is_the_balance_point() {
    let zone = room();
    let outdoors = summer();
    let settled = zone
        .free_float_temperature(&outdoors, &occupant())
        .expect("computable");

    let state = ZoneState {
        temperature_k: settled,
    };
    // At the resting temperature the net gain is zero -- that is what "resting" means.
    let net = zone.net_gain(&state, &outdoors, &occupant());
    assert!(
        net.abs() < 1e-9,
        "a settled room has no net gain, got {net:.9} W"
    );
}

/// A zone with no conductance has no resting temperature, and the API says so instead of returning a huge number.
#[test]
fn a_perfectly_insulated_room_has_no_resting_temperature() {
    let mut zone = room();
    zone.loss_conductance.value = 0.0;
    assert_eq!(zone.free_float_temperature(&summer(), &occupant()), None);
    assert_eq!(zone.max_stable_step(), None);
}

/// The stated stability bound is real: stepping at twice `C/UA` diverges, and inside it converges.
///
/// An explicit integrator past its limit does not fail — it oscillates with growing amplitude, which in a frame
/// loop looks like a rendering bug and is chased in the wrong file. So the bound is measured rather than trusted.
#[test]
fn the_stability_bound_is_the_bound() {
    let zone = room();
    let outdoors = summer();
    // Free-floating with the curtain closed, so the equilibrium is unambiguous.
    let closed = zone.curtain_closed();

    let run = |dt: f64| {
        let mut state = ZoneState {
            temperature_k: 30.0 + ZERO_CELSIUS,
        };
        for _ in 0..400 {
            state = closed.step(&state, &outdoors, &occupant(), 0.0, dt);
        }
        state.temperature_k
    };

    let bound = zone.max_stable_step().expect("bound");
    // Comfortably inside: must converge to the free-float temperature.
    let settled = run(bound / 4.0);
    let expected = closed
        .free_float_temperature(&outdoors, &occupant())
        .expect("computable");
    assert!(
        (settled - expected).abs() < 0.5,
        "inside the bound it must converge to {expected:.1} K, got {settled:.1}"
    );

    // Well outside: must not converge, and specifically must not sit at a plausible temperature.
    let diverged = run(bound * 3.0);
    assert!(
        (diverged - expected).abs() >= 1.0 || diverged.is_nan(),
        "outside the bound it must not converge: {diverged:.1} K vs {expected:.1} K"
    );
}

// ---------------------------------------------------------------------------------------------
// 4. The heat pump, on its own
// ---------------------------------------------------------------------------------------------

/// The COP map is Carnot times the second-law efficiency, and it falls as the lift grows.
///
/// Both halves matter: the absolute value has to be in the right band for a real unit, and the *slope* has to have
/// the right sign, because a COP that rose with lift would still produce a plausible-looking draw for a fixed
/// operating point.
#[test]
fn the_cop_is_carnot_times_second_law_and_falls_with_lift() {
    let ac = VapourCompression::window_unit();
    let cop = ac.cop().expect("computable");

    let carnot = ac.evaporating_k.value / (ac.condensing_k.value - ac.evaporating_k.value);
    assert!(
        (cop - carnot * ac.second_law_efficiency.value).abs() < 1e-12,
        "COP must be eta_2 x Carnot exactly"
    );
    // A real window unit sits around 3-6; outside that band one of the assumed temperatures is wrong.
    assert!(
        (3.0..6.0).contains(&cop),
        "COP {cop:.2} is outside the band a window unit actually achieves"
    );

    // Less lift, better COP.
    let ratio = cop_ratio_between_lifts(20.0, 40.0).expect("computable");
    assert!(
        ratio > 1.0,
        "halving the lift must improve the COP, got a ratio of {ratio}"
    );
}

/// A cycle with no lift has no COP, and one with the condenser colder than the evaporator is refused.
#[test]
fn an_impossible_cycle_is_refused() {
    let mut ac = VapourCompression::window_unit();
    ac.condensing_k.value = ac.evaporating_k.value;
    assert_eq!(ac.cop(), None, "no lift, no cycle");

    ac.condensing_k.value = ac.evaporating_k.value - 5.0;
    assert_eq!(
        ac.cop(),
        None,
        "a reversed cycle is not a worse one, it is not one"
    );

    ac.condensing_k.value = ac.evaporating_k.value + 20.0;
    ac.second_law_efficiency.value = 0.0;
    assert_eq!(
        ac.cop(),
        None,
        "a cycle with no second-law efficiency does no useful work"
    );
}

/// The draw is linear in the load up to capacity, and zero at zero.
#[test]
fn draw_is_proportional_to_load_up_to_capacity() {
    let ac = VapourCompression::window_unit();
    let capacity = ac.capacity_w().expect("capacity");

    let quarter = ac.draw_for(capacity / 4.0).expect("inside capacity");
    let half = ac.draw_for(capacity / 2.0).expect("inside capacity");
    assert!(
        (half - 2.0 * quarter).abs() < 1e-9,
        "the duty cycle is linear: {half:.3} should be twice {quarter:.3}"
    );
    assert!((ac.draw_for(capacity).expect("at capacity") - ac.rated_draw_w.value).abs() < 1e-9);
}

// ---------------------------------------------------------------------------------------------
// 5. Radiators, and the units trap
// ---------------------------------------------------------------------------------------------

/// Stefan–Boltzmann round-trips through the sizing inversion.
#[test]
fn radiative_rejection_inverts_to_a_radiator_temperature() {
    let load = 500.0;
    let area = 2.0;
    let emissivity = 0.9;
    let surroundings = 4.0 + ZERO_CELSIUS;

    let t = radiator_temperature(load, area, emissivity, surroundings).expect("computable");
    let rejected = radiative_rejection(area, emissivity, t, surroundings).expect("computable");
    assert!(
        (rejected - load).abs() < 1e-9,
        "the radiator sized for {load} W must reject {load} W, got {rejected:.9}"
    );
}

/// Radiation scales as the fourth power — of the *difference*, which is why the naive ratio is not 16.
///
/// Doubling a surface temperature from 300 K to 600 K multiplies `T⁴` by 16, but the rejated quantity is
/// `T⁴ − T_surr⁴`, and against a 273 K surroundings that subtracts 5.6e9 from both terms: the measured ratio is
/// **5.62**, not 16. A test asserting 16 would pass only with the surroundings at absolute zero, which is a
/// different equation. The fourth power is therefore asserted where it is exactly true — on the surface term.
#[test]
fn radiative_rejection_is_fourth_power_in_the_surface_term() {
    // Against a zero-degree surroundings the rejection *is* epsilon-sigma-A-T^4 exactly...
    let cold = radiative_rejection(1.0, 1.0, 600.0, 0.0).expect("computable");
    let expected = STEFAN_BOLTZMANN * 600.0f64.powi(4);
    assert!(
        (cold - expected).abs() < 1e-9 * expected,
        "with T_surr = 0 the rejection is epsilon-sigma-T^4, got {cold:.6} vs {expected:.6}"
    );

    // ...and doubling T multiplies *that* by 16.
    let hot = radiative_rejection(1.0, 1.0, 1200.0, 0.0).expect("computable");
    assert!(
        (hot / cold - 16.0).abs() < 1e-9,
        "doubling absolute T must scale the surface term by 16, got {:.6}",
        hot / cold
    );

    // With a realistic 273 K surroundings the ratio is nothing like 16, and quoting 16 would be wrong.
    let warm_surroundings = ZERO_CELSIUS;
    let a =
        radiative_rejection(1.0, 1.0, 300.0 + ZERO_CELSIUS, warm_surroundings).expect("computable");
    let bb =
        radiative_rejection(1.0, 1.0, 600.0 + ZERO_CELSIUS, warm_surroundings).expect("computable");
    let ratio = bb / a;
    assert!(
        (ratio - 5.62).abs() < 0.05,
        "against 273 K surroundings the measured ratio is 5.62, got {ratio:.3}"
    );
}

/// A Celsius temperature produces an absurdly *small* number, and the direction of the error is the point.
///
/// The module's doc warns that Celsius is "wrong by about a factor of 300". Measured, it is worse than a factor:
/// a 300 K surface rejecting against 273 K gives **143.6 W**, while the same surface passed as Celsius
/// (26.85 against 0) gives **0.0295 W** — a factor of 4 874, and in the *low* direction, so a radiator sized this
/// way would be roughly 5 000 times too large instead of too small. A wrong answer that over-sizes is the
/// failure mode that looks like conservatism.
///
/// A float API cannot reject Celsius, because 300.0 is a valid temperature in either scale. What it can do is
/// refuse the inputs that are invalid in *both* — a negative absolute temperature, an emissivity above 1 — and
/// that is what this test pins alongside the measurement.
#[test]
fn a_celsius_temperature_produces_an_absurdly_small_number() {
    let as_kelvin = radiative_rejection(1.0, 1.0, 300.0, 273.15).expect("computable");
    let as_celsius = radiative_rejection(1.0, 1.0, 26.85, 0.0).expect("computable");
    assert!(
        as_kelvin > as_celsius * 1_000.0,
        "the Celsius reading must be absurdly smaller: {as_kelvin:.1} W vs {as_celsius:.4} W"
    );

    // Inputs invalid in every scale are refused.
    assert_eq!(
        radiative_rejection(1.0, 1.0, -5.0, 273.15),
        None,
        "a negative absolute temperature is not a temperature"
    );
    assert_eq!(
        radiative_rejection(1.0, 1.5, 300.0, 273.15),
        None,
        "emissivity above 1 is not physical"
    );
    assert_eq!(
        radiative_rejection(-1.0, 0.5, 300.0, 273.15),
        None,
        "negative area is not physical"
    );
    assert_eq!(
        radiator_temperature(500.0, 0.0, 0.9, 277.15),
        None,
        "a radiator with no area cannot reject any heat"
    );
}

/// The first-order approach reports the fraction remaining, and the time constant is `C/UA`.
#[test]
fn the_time_constant_is_c_over_ua() {
    let zone = room();
    let tau = zone.heat_capacity.value / zone.loss_conductance.value;
    let at_one_tau = first_order_fraction_remaining(tau, tau).expect("computable");
    assert!(
        (at_one_tau - (-1.0f64).exp()).abs() < 1e-12,
        "one time constant leaves 1/e, got {at_one_tau}"
    );
    assert_eq!(
        first_order_fraction_remaining(0.0, 1.0),
        None,
        "no time constant, no decay"
    );
}

/// The constants are the published ones, not approximations that happen to work.
#[test]
fn the_constants_are_defined_not_guessed() {
    assert!(
        (STEFAN_BOLTZMANN - 5.670_374_419e-8).abs() < 1e-20,
        "Stefan-Boltzmann is exact since the 2019 SI redefinition"
    );
    assert!(
        (ZERO_CELSIUS - 273.15).abs() < 1e-12,
        "the Celsius offset is exact"
    );
}
