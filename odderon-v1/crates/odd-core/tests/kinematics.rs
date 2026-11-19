use odd_core::frames::{FrameError, KinematicTransform, Rotation, State};

fn assert_vector_near(actual: [f64; 3], expected: [f64; 3], tolerance: f64) {
    for i in 0..3 {
        assert!(
            (actual[i] - expected[i]).abs() <= tolerance,
            "axis {i}: {} != {}",
            actual[i],
            expected[i]
        );
    }
}

#[test]
fn stationary_equatorial_site_has_eastward_inertial_velocity() {
    // IERS Conventions (2010), Table 1.2: nominal mean rotation rate.
    // WGS 84 equatorial radius: 6,378,137 m. Idealized aligned axes.
    // This tests the transport equation, not Earth's full orientation model.
    let earth = KinematicTransform::new(
        [0.0; 3],
        [0.0; 3],
        Rotation::IDENTITY,
        [0.0, 0.0, 7.292_115e-5],
    )
    .unwrap();
    let fixed_site = State {
        position_m: [6_378_137.0, 0.0, 0.0],
        velocity_m_s: [0.0; 3],
    };
    let inertial = earth.to_parent(fixed_site).unwrap();
    assert_vector_near(inertial.position_m, fixed_site.position_m, 0.0);
    assert_vector_near(inertial.velocity_m_s, [0.0, 465.101_084_897_55, 0.0], 1e-9);
    assert_vector_near(
        earth.from_parent(inertial).unwrap().velocity_m_s,
        [0.0; 3],
        1e-12,
    );
}

#[test]
fn rotation_translation_and_velocity_have_the_right_direction() {
    // A quarter-turn of the child about +z: its x-axis points along parent +y.
    let r = Rotation::new([[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]).unwrap();
    let frame = KinematicTransform::new(
        [100.0, 200.0, 300.0],
        [10.0, 20.0, 30.0],
        r,
        [0.0, 0.0, 2.0],
    )
    .unwrap();
    let child = State {
        position_m: [3.0, 0.0, 0.0],
        velocity_m_s: [1.0, 2.0, 3.0],
    };
    let parent = frame.to_parent(child).unwrap();
    assert_vector_near(parent.position_m, [100.0, 203.0, 300.0], 0.0);
    assert_vector_near(parent.velocity_m_s, [2.0, 21.0, 33.0], 0.0);
    let recovered = frame.from_parent(parent).unwrap();
    assert_vector_near(recovered.position_m, child.position_m, 0.0);
    assert_vector_near(recovered.velocity_m_s, child.velocity_m_s, 0.0);
}

#[test]
fn invalid_rotations_and_nonfinite_states_are_rejected() {
    assert_eq!(
        Rotation::new([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]]),
        Err(FrameError::InvalidRotation)
    );
    assert_eq!(
        Rotation::new([[2.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]),
        Err(FrameError::InvalidRotation)
    );
    assert_eq!(
        Rotation::new([[f64::NAN, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]),
        Err(FrameError::NonFinite)
    );
    assert_eq!(
        KinematicTransform::new(
            [0.0; 3],
            [0.0; 3],
            Rotation::IDENTITY,
            [f64::INFINITY, 0.0, 0.0]
        ),
        Err(FrameError::NonFinite)
    );
    let transform =
        KinematicTransform::new([f64::MAX, 0.0, 0.0], [0.0; 3], Rotation::IDENTITY, [0.0; 3])
            .unwrap();
    assert_eq!(
        transform.to_parent(State {
            position_m: [f64::MAX, 0.0, 0.0],
            velocity_m_s: [0.0; 3]
        }),
        Err(FrameError::NonFinite)
    );
}
