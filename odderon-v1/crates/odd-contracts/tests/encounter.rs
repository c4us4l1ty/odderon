//! Lunar encounter finder and patched-preview regression on a controlled
//! geometry. This does not claim the Contract 103 reference epoch encounters
//! the Moon; a 380,000 km apoapsis radius is not an encounter by itself.
use odd_astro::{
    integrator::{integrate, Options},
    spline::Keyframe,
};
use odd_core::frames::State;
use odd_sim::{
    encounter::{find_entry, LunarPatch},
    planner::{ManeuverNode, Plan},
    Engine,
};
const MOON_MU: f64 = 4.902_800_118e12;
#[test]
fn numerical_soi_entry_and_patch_round_trip() {
    // A straight-line numerical path crosses a stationary Moon-centered SOI.
    let initial = Keyframe {
        time: 0.0,
        position: [1.0, 0.0, 0.0],
        velocity: [1000.0, 1.0, 0.0],
        mass: 1000.0,
    };
    let opts = Options {
        relative_tolerance: 1e-12,
        absolute_tolerance: 1e-12,
        initial_step: 1.0,
        max_step: 10.0,
        max_steps: 10_000,
    };
    let path = integrate(initial, 200.0, &[], opts, |_, _, _, _| Ok(([0.0; 3], 0.0))).unwrap();
    let moon = |_| {
        Ok(State {
            position_m: [100_000.0, 0.0, 0.0],
            velocity_m_s: [0.0; 3],
        })
    };
    let entry = find_entry(&path, 20_000.0, MOON_MU, moon)
        .unwrap()
        .expect("entry");
    assert!((entry.time_s - 79.999).abs() < 1e-3);
    assert!((entry.vehicle_primary.position_m[0] - 80_000.0).abs() < 1.0);
    // Ideal plan patch: a coasting craft passes the same SOI.
    // Use a very weak central field so the ideal preview is nearly straight.
    let plan = Plan::compile(
        initial,
        1.0,
        200_000_000,
        900.0,
        Engine {
            thrust_newtons: 1000.0,
            specific_impulse_seconds: 450.0,
            stage: 0,
            engine: 0,
        },
        &[ManeuverNode {
            epoch_us: 1_000_000,
            tnw_m_s: [1.0, 0.0, 0.0],
        }],
    )
    .unwrap();
    let patch = LunarPatch::find(&plan, MOON_MU, 20_000.0, 1_000_000, moon)
        .unwrap()
        .expect("patch");
    assert!(
        (patch.entry.time_s - 80.0).abs() < 1.0,
        "patch entry {}",
        patch.entry.time_s
    );
    let before = patch.sample(&plan, 79_000_000, moon).unwrap();
    let plain = plan.sample(79_000_000).unwrap();
    assert_eq!(before.position, plain.position);
    let after = patch.sample(&plan, 100_000_000, moon).unwrap();
    let d = after
        .position
        .iter()
        .zip([100_000.0, 0.0, 0.0])
        .map(|(a, b)| a - b)
        .collect::<Vec<_>>();
    assert!(d.iter().map(|x| x * x).sum::<f64>().sqrt() < 20_000.0 + 1.0);
}
