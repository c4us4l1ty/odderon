//! oracle_hardening_clamp_and_exact_sweep (Rust half).
//!
//! `BurnLoss::sweep_rad` must be the exact polar-angle integral
//! `∫ |r × v| / |r|² dt`, not a sum of endpoint chord angles. For a planar
//! Keplerian arc the two agree, which gives an independent analytic oracle:
//! the swept angle equals the angle between the endpoint radius vectors.
//! They diverge once thrust tilts the plane, because the 3D angle between two
//! radius vectors is not the polar angle swept in the instantaneous orbital
//! plane — and the gap grows as keyframes are decimated.
//!
//! The clamp half of this gate lives in tests/browser/observatory.mjs.
use odd_astro::{
    finite_burn::measure_burn,
    force_model::ForceModel,
    integrator::{IntegrationError, Options},
    perturb::burn::Burn,
    spline::{Keyframe, TrajectorySpline},
};

const MU: f64 = 3.986_004_418e14;

fn norm(v: [f64; 3]) -> f64 {
    libm::sqrt(v.iter().map(|x| x * x).sum())
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}
fn options() -> Options {
    Options {
        relative_tolerance: 1e-12,
        absolute_tolerance: 1e-12,
        initial_step: 1.0,
        max_step: 20.0,
        max_steps: 2_000_000,
    }
}
fn model() -> ForceModel {
    ForceModel {
        mu: MU,
        zonal: None,
        drag: None,
        radiation: None,
        third_body: false,
        burn: None,
    }
}

/// The superseded formulation, kept here only to prove the replacement is a
/// real improvement rather than a refactor.
fn chord_angle_sum(trajectory: &TrajectorySpline, burn: &Burn) -> f64 {
    let mut total = 0.0;
    for i in 1..trajectory.len() {
        let k0 = trajectory.keyframe(i - 1).unwrap();
        let k1 = trajectory.keyframe(i).unwrap();
        let lo = k0.time.max(burn.start);
        let hi = k1.time.min(burn.end);
        if hi <= lo {
            continue;
        }
        let p0 = trajectory.sample(lo).unwrap().position;
        let p1 = trajectory.sample(hi).unwrap().position;
        total += libm::fabs(libm::atan2(norm(cross(p0, p1)), dot(p0, p1)));
    }
    total
}

/// Keep every `every`-th keyframe (plus the last). The cubic Hermite dense
/// output still passes through the retained states with their exact
/// derivatives, so this isolates quadrature error from integration error.
fn decimate(trajectory: &TrajectorySpline, every: usize) -> TrajectorySpline {
    let mut out = TrajectorySpline::new();
    let n = trajectory.len();
    for i in 0..n {
        if i % every == 0 || i == n - 1 {
            out.push(trajectory.keyframe(i).unwrap()).unwrap();
        }
    }
    out
}

fn scenario(force: [f64; 3], duration: f64) -> (TrajectorySpline, Burn) {
    let r = 6_700_000.0;
    let initial = Keyframe {
        time: 0.0,
        position: [r, 0.0, 0.0],
        velocity: [0.0, libm::sqrt(MU / r), 0.0],
        mass: 1000.0,
    };
    let burn = Burn {
        start: 100.0,
        end: 100.0 + duration,
        force_newtons: force,
        specific_impulse_seconds: 450.0,
    };
    let trajectory = model()
        .propagate_with_burns(initial, burn.end + 10.0, options(), &[burn], |_| {
            Err(IntegrationError::ForceFailure)
        })
        .expect("two-body burn propagates");
    (trajectory, burn)
}

#[test]
fn oracle_hardening_exact_sweep_matches_analytic_arc() {
    // Negligible thrust leaves the orbit Keplerian to ~1e-10 relative, so the
    // swept polar angle is analytically the angle between the endpoint radii.
    let (trajectory, burn) = scenario([0.0, 1e-6, 0.0], 900.0);
    let p0 = trajectory.sample(burn.start).unwrap().position;
    let p1 = trajectory.sample(burn.end).unwrap().position;
    let analytic = libm::atan2(norm(cross(p0, p1)), dot(p0, p1));
    assert!(analytic > 0.5 && analytic < std::f64::consts::PI);

    for every in [1usize, 3, 5, 10, 15] {
        let decimated = decimate(&trajectory, every);
        let loss = measure_burn(&decimated, &burn, MU).unwrap();
        let error = (loss.sweep_rad - analytic).abs();
        assert!(
            error < 1e-6,
            "sweep {} vs analytic {analytic} differs by {error:e} rad at decimation {every}",
            loss.sweep_rad
        );
    }
}

#[test]
fn exact_sweep_beats_chord_angles_once_the_plane_tilts() {
    // A dominant orbit-normal component rotates the plane during the burn.
    let (trajectory, burn) = scenario([0.0, 20.0, 200.0], 1800.0);
    let reference = measure_burn(&trajectory, &burn, MU).unwrap().sweep_rad;

    for every in [1usize, 5, 15] {
        let decimated = decimate(&trajectory, every);
        let exact = measure_burn(&decimated, &burn, MU).unwrap().sweep_rad;
        assert!(
            (exact - reference).abs() < 1e-6,
            "exact sweep drifted {:e} rad at decimation {every}",
            exact - reference
        );
    }

    // At 15x decimation the retired chord sum misses by >1e-6 rad while the
    // integral stays two orders of magnitude tighter. This is the defect the
    // hardening pass removes; it is asserted so a revert cannot pass silently.
    let decimated = decimate(&trajectory, 15);
    let exact_error = (measure_burn(&decimated, &burn, MU).unwrap().sweep_rad - reference).abs();
    let chord_error = (chord_angle_sum(&decimated, &burn) - reference).abs();
    assert!(
        chord_error > 1e-6,
        "fixture no longer exercises the chord defect (error {chord_error:e})"
    );
    assert!(
        exact_error * 10.0 < chord_error,
        "exact {exact_error:e} should be far tighter than chord {chord_error:e}"
    );
}

#[test]
fn sweep_is_positive_and_additive_across_burns() {
    use odd_astro::finite_burn::measure_burns;
    let r = 6_700_000.0;
    let initial = Keyframe {
        time: 0.0,
        position: [r, 0.0, 0.0],
        velocity: [0.0, libm::sqrt(MU / r), 0.0],
        mass: 1000.0,
    };
    let burns = [
        Burn {
            start: 100.0,
            end: 400.0,
            force_newtons: [0.0, 150.0, 40.0],
            specific_impulse_seconds: 450.0,
        },
        Burn {
            start: 1200.0,
            end: 1500.0,
            force_newtons: [0.0, 150.0, 40.0],
            specific_impulse_seconds: 450.0,
        },
    ];
    let trajectory = model()
        .propagate_with_burns(initial, 1600.0, options(), &burns, |_| {
            Err(IntegrationError::ForceFailure)
        })
        .unwrap();
    let total = measure_burns(&trajectory, &burns, MU).unwrap();
    let parts: Vec<_> = burns
        .iter()
        .map(|b| measure_burn(&trajectory, b, MU).unwrap())
        .collect();
    assert!(parts.iter().all(|p| p.sweep_rad > 0.0));
    assert!((total.sweep_rad - parts.iter().map(|p| p.sweep_rad).sum::<f64>()).abs() < 1e-12);
    // The exact loss identity must survive the sweep change untouched.
    assert!(total.identity_residual_m_s.abs() < 1e-6);
}
