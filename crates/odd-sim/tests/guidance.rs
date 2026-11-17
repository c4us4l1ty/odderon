//! Guided TNW compilation: the ideal preview and finite command share the
//! same inertial impulse even when the reference state is not the ideal one.
use odd_astro::spline::Keyframe;
use odd_core::frames::State;
use odd_sim::{
    planner::{ManeuverNode, Plan, PlanError},
    Engine,
};
const MU: f64 = 3.986_004_418e14;
fn initial() -> Keyframe {
    let r = 6_578_137.0;
    Keyframe {
        time: 0.0,
        position: [r, 0.0, 0.0],
        velocity: [0.0, libm::sqrt(MU / r), 0.0],
        mass: 18_000.0,
    }
}
fn engine() -> Engine {
    Engine {
        thrust_newtons: 18_000.0,
        specific_impulse_seconds: 450.0,
        stage: 0,
        engine: 0,
    }
}
#[test]
fn guided_frame_freezes_inertial_direction_for_both_levels() {
    let nodes = [ManeuverNode {
        epoch_us: 1_600_000_000,
        tnw_m_s: [100.0, 0.0, 0.0],
    }];
    // A deliberately different pre-burn reference: rotated velocity direction.
    let guide = State {
        position_m: [6_578_137.0, 0.0, 0.0],
        velocity_m_s: [100.0, 7700.0, 0.0],
    };
    let plan = Plan::compile_guided(
        initial(),
        MU,
        10_000_000_000,
        5200.0,
        engine(),
        &nodes,
        &[Some(guide)],
        None,
    )
    .unwrap();
    let ideal_dv = plan.impulses[0].delta_v_m_s;
    let burns = plan.timeline.burns(&[engine()]).unwrap();
    let force = burns[0].force_newtons;
    let thrust = libm::sqrt(force.iter().map(|x| x * x).sum::<f64>());
    let dir = force.map(|x| x / thrust);
    let dv_dir = ideal_dv.map(|x| x / 100.0);
    for i in 0..3 {
        assert!(
            (dir[i] - dv_dir[i]).abs() < 1e-4,
            "axis {i}: {dir:?} vs {dv_dir:?}"
        );
    }
    // Empty guidance uses the ideal frame and therefore a different direction.
    let ideal = Plan::compile(initial(), MU, 10_000_000_000, 5200.0, engine(), &nodes).unwrap();
    assert!((ideal.impulses[0].delta_v_m_s[0] - ideal_dv[0]).abs() > 1.0);
    let empty = Plan::compile_guided(
        initial(),
        MU,
        10_000_000_000,
        5200.0,
        engine(),
        &nodes,
        &[],
        None,
    )
    .unwrap();
    assert_eq!(empty.impulses[0].delta_v_m_s, ideal.impulses[0].delta_v_m_s);
    assert!(matches!(
        Plan::compile_guided(
            initial(),
            MU,
            10_000_000_000,
            5200.0,
            engine(),
            &nodes,
            &[None, None],
            None
        ),
        Err(PlanError::InvalidInput)
    ));
}

#[test]
fn thermal_boiloff_reduces_mass_before_burn() {
    let nodes = [ManeuverNode {
        epoch_us: 1_600_000_000,
        tnw_m_s: [3120.0, 0.0, 0.0],
    }];
    // No boil-off: baseline plan.
    let baseline = Plan::compile_guided(
        initial(),
        MU,
        10_000_000_000,
        5200.0,
        engine(),
        &nodes,
        &[],
        None,
    )
    .unwrap();
    let baseline_mass = baseline.sample(1_600_000_000).unwrap().mass;

    // Constant 1 kg/s boil-off over 1600 s = 1600 kg lost before the burn.
    let mut loss_kg = 0.0;
    let with_boiloff = Plan::compile_guided(
        initial(),
        MU,
        10_000_000_000,
        5200.0,
        engine(),
        &nodes,
        &[],
        Some(&mut |start_s, end_s| {
            let loss = (end_s - start_s) * 1.0;
            loss_kg += loss;
            loss
        }),
    )
    .unwrap();
    let boiloff_mass = with_boiloff.sample(1_600_000_000).unwrap().mass;

    // Mass should be lower — boil-off reduces what the burn can draw on.
    // (Not exactly `loss_kg` because the rocket equation burns less
    // propellant when the pre-burn mass is lower.)
    assert!(
        boiloff_mass < baseline_mass,
        "{boiloff_mass} vs {baseline_mass}"
    );
}

#[test]
fn thermal_boiloff_can_cause_propellant_deficit() {
    let nodes = [ManeuverNode {
        epoch_us: 1_600_000_000,
        tnw_m_s: [3120.0, 0.0, 0.0],
    }];
    // 20 kg/s boil-off over 1600 s = 32,000 kg lost — far more than the
    // 18,000 kg initial mass, so the burn cannot proceed.
    let result = Plan::compile_guided(
        initial(),
        MU,
        10_000_000_000,
        5200.0,
        engine(),
        &nodes,
        &[],
        Some(&mut |_, _| 20_000.0),
    );
    assert!(matches!(result, Err(PlanError::PropellantDeficit)));
}

#[test]
fn thermal_boiloff_none_matches_legacy() {
    let nodes = [ManeuverNode {
        epoch_us: 1_600_000_000,
        tnw_m_s: [3120.0, 0.0, 0.0],
    }];
    let legacy = Plan::compile(initial(), MU, 10_000_000_000, 5200.0, engine(), &nodes).unwrap();
    let none = Plan::compile_guided(
        initial(),
        MU,
        10_000_000_000,
        5200.0,
        engine(),
        &nodes,
        &[],
        None,
    )
    .unwrap();
    assert_eq!(
        legacy.sample(1_600_000_000).unwrap().mass,
        none.sample(1_600_000_000).unwrap().mass
    );
    assert_eq!(legacy.commanded_propellant_kg, none.commanded_propellant_kg);
}
