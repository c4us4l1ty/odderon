use odd_core::epoch::Epoch;
use odd_ephem::orientation::{
    earth_approx_orientation, earth_mean_angles, moon_orientation, sun_orientation,
    OrientationError,
};

#[test]
fn iau_earth_angles_match_erfa() {
    // pyerfa 2.0.1.5, gmst06 and obl06 with split J2000 Julian dates.
    for (ut1, tt, gmst, obliq) in [
        (0.0, 0.0, 4.894_961_283_150_828_5, 0.409_092_600_600_582_9),
        (
            9496.5,
            9496.5008,
            1.756_863_130_878_143_3,
            0.409_033_562_229_685_3,
        ),
        (
            -3652.5,
            -3652.4993,
            1.752_024_834_086_355_3,
            0.409_115_307_684_031_26,
        ),
        (
            10000.123456,
            10000.124257,
            1.771_511_588_443_353,
            0.409_030_431_291_769_2,
        ),
    ] {
        let actual = earth_mean_angles(ut1, tt).unwrap();
        assert!((actual.0 - gmst).abs() < 1e-12, "{:?}", actual);
        assert!((actual.1 - obliq).abs() < 1e-14);
    }
    assert_eq!(
        earth_mean_angles(f64::NAN, 0.0),
        Err(OrientationError::NonFinite)
    );
    assert_eq!(
        earth_mean_angles(50_000.0, 0.0),
        Err(OrientationError::OutsideModelRange)
    );
}

#[test]
fn iau_body_orientations_against_cspice() {
    // SPICE pxform(IAU frame, J2000, ET), pck00011.tpc.
    let t = Epoch::new(820_497_600, 0.0).unwrap();
    let moon = [
        [
            -0.3809166339122676,
            0.9245726743189681,
            0.008239412091818358,
        ],
        [
            -0.8584638128298032,
            -0.3503423217258855,
            -0.3745666024479929,
        ],
        [
            -0.34342743057400654,
            -0.14975188650026236,
            0.9271634011429423,
        ],
    ];
    let sun = [
        [-0.9377684340269525, -0.324992287258874, 0.12235349347232778],
        [
            0.2469308097500254,
            -0.8717999697374613,
            -0.42307208364764326,
        ],
        [0.24416293604647327, -0.3665307981439039, 0.8977971010607901],
    ];
    let earth = [
        [
            -0.17285532030337275,
            -0.9849439829860306,
            0.002527572438582451,
        ],
        [
            0.9849471259798965,
            -0.17285589075842353,
            -0.000007352136828201134,
        ],
        [
            0.00044414722825863697,
            0.0024882543531214306,
            0.9999968056566549,
        ],
    ];
    for (actual, reference) in [
        (moon_orientation(t).unwrap(), moon),
        (sun_orientation(t).unwrap(), sun),
        (earth_approx_orientation(t).unwrap(), earth),
    ] {
        for (row, expected) in actual.body_to_j2000.iter().zip(reference) {
            for (a, b) in row.iter().zip(expected) {
                assert!((a - b).abs() < 1e-10, "{a} vs {b}");
            }
        }
        assert!(odd_core::frames::Rotation::new(actual.body_to_j2000).is_ok());
    }
    assert_eq!(
        moon_orientation(Epoch::J2000),
        Err(OrientationError::OutsideModelRange)
    );
}
