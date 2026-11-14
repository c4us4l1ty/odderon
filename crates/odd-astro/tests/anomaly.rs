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

#[test]
fn independent_satkit_elliptic_anomalies() {
    use odd_astro::anomaly::{
        elliptic_from_mean, elliptic_from_true, elliptic_mean, true_from_elliptic,
    };
    let mut count = 0;
    for line in include_str!("../../../data/oracles/elliptic-anomaly-satkit.csv")
        .lines()
        .skip(1)
    {
        let row: Vec<f64> = line.split(',').map(|x| x.parse().unwrap()).collect();
        let (e, anomaly, mean, nu) = (row[0], row[1], row[2], row[3]);
        near(elliptic_mean(e, anomaly).unwrap(), mean, 2e-14);
        near(elliptic_from_mean(e, mean).unwrap(), anomaly, 5e-14);
        near(true_from_elliptic(e, anomaly).unwrap(), nu, 2e-14);
        near(elliptic_from_true(e, nu).unwrap(), anomaly, 2e-14);
        count += 1;
    }
    assert_eq!(count, 100);
}

#[test]
fn unrepresentably_small_anomalies_are_errors() {
    use odd_astro::anomaly::elliptic_mean;
    assert_eq!(
        hyperbolic_from_mean(1e300, 1e-100),
        Err(AnomalyError::NumericalRange)
    );
    assert_eq!(
        elliptic_mean(1.0 - f64::EPSILON, f64::from_bits(1)),
        Err(AnomalyError::NumericalRange)
    );
    assert_eq!(
        hyperbolic_mean(1.0 + f64::EPSILON, f64::from_bits(1)),
        Err(AnomalyError::NumericalRange)
    );
}

#[test]
fn hyperbolic_tiny_angles_remain_representable() {
    let e = 1.0 + f64::EPSILON;
    let f = f64::from_bits(1);
    let nu = true_from_hyperbolic(e, f).unwrap();
    assert!(nu > 0.0);
    let recovered = hyperbolic_from_true(e, nu).unwrap();
    assert_eq!(recovered, f);
}

#[test]
fn near_parabolic_elliptic_anomalies_avoid_cancellation() {
    use odd_astro::anomaly::{elliptic_from_true, true_from_elliptic};
    // 80-digit mpmath half-angle calculation, with the exact binary64 e.
    let e = 1.0 - f64::EPSILON;
    for (eccentric, expected_true) in [
        (1e-8, 0.886_131_745_558_101_2),
        (1e-9, 0.094_835_125_079_021),
        (1e-7, 2.726_202_012_841_388),
    ] {
        let result = true_from_elliptic(e, eccentric).unwrap();
        near(result, expected_true, 1e-15);
        near(elliptic_from_true(e, result).unwrap(), eccentric, 1e-22);
    }
    let smallest = f64::from_bits(1);
    assert!(true_from_elliptic(e, smallest).unwrap() > 0.0);
    assert_eq!(
        elliptic_from_true(e, smallest),
        Err(AnomalyError::NumericalRange)
    );
}

#[test]
fn hyperbolic_near_asymptote_uses_stable_denominator() {
    // 80-digit mpmath using the exact binary64 angle and eccentricity.
    let e = 1.0 + f64::EPSILON;
    let nu = PI - 3e-8;
    let result = hyperbolic_from_true(e, nu).unwrap();
    near(result, 1.744_231_428_086_033_4, 2e-15);
}
