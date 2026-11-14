use odd_core::epoch::Epoch;
use odd_ephem::{Body, Ephemeris, EphemerisError};

fn norm(v: [f64; 3]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}
fn difference(a: [f64; 3], b: [f64; 3]) -> f64 {
    norm(std::array::from_fn(|i| a[i] - b[i]))
}

#[test]
fn horizons_earth_moon_january_2026() {
    // JPL Horizons API, geometric ICRF, Moon (301) relative Earth (399),
    // DE441, 2026-01-01 00:00:00 TDB, output KM-S, retrieved 2026-11-14.
    // See docs/ephemeris.md for the complete query. DE440 != DE441.
    let reference_position = [
        144_325_727.491_927_3,
        289_584_157.544_932_9,
        160_158_923.001_684_2,
    ];
    let reference_velocity = [
        -1_004.314_137_672_221,
        383.914_610_117_644,
        172.534_896_948_401_4,
    ];
    let e = Ephemeris::bundled().unwrap();
    let epoch = Epoch::from_julian_date_tdb(2_461_041.5).unwrap();
    let got = e.state(Body::Moon, Body::Earth, epoch).unwrap().state;
    assert!(difference(got.position_m, reference_position) / norm(reference_position) < 0.001);
    assert!(difference(got.velocity_m_s, reference_velocity) / norm(reference_velocity) < 0.001);
    assert!((norm(got.position_m) - 361_026_010.883_069_2).abs() / 361_026_010.883_069_2 < 0.001);
    // Also guard against a velocity or frame sign error with a much tighter
    // observed cross-solution agreement, without asserting DE440 equals DE441.
    assert!(difference(got.position_m, reference_position) < 10.0);
    assert!(difference(got.velocity_m_s, reference_velocity) < 0.000_1);
}

#[test]
fn cspice_independent_evaluation_at_boundaries_and_fractional_epoch() {
    // CSPICE N0067 via spiceypy 8.4, same SPK; meters and m/s.
    let vectors = [
        (
            631_108_800,
            0.0,
            Body::Moon,
            Body::Earth,
            [
                390_185_638.892_563_16,
                -76_522_596.936_924_76,
                -70_724_654.719_421_31,
            ],
            [
                248.727_722_117_026_05,
                872.460_718_160_424_1,
                340.065_125_676_439_04,
            ],
        ),
        (
            820_497_600,
            0.25,
            Body::Moon,
            Body::Earth,
            [
                144_325_478.673_459_95,
                289_584_252.660_901_5,
                160_158_965.745_581_95,
            ],
            [
                -1_004.314_447_402_294_5,
                383.914_000_253_528_97,
                172.534_558_648_144_18,
            ],
        ),
        (
            820_411_200,
            0.0,
            Body::Earth,
            Body::SolarSystemBarycenter,
            [
                -23_954_322_547.227_02,
                132_471_562_877.552_22,
                57_445_228_531.832_91,
            ],
            [
                -29_867.428_715_523_973,
                -4_473.999_398_036_707,
                -1_939.863_428_231_229_6,
            ],
        ),
        (
            820_497_600,
            0.0,
            Body::Sun,
            Body::SolarSystemBarycenter,
            [
                -458_863_967.403_542_1,
                -767_304_103.760_174,
                -311_195_587.258_137_6,
            ],
            [
                12.425_057_943_606_113,
                0.376_060_076_384_190_53,
                -0.093_399_345_216_056_76,
            ],
        ),
        (
            1_136_030_399,
            0.0,
            Body::Moon,
            Body::Earth,
            [
                235_487_963.932_228_42,
                -308_350_212.913_846_8,
                -119_410_712.282_131_33,
            ],
            [
                783.080_236_140_043_2,
                554.003_830_448_731_4,
                151.167_198_235_335_72,
            ],
        ),
    ];
    let e = Ephemeris::bundled().unwrap();
    for (s, f, target, center, pos, vel) in vectors {
        let got = e
            .state(target, center, Epoch::new(s, f).unwrap())
            .unwrap()
            .state;
        assert!(
            difference(got.position_m, pos) < 0.001,
            "{target:?} position {:?} vs {pos:?}",
            got.position_m
        );
        assert!(
            difference(got.velocity_m_s, vel) < 0.000_001,
            "{target:?} velocity {:?} vs {vel:?}",
            got.velocity_m_s
        );
    }
}

#[test]
fn coverage_origins_and_relative_states() {
    let e = Ephemeris::bundled().unwrap();
    let (start, end) = e.coverage();
    assert_eq!((start, end), (631_108_800, 1_136_030_400));
    assert_eq!(
        e.state(
            Body::Earth,
            Body::Moon,
            Epoch::new(start - 1, 0.999).unwrap()
        ),
        Err(EphemerisError::OutsideCoverage)
    );
    assert_eq!(
        e.state(Body::Earth, Body::Earth, Epoch::new(end, 0.0).unwrap()),
        Err(EphemerisError::OutsideCoverage)
    );
    let epoch = Epoch::new(820_497_600, 0.123).unwrap();
    let m = e.state(Body::Moon, Body::Earth, epoch).unwrap().state;
    let inv = e.state(Body::Earth, Body::Moon, epoch).unwrap().state;
    assert_eq!(m.position_m, inv.position_m.map(|x| -x));
    assert_eq!(m.velocity_m_s, inv.velocity_m_s.map(|x| -x));
    let zero = e.state(Body::Sun, Body::Sun, epoch).unwrap().state;
    assert_eq!(zero.position_m, [0.0; 3]);
    assert_eq!(zero.velocity_m_s, [0.0; 3]);
    let moon = e
        .state(Body::Moon, Body::SolarSystemBarycenter, epoch)
        .unwrap()
        .state;
    let earth = e
        .state(Body::Earth, Body::SolarSystemBarycenter, epoch)
        .unwrap()
        .state;
    assert!(
        difference(
            m.position_m,
            std::array::from_fn(|i| moon.position_m[i] - earth.position_m[i])
        ) < 0.001
    );
    assert!(
        difference(
            m.velocity_m_s,
            std::array::from_fn(|i| moon.velocity_m_s[i] - earth.velocity_m_s[i])
        ) < 0.000_000_1
    );
}

#[test]
fn decoder_rejects_corruption_without_panicking() {
    let data = include_bytes!("../../../data/packed/de440s-2020-2036.ephb");
    for end in [0, 8, 12, 24, 59, data.len() - 1] {
        assert_eq!(
            Ephemeris::from_bytes(&data[..end]).unwrap_err(),
            EphemerisError::InvalidData
        );
    }
    let mut x = data.to_vec();
    x[0] = 0;
    assert!(Ephemeris::from_bytes(&x).is_err());
    let mut x = data.to_vec();
    x[52..56].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(Ephemeris::from_bytes(&x).is_err());
    let mut x = data.to_vec();
    x[60..68].copy_from_slice(&f64::NAN.to_le_bytes());
    assert!(Ephemeris::from_bytes(&x).is_err());
    let mut x = data.to_vec();
    x.push(0);
    assert!(Ephemeris::from_bytes(&x).is_err());
}

#[test]
fn horizons_barycentric_states_2026() {
    // Independent Horizons DE441 geometric ICRF vectors (2026-11-14).
    let references = [
        (
            Body::Earth,
            [
                -26_531_002_415.565_48,
                132_064_399_546.259_2,
                57_268_703_374.958_59,
            ],
            [
                -29_776.506_107_704_64,
                -4_950.768_712_267_514,
                -2_146.229_769_311_948,
            ],
        ),
        (
            Body::EarthMoonBarycenter,
            [
                -26_529_248_773.633_1,
                132_067_918_163.005_1,
                57_270_649_399.469_26,
            ],
            [
                -29_788.709_111_394_35,
                -4_946.103_925_396_488,
                -2_144.133_369_485_351,
            ],
        ),
        (
            Body::Sun,
            [
                -458_863_967.403_542_1,
                -767_304_103.760_174,
                -311_195_587.258_137_6,
            ],
            [
                12.425_057_943_606_11,
                0.376_060_076_384_190_5,
                -0.093_399_345_216_056_77,
            ],
        ),
    ];
    let e = Ephemeris::bundled().unwrap();
    let t = Epoch::new(820_497_600, 0.0).unwrap();
    for (body, p, v) in references {
        let actual = e.state(body, Body::SolarSystemBarycenter, t).unwrap().state;
        assert!(difference(actual.position_m, p) < 1.0, "{body:?}");
        assert!(difference(actual.velocity_m_s, v) < 0.0001, "{body:?}");
    }
}

#[test]
fn horizons_relative_vectors_across_coverage() {
    // Horizons DE441 geometric Moon relative Earth, sampled on Jan 1 TDB.
    let fixtures = [
        (
            2_458_849.5,
            [
                390_185_639.340_002_8,
                -76_522_595.353_777_91,
                -70_724_654.104_453_4,
            ],
            [
                248.727_717_750_581_4,
                872.460_718_991_747_2,
                340.065_126_450_2,
            ],
        ),
        (
            2_460_310.5,
            [
                -367_952_531.141_940_3,
                142_774_973.142_968_2,
                89_342_281.180_850_49,
            ],
            [
                -409.767_849_622_084_9,
                -779.797_775_074_751_7,
                -402.679_166_164_837_1,
            ],
        ),
        (
            2_464_328.5,
            [
                -391_640_178.488_842_8,
                -36_656_630.519_584_92,
                -3_853_625.397_539_497,
            ],
            [
                30.486_136_732_401_92,
                -942.433_468_376_086_8,
                -315.889_649_711_020_3,
            ],
        ),
    ];
    let ephem = Ephemeris::bundled().unwrap();
    for (jd, p, v) in fixtures {
        let state = ephem
            .state(
                Body::Moon,
                Body::Earth,
                Epoch::from_julian_date_tdb(jd).unwrap(),
            )
            .unwrap()
            .state;
        assert!(
            difference(state.position_m, p) / norm(p) < 0.001,
            "JD {jd} position"
        );
        assert!(
            difference(state.velocity_m_s, v) / norm(v) < 0.001,
            "JD {jd} velocity"
        );
    }
}

#[test]
fn finite_segment_states_cannot_overflow_in_composition() {
    // Each evaluated segment is finite, but Moon→EMB plus EMB→SSB exceeds
    // f64 range. Even structurally valid caller-supplied data must fail closed.
    let mut bytes = b"ODEPHEM1".to_vec();
    bytes.extend(4_u32.to_le_bytes());
    for (target, center) in [(3_i32, 0_i32), (10, 0), (301, 3), (399, 3)] {
        bytes.extend(target.to_le_bytes());
        bytes.extend(center.to_le_bytes());
        for x in [0.0_f64, 10.0, 0.0, 10.0] {
            bytes.extend(x.to_le_bytes());
        }
        bytes.extend(1_u32.to_le_bytes());
        bytes.extend(1_u32.to_le_bytes());
        let coefficient = if target == 3 || target == 301 {
            1e305_f64
        } else {
            0.0
        };
        for x in [coefficient, 0.0, 0.0] {
            bytes.extend(x.to_le_bytes());
        }
    }
    let ephem = Ephemeris::from_bytes(&bytes).unwrap();
    assert_eq!(
        ephem.state(Body::Moon, Body::Sun, Epoch::J2000),
        Err(EphemerisError::InvalidData)
    );
}

#[test]
fn physical_parameters_are_named_and_barycenters_are_not_bodies() {
    use odd_ephem::physical::physical;
    assert!(physical(Body::SolarSystemBarycenter).is_none());
    assert!(physical(Body::EarthMoonBarycenter).is_none());
    assert_eq!(
        physical(Body::Earth).unwrap().gravitational_parameter_m3_s2,
        398_600.435_507e9
    );
    assert_eq!(physical(Body::Moon).unwrap().radii_m, [1_737_400.0; 3]);
    assert_eq!(physical(Body::Sun).unwrap().radii_m, [695_700_000.0; 3]);
}

fn compare_cspice_corpus(data: &str) -> usize {
    let ephem = Ephemeris::bundled().unwrap();
    let mut n = 0;
    for line in data.lines().filter(|line| !line.starts_with('#')) {
        let mut fields = line.split(',');
        let body = |id| match id {
            "0" => Body::SolarSystemBarycenter,
            "3" => Body::EarthMoonBarycenter,
            "10" => Body::Sun,
            "301" => Body::Moon,
            "399" => Body::Earth,
            _ => panic!("unknown body"),
        };
        let target = body(fields.next().unwrap());
        let center = body(fields.next().unwrap());
        let seconds: i64 = fields.next().unwrap().parse().unwrap();
        let fraction: f64 = fields.next().unwrap().parse().unwrap();
        let components: Vec<f64> = fields.map(|s| s.parse().unwrap()).collect();
        assert_eq!(components.len(), 6);
        let state = ephem
            .state(target, center, Epoch::new(seconds, fraction).unwrap())
            .unwrap()
            .state;
        for axis in 0..3 {
            assert!(
                (state.position_m[axis] - components[axis]).abs() < 0.001,
                "position at {seconds}+{fraction}, {target:?} axis {axis}: {} vs {}",
                state.position_m[axis],
                components[axis]
            );
            assert!(
                (state.velocity_m_s[axis] - components[axis + 3]).abs() < 0.000_001,
                "velocity at {seconds}+{fraction}, {target:?} axis {axis}"
            );
        }
        n += 1;
    }
    n
}

#[test]
fn full_kernel_cspice_corpus_covers_interval_and_record_edges() {
    // Full DE440s, CSPICE_N0067; see generator for provenance and sampling.
    assert_eq!(
        compare_cspice_corpus(include_str!("../../../data/oracles/de440s-cspice.csv")),
        452
    );
}

#[test]
fn full_kernel_all_supported_body_pairs_match_cspice() {
    // Five epochs exercise every target/center pair, including self and SSB.
    assert_eq!(
        compare_cspice_corpus(include_str!(
            "../../../data/oracles/de440s-cspice-pairs.csv"
        )),
        125
    );
}

#[test]
fn custom_coverage_supports_past_and_future_de440_records() {
    // CSPICE N0067 and full DE440s; two compact eight-day fixtures.
    type Fixture<'a> = (&'a [u8], i64, [f64; 3], [f64; 3]);
    let samples: [Fixture<'_>; 2] = [
        (
            include_bytes!("../../../data/oracles/de440s-1900-8d.ephb"),
            -3_155_716_800,
            [
                24_464_926.520_781_275,
                -339_984_249.046_225_7,
                -139_725_244.190_060_65,
            ],
            [
                1_060.165_900_602_653_4,
                70.311_857_995_211_58,
                133.924_978_377_331_85,
            ],
        ),
        (
            include_bytes!("../../../data/oracles/de440s-2100-8d.ephb"),
            3_155_716_800,
            [
                -339_519_599.086_461_5,
                135_866_823.268_439,
                66_602_106.591_248_68,
            ],
            [
                -416.859_643_819_902_26,
                -849.998_439_012_363_9,
                -467.702_941_766_646_6,
            ],
        ),
    ];
    for (data, start, pos, vel) in samples {
        let e = Ephemeris::from_bytes(data).unwrap();
        assert_eq!(e.coverage(), (start, start + 691_200));
        let got = e
            .state(Body::Moon, Body::Earth, Epoch::new(start, 0.0).unwrap())
            .unwrap()
            .state;
        assert!(difference(got.position_m, pos) < 0.001);
        assert!(difference(got.velocity_m_s, vel) < 0.000_001);
        assert_eq!(
            e.state(
                Body::Moon,
                Body::Earth,
                Epoch::new(start + 691_200, 0.0).unwrap()
            ),
            Err(EphemerisError::OutsideCoverage)
        );
    }
}
