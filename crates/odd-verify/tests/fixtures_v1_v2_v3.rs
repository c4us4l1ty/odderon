//! `oracle_v1_v2_v3_backward_compatibility`.
//!
//! FDR v4 couples boil-off into the live mass, scores a flight on its fitted
//! dry mass and unifies the catalog tank volume. None of that may change what
//! a **historical** certificate means. The only honest way to prove that is to
//! keep real, signed records from the frozen schemas checked into the tree and
//! verify them with today's build, rather than regenerating them from whatever
//! the current code happens to compute.
//!
//! The fixtures under `tests/fixtures/` were produced by the last build that
//! issued each schema (commit `7cb0a55`, schema v3 frozen). They are base64url
//! `#verify=` payloads: canonical bytes ‖ public key ‖ signature, so verifying
//! one exercises the signature domain, the byte layout, the engine
//! certification, the contract framing and a full Level-3 replay.
//!
//! Regenerating them requires the *old* build. `ODD_FIXTURE_OUT=<dir> cargo test
//! -p odd-verify --test fixtures_v1_v2_v3 capture -- --ignored` writes the
//! fixtures this build would issue today; only run it against a checkout whose
//! `SCHEMA_VERSION` is the schema you are capturing, and never overwrite a
//! fixture with a different schema's bytes.

use odd_contracts::{Contract, ThermalDesign};
use odd_identity::License;
use odd_report::{AttemptMode, Command, Fdr, LEGACY_SCHEMA_VERSION, METRIC_COUNT};

/// The contract fixes its own start epoch; a record cannot choose another.
const EPOCH: i64 = odd_contracts::START_EPOCH;

/// Schema of the shipped record. Kept explicit rather than `SCHEMA_VERSION`
/// so the capture helper cannot silently start emitting a newer layout.
const FIXTURE_SCHEMA: u16 = 3;

fn rescue_commands() -> Vec<(i64, [f64; 3])> {
    // The same two-impulse Hohmann rescue `portable_replay` flies: a real
    // flight, not a coast, so the fixture exercises the burn timeline.
    let mu = odd_contracts::EARTH_MU;
    let r0 = odd_contracts::EARTH_EQUATORIAL_RADIUS + 260_000.0;
    let r1 = odd_contracts::EARTH_EQUATORIAL_RADIUS + 550_000.0;
    let a = (r0 + r1) * 0.5;
    let coast = std::f64::consts::PI * libm::sqrt(a * a * a / mu);
    let dv1 = libm::sqrt(mu / r0) * (libm::sqrt(r1 / a) - 1.0);
    let dv2 = libm::sqrt(mu / r1) * (1.0 - libm::sqrt(r0 / a));
    vec![
        (200_000_000, [dv1, 0.0, 0.0]),
        (((200.0 + coast) * 1e6).round() as i64, [dv2, 0.0, 0.0]),
    ]
}

/// Fly a storable Contract 101 rescue and hand back the recorder fields that
/// do **not** depend on the schema being captured.
fn rescore_rescue(schema: u16) -> Fdr {
    let contract = Contract::get(101).unwrap();
    let commands = rescue_commands();
    let spec = odd_verify::FlightSpec {
        thermal: None,
        delegation: odd_contracts::DelegationMask::MANUAL,
        contract_id: contract.id,
        epoch_s: EPOCH,
        duration_us: contract.duration_us,
        initial_mass_kg: contract.initial.mass,
        dry_mass_kg: contract.dry_mass_kg,
        thrust_n: contract.engine.thrust_newtons,
        specific_impulse_s: contract.engine.specific_impulse_seconds,
        initial_state: [
            contract.initial.position[0],
            contract.initial.position[1],
            contract.initial.position[2],
            contract.initial.velocity[0],
            contract.initial.velocity[1],
            contract.initial.velocity[2],
        ],
        node_metric: false,
        // A locked prediction, so the score fields are defined too.
        prediction: [180.0, 550_000.0, 260_000.0, 120.0, 10_800.0, f64::NAN],
        commands: commands
            .iter()
            .map(|(time_us, tnw)| (*time_us, *tnw, None))
            .collect(),
    };
    let outcome = odd_verify::replay_flight(&spec).expect("the reference rescue must replay");
    Fdr {
        schema_version: schema,
        contract_id: contract.id,
        attempt_mode: AttemptMode::Ranked,
        callsign: "FROZEN V3".into(),
        epoch_s: EPOCH,
        duration_us: contract.duration_us,
        initial_mass_kg: contract.initial.mass,
        dry_mass_kg: contract.dry_mass_kg,
        thrust_n: contract.engine.thrust_newtons,
        specific_impulse_s: contract.engine.specific_impulse_seconds,
        // A schema-1 record carries no engine design at all.
        engine: (schema > LEGACY_SCHEMA_VERSION).then_some(contract.engine_spec),
        initial_state: spec.initial_state,
        node_metric: false,
        prediction_baseline: spec.prediction,
        commands: commands
            .iter()
            .map(|(time_us, tnw)| Command {
                time_us: *time_us,
                delta_v_tnw: *tnw,
                guidance: None,
            })
            .collect(),
        metrics: outcome.report,
        final_state_hash: outcome.final_state_hash,
        elo_before: 1200.0,
        elo_after: 1211.25,
        // Schema 1 and 2 predate the tank model entirely.
        thermal_specs: Vec::new(),
        delegation_mask: 0,
        vent_events: Vec::new(),
        boil_off_kg: [0.0; 2],
        cryocooler_energy_j: 0.0,
        flown_burn_windows: vec![],
        flameout_time_s: None,
        propellant_remaining_kg: None,
        public_key: [0; 32],
        signature: [0; 64],
    }
}

/// Fly the cryogenic Contract 103 tug with the baseline blanket and no burns,
/// so the record carries a real, replayed vent history.
fn cryogenic_tug(schema: u16, design: &ThermalDesign) -> Fdr {
    let tug = Contract::get(103).unwrap();
    let load = tug.initial.mass - tug.dry_mass_kg;
    let inputs = odd_verify::ThermalInputs {
        engine_spec: tug.engine_spec,
        design: *design,
        load_kg: load,
    };
    let stage = design
        .stage(&tug, &tug.engine_spec, load)
        .expect("the tug is cryogenic");
    let spec = odd_verify::FlightSpec {
        thermal: Some(inputs),
        delegation: odd_contracts::DelegationMask::MANUAL,
        contract_id: tug.id,
        epoch_s: EPOCH,
        duration_us: tug.duration_us,
        initial_mass_kg: tug.initial.mass,
        dry_mass_kg: tug.dry_mass_kg,
        thrust_n: tug.engine.thrust_newtons,
        specific_impulse_s: tug.engine.specific_impulse_seconds,
        initial_state: [
            tug.initial.position[0],
            tug.initial.position[1],
            tug.initial.position[2],
            tug.initial.velocity[0],
            tug.initial.velocity[1],
            tug.initial.velocity[2],
        ],
        node_metric: false,
        prediction: [f64::NAN; 6],
        commands: Vec::new(),
    };
    let outcome = odd_verify::replay_flight(&spec).expect("the cryogenic coast must replay");
    let solution = odd_propulsion::solve_engine(&tug.engine_spec, 0.0).unwrap();
    Fdr {
        schema_version: schema,
        contract_id: tug.id,
        attempt_mode: AttemptMode::Ranked,
        callsign: "FROZEN CRYO".into(),
        epoch_s: EPOCH,
        duration_us: tug.duration_us,
        initial_mass_kg: tug.initial.mass,
        dry_mass_kg: tug.dry_mass_kg,
        thrust_n: solution.thrust_n,
        specific_impulse_s: solution.specific_impulse_s,
        engine: Some(tug.engine_spec),
        initial_state: spec.initial_state,
        node_metric: false,
        prediction_baseline: [f64::NAN; 6],
        commands: Vec::new(),
        metrics: outcome.report,
        final_state_hash: outcome.final_state_hash,
        elo_before: 1200.0,
        elo_after: 1187.5,
        thermal_specs: vec![odd_report::StageThermalSpec {
            stage: 0,
            tank_volumes_m3: [stage.oxidizer.volume_m3, stage.fuel.volume_m3],
            propellant_id: tug.engine_spec.propellant as u8,
            mli_layers: design.mli_layers,
            epsilon_eff: design.epsilon_eff,
            tank_shell_material: design.shell,
            absorptivity: design.absorptivity,
            outer_epsilon: design.outer_epsilon,
            strut_conductance_w_k: design.strut_conductance_w_k,
            relief_pressure_pa: design.relief_pressure_pa,
            radiator_area_m2: design.radiator_area_m2,
            radiator_epsilon: design.radiator_epsilon,
            cryocooler_w: design.cryocooler_lift_w,
            cryocooler_carnot_fraction: design.cryocooler_carnot_fraction,
            array_w_at_1au: design.bus.array_w_at_1au,
            avionics_w: design.bus.avionics_w,
            electric_propulsion_w: design.bus.electric_propulsion_w,
            structure_temperature_k: design.structure_temperature_k,
            sink_temperature_k: design.sink_temperature_k,
        }],
        delegation_mask: 0,
        boil_off_kg: outcome.boil_off_kg,
        cryocooler_energy_j: outcome.cryocooler_energy_j,
        vent_events: outcome.vent_events.clone(),
        flown_burn_windows: vec![],
        flameout_time_s: None,
        propellant_remaining_kg: None,
        public_key: [0; 32],
        signature: [0; 64],
    }
}

fn signed(mut fdr: Fdr) -> Fdr {
    fdr.sign(&License::from_entropy([7u8; 16]))
        .expect("fixture shape must be valid");
    fdr
}

/// A frozen record per shipped schema. Schema 1 and 2 flew the storable
/// rescue; schema 3 adds a cryogenic tug so the tank block is exercised too.
fn fixtures() -> Vec<(&'static str, Fdr)> {
    vec![
        (
            "v1-rescue.txt",
            signed(rescore_rescue(LEGACY_SCHEMA_VERSION)),
        ),
        (
            "v2-rescue.txt",
            signed(rescore_rescue(odd_report::ENGINE_SCHEMA_VERSION)),
        ),
        ("v3-rescue.txt", signed(rescore_rescue(FIXTURE_SCHEMA))),
        (
            "v3-cryo-tug.txt",
            signed(cryogenic_tug(FIXTURE_SCHEMA, &ThermalDesign::baseline())),
        ),
    ]
}

fn fixture_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// The gate. Every checked-in historical record must verify, on the signature
/// and on a full replay, exactly as it did when it was issued.
#[test]
fn oracle_v1_v2_v3_records_still_verify() {
    let dir = fixture_dir();
    let mut checked = 0;
    for (name, _) in fixtures() {
        let path = dir.join(name);
        let payload = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("fixture {} is missing ({e}); it is a signed historical record and must not be regenerated by this build", path.display()));
        let fdr = Fdr::from_verify_payload(payload.trim())
            .unwrap_or_else(|e| panic!("fixture {name} does not parse: {e:?}"));
        // The declared schema is a property of the record, not of this build.
        assert!(fdr.to_json().contains("\"schema_version\""));
        let outcome = odd_verify::verify_record(&fdr)
            .unwrap_or_else(|e| panic!("fixture {name} no longer verifies: {e:?}"));
        assert_eq!(outcome.report.len(), METRIC_COUNT);
        // A transport round trip must not change a single signed byte.
        let again = Fdr::from_verify_payload(&fdr.verify_payload()).unwrap();
        assert_eq!(again.canonical_bytes(), fdr.canonical_bytes(), "{name}");
        checked += 1;
    }
    assert_eq!(checked, 4, "all four frozen fixtures must be present");
}

/// The frozen records must keep their byte-for-byte digests: a dossier QR that
/// decoded yesterday must still decode to the same verdict tomorrow.
#[test]
fn frozen_fixture_digests_are_stable() {
    let dir = fixture_dir();
    for (name, expected_digest) in [
        ("v1-rescue.txt", FROZEN_DIGESTS[0]),
        ("v2-rescue.txt", FROZEN_DIGESTS[1]),
        ("v3-rescue.txt", FROZEN_DIGESTS[2]),
        ("v3-cryo-tug.txt", FROZEN_DIGESTS[3]),
    ] {
        let Ok(payload) = std::fs::read_to_string(dir.join(name)) else {
            // The other test reports a missing fixture; do not double-report.
            continue;
        };
        let fdr = Fdr::from_verify_payload(payload.trim()).unwrap();
        assert_eq!(
            odd_report::hex::encode(&fdr.digest()),
            expected_digest,
            "{name} canonical digest changed: the signed layout of a frozen schema moved"
        );
    }
}

/// Digests of the shipped fixtures, recorded when they were issued.
pub const FROZEN_DIGESTS: [&str; 4] = [
    "961ca2df446990309831dee5a484be90b7ce7a9031569df7a3be426c097f7dce",
    "38b753a206d427ca79829219949c0622327dbab4ef7feb80289b33c8c0a84f65",
    "83a7c69eaa267fecb79fa4489062004716379ec3a25485d5575b92b6569a569c",
    "97805fdcebfb066d072481babe4f46183c96e3825de820d8ed841bafb171b1d1",
];

/// Capture helper. Only meaningful on a checkout that still issues
/// `FIXTURE_SCHEMA`; see the module docs.
#[test]
#[ignore = "capture helper: writes fixtures, never part of the gate"]
fn capture() {
    let Ok(out) = std::env::var("ODD_FIXTURE_OUT") else {
        panic!("set ODD_FIXTURE_OUT to the directory that should receive the fixtures");
    };
    std::fs::create_dir_all(&out).unwrap();
    for (name, fdr) in fixtures() {
        std::fs::write(std::path::Path::new(&out).join(name), fdr.verify_payload()).unwrap();
        println!("{name}: {}", odd_report::hex::encode(&fdr.digest()));
    }
}
