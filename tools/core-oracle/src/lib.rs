//! Shared native/wasm WP 0.1 parity fixture, not a security attestation.
use odd_core::canon::canonical_hash;
use odd_core::epoch::Epoch;
use odd_core::frames::{
    FrameId, FrameTree, FramedState, GalacticAnchor, KinematicTransform, Rotation, State,
};
use odd_core::utc::UtcDateTime;

pub fn fingerprint() -> [u8; 32] {
    let epoch = Epoch::from_julian_date_tdb(2_451_545.0).unwrap();
    assert_eq!(epoch, Epoch::J2000);
    let century = epoch.checked_add_seconds(86_400.0 * 36_525.0).unwrap();
    assert_eq!(century.seconds(), 3_155_760_000);
    let utc = UtcDateTime {
        year: 2004,
        month: 6,
        day: 11,
        hour: 19,
        minute: 32,
        second: 0,
        fraction: 0.0,
    }
    .to_tdb()
    .unwrap();
    assert!((utc.seconds() as f64 + utc.fraction() - 140_254_384.184_625_2).abs() < 0.000_001);
    let mk = |p| KinematicTransform::new(p, [0.0; 3], Rotation::IDENTITY, [0.0; 3]).unwrap();
    let mut tree = FrameTree::new(epoch);
    tree.add_stellar_system(
        1,
        GalacticAnchor {
            milliparsecs: [2_452_809, 0, 0],
        },
        mk([10_000_000_000_000.125, -7_000_000_000_000.5, 0.0]),
    )
    .unwrap();
    tree.add_frame(
        FrameId::BodyCentered(1),
        FrameId::StellarSystem(1),
        mk([149_597_870_700.123_44, -50_000_000_000.25, 10.0]),
    )
    .unwrap();
    tree.add_frame(
        FrameId::Camera(1),
        FrameId::BodyCentered(1),
        mk([20.0, 30.0, 40.0]),
    )
    .unwrap();
    let point = |x| FramedState {
        frame: FrameId::Camera(1),
        epoch,
        state: State {
            position_m: [x, 0.0, 0.0],
            velocity_m_s: [1.0, 0.0, 0.0],
        },
    };
    let a = tree
        .from_galactic(
            tree.to_galactic(point(0.123_456_789)).unwrap(),
            FrameId::Camera(1),
        )
        .unwrap();
    let b = tree
        .from_galactic(
            tree.to_galactic(point(0.623_456_789)).unwrap(),
            FrameId::Camera(1),
        )
        .unwrap();
    assert!((a.state.position_m[0] - 0.123_456_789).abs() < 1e-9);
    assert!((b.state.position_m[0] - 0.623_456_789).abs() < 1e-9);
    let separation = b.state.position_m[0] - a.state.position_m[0];
    assert!((separation - 0.5).abs() < 1e-9);
    assert_ne!(
        canonical_hash(1, &1.0_f64).unwrap(),
        canonical_hash(1, &f64::from_bits(1.0_f64.to_bits() + 1)).unwrap()
    );
    let values = [
        century.seconds() as u64,
        century.fraction().to_bits(),
        utc.seconds() as u64,
        utc.fraction().to_bits(),
        separation.to_bits(),
        b.state.velocity_m_s[0].to_bits(),
    ];
    canonical_hash(1, &values).unwrap()
}

#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn oracle_byte(index: u32) -> u32 {
    fingerprint()
        .get(index as usize)
        .copied()
        .map_or(256, u32::from)
}
