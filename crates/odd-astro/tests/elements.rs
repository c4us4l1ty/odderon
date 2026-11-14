use odd_astro::elements::{Elements, ElementsError, SpecialAngles};
use odd_core::frames::State;
use std::f64::consts::{PI, TAU};

const MU: f64 = 3.986_004_418e14;
fn near(a: f64, b: f64, tol: f64) {
    assert!(
        (a - b).abs() <= tol,
        "{a:.15e} != {b:.15e}, tolerance {tol}"
    );
}
fn same_state(a: State, b: State, position_tol: f64, velocity_tol: f64) {
    for j in 0..3 {
        near(a.position_m[j], b.position_m[j], position_tol);
        near(a.velocity_m_s[j], b.velocity_m_s[j], velocity_tol);
    }
}
fn elements(e: f64, i: f64, raan: f64, arg: f64, nu: f64) -> Elements {
    Elements {
        semi_latus_rectum_m: 8_000_000.0,
        eccentricity: e,
        inclination_rad: i,
        ascending_node_rad: raan,
        argument_of_periapsis_rad: arg,
        true_anomaly_rad: nu,
    }
}

#[test]
fn vallado_state_and_independent_satkit_oracle() {
    // satkit 0.24.3 from_pv with the exact specified μ and input; the SatKit
    // tutorial independently publishes the rounded Vallado example values.
    let state = State {
        position_m: [6_524_834.0, 6_862_875.0, 6_448_296.0],
        velocity_m_s: [4_901.327, 5_533.756, -1_976.341],
    };
    let el = Elements::from_state(state, MU).unwrap();
    near(el.semi_major_axis_m().unwrap(), 36_127_337.619_678_68, 1e-5);
    near(el.eccentricity, 0.832_853_398_487_521_5, 1e-14);
    for (value, expected) in [
        (el.inclination_rad, 1.533_605_562_639_449),
        (el.ascending_node_rad, 3.977_575_002_801_695),
        (el.argument_of_periapsis_rad, 0.931_742_810_240_855_7),
        (el.true_anomaly_rad, 1.611_552_500_844_404),
    ] {
        near(value, expected, 2e-14);
    }
    same_state(el.to_state(MU).unwrap(), state, 1e-7, 1e-10);
    // Rounded reference angle 53.38° cannot itself satisfy the literal 0.001%
    // gate, since the independently computed value is 53.3849306... degrees.
    near(el.argument_of_periapsis_rad.to_degrees(), 53.38, 0.005);
}

#[test]
fn circular_and_equatorial_including_retrograde() {
    for &(i, raan, arg, nu) in &[
        (0.0, 0.0, 0.0, 1.2),
        (PI, 0.0, 0.0, 1.2),
        (0.7, 2.3, 0.0, 1.2),
        (1e-10, 1.3, 0.8, 2.1),
        (PI - 1e-10, 1.3, 0.8, 2.1),
    ] {
        for &e in &[0.0, 1e-10, 0.3] {
            let start = elements(e, i, raan, arg, nu).to_state(MU).unwrap();
            let result = Elements::from_state(start, MU).unwrap();
            same_state(result.to_state(MU).unwrap(), start, 2e-7, 2e-10);
            let class = result.special_angles().unwrap();
            let circ = e < 1e-8;
            let equat = i.sin().abs() < 1e-8;
            assert!(matches!(
                (circ, equat, class),
                (true, true, SpecialAngles::CircularEquatorial { .. })
                    | (true, false, SpecialAngles::CircularInclined { .. })
                    | (false, true, SpecialAngles::EccentricEquatorial { .. })
                    | (false, false, SpecialAngles::General)
            ));
        }
    }
    let retro = elements(0.0, PI, 0.0, 0.0, 1.2);
    match retro.special_angles().unwrap() {
        SpecialAngles::CircularEquatorial { true_longitude_rad } => {
            near(true_longitude_rad, TAU - 1.2, 1e-14)
        }
        _ => panic!("wrong class"),
    }
    let retro = elements(0.2, PI, 0.0, 1.2, 0.4);
    match retro.special_angles().unwrap() {
        SpecialAngles::EccentricEquatorial {
            longitude_of_periapsis_rad,
        } => near(longitude_of_periapsis_rad, TAU - 1.2, 1e-14),
        _ => panic!("wrong class"),
    }
}

#[test]
fn representative_conics_round_trip() {
    for &e in &[0.00001, 0.2, 0.9, 0.999999, 1.0, 1.000001, 1.5, 4.0] {
        for &i in &[0.0, 1e-9, 0.4, PI / 2.0, 2.3, PI - 1e-9, PI] {
            for &nu in &[-1.0, 0.0, 1.0] {
                let source = elements(e, i, 0.8, 2.1, nu);
                let state = source.to_state(MU).unwrap();
                let recovered = Elements::from_state(state, MU).unwrap();
                same_state(recovered.to_state(MU).unwrap(), state, 1e-6, 1e-9);
            }
        }
    }
    assert!(elements(1.0, 0.5, 0.0, 0.0, 0.0)
        .semi_major_axis_m()
        .is_none());
    assert!(
        elements(1.5, 0.5, 0.0, 0.0, 0.0)
            .semi_major_axis_m()
            .unwrap()
            < 0.0
    );
}

#[test]
fn rejects_invalid_and_nonphysical_inputs() {
    assert_eq!(
        Elements::from_state(
            State {
                position_m: [0.0; 3],
                velocity_m_s: [0.0; 3]
            },
            MU
        ),
        Err(ElementsError::InvalidInput)
    );
    assert_eq!(
        Elements::from_state(
            State {
                position_m: [1.0, 0.0, 0.0],
                velocity_m_s: [2.0, 0.0, 0.0]
            },
            MU
        ),
        Err(ElementsError::RadialTrajectory)
    );
    assert_eq!(
        elements(1.5, 0.5, 0.0, 0.0, PI).to_state(MU),
        Err(ElementsError::OutsidePhysicalBranch)
    );
    assert_eq!(
        elements(-1.0, 0.5, 0.0, 0.0, 0.0).to_state(MU),
        Err(ElementsError::InvalidInput)
    );
    assert_eq!(
        elements(0.1, PI + 0.01, 0.0, 0.0, 0.0).to_state(MU),
        Err(ElementsError::InvalidInput)
    );
    assert_eq!(
        elements(0.1, 0.5, 0.0, 0.0, f64::NAN).to_state(MU),
        Err(ElementsError::InvalidInput)
    );
}

#[test]
fn independent_satkit_elliptical_states() {
    // See tools/astro-oracle/generate_satkit.py for the independent generator.
    let fixtures = include_str!("../../../data/oracles/orbits-satkit.csv");
    let mut count = 0;
    for line in fixtures.lines().skip(1) {
        let v: Vec<f64> = line.split(',').map(|s| s.parse().unwrap()).collect();
        assert_eq!(v.len(), 12);
        let input = Elements {
            semi_latus_rectum_m: v[0],
            eccentricity: v[1],
            inclination_rad: v[2],
            ascending_node_rad: v[3],
            argument_of_periapsis_rad: v[4],
            true_anomaly_rad: v[5],
        };
        let expected = State {
            position_m: [v[6], v[7], v[8]],
            velocity_m_s: [v[9], v[10], v[11]],
        };
        let actual = input.to_state(MU).unwrap();
        // Absolute tolerances bound component errors even near a zero crossing.
        same_state(actual, expected, 3e-6, 2e-9);
        let recovered = Elements::from_state(expected, MU).unwrap();
        same_state(recovered.to_state(MU).unwrap(), expected, 3e-6, 2e-9);
        count += 1;
    }
    assert_eq!(count, 100);
}

#[test]
fn independent_cspice_hyperbolic_states() {
    use odd_astro::anomaly::{
        hyperbolic_from_mean, hyperbolic_from_true, hyperbolic_mean, true_from_hyperbolic,
    };
    let fixtures = include_str!("../../../data/oracles/hyperbolic-cspice.csv");
    let mut count = 0;
    for line in fixtures.lines().skip(1) {
        let v: Vec<f64> = line.split(',').map(|s| s.parse().unwrap()).collect();
        assert_eq!(v.len(), 12);
        let true_anomaly =
            true_from_hyperbolic(v[1], hyperbolic_from_mean(v[1], v[5]).unwrap()).unwrap();
        let input = Elements {
            semi_latus_rectum_m: v[0],
            eccentricity: v[1],
            inclination_rad: v[2],
            ascending_node_rad: v[3],
            argument_of_periapsis_rad: v[4],
            true_anomaly_rad: true_anomaly,
        };
        let expected = State {
            position_m: [v[6], v[7], v[8]],
            velocity_m_s: [v[9], v[10], v[11]],
        };
        same_state(input.to_state(MU).unwrap(), expected, 5e-4, 1e-8);
        let out = Elements::from_state(expected, MU).unwrap();
        same_state(out.to_state(MU).unwrap(), expected, 5e-4, 1e-8);
        near(out.eccentricity, v[1], 2e-13);
        near(
            hyperbolic_mean(
                out.eccentricity,
                hyperbolic_from_true(out.eccentricity, out.true_anomaly_rad).unwrap(),
            )
            .unwrap(),
            v[5],
            2e-12,
        );
        count += 1;
    }
    assert_eq!(count, 100);
}

#[test]
fn independent_cspice_parabolic_states() {
    let fixtures = include_str!("../../../data/oracles/parabolic-cspice.csv");
    let mut count = 0;
    for line in fixtures.lines().skip(1) {
        let v: Vec<f64> = line.split(',').map(|s| s.parse().unwrap()).collect();
        assert_eq!(v.len(), 11);
        let input = Elements {
            semi_latus_rectum_m: v[0],
            eccentricity: 1.0,
            inclination_rad: v[1],
            ascending_node_rad: v[2],
            argument_of_periapsis_rad: v[3],
            true_anomaly_rad: v[4],
        };
        let expected = State {
            position_m: [v[5], v[6], v[7]],
            velocity_m_s: [v[8], v[9], v[10]],
        };
        same_state(input.to_state(MU).unwrap(), expected, 1e-5, 1e-9);
        let out = Elements::from_state(expected, MU).unwrap();
        same_state(out.to_state(MU).unwrap(), expected, 1e-5, 1e-9);
        near(out.eccentricity, 1.0, 2e-14);
        count += 1;
    }
    assert_eq!(count, 50);
}

#[test]
fn classical_axes_and_degenerate_parabolic_size() {
    for (a, e) in [(12_000_000.0, 0.4), (-12_000_000.0, 1.4)] {
        let el = Elements::from_classical(a, e, 0.3, 0.7, 0.9, 0.2).unwrap();
        near(el.semi_major_axis_m().unwrap(), a, 1e-8);
        let state = el.to_state(MU).unwrap();
        same_state(
            Elements::from_state(state, MU)
                .unwrap()
                .to_state(MU)
                .unwrap(),
            state,
            2e-7,
            2e-10,
        );
    }
    assert_eq!(
        Elements::from_classical(10.0, 1.0, 0.0, 0.0, 0.0, 0.0),
        Err(ElementsError::InvalidInput)
    );
    assert_eq!(
        Elements::from_classical(-10.0, 0.5, 0.0, 0.0, 0.0, 0.0),
        Err(ElementsError::InvalidInput)
    );
    assert_eq!(
        Elements::from_classical(-10.0, 2.0, 0.0, 0.0, 0.0, PI),
        Err(ElementsError::OutsidePhysicalBranch)
    );
}

#[test]
fn singular_combinations_remain_correct_for_nearby_states() {
    for &i in &[1e-10, PI - 1e-10] {
        let original = elements(1e-10, i, 1.3, 0.8, 2.1);
        let state = original.to_state(MU).unwrap();
        let out = Elements::from_state(state, MU).unwrap();
        let actual = match out.special_angles().unwrap() {
            SpecialAngles::CircularEquatorial { true_longitude_rad } => true_longitude_rad,
            _ => panic!("wrong class"),
        };
        let expected = state.position_m[1]
            .atan2(state.position_m[0])
            .rem_euclid(TAU);
        near(actual, expected, 1e-14);
        let original = elements(0.3, i, 1.3, 0.8, 2.1);
        let out = Elements::from_state(original.to_state(MU).unwrap(), MU).unwrap();
        let actual = match out.special_angles().unwrap() {
            SpecialAngles::EccentricEquatorial {
                longitude_of_periapsis_rad,
            } => longitude_of_periapsis_rad,
            _ => panic!("wrong class"),
        };
        let direction = if i > PI / 2.0 { -1.0 } else { 1.0 };
        near(actual, (1.3_f64 + direction * 0.8).rem_euclid(TAU), 1e-13);
    }
    let original = elements(1e-10, 0.8, 1.3, 0.8, 2.1);
    let out = Elements::from_state(original.to_state(MU).unwrap(), MU).unwrap();
    match out.special_angles().unwrap() {
        SpecialAngles::CircularInclined {
            argument_of_latitude_rad,
        } => near(argument_of_latitude_rad, 2.9, 1e-13),
        _ => panic!("wrong class"),
    }
}
