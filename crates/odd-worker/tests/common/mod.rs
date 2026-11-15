//! Shared test fixtures: a verifying rescue record and commit/submit helpers.
#![allow(dead_code)]

use odd_contracts::{Contract, EARTH_EQUATORIAL_RADIUS, EARTH_MU};
use odd_ephem::Ephemeris;
use odd_identity::License;
use odd_report::{Fdr, SCHEMA_VERSION};
use odd_sim::mission::{LockedMission, PredictionBaseline};
use odd_sim::planner::{ManeuverNode, Plan};
use odd_verify::{compute, FlightSettings};
use odd_worker::{AuthorityEngine, CommitRequest, MemoryStore, ServerKey, Store, SubmitRequest};

pub const NOW: i64 = 1_700_000_000;

pub fn server_key() -> ServerKey {
    ServerKey::from_seed([0x5e; 32])
}

pub fn engine() -> AuthorityEngine<MemoryStore> {
    AuthorityEngine::new(MemoryStore::new(), server_key())
}

/// The reference two-impulse rescue, compiled for contract 101.
pub fn rescue_plan(contract: &Contract) -> (Plan, [ManeuverNode; 2]) {
    let r0 = EARTH_EQUATORIAL_RADIUS + 260_000.0;
    let r1 = EARTH_EQUATORIAL_RADIUS + 550_000.0;
    let a = (r0 + r1) * 0.5;
    let coast = std::f64::consts::PI * libm::sqrt(a * a * a / EARTH_MU);
    let v0 = libm::sqrt(EARTH_MU / r0);
    let v1 = libm::sqrt(EARTH_MU / r1);
    let nodes = [
        ManeuverNode {
            epoch_us: 200_000_000,
            tnw_m_s: [v0 * (libm::sqrt(a / r0) - 1.0), 0.0, 0.0],
        },
        ManeuverNode {
            epoch_us: ((200.0 + coast) * 1e6) as i64,
            tnw_m_s: [v1 * (1.0 - libm::sqrt(r0 / a)), 0.0, 0.0],
        },
    ];
    let plan = Plan::compile(
        contract.initial,
        EARTH_MU,
        contract.duration_us,
        contract.dry_mass_kg,
        contract.engine,
        &nodes,
    )
    .expect("the reference rescue compiles");
    (plan, nodes)
}

pub fn prediction_from_nodes(nodes: &[ManeuverNode; 2]) -> [f64; 6] {
    [
        nodes[0].tnw_m_s[0] + nodes[1].tnw_m_s[0],
        550_000.0,
        550_000.0,
        120.0,
        10_800.0,
        f64::NAN,
    ]
}

pub fn settings(contract: &Contract) -> FlightSettings {
    FlightSettings {
        contract_id: contract.id,
        epoch: odd_contracts::START_EPOCH,
        initial: contract.initial,
        duration_us: contract.duration_us,
        dry_mass: contract.dry_mass_kg,
        engine: contract.engine,
        node_metric: false,
        thermal: None,
        delegation: odd_contracts::DelegationMask::MANUAL,
    }
}

/// Fly and sign a rescue record whose locked prediction is `pred`. The
/// metrics are taken from the same locked replay the verifier will run, so
/// the record verifies on its own — a commitment failure is therefore
/// isolated from any replay failure.
pub fn signed_rescue(pred: [f64; 6]) -> (Fdr, License) {
    let eph = Ephemeris::bundled().unwrap();
    let contract = Contract::get(101).unwrap();
    let (plan, _nodes) = rescue_plan(&contract);
    let prediction = PredictionBaseline {
        delta_v_m_s: pred[0],
        apoapsis_altitude_m: pred[1],
        periapsis_altitude_m: pred[2],
        remaining_propellant_kg: pred[3],
        time_of_flight_s: pred[4],
        target_miss_distance_m: if pred[5].is_finite() && pred[5] >= 0.0 {
            Some(pred[5])
        } else {
            None
        },
    };
    let locked = LockedMission::new(plan.clone(), prediction, contract.dry_mass_kg).unwrap();
    let outcome = compute(&settings(&contract), &plan, Some(&locked), &eph).unwrap();
    let commands = _nodes
        .iter()
        .map(|n| odd_report::fdr::Command {
            time_us: n.epoch_us,
            delta_v_tnw: n.tnw_m_s,
            guidance: None,
        })
        .collect();
    let mut fdr = Fdr {
        schema_version: SCHEMA_VERSION,
        contract_id: contract.id,
        attempt_mode: odd_report::AttemptMode::Ranked,
        callsign: "SERVER ORACLE".into(),
        epoch_s: odd_contracts::START_EPOCH,
        duration_us: contract.duration_us,
        initial_mass_kg: contract.initial.mass,
        dry_mass_kg: contract.dry_mass_kg,
        thrust_n: contract.engine.thrust_newtons,
        specific_impulse_s: contract.engine.specific_impulse_seconds,
        engine: Some(contract.engine_spec),
        initial_state: [
            contract.initial.position[0],
            contract.initial.position[1],
            contract.initial.position[2],
            contract.initial.velocity[0],
            contract.initial.velocity[1],
            contract.initial.velocity[2],
        ],
        node_metric: false,
        prediction_baseline: pred,
        commands,
        metrics: outcome.report,
        final_state_hash: odd_report::fdr::final_state_hash(
            outcome.report[29],
            &[
                outcome.final_state.position[0],
                outcome.final_state.position[1],
                outcome.final_state.position[2],
                outcome.final_state.velocity[0],
                outcome.final_state.velocity[1],
                outcome.final_state.velocity[2],
            ],
            outcome.final_state.mass,
        ),
        elo_before: 1200.0,
        elo_after: 1200.0,
        thermal_specs: Vec::new(),
        delegation_mask: 0,
        boil_off_kg: [0.0, 0.0],
        cryocooler_energy_j: 0.0,
        vent_events: Vec::new(),
        public_key: [0; 32],
        signature: [0; 64],
    };
    let license = License::from_entropy([21u8; 16]);
    fdr.sign(&license).unwrap();
    (fdr, license)
}

/// Commit to a record's design and prediction, signing as the pilot.
pub fn commit_for<S: Store>(
    engine: &mut AuthorityEngine<S>,
    fdr: &Fdr,
    license: &License,
) -> odd_worker::CommitTicket {
    let mut req = CommitRequest {
        contract_id: fdr.contract_id,
        pubkey: license.public_key(),
        design_hash: odd_worker::commitment::design_hash(fdr),
        prediction_hash: odd_worker::commitment::prediction_hash(fdr),
        client_sig: [0u8; 64],
    };
    req.client_sig = license.sign(&req.canonical_bytes());
    engine.commit(req, NOW).expect("the commit is accepted")
}

pub fn submit_request(fdr: &Fdr, ticket: odd_worker::CommitTicket) -> SubmitRequest {
    SubmitRequest {
        fdr_payload: fdr.verify_payload(),
        ticket,
        segments: None,
    }
}

/// The reference rescue's locked prediction, for tests that just need a
/// valid honest prediction without rebuilding the plan themselves.
pub fn prediction_a() -> [f64; 6] {
    let contract = Contract::get(101).unwrap();
    let (_plan, nodes) = rescue_plan(&contract);
    prediction_from_nodes(&nodes)
}
