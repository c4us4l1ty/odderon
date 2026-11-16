//! Canonical design/plan and prediction hashes for the launch commitment.
//!
//! The honest browser commits to `H(design + plan)` and `H(prediction)` at
//! LOCK & LAUNCH, before it starts the modeled flight. At submit the server
//! recomputes both from the presented FDR and refuses a mismatch. Re-signing
//! a changed prediction fails `PredictionCommitmentMismatch`, but a commitment
//! cannot prove that an untrusted client did not simulate beforehand.

use crate::protocol::{put_f64, put_i64, put_u32, Hash};
use odd_report::fdr::Fdr;
use sha2::{Digest, Sha256};

/// Domain separator for the vehicle-and-plan hash. V2 adds every maneuver;
/// old v1 tickets cannot authorize a new submission under this rule.
pub const DESIGN_DOMAIN: &[u8] = b"odderon-design-hash-v2";
/// Domain separator for the locked prediction hash.
pub const PREDICTION_DOMAIN: &[u8] = b"odderon-prediction-hash-v1";

/// Hash the physical vehicle and the locked maneuver plan: masses, engine,
/// cryogenic fit-out, delegation, epochs, delta-v and optional guidance.
/// Claimed outcomes are excluded and are checked by the replay instead.
pub fn design_hash(fdr: &Fdr) -> Hash {
    let mut out = Vec::new();
    out.extend_from_slice(DESIGN_DOMAIN);
    put_f64(&mut out, fdr.dry_mass_kg);
    put_f64(&mut out, fdr.initial_mass_kg);
    put_f64(&mut out, fdr.thrust_n);
    put_f64(&mut out, fdr.specific_impulse_s);
    if let Some(engine) = fdr.engine {
        out.push(1);
        out.push(engine.propellant as u8);
        put_f64(&mut out, engine.of_ratio);
        put_f64(&mut out, engine.p_c_pa);
        put_f64(&mut out, engine.r_throat_m);
        put_f64(&mut out, engine.r_exit_m);
        put_f64(&mut out, engine.length_fraction);
    } else {
        out.push(0);
    }
    out.extend_from_slice(&(fdr.thermal_specs.len() as u16).to_be_bytes());
    for spec in &fdr.thermal_specs {
        out.push(spec.stage);
        for v in &spec.tank_volumes_m3 {
            put_f64(&mut out, *v);
        }
        out.push(spec.propellant_id);
        put_u32(&mut out, spec.mli_layers);
        put_f64(&mut out, spec.epsilon_eff);
        out.push(spec.tank_shell_material);
        put_f64(&mut out, spec.absorptivity);
        put_f64(&mut out, spec.outer_epsilon);
        put_f64(&mut out, spec.strut_conductance_w_k);
        put_f64(&mut out, spec.relief_pressure_pa);
        put_f64(&mut out, spec.radiator_area_m2);
        put_f64(&mut out, spec.radiator_epsilon);
        put_f64(&mut out, spec.cryocooler_w);
        put_f64(&mut out, spec.cryocooler_carnot_fraction);
        put_f64(&mut out, spec.array_w_at_1au);
        put_f64(&mut out, spec.avionics_w);
        put_f64(&mut out, spec.electric_propulsion_w);
        put_f64(&mut out, spec.structure_temperature_k);
        put_f64(&mut out, spec.sink_temperature_k);
    }
    out.push(fdr.delegation_mask);
    out.extend_from_slice(&(fdr.commands.len() as u16).to_be_bytes());
    for command in &fdr.commands {
        put_i64(&mut out, command.time_us);
        for value in &command.delta_v_tnw {
            put_f64(&mut out, *value);
        }
        match command.guidance {
            None => out.push(0),
            Some(values) => {
                out.push(1);
                for value in &values {
                    put_f64(&mut out, *value);
                }
            }
        }
    }
    Sha256::digest(&out).into()
}

/// Hash the six locked prediction numbers. NaNs hash as their bit pattern,
/// so an unpredicted field is still bound by the commitment.
pub fn prediction_hash(fdr: &Fdr) -> Hash {
    let mut out = Vec::new();
    out.extend_from_slice(PREDICTION_DOMAIN);
    for v in &fdr.prediction_baseline {
        put_f64(&mut out, *v);
    }
    Sha256::digest(&out).into()
}

/// Hash an explicit prediction array, for a client committing before it has
/// assembled a full FDR.
pub fn prediction_hash_from(prediction: &[f64; 6]) -> Hash {
    let mut out = Vec::new();
    out.extend_from_slice(PREDICTION_DOMAIN);
    for v in prediction {
        put_f64(&mut out, *v);
    }
    Sha256::digest(&out).into()
}
