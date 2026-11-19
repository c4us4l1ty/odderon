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

#[test]
fn tiny_inbound_anomalies_do_not_wrap_away() {
    for e in [0.5, 2.0] {
        let source = elements(e, 0.0, 0.0, 0.0, -1e-16);
        let out = Elements::from_state(source.to_state(MU).unwrap(), MU).unwrap();
        near(out.true_anomaly_rad, -1e-16, 1e-29);
    }
}

#[test]
fn seeded_conic_round_trips_cover_multiple_scales() {
    use odd_astro::anomaly::true_from_hyperbolic;
    let mut seed: u64 = 0x56b5_8adf_2291_0101;
    let mut sample = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((seed >> 11) as f64) / ((1u64 << 53) as f64)
    };
    for case in 0..2000 {
        let p = 1e5 * 10f64.powf(5.0 * sample());
        let e = if case % 2 == 0 {
            0.999 * sample()
        } else {
            1.001 + 4.0 * sample()
        };
        let i = if case % 7 == 0 {
            if case % 14 == 0 {
                1e-10
            } else {
                PI - 1e-10
            }
        } else {
            PI * sample()
        };
        let nu = if e < 1.0 {
            TAU * sample() - PI
        } else {
            true_from_hyperbolic(e, 6.0 * sample() - 3.0).unwrap()
        };
        let el = Elements {
            semi_latus_rectum_m: p,
            eccentricity: e,
            inclination_rad: i,
            ascending_node_rad: TAU * sample(),
            argument_of_periapsis_rad: TAU * sample(),
            true_anomaly_rad: nu,
        };
        let state = el.to_state(MU).unwrap();
        let back = Elements::from_state(state, MU)
            .unwrap()
            .to_state(MU)
            .unwrap();
        let radius = state.position_m.iter().map(|x| x * x).sum::<f64>().sqrt();
        let speed = state.velocity_m_s.iter().map(|x| x * x).sum::<f64>().sqrt();
        same_state(back, state, 2e-11 * radius, 2e-11 * speed);
    }
}

#[test]
fn near_parabolic_apocenter_radius_avoids_cancellation() {
    // 80-digit mpmath oracle using exact binary64 inputs, including ν.
    let source = elements(1.0 - f64::EPSILON, 0.0, 0.0, 0.0, PI - 1e-8);
    let state = source.to_state(MU).unwrap();
    let radius = state.position_m[0].hypot(state.position_m[1]);
    let expected = 2.940_694_223_310_72e22;
    assert!(
        (radius / expected - 1.0).abs() < 2e-15,
        "radius {radius} versus {expected}"
    );
    assert_eq!(
        elements(1.0, 0.0, 0.0, 0.0, PI).to_state(MU),
        Err(ElementsError::OutsidePhysicalBranch)
    );
}

#[test]
fn construct_exact_singular_orbits_from_combined_angles() {
    use odd_astro::elements::EquatorialDirection::{Prograde, Retrograde};
    let radius = 7_000_000.0;
    let longitude = 0.7;
    for direction in [Prograde, Retrograde] {
        let el = Elements::circular_equatorial(radius, direction, longitude).unwrap();
        let state = el.to_state(MU).unwrap();
        near(state.position_m[0], radius * longitude.cos(), 1e-8);
        near(state.position_m[1], radius * longitude.sin(), 1e-8);
        let sign = if direction == Prograde { 1.0 } else { -1.0 };
        let speed = (MU / radius).sqrt();
        near(
            state.velocity_m_s[0],
            -sign * speed * longitude.sin(),
            1e-10,
        );
        near(state.velocity_m_s[1], sign * speed * longitude.cos(), 1e-10);
        let recovered = Elements::from_state(state, MU).unwrap();
        match recovered.special_angles().unwrap() {
            SpecialAngles::CircularEquatorial { true_longitude_rad } => {
                near(true_longitude_rad, longitude, 1e-14)
            }
            _ => panic!("wrong class"),
        }
        let peri = 1.3;
        let el = Elements::eccentric_equatorial(radius, 0.4, direction, peri, 0.0).unwrap();
        let state = el.to_state(MU).unwrap();
        near(state.position_m[1].atan2(state.position_m[0]), peri, 1e-14);
        let recovered = Elements::from_state(state, MU).unwrap();
        match recovered.special_angles().unwrap() {
            SpecialAngles::EccentricEquatorial {
                longitude_of_periapsis_rad,
            } => near(longitude_of_periapsis_rad, peri, 1e-14),
            _ => panic!("wrong class"),
        }
    }
    let el = Elements::circular_inclined(radius, 0.8, 1.2, 2.3).unwrap();
    let recovered = Elements::from_state(el.to_state(MU).unwrap(), MU).unwrap();
    match recovered.special_angles().unwrap() {
        SpecialAngles::CircularInclined {
            argument_of_latitude_rad,
        } => near(argument_of_latitude_rad, 2.3, 1e-14),
        _ => panic!("wrong class"),
    }
    assert_eq!(
        Elements::circular_inclined(radius, 0.0, 0.0, 0.0),
        Err(ElementsError::InvalidInput)
    );
    assert_eq!(
        Elements::eccentric_equatorial(radius, 2.0, Prograde, 0.0, PI),
        Err(ElementsError::OutsidePhysicalBranch)
    );
}

#[test]
fn equinoctial_near_singular_roundtrip() {
    use odd_astro::elements::ModifiedEquinoctialElements as Mee;
    for (e, i) in [(0.0, 0.0), (1e-10, 1e-10), (0.5, 1.2), (1.5, 0.9)] {
        let original = elements(e, i, 1.1, 0.7, 0.5).to_state(MU).unwrap();
        let equinoctial = Mee::from_state(original, MU).unwrap();
        // Independently compare the documented classical-to-MEE identities,
        // then check both representations against the original Cartesian state.
        near(equinoctial.p, 8_000_000.0, 1e-7);
        near(equinoctial.f, e * (1.1_f64 + 0.7).cos(), 2e-15);
        near(equinoctial.g, e * (1.1_f64 + 0.7).sin(), 2e-15);
        near(equinoctial.h, (i * 0.5).tan() * 1.1_f64.cos(), 2e-15);
        near(equinoctial.k, (i * 0.5).tan() * 1.1_f64.sin(), 2e-15);
        let classical_back = Elements::from_state(original, MU)
            .unwrap()
            .to_state(MU)
            .unwrap();
        for (got, want) in [
            (classical_back.position_m, original.position_m),
            (classical_back.velocity_m_s, original.velocity_m_s),
        ] {
            let err = got
                .iter()
                .zip(want)
                .map(|(x, y)| (x - y) * (x - y))
                .sum::<f64>()
                .sqrt();
            let scale = want.iter().map(|x| x * x).sum::<f64>().sqrt();
            assert!(err / scale < 1e-13, "classical {e} {i}: {err} / {scale}");
        }
        let back = equinoctial.to_state(MU).unwrap();
        for (got, want) in [
            (back.position_m, original.position_m),
            (back.velocity_m_s, original.velocity_m_s),
        ] {
            let err = got
                .iter()
                .zip(want)
                .map(|(x, y)| (x - y) * (x - y))
                .sum::<f64>()
                .sqrt();
            let scale = want.iter().map(|x| x * x).sum::<f64>().sqrt();
            assert!(err / scale < 1e-13, "{e} {i}: {err} / {scale}");
        }
    }
    let retro = Elements::circular_equatorial(
        7e6,
        odd_astro::elements::EquatorialDirection::Retrograde,
        0.0,
    )
    .unwrap()
    .to_state(MU)
    .unwrap();
    // sin(π) is not exactly zero in binary64; check an exact retrograde state.
    let exact = odd_core::frames::State {
        position_m: [7e6, 0.0, 0.0],
        velocity_m_s: [0.0, -(MU / 7e6).sqrt(), 0.0],
    };
    assert_eq!(
        Mee::from_state(exact, MU),
        Err(ElementsError::EquinoctialSingularity)
    );
    let _ = retro;
}

#[test]
fn equinoctial_near_parabolic_radius_is_stable() {
    use odd_astro::elements::ModifiedEquinoctialElements as Mee;
    let el = Mee {
        p: 8_000_000.0,
        f: 1.0 - f64::EPSILON,
        g: 0.0,
        h: 0.0,
        k: 0.0,
        l: PI - 1e-8,
    };
    let state = el.to_state(MU).unwrap();
    let radius = state.position_m[0].hypot(state.position_m[1]);
    assert!((radius / 2.940_694_223_310_72e22 - 1.0).abs() < 2e-15);
    // mpmath 80-digit transverse velocity from the exact f64 inputs.
    assert!((state.velocity_m_s[1] / -1.214_408_927_286_618_2e-12 - 1.0).abs() < 2e-15);
}

#[test]
fn equinoctial_large_longitude_preserves_phase() {
    use odd_astro::elements::ModifiedEquinoctialElements as Mee;
    // 90-digit mpmath with exact binary64 inputs (including longitude).
    let parabolic = Mee {
        p: 8e6,
        f: 1.0,
        g: 0.0,
        h: 0.0,
        k: 0.0,
        l: PI,
    };
    assert_eq!(
        parabolic.to_state(MU),
        Err(ElementsError::OutsidePhysicalBranch)
    );
    for (f, g, l, expected) in [
        (0.4, 0.3, 1e16, 8_134_718.067_552_062),
        (0.6, 0.4, 1e17, 28_283_186.427_805_5),
    ] {
        let state = Mee {
            p: 8_000_000.0,
            f,
            g,
            h: 0.0,
            k: 0.0,
            l,
        }
        .to_state(MU)
        .unwrap();
        let radius = state.position_m[0].hypot(state.position_m[1]);
        assert!(
            (radius / expected - 1.0).abs() < 2e-15,
            "{radius} != {expected}"
        );
    }
}
