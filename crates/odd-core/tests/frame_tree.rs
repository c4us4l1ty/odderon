use odd_core::epoch::Epoch;
use odd_core::frames::{
    FrameId, FrameTree, FramedState, GalacticAnchor, KinematicTransform, Rotation, State, TreeError,
};

fn translate(origin: [f64; 3]) -> KinematicTransform {
    KinematicTransform::new(origin, [0.0; 3], Rotation::IDENTITY, [0.0; 3]).unwrap()
}
fn sample_tree() -> FrameTree {
    let mut tree = FrameTree::new(Epoch::J2000);
    tree.add_stellar_system(
        1,
        GalacticAnchor {
            milliparsecs: [2_452_809, 0, 0],
        },
        translate([0.0; 3]),
    )
    .unwrap();
    tree.add_frame(
        FrameId::BodyCentered(1),
        FrameId::StellarSystem(1),
        translate([100_000.0, -500_000.0, 10.0]),
    )
    .unwrap();
    tree.add_frame(
        FrameId::Camera(1),
        FrameId::BodyCentered(1),
        translate([20.0, 30.0, 40.0]),
    )
    .unwrap();
    tree
}

#[test]
fn split_8000_light_year_anchor_preserves_local_docking_separation() {
    // 2,452,809 milliparsecs, not parsecs, is approximately 8,000 ly.
    let tree = sample_tree();
    let state = |x| FramedState {
        frame: FrameId::Camera(1),
        epoch: Epoch::J2000,
        state: State {
            position_m: [x, 0.0, 0.0],
            velocity_m_s: [1.0, 0.0, 0.0],
        },
    };
    let g1 = tree.to_galactic(state(0.0)).unwrap();
    let g2 = tree.to_galactic(state(0.5)).unwrap();
    assert_eq!(g1.anchor.milliparsecs[0], 2_452_809);
    let p1 = tree.from_galactic(g1, FrameId::Camera(1)).unwrap();
    let p2 = tree.from_galactic(g2, FrameId::Camera(1)).unwrap();
    assert!((p2.state.position_m[0] - p1.state.position_m[0] - 0.5).abs() < 1e-9);
    assert_eq!(p2.state.velocity_m_s, [1.0, 0.0, 0.0]);
    assert_eq!(
        tree.transform(state(0.5), FrameId::BodyCentered(1))
            .unwrap()
            .state
            .position_m,
        [20.5, 30.0, 40.0]
    );
}

#[test]
fn crossing_systems_and_invalid_hierarchies() {
    let mut tree = sample_tree();
    tree.add_stellar_system(
        2,
        GalacticAnchor {
            milliparsecs: [2_452_810, 0, 0],
        },
        translate([5.0, 0.0, 0.0]),
    )
    .unwrap();
    let source = FramedState {
        frame: FrameId::StellarSystem(1),
        epoch: Epoch::J2000,
        state: State {
            position_m: [0.0; 3],
            velocity_m_s: [0.0; 3],
        },
    };
    let other = tree.transform(source, FrameId::StellarSystem(2)).unwrap();
    let expected = -odd_core::consts::PARSEC_M / 1000.0 - 5.0;
    assert!((other.state.position_m[0] - expected).abs() < 0.01);
    assert_eq!(
        tree.transform(source, FrameId::Galactic),
        Err(TreeError::GalacticNeedsSplitState)
    );
    assert_eq!(
        tree.add_frame(
            FrameId::BodyCentered(7),
            FrameId::BodyCentered(7),
            translate([0.0; 3])
        ),
        Err(TreeError::UnknownFrame)
    );
    assert_eq!(
        tree.add_frame(
            FrameId::Camera(1),
            FrameId::StellarSystem(1),
            translate([0.0; 3])
        ),
        Err(TreeError::DuplicateFrame)
    );
    assert_eq!(
        tree.transform(
            FramedState {
                epoch: Epoch::new(1, 0.0).unwrap(),
                ..source
            },
            FrameId::Camera(1)
        ),
        Err(TreeError::EpochMismatch)
    );
}

#[test]
fn rotated_sibling_frames_use_lowest_common_ancestor() {
    let mut tree = sample_tree();
    let rot = Rotation::new([[0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]).unwrap();
    let fixed =
        KinematicTransform::new([1.0, 2.0, 0.0], [3.0, 0.0, 0.0], rot, [0.0, 0.0, 2.0]).unwrap();
    tree.add_frame(FrameId::BodyFixed(1), FrameId::BodyCentered(1), fixed)
        .unwrap();
    let start = FramedState {
        frame: FrameId::BodyFixed(1),
        epoch: Epoch::J2000,
        state: State {
            position_m: [3.0, 0.0, 0.0],
            velocity_m_s: [0.0; 3],
        },
    };
    let camera = tree.transform(start, FrameId::Camera(1)).unwrap();
    assert_eq!(camera.state.position_m, [-19.0, -25.0, -40.0]);
    assert_eq!(camera.state.velocity_m_s, [-3.0, 0.0, 0.0]);
    assert_eq!(
        tree.transform(camera, FrameId::BodyFixed(1)).unwrap(),
        start
    );
}

#[test]
fn render_conversion_is_not_a_reversible_simulation_transform() {
    // At 1 m the neighboring f32 spacing is roughly 1.19e-7 m.
    let a = odd_core::frames::render_position([1.0, 0.0, 0.0]).unwrap();
    let b = odd_core::frames::render_position([1.0 + 5e-8, 0.0, 0.0]).unwrap();
    assert_eq!(a, b);
}

#[test]
fn galactic_round_trip_preserves_docking_with_realistic_large_offsets() {
    let mut tree = FrameTree::new(Epoch::J2000);
    tree.add_stellar_system(
        17,
        GalacticAnchor {
            milliparsecs: [2_452_809, 14_111, -90_001],
        },
        translate([10_000_000_000_000.125, -7_000_000_000_000.5, 0.0]),
    )
    .unwrap();
    tree.add_frame(
        FrameId::BodyCentered(17),
        FrameId::StellarSystem(17),
        translate([149_597_870_700.123_44, -50_000_000_000.25, 0.0]),
    )
    .unwrap();
    tree.add_frame(
        FrameId::Camera(17),
        FrameId::BodyCentered(17),
        translate([20.0, 30.0, 40.0]),
    )
    .unwrap();
    let point = |x| FramedState {
        frame: FrameId::Camera(17),
        epoch: Epoch::J2000,
        state: State {
            position_m: [x, 0.0, 0.0],
            velocity_m_s: [0.0; 3],
        },
    };
    let a = point(0.123_456_789);
    let b = point(0.623_456_789);
    let a_back = tree
        .from_galactic(tree.to_galactic(a).unwrap(), a.frame)
        .unwrap();
    let b_back = tree
        .from_galactic(tree.to_galactic(b).unwrap(), b.frame)
        .unwrap();
    assert!((a_back.state.position_m[0] - a.state.position_m[0]).abs() < 1e-9);
    assert!((b_back.state.position_m[0] - b.state.position_m[0]).abs() < 1e-9);
    assert!((b_back.state.position_m[0] - a_back.state.position_m[0] - 0.5).abs() < 1e-9);
}
