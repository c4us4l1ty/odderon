//! Test helper: derive a storable flight's checkpoint chain from its signed
//! recorder using the same native replay that the authority runs.
use odd_report::Fdr;
use odd_worker::http::{checkpoint_to_json, json::Json};
use odd_worker::segment::build_chain;

fn main() -> Result<(), String> {
    let payload = std::env::args()
        .nth(1)
        .ok_or_else(|| "expected an FDR verify payload".to_string())?;
    let fdr = Fdr::from_verify_payload(&payload).map_err(|e| format!("{e:?}"))?;
    if !fdr.thermal_specs.is_empty() {
        return Err("cryogenic checkpoint temperatures are not supported".into());
    }
    let replay = odd_verify::verify_record(&fdr).map_err(|e| format!("{e:?}"))?;
    let end = (replay.report[29] * 1e6).round() as i64;
    let samples = [end / 3, 2 * end / 3, end].map(|t| {
        let frame = replay.trajectory.sample(t as f64 / 1e6).unwrap();
        (
            t,
            [
                frame.position[0],
                frame.position[1],
                frame.position[2],
                frame.velocity[0],
                frame.velocity[1],
                frame.velocity[2],
                frame.mass,
            ],
            frame.mass - fdr.dry_mass_kg,
            [0.0, 0.0],
        )
    });
    println!(
        "{}",
        Json::Arr(
            build_chain(&samples)
                .iter()
                .map(checkpoint_to_json)
                .collect()
        )
    );
    Ok(())
}
