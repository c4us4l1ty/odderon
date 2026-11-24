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
//! 2. **The coupling.** Closing the curtain must lower the *draw*, not just the solar gain — via the load and the
//!    duty cycle, with the cycling penalty working against the drop rather than being absent from it.
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
/// Measured with the module's own defaults, which are a real 12 000 BTU/h unit: **713 W open, 304 W closed** — a
/// **57.4 %** drop. The absolute numbers depend on the assumptions and are not the claim; the drop is. They are
/// pinned anyway, because a drop that is right for the wrong reason is the failure this file exists to catch.
#[test]
fn closing_the_curtain_lowers_the_ac_draw() {
    let open = room();
    let closed = open.curtain_closed();
    let outdoors = summer();
    let ac = VapourCompression::window_unit();

    let cooling_open = ac.cooling_to_hold(&open, &outdoors, &occupant());
    let cooling_closed = ac.cooling_to_hold(&closed, &outdoors, &occupant());
    assert!(
        (cooling_open - 2_235.0).abs() < 1.0,
        "the open-curtain load is 1440 W solar + 300 W internal less 45 K/W x 11 K, got {cooling_open:.1} W"
    );
    assert!(
        (cooling_closed - 915.0).abs() < 1.0,
        "the closed-curtain load is 120 W + 300 W less the same loss, got {cooling_closed:.1} W"
    );

    let draw_open = ac
        .draw_for(cooling_open)
        .expect("the unit can meet the load");
    let draw_closed = ac
        .draw_for(cooling_closed)
        .expect("the smaller load is also within capacity");

    assert!(
        (draw_open - 713.0).abs() < 2.0,
        "measured draw with the curtain open is 713 W, got {draw_open:.1}"
    );
    assert!(
        (draw_closed - 304.0).abs() < 2.0,
        "measured draw with the curtain closed is 304 W, got {draw_closed:.1}"
    );

    let drop = draw_open - draw_closed;
    assert!(
        drop > 200.0,
        "a 92 % solar reduction must drop the draw by hundreds of watts, got {drop:.0} W"
    );
    let fraction = drop / draw_open;
    assert!(
        (fraction - 0.574).abs() < 0.005,
        "the drop is 57.4 %, got {:.1} %",
        100.0 * fraction
    );

    // The drop is NOT the load ratio, because the cycling penalty bites harder at low load. Asserting equality
    // here is how the first version of this test hid that the model had no part-load behaviour at all.
    let load_ratio = cooling_closed / cooling_open;
    assert!(
        fraction < 1.0 - load_ratio,
        "cycling degradation must make the draw drop LESS than the load does: {:.1} % vs {:.1} %",
        100.0 * fraction,
        100.0 * (1.0 - load_ratio)
    );
}

/// The correlation is anchored to a real appliance, and it says so.
///
/// This is the test that would have caught the first version's unit: it computed COP 4.82 (EER 16.4) from an
/// `eta_2` of 0.45, while its own `rated_draw_w` basis claimed a 3 500 BTU/h appliance. A window unit at
/// EER 16.4 is an inverter model, and the capacity implied by the two numbers disagreed with the basis by 5x.
#[test]
fn the_cop_agrees_with_a_real_nameplate() {
    let ac = VapourCompression::window_unit();
    let predicted = ac.cop().expect("computable");
    let measured = ac.nameplate_cop().expect("computable");

    // 12 000 BTU/h at EER 11 is COP 3.224. The correlation must land on it to better than 1 %.
    assert!(
        (measured - 3.224).abs() < 0.002,
        "12000 BTU/h / 1091 W is COP 3.224, got {measured:.4}"
    );
    assert!(
        (predicted - measured).abs() / measured < 0.01,
        "the correlation ({predicted:.4}) must reproduce the nameplate ({measured:.4}) to 1 %"
    );

    // And the result must be in the band a window unit occupies: EER 9-12 is COP 2.6-3.5.
    let eer = 3.412 * predicted;
    assert!(
        (9.0..=12.0).contains(&eer),
        "EER {eer:.1} is outside the measured window-unit band of 9-12"
    );

    // The capacity that falls out must match the nameplate capacity, which is the check the old unit failed.
    let capacity = ac.capacity_w().expect("computable");
    assert!(
        (capacity - ac.nameplate_cooling_w.value).abs() / ac.nameplate_cooling_w.value < 0.02,
        "capacity {capacity:.0} W should be the nameplate's {:.0} W",
        ac.nameplate_cooling_w.value
    );
}

/// The cycling penalty raises the draw at part load, and vanishes at full load.
///
/// Direction and endpoints both. An earlier version of `draw_for` argued the opposite direction in a comment, so
/// the sign is now measured rather than described.
#[test]
fn cycling_degrades_efficiency_at_part_load_only() {
    let ac = VapourCompression::window_unit();
    let capacity = ac.capacity_w().expect("computable");

    // At full load the runtime fraction is 1, so there is nothing to degrade to.
    let at_capacity = ac.draw_for(capacity).expect("at capacity");
    assert!(
        (at_capacity - ac.rated_draw_w.value).abs() < 1e-9,
        "at full load the draw is exactly the nameplate draw, got {at_capacity:.3}"
    );

    // At part load the penalty makes the draw strictly greater than the linear duty-cycle model predicts.
    let quarter = capacity / 4.0;
    let actual = ac.draw_for(quarter).expect("inside capacity");
    let linear = ac.rated_draw_w.value * quarter / capacity;
    assert!(
        actual > linear,
        "cycling must cost something: {actual:.1} W vs the linear {linear:.1} W"
    );

    // And the size of the penalty is the ASHRAE factor: draw_actual/draw_linear = 1/(1 - Cd(1-PLF)).
    let plf = quarter / capacity;
    let expected_ratio = 1.0 / (1.0 - ac.cycling_degradation.value * (1.0 - plf));
    assert!(
        (actual / linear - expected_ratio).abs() < 1e-12,
        "the penalty is 1/(1 - Cd(1-PLF)), got a ratio of {:.6}",
        actual / linear
    );
}

/// A unit with a nonsensical degradation coefficient is refused rather than quietly believed.
#[test]
fn an_invalid_degradation_coefficient_is_refused() {
    let mut ac = VapourCompression::window_unit();
    let capacity = ac.capacity_w().expect("computable");
    ac.cycling_degradation.value = 1.5;
    assert_eq!(
        ac.draw_for(capacity / 2.0),
        None,
        "Cd above 1 is not physical"
    );
    ac.cycling_degradation.value = -0.1;
    assert_eq!(
        ac.draw_for(capacity / 2.0),
        None,
        "a negative penalty is not physical"
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

/// Energy is conserved across a step — against an independently computed heat balance.
///
/// **This test used to be a tautology.** It computed `expected` by calling the same `net_gain` the code under test
/// calls, so it only proved that `C·(net·dt/C) == net·dt`, which is true by rearrangement and would have passed
/// with every term in `net_gain` carrying the wrong sign. The coefficients are now inlined here from the physics,
/// so the test can disagree with the implementation.
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

    // Written out: Q_solar + Q_internal − UA(T_zone − T_out) − Q_cooling, over the interval.
    let solar = 3.0 * 800.0 * 0.60;
    let internal = 300.0;
    let ua = 45.0;
    let loss = ua * (state.temperature_k - outdoors.temperature_k);
    let supplied = solar + internal - loss - cooling;
    let expected = supplied * dt;

    let stored = 120_000.0 * (after.temperature_k - state.temperature_k);
    assert!(
        (stored - expected).abs() < 1e-9 * expected.abs().max(1.0),
        "stored {stored:.6} J vs an independent balance of {expected:.6} J"
    );

    // Sign checks either side of the free-float point -- taken *from* that point rather than from numbers guessed
    // to be either side of it. The first version used 60 C for "above", which is below this room's 73.67 C
    // equilibrium: the room correctly warmed, and the test failed. Guessing a temperature and hoping it lands on
    // the right side of an unstated equilibrium is the same error as guessing the equilibrium.
    let equilibrium = outdoors.temperature_k + (solar + internal) / ua;
    assert!(
        (equilibrium - ZERO_CELSIUS - 73.67).abs() < 0.01,
        "this room's free-float point is 73.67 C, got {:.2} C",
        equilibrium - ZERO_CELSIUS
    );
    let below = ZoneState {
        temperature_k: equilibrium - 5.0,
    };
    let above = ZoneState {
        temperature_k: equilibrium + 5.0,
    };
    assert!(
        zone.step(&below, &outdoors, &occupant(), 0.0, dt)
            .temperature_k
            > below.temperature_k,
        "a room below its equilibrium must warm"
    );
    assert!(
        zone.step(&above, &outdoors, &occupant(), 0.0, dt)
            .temperature_k
            < above.temperature_k,
        "a room above its equilibrium must cool"
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
    // Against the closed form, computed independently: T_out + Q/UA.
    let expected = outdoors.temperature_k + (3.0 * 800.0 * 0.60 + 300.0) / 45.0;
    assert!(
        (settled - expected).abs() < 1e-9,
        "free float should be T_out + Q/UA = {expected:.4} K, got {settled:.4}"
    );

    // And the fixed point is real: the net gain there is zero, and it is a stable point (a room displaced from it
    // moves back), which the equilibrium identity alone does not demonstrate.
    let net = zone.net_gain(&state, &outdoors, &occupant());
    assert!(
        net.abs() < 1e-9,
        "a settled room has no net gain, got {net:.9} W"
    );
    let dt = 600.0;
    let above = zone.step(
        &ZoneState {
            temperature_k: settled + 5.0,
        },
        &outdoors,
        &occupant(),
        0.0,
        dt,
    );
    let below = zone.step(
        &ZoneState {
            temperature_k: settled - 5.0,
        },
        &outdoors,
        &occupant(),
        0.0,
        dt,
    );
    assert!(above.temperature_k < settled + 5.0, "stable from above");
    assert!(below.temperature_k > settled - 5.0, "stable from below");
    assert!(
        (above.temperature_k - settled).abs() < (below.temperature_k - settled).abs() * 1.01 + 1e-9,
        "symmetric approach: {:.4} K from above vs {:.4} K from below",
        (above.temperature_k - settled).abs(),
        (below.temperature_k - settled).abs()
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
    // A window unit sits at COP 2.6-3.5 (EER 9-12). The earlier band here was "3-6", which is why an EER of
    // 16.4 passed: the band was wide enough to admit a machine that does not exist in this class.
    assert!(
        (2.6..=3.5).contains(&cop),
        "COP {cop:.2} is outside the EER 9-12 band a window unit actually achieves"
    );

    // Less lift, better COP -- and the *value* is pinned, not just its sign. `cop_ratio_between_lifts` returns
    // lift_2/lift_1, which equals the COP ratio only because Copeland's COP is inversely proportional to lift.
    let ratio = cop_ratio_between_lifts(20.0, 40.0).expect("computable");
    assert!(
        (ratio - 2.0).abs() < 1e-12,
        "halving the lift doubles the COP exactly in this map, got {ratio}"
    );
    // The inverse calls must be reciprocal, which pins the direction the doc got wrong.
    let back = cop_ratio_between_lifts(40.0, 20.0).expect("computable");
    assert!(
        (ratio * back - 1.0).abs() < 1e-12,
        "the two directions must be reciprocal, got {ratio} and {back}"
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

/// The draw is *not* linear in the load, and the deviation is the cycling penalty and nothing else.
///
/// The first version of this test asserted exact linearity, which pinned the model's absence of part-load
/// behaviour as if it were a feature. What is true: at full load the draw is the nameplate, at zero load it is
/// zero, and in between it exceeds the straight line by exactly `1/(1 − C_d(1 − PLF))`.
#[test]
fn draw_is_zero_at_zero_and_the_nameplate_at_full_load() {
    let ac = VapourCompression::window_unit();
    let capacity = ac.capacity_w().expect("capacity");

    assert_eq!(ac.draw_for(0.0), Some(0.0), "an idle unit draws nothing");
    assert!(
        (ac.draw_for(capacity).expect("at capacity") - ac.rated_draw_w.value).abs() < 1e-9,
        "at capacity the draw is the nameplate"
    );

    // Every part load is above the straight line from (0, 0) to (capacity, rated_draw).
    for fraction in [0.1, 0.25, 0.5, 0.75, 0.9, 0.99] {
        let actual = ac.draw_for(capacity * fraction).expect("inside capacity");
        let linear = ac.rated_draw_w.value * fraction;
        assert!(
            actual > linear,
            "at {fraction:.0} % load the cycling penalty must raise the draw: {actual:.1} vs {linear:.1}"
        );
        // ...but never by more than the ASHRAE maximum at zero runtime fraction.
        assert!(
            actual < linear / (1.0 - ac.cycling_degradation.value),
            "the penalty cannot exceed the Cd bound at {fraction:.0} % load"
        );
    }
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

    // The round trip alone proves only that the two functions invert each other -- they could both be wrong. The
    // absolute temperature is therefore pinned too: 500 W over 2 m² at ε 0.9 against 4 °C is 49.21 °C.
    assert!(
        (t - ZERO_CELSIUS - 49.21).abs() < 0.01,
        "the sized radiator runs at 49.21 C, got {:.2} C",
        t - ZERO_CELSIUS
    );

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
