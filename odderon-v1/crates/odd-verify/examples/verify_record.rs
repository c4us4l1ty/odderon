//! Headless proof-of-trajectory verifier.
//!
//! ```sh
//! cargo run --locked -p odd-verify --example verify_record -- '#verify=...'
//! cargo run --locked -p odd-verify --example verify_record -- record.txt
//! ```
//!
//! This is the whole point of the portable crate: no browser, no wasm, no
//! DOM. It calls the same `verify_record` the flight page calls, so a
//! recruiter (or an edge worker) reaches the same verdict by the same code.
//! Exit status: 0 verified, 1 rejected, 2 unreadable input.
use std::io::Read;

fn main() {
    let argument = std::env::args().nth(1);
    let text = match argument {
        Some(path) if path != "-" && !path.starts_with('#') => match std::fs::read_to_string(&path)
        {
            Ok(text) => text,
            Err(error) => {
                eprintln!("cannot read {path}: {error}");
                std::process::exit(2);
            }
        },
        Some(inline) => inline,
        None => {
            let mut text = String::new();
            if std::io::stdin().read_to_string(&mut text).is_err() {
                eprintln!("cannot read stdin");
                std::process::exit(2);
            }
            text
        }
    };
    let fdr = match odd_report::Fdr::from_verify_payload(&text) {
        Ok(fdr) => fdr,
        Err(error) => {
            eprintln!("rejected: {error}");
            std::process::exit(1);
        }
    };
    println!("contract   {}", fdr.contract_id);
    println!("callsign   {}", fdr.callsign);
    println!("mode       {}", fdr.attempt_mode.label());
    println!("digest     {}", odd_report::hex::encode(&fdr.digest()));
    match odd_verify::verify_record(&fdr) {
        Ok(outcome) => {
            println!("{}", odd_report::VERIFIED_BANNER);
            println!("score      {:.6}", outcome.report[4]);
            println!("final mass {:.6} kg", outcome.report[0]);
        }
        Err(error) => {
            eprintln!("rejected: {error}");
            std::process::exit(1);
        }
    }
}
