//! Print the crate's determinism hash.
//!
//! Exists so CI can ask the *native* target for the hash, and then ask the *wasm* target for the same
//! hash, in the same job — see `.github/workflows/ci.yml`. If the two disagree by one bit, the replay
//! and Proof-of-Flight guarantees in `docs/CREDIBILITY.md` §5 are false, so the build fails.
//!
//! Usage: `cargo run -p odderon-dmath --example hash`

fn main() {
    println!("{:#018x}", odderon_dmath::golden_hash());
}
