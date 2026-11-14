use odd_core::consts::{JULIAN_YEAR_S, PARSEC_M, SPEED_OF_LIGHT_M_S};
use odd_core::epoch::Epoch;
use odd_core::frames::{render_position, GalacticAnchor, Translation};

#[test]
fn j2000_and_julian_century() {
    assert_eq!(
        Epoch::from_julian_date_tdb(2_451_545.0).unwrap(),
        Epoch::J2000
    );
    let later = Epoch::J2000
        .checked_add_seconds(86_400.0 * 36_525.0)
        .unwrap();
    assert_eq!(later.seconds(), 3_155_760_000);
    assert_eq!(later.fraction(), 0.0);
}

#[test]
fn epoch_normalizes_and_checks_bounds() {
    let epoch = Epoch::new(1, -0.25).unwrap();
    assert_eq!((epoch.seconds(), epoch.fraction()), (0, 0.75));
    let earlier = epoch.checked_add_seconds(-1.5).unwrap();
    assert_eq!((earlier.seconds(), earlier.fraction()), (-1, 0.25));
    assert_eq!(epoch.seconds_since(earlier), 1.5);
    assert!(Epoch::new(0, f64::NAN).is_err());
    assert!(Epoch::new(i64::MAX, 1.0).is_err());
    assert!(Epoch::new(i64::MIN, -1.0).is_err());
    let at_min = Epoch::new(i64::MIN, 0.75).unwrap();
    assert_eq!(at_min.checked_add_seconds(-0.5).unwrap().fraction(), 0.25);
    assert!(Epoch::J2000.checked_add_seconds(f64::INFINITY).is_err());
}

#[test]
fn eight_thousand_light_years_is_about_2453_parsecs() {
    let pc = 8_000.0 * SPEED_OF_LIGHT_M_S * JULIAN_YEAR_S / PARSEC_M;
    assert!((pc - 2452.8).abs() < 0.1);
}

#[test]
fn nearby_docking_ports_stay_local_in_a_distant_system() {
    // Galactic index does not enter the local arithmetic.
    let _star = GalacticAnchor {
        milliparsecs: [2_452_809, 0, 0],
    };
    let body = Translation::new([100_000.0, -500_000.0, 10.0]).unwrap();
    let camera = Translation::new([20.0, 30.0, 40.0]).unwrap();
    let p1 = [20.0, 30.0, 40.0];
    let p2 = [20.5, 30.0, 40.0];
    for p in [p1, p2] {
        let parent = body.to_parent(p).unwrap();
        let back = body.from_parent(parent).unwrap();
        assert_eq!(back, p);
    }
    let c1 = camera.from_parent(p1).unwrap();
    let c2 = camera.from_parent(p2).unwrap();
    assert!(((c2[0] - c1[0]) - 0.5).abs() < 1e-9);
    assert_eq!(render_position(c2).unwrap(), [0.5, 0.0, 0.0]);
}

#[test]
fn rendering_is_lossy_and_invalid_inputs_are_rejected() {
    let projected = render_position([1.0 + 1e-8, 0.0, 0.0]).unwrap();
    assert_eq!(projected[0], 1.0);
    assert!(render_position([f64::MAX, 0.0, 0.0]).is_err());
    assert!(Translation::new([f64::NAN, 0.0, 0.0]).is_err());
    assert!(Translation::new([0.0; 3])
        .unwrap()
        .to_parent([f64::INFINITY, 0.0, 0.0])
        .is_err());
}

#[test]
fn split_julian_date_preserves_small_fractions() {
    let epoch = Epoch::from_julian_date_parts_tdb(2_451_545.0, 1e-12).unwrap();
    assert!((epoch.seconds_since(Epoch::J2000) - 8.64e-8).abs() < 1e-20);
    assert_eq!(
        Epoch::from_julian_date_tdb(2_451_545.0 + 1e-12).unwrap(),
        Epoch::J2000
    );
    let before = Epoch::J2000.checked_sub_seconds(1.25).unwrap();
    assert_eq!((before.seconds(), before.fraction()), (-2, 0.75));
    assert_eq!(before.checked_add_seconds(1.25).unwrap(), Epoch::J2000);
    assert!(Epoch::J2000.checked_sub_seconds(f64::NAN).is_err());
}
