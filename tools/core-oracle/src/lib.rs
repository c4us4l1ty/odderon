//! Shared native/wasm foundation and ephemeris parity fixture, not a security attestation.
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
    let small_step = f64::from_bits((1023_u64 - 54) << 52);
    let precise_fraction = Epoch::new(0, 0.5)
        .unwrap()
        .checked_sub_seconds(small_step)
        .unwrap()
        .fraction();
    assert_eq!(precise_fraction, 0.5 - small_step);
    assert_eq!(
        Epoch::new(i64::MIN, 0.0)
            .unwrap()
            .checked_add_seconds(9_223_372_036_854_775_808.0)
            .unwrap(),
        Epoch::J2000
    );
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
    let moon = odd_ephem::Ephemeris::bundled()
        .unwrap()
        .state(
            odd_ephem::Body::Moon,
            odd_ephem::Body::Earth,
            Epoch::new(820_497_600, 0.25).unwrap(),
        )
        .unwrap()
        .state;
    assert!((moon.position_m[0] - 144_325_478.673_459_95).abs() < 0.001);
    assert!((moon.velocity_m_s[0] - (-1_004.314_447_402_294_5)).abs() < 0.000_001);
    let lunar_orientation =
        odd_ephem::orientation::moon_orientation(Epoch::new(820_497_600, 0.25).unwrap()).unwrap();
    let (gmst, obliquity) = odd_ephem::orientation::earth_mean_angles(9496.5, 9496.5008).unwrap();
    let earth_orientation =
        odd_ephem::orientation::earth_approx_orientation(Epoch::new(820_497_600, 0.25).unwrap())
            .unwrap();
    // Exercise WP 1.1 in both native and browser Wasm with numerical
    // tolerances; transcendental functions are not promised bit-identical.
    let orbit_state = State {
        position_m: [6_524_834.0, 6_862_875.0, 6_448_296.0],
        velocity_m_s: [4_901.327, 5_533.756, -1_976.341],
    };
    let orbit = odd_astro::elements::Elements::from_state(orbit_state, 3.986_004_418e14).unwrap();
    assert!((orbit.eccentricity - 0.832_853_398_487_521_5).abs() < 1e-12);
    assert!((orbit.argument_of_periapsis_rad - 0.931_742_810_240_855_7).abs() < 1e-12);
    let round_trip = orbit.to_state(3.986_004_418e14).unwrap();
    for i in 0..3 {
        assert!((round_trip.position_m[i] - orbit_state.position_m[i]).abs() < 1e-5);
        assert!((round_trip.velocity_m_s[i] - orbit_state.velocity_m_s[i]).abs() < 1e-8);
    }
    let anomaly = odd_astro::anomaly::hyperbolic_from_mean(2.0, 1.350_402_387_287_602_8).unwrap();
    assert!((anomaly - 1.0).abs() < 1e-12);
    let values = [
        century.seconds() as u64,
        century.fraction().to_bits(),
        precise_fraction.to_bits(),
        utc.seconds() as u64,
        utc.fraction().to_bits(),
        separation.to_bits(),
        b.state.velocity_m_s[0].to_bits(),
        moon.position_m[0].to_bits(),
        moon.position_m[1].to_bits(),
        moon.position_m[2].to_bits(),
        moon.velocity_m_s[0].to_bits(),
        moon.velocity_m_s[1].to_bits(),
        moon.velocity_m_s[2].to_bits(),
        lunar_orientation.body_to_j2000[0][0].to_bits(),
        lunar_orientation.body_to_j2000[2][2].to_bits(),
        earth_orientation.body_to_j2000[0][0].to_bits(),
        gmst.to_bits(),
        obliquity.to_bits(),
    ];
    canonical_hash(1, &values).unwrap()
}

#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn oracle_byte(index: u32) -> u32 {
    // The JS harness reads the hash one byte at a time. Decode the ephemeris
    // and evaluate the fixture only once per WebAssembly instance.
    static FINGERPRINT: std::sync::OnceLock<[u8; 32]> = std::sync::OnceLock::new();
    FINGERPRINT
        .get_or_init(fingerprint)
        .get(index as usize)
        .copied()
        .map_or(256, u32::from)
}
