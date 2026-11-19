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
        FrameId::BodyFixed(9001),
        FrameId::BodyCentered(1),
        mk([20.0, 30.0, 40.0]),
    )
    .unwrap();
    let point = |x| FramedState {
        frame: FrameId::BodyFixed(9001),
        epoch,
        state: State {
            position_m: [x, 0.0, 0.0],
            velocity_m_s: [1.0, 0.0, 0.0],
        },
    };
    let a = tree
        .from_galactic(
            tree.to_galactic(point(0.123_456_789)).unwrap(),
            FrameId::BodyFixed(9001),
        )
        .unwrap();
    let b = tree
        .from_galactic(
            tree.to_galactic(point(0.623_456_789)).unwrap(),
            FrameId::BodyFixed(9001),
        )
        .unwrap();
    assert!((a.state.position_m[0] - 0.123_456_789).abs() < 1e-9);
    assert!((b.state.position_m[0] - 0.623_456_789).abs() < 1e-9);
    let separation = b.state.position_m[0] - a.state.position_m[0];
    assert!((separation - 0.5).abs() < 1e-9);
    let relative = tree.transform_relative(b, a).unwrap();
    assert!((relative.state.position_m[0] - 0.5).abs() < 1e-12);
    tree.add_frame(
        FrameId::Camera(1),
        FrameId::BodyFixed(9001),
        mk([0.123_456_789, 0.0, 0.0]),
    )
    .unwrap();
    let projection = tree
        .project_to_camera(point(0.623_456_789), FrameId::Camera(1))
        .unwrap();
    assert!((f64::from(projection[0]) - 0.5).abs() < 1e-7);
    assert!(tree.transform(point(0.0), FrameId::Camera(1)).is_err());
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
    // The lower-accuracy tier is explicitly tagged and exercised in native,
    // Node and browser Wasm without assuming bit-identical transcendental math.
    let future = Epoch::from_utc(2150, 6, 15, 0, 0, 0.0).unwrap();
    let fallback = odd_ephem::Ephemeris::bundled()
        .unwrap()
        .state(odd_ephem::Body::Moon, odd_ephem::Body::Earth, future)
        .unwrap();
    assert_eq!(fallback.model, odd_ephem::EphemerisModel::Analytical);
    let distance = fallback
        .state
        .position_m
        .iter()
        .map(|v| v * v)
        .sum::<f64>()
        .sqrt();
    assert!((distance - 399_833_795.0).abs() < 100.0);
    let mee = odd_astro::elements::ModifiedEquinoctialElements::from_state(
        State {
            position_m: [7e6, 0.0, 0.0],
            velocity_m_s: [0.0, (3.986_004_418e14_f64 / 7e6).sqrt(), 0.0],
        },
        3.986_004_418e14,
    )
    .unwrap();
    let restored = mee.to_state(3.986_004_418e14).unwrap();
    assert!((restored.position_m[0] - 7e6).abs() < 1e-6);
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
    let near_parabolic = odd_astro::anomaly::true_from_elliptic(1.0 - f64::EPSILON, 1e-8).unwrap();
    assert!((near_parabolic - 0.886_131_745_558_101_2).abs() < 1e-14);
    let remote = odd_astro::elements::Elements {
        semi_latus_rectum_m: 8_000_000.0,
        eccentricity: 1.0 - f64::EPSILON,
        inclination_rad: 0.0,
        ascending_node_rad: 0.0,
        argument_of_periapsis_rad: 0.0,
        true_anomaly_rad: std::f64::consts::PI - 1e-8,
    }
    .to_state(3.986_004_418e14)
    .unwrap();
    let radius = remote.position_m[0].hypot(remote.position_m[1]);
    assert!((radius / 2.940_694_223_310_72e22 - 1.0).abs() < 1e-12);
    let a = 1e18_f64;
    let near_apocenter = f64::from_bits((2.0 * a).to_bits() - 1);
    let slow = odd_astro::elliptical_speed(3.986_004_418e14, near_apocenter, a).unwrap();
    assert!((slow / 2.258_779_682_713_655e-10 - 1.0).abs() < 1e-12);
    let hohmann =
        odd_astro::hohmann_transfer(3.986_004_418e14, 7_000_000.0, 7_000_000.000_001).unwrap();
    assert!((hohmann.departure_delta_v_m_s / 2.695_667_036_401_499e-10 - 1.0).abs() < 1e-12);
    let bi = odd_astro::patched_conics::bi_elliptic_transfer(
        3.986_004_418e14,
        7_000_000.0,
        7_000_000.000_001,
        7_000_000.000_002,
    )
    .unwrap();
    assert!((bi.intermediate_delta_v_m_s / 2.695_667_036_401_307e-10 - 1.0).abs() < 1e-12);
    assert!(
        odd_astro::anomaly::hyperbolic_from_true(1.0 + f64::EPSILON, f64::from_bits(1)).is_err()
    );
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

/// Canonical final-state hash for a ten-day perturbed, event-split orbit.
pub fn integrator_fingerprint() -> [u8; 32] {
    integrator_fingerprint_epoch(820_497_600)
}

/// Ten-day equivalent through the packed/analytical transition.
pub fn blended_integrator_fingerprint() -> [u8; 32] {
    integrator_fingerprint_epoch(631_108_800)
}

/// Ten-day equivalent entirely in the long-range analytical ephemeris.
pub fn analytical_integrator_fingerprint() -> [u8; 32] {
    integrator_fingerprint_epoch(4_733_640_000)
}

fn integrator_fingerprint_epoch(start_epoch: i64) -> [u8; 32] {
    use odd_astro::{
        force_model::{Environment, ForceModel, Radiation, ThirdBody, Zonal},
        integrator::Options,
        perturb::{burn::Burn, drag::DragParameters},
        spline::Keyframe,
    };
    let eph = odd_ephem::Ephemeris::bundled().unwrap();
    let model = ForceModel {
        mu: 3.986_004_418e14,
        zonal: Some(Zonal {
            pole: [0.0, 0.0, 1.0],
            radius: 6_378_137.0,
            j2: 1.082_626_68e-3,
        }),
        drag: Some(DragParameters {
            pole: [0.0, 0.0, 1.0],
            omega: [0.0, 0.0, 7.292_115e-5],
            equatorial_radius: 6_378_137.0,
            polar_radius: 6_356_752.314,
            cd: 2.2,
            area: 4.0,
            mass: 500.0,
        }),
        radiation: Some(Radiation {
            cr: 1.3,
            area: 4.0,
            central_radius: 6_378_137.0,
        }),
        third_body: true,
        burn: Some(Burn {
            start: 350_000.0,
            end: 350_600.0,
            force_newtons: [0.0, 0.02, 0.0],
            specific_impulse_seconds: 450.0,
        }),
    };
    let initial = Keyframe {
        time: 0.0,
        position: [7_000_000.0, 0.0, 0.0],
        velocity: [0.0, 7_500.0, 1000.0],
        mass: 500.0,
    };
    let end = 864_000.0;
    let trajectory = model
        .propagate(
            initial,
            end,
            Options {
                relative_tolerance: 1e-9,
                absolute_tolerance: 1e-9,
                initial_step: 10.0,
                max_step: 120.0,
                max_steps: 1_000_000,
            },
            |t| {
                let epoch = Epoch::new(start_epoch, t).unwrap();
                let sun = eph
                    .state(odd_ephem::Body::Sun, odd_ephem::Body::Earth, epoch)
                    .unwrap()
                    .state
                    .position_m;
                let moon = eph
                    .state(odd_ephem::Body::Moon, odd_ephem::Body::Earth, epoch)
                    .unwrap()
                    .state
                    .position_m;
                Ok(Environment {
                    sun: ThirdBody {
                        position: sun,
                        mu: 1.327_124_400_412_794_2e20,
                        radius: 695_700_000.0,
                    },
                    companion: ThirdBody {
                        position: moon,
                        mu: 4.902_800_118e12,
                        radius: 1_737_400.0,
                    },
                })
            },
        )
        .unwrap();
    let state = trajectory.sample(end).unwrap();
    let values = [
        state.position[0].to_bits(),
        state.position[1].to_bits(),
        state.position[2].to_bits(),
        state.velocity[0].to_bits(),
        state.velocity[1].to_bits(),
        state.velocity[2].to_bits(),
        state.mass.to_bits(),
    ];
    canonical_hash(1, &values).unwrap()
}

#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn integrator_oracle_byte(index: u32) -> u32 {
    static FINGERPRINT: std::sync::OnceLock<[u8; 32]> = std::sync::OnceLock::new();
    FINGERPRINT
        .get_or_init(integrator_fingerprint)
        .get(index as usize)
        .copied()
        .map_or(256, u32::from)
}

#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn blended_integrator_oracle_byte(index: u32) -> u32 {
    static FINGERPRINT: std::sync::OnceLock<[u8; 32]> = std::sync::OnceLock::new();
    FINGERPRINT
        .get_or_init(blended_integrator_fingerprint)
        .get(index as usize)
        .copied()
        .map_or(256, u32::from)
}

#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn analytical_integrator_oracle_byte(index: u32) -> u32 {
    static FINGERPRINT: std::sync::OnceLock<[u8; 32]> = std::sync::OnceLock::new();
    FINGERPRINT
        .get_or_init(analytical_integrator_fingerprint)
        .get(index as usize)
        .copied()
        .map_or(256, u32::from)
}

/// Canonical integer command timeline for a two-node rescue plan.
pub fn maneuver_fingerprint() -> [u8; 32] {
    use odd_astro::spline::Keyframe;
    use odd_sim::{
        planner::{ManeuverNode, Plan},
        CommandKind, Engine,
    };
    let initial = Keyframe {
        time: 0.0,
        position: [6_638_137.0, 0.0, 0.0],
        velocity: [0.0, 7749.104, 0.0],
        mass: 1200.0,
    };
    let engine = Engine {
        thrust_newtons: 2400.0,
        specific_impulse_seconds: 310.0,
        stage: 0,
        engine: 0,
    };
    let nodes = [
        ManeuverNode {
            epoch_us: 200_000_000,
            tnw_m_s: [82.38547669, 0.0, 0.0],
        },
        ManeuverNode {
            epoch_us: 2_979_885_241,
            tnw_m_s: [81.50942505, 0.0, 0.0],
        },
    ];
    let plan = Plan::compile(
        initial,
        3.986_004_418e14,
        10_800_000_000,
        1020.0,
        engine,
        &nodes,
    )
    .unwrap();
    let mut values = Vec::new();
    for command in plan.timeline.commands() {
        values.push(command.t_us as u64);
        let CommandKind::Burn {
            stage,
            engine,
            throttle_u16,
            dur_us,
            quat_i16,
        } = command.kind;
        values.extend([
            u64::from(stage),
            u64::from(engine),
            u64::from(throttle_u16),
            dur_us as u64,
        ]);
        values.extend(quat_i16.map(|q| q as i64 as u64));
    }
    canonical_hash(1, &values).unwrap()
}
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn maneuver_oracle_byte(index: u32) -> u32 {
    static FINGERPRINT: std::sync::OnceLock<[u8; 32]> = std::sync::OnceLock::new();
    FINGERPRINT
        .get_or_init(maneuver_fingerprint)
        .get(index as usize)
        .copied()
        .map_or(256, u32::from)
}

/// `oracle_wasm_bit_parity_propulsion`, native half.
///
/// WP 1.5 propulsion parity grid: 32 engine designs spanning all four
/// propellant pairs, mixture ratio, chamber pressure, expansion ratio, bell
/// length and ambient altitude. Every field of the solution is hashed as raw
/// IEEE-754 bits, so a single-ULP divergence between native and wasm fails.
pub fn propulsion_fingerprint() -> [u8; 32] {
    use odd_propulsion::{nozzle, solve_engine, EngineSpec, Propellant};
    let mut values: Vec<u64> = Vec::new();
    for index in 0..32_u32 {
        let propellant = Propellant::from_u8((index % 4) as u8).unwrap();
        let (lo, hi) = propellant.of_band();
        let step = f64::from((index / 4) % 4) / 3.0;
        let of_ratio = lo + (hi - lo) * (0.25 + 0.5 * step);
        let p_c_pa = 1.5e6 + 6.0e6 * f64::from(index % 5);
        let r_throat_m = 0.02 + 0.03 * f64::from((index / 2) % 4);
        let expansion = 8.0 + 20.0 * f64::from(index % 4);
        let spec = EngineSpec {
            propellant,
            of_ratio,
            p_c_pa,
            r_throat_m,
            r_exit_m: r_throat_m * libm::sqrt(expansion),
            length_fraction: 0.6 + 0.1 * f64::from(index % 5),
        };
        let altitude = f64::from(index % 8) * 12_500.0;
        let ambient = nozzle::ambient_pressure(altitude);
        let solved = solve_engine(&spec, ambient).unwrap();
        values.extend([
            solved.ambient_pressure_pa.to_bits(),
            solved.thrust_n.to_bits(),
            solved.specific_impulse_s.to_bits(),
            solved.mass_flow_kg_s.to_bits(),
            solved.exhaust_velocity_m_s.to_bits(),
            solved.c_star_m_s.to_bits(),
            solved.chamber_temperature_k.to_bits(),
            solved.molar_mass_kg_mol.to_bits(),
            solved.gamma.to_bits(),
            solved.bulk_density_kg_m3.to_bits(),
            solved.expansion_ratio.to_bits(),
            solved.exit_mach.to_bits(),
            solved.exit_pressure_pa.to_bits(),
            solved.exit_temperature_k.to_bits(),
            solved.divergence_factor.to_bits(),
            solved.nozzle_length_m.to_bits(),
            solved.throat_heat_flux_w_m2.to_bits(),
            solved.throat_wall_temperature_k.to_bits(),
            solved.film_coefficient_w_m2_k.to_bits(),
            solved.dry_mass_kg.to_bits(),
            solved.pump_power_w.to_bits(),
            solved.chamber_radius_m.to_bits(),
            solved.chamber_length_m.to_bits(),
            u64::from(solved.thermal_limit_exceeded),
        ]);
        match solved.separation {
            Some(sep) => values.extend([
                1,
                sep.x_norm.to_bits(),
                sep.area_ratio_sep.to_bits(),
                sep.side_load_kn.to_bits(),
            ]),
            None => values.extend([0, 0, 0, 0]),
        }
        // The sampled flow field is part of the contract too: the browser
        // draws it and the dossier prints it.
        for station in odd_propulsion::flow_profile(&spec, ambient, 24).unwrap() {
            values.extend([
                station.x_norm.to_bits(),
                station.radius_m.to_bits(),
                station.mach.to_bits(),
                station.pressure_pa.to_bits(),
                station.density_kg_m3.to_bits(),
            ]);
        }
    }
    canonical_hash(1, &values).unwrap()
}
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn propulsion_oracle_byte(index: u32) -> u32 {
    static FINGERPRINT: std::sync::OnceLock<[u8; 32]> = std::sync::OnceLock::new();
    FINGERPRINT
        .get_or_init(propulsion_fingerprint)
        .get(index as usize)
        .copied()
        .map_or(256, u32::from)
}
