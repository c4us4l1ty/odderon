//! Local HTTP/SQLite authority. A reverse proxy serves web/ and forwards
//! /api/v1/ to this process. The signing seed is configured out of band.
use odd_worker::{http, sqlite::SqliteStore, AuthorityEngine, ServerKey};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let seed = std::env::var("ODDERON_SERVER_SEED")
        .map_err(|_| "set ODDERON_SERVER_SEED to a private 32-byte hex seed")?;
    let seed: [u8; 32] = odd_report::hex::decode(&seed)
        .and_then(|b| b.try_into().ok())
        .ok_or("ODDERON_SERVER_SEED must be exactly 64 hex characters")?;
    let key = ServerKey::from_seed(seed);
    let db = std::env::var("ODDERON_DB").unwrap_or_else(|_| "odderon.sqlite".into());
    let addr = std::env::var("ODDERON_BIND").unwrap_or_else(|_| "0.0.0.0:8787".into());
    let listener = std::net::TcpListener::bind(&addr)?;
    println!(
        "Authority public key: {}",
        odd_report::hex::encode(&key.public_key())
    );
    println!("Authority listening on {}", listener.local_addr()?);
    http::serve_listener(listener, AuthorityEngine::new(SqliteStore::open(&db)?, key))?;
    Ok(())
}
