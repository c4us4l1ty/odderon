use odd_astro::anomaly::{
    hyperbolic_from_mean, hyperbolic_from_true, hyperbolic_mean, true_from_hyperbolic, AnomalyError,
};
use std::f64::consts::PI;
fn near(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a:.17e} != {b:.17e} (tol {tol})");
}

#[test]
fn hyperbolic_worked_identity_and_branch() {
    let mean = hyperbolic_mean(2.0, 1.0).unwrap();
    near(mean, 1.3504023872876028, 1e-14);
    near(hyperbolic_from_mean(2.0, mean).unwrap(), 1.0, 1e-13);
    let nu = true_from_hyperbolic(2.0, 1.0).unwrap();
    near(hyperbolic_from_true(2.0, nu).unwrap(), 1.0, 1e-13);
    assert_eq!(
        hyperbolic_from_true(2.0, PI),
        Err(AnomalyError::OutsidePhysicalBranch)
    );
    assert_eq!(
        hyperbolic_from_mean(1.0, 1.0),
        Err(AnomalyError::InvalidInput)
    );
    assert_eq!(
        hyperbolic_from_mean(2.0, f64::NAN),
        Err(AnomalyError::InvalidInput)
    );
}
#[test]
fn hyperbolic_solver_round_trips_across_scales() {
    for e in [1.0 + 1e-12, 1.001, 1.5, 10.0, 1000.0] {
        for f in [
            -20.0, -5.0, -1.0, -0.01, -1e-7, -1e-20, 0.0, 1e-20, 1e-7, 0.01, 1.0, 5.0, 20.0,
        ] {
            let mean = hyperbolic_mean(e, f).unwrap();
            let recovered = hyperbolic_from_mean(e, mean).unwrap();
            near(recovered, f, 2e-12 * f.abs().max(1e-20));
            if f.abs() < 5.0 {
                let nu = true_from_hyperbolic(e, f).unwrap();
                // For e near 1, ν is almost the asymptote and inversion is ill-conditioned.
                if e > 1.00001 {
                    near(
                        hyperbolic_from_true(e, nu).unwrap(),
                        f,
                        2e-11 * f.abs().max(1.0),
                    );
                }
            }
        }
    }
}

#[test]
fn elliptic_kepler_round_trips_at_singular_limits() {
    use odd_astro::anomaly::{
        elliptic_from_mean, elliptic_from_true, elliptic_mean, true_from_elliptic,
    };
    for e in [0.0, 0.4, 0.9, 0.999, 1.0 - 1e-12] {
        for a in [
            -3.0, -1.0, -0.01, -1e-7, -1e-20, 0.0, 1e-20, 1e-7, 0.01, 1.0, 3.0,
        ] {
            let mean = elliptic_mean(e, a).unwrap();
            near(
                elliptic_from_mean(e, mean).unwrap(),
                a,
                3e-12 * a.abs().max(1e-20),
            );
            if e < 0.99999 {
                let nu = true_from_elliptic(e, a).unwrap();
                near(
                    elliptic_from_true(e, nu).unwrap(),
                    a,
                    3e-12 * a.abs().max(1.0),
                );
            }
        }
    }
    near(elliptic_from_mean(0.0, 2.0 * PI + 0.5).unwrap(), 0.5, 1e-14);
    assert_eq!(
        elliptic_from_mean(1.0, 0.0),
        Err(AnomalyError::InvalidInput)
    );
}
