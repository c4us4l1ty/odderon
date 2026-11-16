//! `oracle_true_single_segment_resumption_cost`.
//!
//! The v3 checkpoint path was honest about its limits: it verified a chain
//! *after* replaying the whole flight, so a resumption at day 4 still paid for
//! days 0–3. v4's coupled propagator is state-complete — position, velocity,
//! the propellant pool, both tank columns and the accumulated cooler draw —
//! so a segment can start where the previous one stopped.
//!
//! This oracle measures that instead of asserting it. Three claims:
//!
//! 1. **Resuming is exact.** A leg flown `[0, t₁]` and resumed at `t₁` reaches
//!    the same terminal state as one flown in a single pass. Step-size history
//!    differs between the two, so position, velocity and the integrated burn
//!    draw are compared to a tolerance far below any physical effect; the
//!    accounting identity `flown mass = dry + pool` is compared exactly.
//! 2. **The cost is the segment's, not the mission's.** Right-hand-side
//!    evaluations for a one-day leg are a fraction of the four-day flight's,
//!    not merely reduced.
//! 3. **The cost does not depend on *when* the segment starts.** Two legs of
//!    equal length, one early and one late, cost the same within a small
//!    factor. That is what "O(1) resumption" means here: starting later is
//!    free, and only the work actually requested is charged.

use odd_astro::coupled::{integrate_coupled, CoupledCheckpoint, CoupledOutcome, CoupledSetup};
use odd_astro::integrator::{IntegrationError, Options};
use odd_astro::perturb::burn::Burn;
use odd_astro::perturb::pool::PropellantPool;
use odd_astro::spline::Keyframe;
use odd_systems::{
    BusSpec, Fluid, ShellMaterial, StageTankModel, TankSpec, ThermalContext, ThermalEnvironment,
};

const MU: f64 = 3.986_004_418e14;
const EARTH: f64 = 6_378_137.0;
const DAY: f64 = 86_400.0;

fn options() -> Options {
    Options {
        relative_tolerance: 1e-10,
        absolute_tolerance: 1e-10,
        initial_step: 10.0,
        max_step: 120.0,
        max_steps: 4_000_000,
    }
}

fn parking() -> Keyframe {
    let r = EARTH + 200_000.0;
    Keyframe {
        time: 0.0,
        position: [r, 0.0, 0.0],
        velocity: [0.0, libm::sqrt(MU / r), 0.0],
        mass: 1.0,
    }
}

fn bare_tank(fluid: Fluid) -> TankSpec {
    TankSpec {
        fluid,
        volume_m3: 20.0,
        area_m2: 20.0,
        mli_layers: 2,
        epsilon_eff: odd_systems::thermal::ideal_epsilon_for_layers(2) * 8.0,
        shell: ShellMaterial::Aluminium2219,
        absorptivity: 0.4,
        outer_epsilon: 0.85,
        strut_conductance_w_k: 2.0,
        relief_pressure_pa: 200_000.0,
        radiator_area_m2: 0.0,
        radiator_epsilon: 0.8,
        cryocooler_lift_w: 0.0,
        cryocooler_carnot_fraction: 0.25,
    }
}

fn sunlit_environment(_t: f64) -> Result<ThermalEnvironment, IntegrationError> {
    let radius = EARTH + 800_000.0;
    Ok(ThermalEnvironment {
        sun: [1.495_978_7e11, 0.0, 0.0],
        earth: [0.0, 0.0, -radius],
        moon: [4.0e8, 0.0, 0.0],
    })
}

type Stage = StageTankModel<fn(f64) -> Result<ThermalEnvironment, IntegrationError>>;

fn stage() -> Stage {
    StageTankModel::new(
        bare_tank(Fluid::Lox),
        bare_tank(Fluid::Lh2),
        BusSpec {
            array_w_at_1au: 0.0,
            avionics_w: 0.0,
            electric_propulsion_w: 0.0,
        },
        ThermalContext {
            structure_temperature_k: 293.0,
            sink_temperature_k: 3.0,
            bus_w: 0.0,
        },
        sunlit_environment as fn(f64) -> Result<ThermalEnvironment, IntegrationError>,
    )
    .expect("a valid cryogenic stage")
}

fn gravity(_t: f64, r: [f64; 3], _v: [f64; 3], _m: f64) -> Result<[f64; 3], IntegrationError> {
    let radius = libm::sqrt(r[0] * r[0] + r[1] * r[1] + r[2] * r[2]);
    Ok([
        -(MU / radius / radius) * (r[0] / radius),
        -(MU / radius / radius) * (r[1] / radius),
        -(MU / radius / radius) * (r[2] / radius),
    ])
}

/// Two burns: a short one near the start, the real one after the long coast.
fn burns() -> Vec<Burn> {
    vec![
        Burn {
            start: 60.0,
            end: 160.0,
            force_newtons: [18_000.0, 0.0, 0.0],
            specific_impulse_seconds: 450.0,
        },
        Burn {
            start: 4.0 * DAY,
            end: 4.0 * DAY + 300.0,
            force_newtons: [18_000.0, 0.0, 0.0],
            specific_impulse_seconds: 450.0,
        },
    ]
}

const DRY: f64 = 5_182.1;
const LOAD: f64 = 12_800.0;

fn pool() -> PropellantPool {
    PropellantPool::new(LOAD, 5.2).unwrap()
}

fn fly(start_s: f64, end_s: f64, resumed: Option<&CoupledCheckpoint>) -> CoupledOutcome {
    let burns = burns();
    let setup = CoupledSetup {
        dry_mass_kg: DRY,
        pool: pool(),
        burns: &burns,
        options: options(),
    };
    let mut tanks = stage();
    integrate_coupled(
        &setup,
        parking(),
        start_s,
        end_s,
        gravity,
        &mut tanks,
        resumed,
    )
    .expect("the coupled leg must propagate")
}

fn terminal(outcome: &CoupledOutcome) -> Keyframe {
    outcome
        .trajectory
        .keyframe(outcome.trajectory.len() - 1)
        .expect("a non-empty trajectory")
}

fn relative(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3)
        .map(|i| (a[i] - b[i]).abs() / b[i].abs().max(1.0))
        .fold(0.0, f64::max)
}

#[test]
fn oracle_true_single_segment_resumption_cost() {
    let end_s = 4.0 * DAY + 400.0;

    // --- 1. A resumed leg reproduces the single-pass flight. ---
    let full = fly(0.0, end_s, None);
    let first_leg_end = 3.0 * DAY;
    let first_leg = fly(0.0, first_leg_end, None);
    let checkpoint = odd_astro::coupled::checkpoint_at(&first_leg, first_leg_end)
        .expect("the first leg ends on a keyframe");
    // The checkpoint is the whole state, not a summary: if any of these were
    // missing, the resumed leg could not reproduce the flight.
    assert!(checkpoint.position.iter().all(|v| v.is_finite()));
    assert!(checkpoint.pool.remaining_kg() > 0.0);
    assert!(checkpoint.tanks[1][1] < LOAD, "the tanks must have vented");
    assert_eq!(checkpoint.cooler_joules.to_bits(), 0.0f64.to_bits());

    let resumed = fly(first_leg_end, end_s, Some(&checkpoint));
    let a = terminal(&resumed);
    let b = terminal(&full);
    let pos_error = relative(a.position, b.position);
    let vel_error = relative(a.velocity, b.velocity);
    println!(
        "full rhs {} ({} keyframes), resumed rhs {} ({} keyframes), pos err {pos_error:.3e}, \
         vel err {vel_error:.3e}",
        full.rhs_evaluations,
        full.trajectory.len(),
        resumed.rhs_evaluations,
        resumed.trajectory.len()
    );
    assert!(
        pos_error < 1e-7,
        "a resumed segment must reproduce the trajectory, position error {pos_error:e}"
    );
    assert!(
        vel_error < 1e-7,
        "a resumed segment must reproduce the trajectory, velocity error {vel_error:e}"
    );
    // Mass agrees to solver accuracy, not to the last bit: burn draw is
    // integrated step by step (`draw_burn(.., width)`), and a resumed leg
    // starts a fresh step exactly at t1 where the single pass had one
    // straddling it, so the sums differ in their last few bits. What must be
    // exact is the accounting identity itself: the flown mass is always the
    // dry stage plus the live pool, in both flights.
    let mass_error = (a.mass - b.mass).abs() / b.mass;
    assert!(
        mass_error < 1e-12,
        "resumed final mass {} vs single pass {} ({mass_error:e})",
        a.mass,
        b.mass
    );
    for (label, outcome, frame) in [("resumed", &resumed, a), ("single pass", &full, b)] {
        assert_eq!(
            frame.mass.to_bits(),
            (DRY + outcome.pool.remaining_kg()).to_bits(),
            "{label} must fly exactly dry mass plus the live pool"
        );
    }
    let pool_error =
        (resumed.pool.remaining_kg() - full.pool.remaining_kg()).abs() / full.pool.remaining_kg();
    assert!(pool_error < 1e-12, "pool agreement {pool_error:e}");
    assert_eq!(resumed.flameout.is_none(), full.flameout.is_none());
    // The vent carried into the second leg is the first leg's, not a replay's.
    assert_eq!(checkpoint.pool.vented_kg(), first_leg.pool.vented_kg());

    // --- 2. The charge is the segment's, not the mission's. ---
    // One day out of four, and the resumed leg never touches days 0–2.
    let share = resumed.rhs_evaluations as f64 / full.rhs_evaluations as f64;
    assert!(
        share < 0.5,
        "resuming the last day of four must cost well under half the full flight, \
         got {share:.3} ({} vs {})",
        resumed.rhs_evaluations,
        full.rhs_evaluations
    );
    // The first leg plus the resumed leg together must cost about one flight:
    // segmenting is nearly free, so the saving is real work not done.
    let total = first_leg.rhs_evaluations + resumed.rhs_evaluations;
    assert!(
        (total as f64) < 1.35 * full.rhs_evaluations as f64,
        "two legs must not cost much more than one ({total} vs {})",
        full.rhs_evaluations
    );

    // --- 3. Starting later is free for a fixed segment length. ---
    let early_end = 0.5 * DAY;
    let early = fly(0.0, early_end, None);
    let early_cp = odd_astro::coupled::checkpoint_at(&early, early_end).unwrap();
    let early_resumed = fly(early_end, early_end + 0.5 * DAY, Some(&early_cp));

    let late_start = 3.5 * DAY;
    let late_cp =
        odd_astro::coupled::checkpoint_at(&fly(0.0, late_start, None), late_start).unwrap();
    let late_resumed = fly(late_start, late_start + 0.5 * DAY, Some(&late_cp));

    println!(
        "half-day leg early rhs {}, late rhs {}",
        early_resumed.rhs_evaluations, late_resumed.rhs_evaluations
    );
    let ratio = late_resumed.rhs_evaluations as f64 / early_resumed.rhs_evaluations as f64;
    assert!(
        (0.5..2.0).contains(&ratio),
        "equal-length legs must cost the same whether they start early or late, \
         early {} vs late {} (ratio {ratio:.2})",
        early_resumed.rhs_evaluations,
        late_resumed.rhs_evaluations
    );
    // And the late leg is charged nothing for the 3.5 days before it.
    assert!(
        late_resumed.rhs_evaluations < full.rhs_evaluations / 2,
        "the late leg must not pay for the coast it did not replay: {} vs {}",
        late_resumed.rhs_evaluations,
        full.rhs_evaluations
    );
}
