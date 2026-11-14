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
