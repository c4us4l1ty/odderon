//! Scalar WASM boundary for the Aerospace Flight License.
//!
//! Follows the mission bridge conventions: no pointers cross the boundary,
//! strings are read as UTF-8 length + byte, and byte payloads travel through
//! one absorb channel because wasm32 has no shared memory with the host here.
//!
//! The secret never leaves this module. Only the public key, the public ID
//! and the two user-facing encodings are exported, and the mnemonic/flight
//! key are exported solely so the profile dialog can show the backup the
//! player explicitly asked to see.
use odd_identity::{License, ENTROPY_BYTES};
use std::sync::Mutex;

/// Upper bound on an absorbed payload. A dossier JSON is a few kilobytes;
/// this stops a hostile page from growing the buffer without limit.
const MAX_INPUT: usize = 1 << 20;

static INPUT: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static LICENSE: Mutex<Option<License>> = Mutex::new(None);
static SIGNATURE: Mutex<[u8; 64]> = Mutex::new([0u8; 64]);

/// Clear the shared byte channel.
#[no_mangle]
pub extern "C" fn mission_input_reset() {
    INPUT.lock().unwrap().clear();
}

/// Append one byte to the shared channel. Returns 0 if the value is not a
/// byte or the buffer is full.
#[no_mangle]
pub extern "C" fn mission_input_absorb(byte: f64) -> i32 {
    if !byte.is_finite() || !(0.0..=255.0).contains(&byte) || byte.fract() != 0.0 {
        return 0;
    }
    let mut input = INPUT.lock().unwrap();
    if input.len() >= MAX_INPUT {
        return 0;
    }
    input.push(byte as u8);
    1
}

#[no_mangle]
pub extern "C" fn mission_input_len() -> u32 {
    INPUT.lock().unwrap().len() as u32
}

fn absorbed_text() -> Option<String> {
    String::from_utf8(INPUT.lock().unwrap().clone()).ok()
}

/// Load a license from 16 entropy bytes packed as four big-endian u32 words.
/// Each word is passed as an f64 because u32 values are exactly representable.
#[no_mangle]
pub extern "C" fn mission_identity_from_entropy(w0: f64, w1: f64, w2: f64, w3: f64) -> i32 {
    let mut entropy = [0u8; ENTROPY_BYTES];
    for (i, word) in [w0, w1, w2, w3].into_iter().enumerate() {
        if !word.is_finite() || word < 0.0 || word > u32::MAX as f64 || word.fract() != 0.0 {
            return 0;
        }
        entropy[i * 4..i * 4 + 4].copy_from_slice(&(word as u32).to_be_bytes());
    }
    *LICENSE.lock().unwrap() = Some(License::from_entropy(entropy));
    1
}

/// Restore from the twelve words in the byte channel.
#[no_mangle]
pub extern "C" fn mission_identity_from_mnemonic() -> i32 {
    let Some(text) = absorbed_text() else {
        return 0;
    };
    match License::from_mnemonic(&text) {
        Ok(license) => {
            *LICENSE.lock().unwrap() = Some(license);
            1
        }
        Err(_) => 0,
    }
}

/// Restore from an `ODD-` flight key in the byte channel.
#[no_mangle]
pub extern "C" fn mission_identity_from_key() -> i32 {
    let Some(text) = absorbed_text() else {
        return 0;
    };
    match License::from_flight_key(&text) {
        Ok(license) => {
            *LICENSE.lock().unwrap() = Some(license);
            1
        }
        Err(_) => 0,
    }
}

/// Validate a mnemonic without loading it, for live feedback while typing.
/// Returns 1 valid, 0 invalid. Does not disturb the active license.
#[no_mangle]
pub extern "C" fn mission_identity_mnemonic_valid() -> i32 {
    match absorbed_text() {
        Some(text) => i32::from(License::from_mnemonic(&text).is_ok()),
        None => 0,
    }
}

/// Validate an `ODD-` key without loading it.
#[no_mangle]
pub extern "C" fn mission_identity_key_valid() -> i32 {
    match absorbed_text() {
        Some(text) => i32::from(License::from_flight_key(&text).is_ok()),
        None => 0,
    }
}

/// Forget the active license (used when switching accounts).
#[no_mangle]
pub extern "C" fn mission_identity_clear() {
    *LICENSE.lock().unwrap() = None;
    SIGNATURE.lock().unwrap().fill(0);
}

#[no_mangle]
pub extern "C" fn mission_identity_ready() -> i32 {
    i32::from(LICENSE.lock().unwrap().is_some())
}

/// Public key byte 0..31; NaN when absent or out of range.
#[no_mangle]
pub extern "C" fn mission_identity_pubkey(index: u32) -> f64 {
    LICENSE
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|l| l.public_key().get(index as usize).copied())
        .map_or(f64::NAN, |b| b as f64)
}

fn identity_string(field: u32) -> Option<String> {
    let guard = LICENSE.lock().unwrap();
    let license = guard.as_ref()?;
    Some(match field {
        0 => license.public_id(),
        1 => license.mnemonic(),
        2 => license.flight_key(),
        _ => return None,
    })
}

/// UTF-8 length of an identity string (0 public ID, 1 mnemonic, 2 flight key).
#[no_mangle]
pub extern "C" fn mission_identity_string_len(field: u32) -> u32 {
    identity_string(field).map_or(0, |s| s.len() as u32)
}

/// One UTF-8 byte of an identity string; NaN when out of range.
#[no_mangle]
pub extern "C" fn mission_identity_string_byte(field: u32, index: u32) -> f64 {
    identity_string(field)
        .and_then(|s| s.as_bytes().get(index as usize).copied())
        .map_or(f64::NAN, |b| b as f64)
}

/// Sign the byte channel with the active license. Returns 0 without a license.
#[no_mangle]
pub extern "C" fn mission_identity_sign() -> i32 {
    let guard = LICENSE.lock().unwrap();
    let Some(license) = guard.as_ref() else {
        return 0;
    };
    let message = INPUT.lock().unwrap().clone();
    *SIGNATURE.lock().unwrap() = license.sign(&message);
    1
}

#[no_mangle]
pub extern "C" fn mission_signature_len() -> u32 {
    64
}

#[no_mangle]
pub extern "C" fn mission_signature_byte(index: u32) -> f64 {
    SIGNATURE
        .lock()
        .unwrap()
        .get(index as usize)
        .map_or(f64::NAN, |b| *b as f64)
}

/// Verify the byte channel against a public key and signature supplied as
/// hex in the two static strings set by mission_verify_material.
static VERIFY_KEY: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static VERIFY_SIG: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// Reset the verification material buffers.
#[no_mangle]
pub extern "C" fn mission_verify_reset() {
    VERIFY_KEY.lock().unwrap().clear();
    VERIFY_SIG.lock().unwrap().clear();
}

/// Append a key byte (channel 0) or signature byte (channel 1).
#[no_mangle]
pub extern "C" fn mission_verify_absorb(channel: u32, byte: f64) -> i32 {
    if !byte.is_finite() || !(0.0..=255.0).contains(&byte) || byte.fract() != 0.0 {
        return 0;
    }
    let mut guard = match channel {
        0 => VERIFY_KEY.lock().unwrap(),
        1 => VERIFY_SIG.lock().unwrap(),
        _ => return 0,
    };
    let limit = if channel == 0 { 32 } else { 64 };
    if guard.len() >= limit {
        return 0;
    }
    guard.push(byte as u8);
    1
}

/// Verify the message in the byte channel. 1 valid, 0 invalid.
#[no_mangle]
pub extern "C" fn mission_verify() -> i32 {
    let key = VERIFY_KEY.lock().unwrap();
    let sig = VERIFY_SIG.lock().unwrap();
    let Ok(public_key) = <[u8; 32]>::try_from(key.as_slice()) else {
        return 0;
    };
    let Ok(signature) = <[u8; 64]>::try_from(sig.as_slice()) else {
        return 0;
    };
    let message = INPUT.lock().unwrap().clone();
    i32::from(odd_identity::verify(&public_key, &message, &signature).is_ok())
}

/// Run the RFC 8032 section 7.1 vectors inside this wasm instance.
/// Returns the number of vectors that reproduced exactly (3 when healthy).
/// This is the wasm half of oracle_identity_roundtrip_and_rfc8032: it proves
/// the compiled module signs identically to the native build.
#[no_mangle]
pub extern "C" fn mission_ed25519_selftest() -> u32 {
    use ed25519_compact::{KeyPair, Seed};
    type Vector = ([u8; 32], [u8; 32], &'static [u8], [u8; 64]);
    const VECTORS: [Vector; 3] = [
        (
            [
                0x9d, 0x61, 0xb1, 0x9d, 0xef, 0xfd, 0x5a, 0x60, 0xba, 0x84, 0x4a, 0xf4, 0x92, 0xec,
                0x2c, 0xc4, 0x44, 0x49, 0xc5, 0x69, 0x7b, 0x32, 0x69, 0x19, 0x70, 0x3b, 0xac, 0x03,
                0x1c, 0xae, 0x7f, 0x60,
            ],
            [
                0xd7, 0x5a, 0x98, 0x01, 0x82, 0xb1, 0x0a, 0xb7, 0xd5, 0x4b, 0xfe, 0xd3, 0xc9, 0x64,
                0x07, 0x3a, 0x0e, 0xe1, 0x72, 0xf3, 0xda, 0xa6, 0x23, 0x25, 0xaf, 0x02, 0x1a, 0x68,
                0xf7, 0x07, 0x51, 0x1a,
            ],
            b"",
            [
                0xe5, 0x56, 0x43, 0x00, 0xc3, 0x60, 0xac, 0x72, 0x90, 0x86, 0xe2, 0xcc, 0x80, 0x6e,
                0x82, 0x8a, 0x84, 0x87, 0x7f, 0x1e, 0xb8, 0xe5, 0xd9, 0x74, 0xd8, 0x73, 0xe0, 0x65,
                0x22, 0x49, 0x01, 0x55, 0x5f, 0xb8, 0x82, 0x15, 0x90, 0xa3, 0x3b, 0xac, 0xc6, 0x1e,
                0x39, 0x70, 0x1c, 0xf9, 0xb4, 0x6b, 0xd2, 0x5b, 0xf5, 0xf0, 0x59, 0x5b, 0xbe, 0x24,
                0x65, 0x51, 0x41, 0x43, 0x8e, 0x7a, 0x10, 0x0b,
            ],
        ),
        (
            [
                0x4c, 0xcd, 0x08, 0x9b, 0x28, 0xff, 0x96, 0xda, 0x9d, 0xb6, 0xc3, 0x46, 0xec, 0x11,
                0x4e, 0x0f, 0x5b, 0x8a, 0x31, 0x9f, 0x35, 0xab, 0xa6, 0x24, 0xda, 0x8c, 0xf6, 0xed,
                0x4f, 0xb8, 0xa6, 0xfb,
            ],
            [
                0x3d, 0x40, 0x17, 0xc3, 0xe8, 0x43, 0x89, 0x5a, 0x92, 0xb7, 0x0a, 0xa7, 0x4d, 0x1b,
                0x7e, 0xbc, 0x9c, 0x98, 0x2c, 0xcf, 0x2e, 0xc4, 0x96, 0x8c, 0xc0, 0xcd, 0x55, 0xf1,
                0x2a, 0xf4, 0x66, 0x0c,
            ],
            b"\x72",
            [
                0x92, 0xa0, 0x09, 0xa9, 0xf0, 0xd4, 0xca, 0xb8, 0x72, 0x0e, 0x82, 0x0b, 0x5f, 0x64,
                0x25, 0x40, 0xa2, 0xb2, 0x7b, 0x54, 0x16, 0x50, 0x3f, 0x8f, 0xb3, 0x76, 0x22, 0x23,
                0xeb, 0xdb, 0x69, 0xda, 0x08, 0x5a, 0xc1, 0xe4, 0x3e, 0x15, 0x99, 0x6e, 0x45, 0x8f,
                0x36, 0x13, 0xd0, 0xf1, 0x1d, 0x8c, 0x38, 0x7b, 0x2e, 0xae, 0xb4, 0x30, 0x2a, 0xee,
                0xb0, 0x0d, 0x29, 0x16, 0x12, 0xbb, 0x0c, 0x00,
            ],
        ),
        (
            [
                0xc5, 0xaa, 0x8d, 0xf4, 0x3f, 0x9f, 0x83, 0x7b, 0xed, 0xb7, 0x44, 0x2f, 0x31, 0xdc,
                0xb7, 0xb1, 0x66, 0xd3, 0x85, 0x35, 0x07, 0x6f, 0x09, 0x4b, 0x85, 0xce, 0x3a, 0x2e,
                0x0b, 0x44, 0x58, 0xf7,
            ],
            [
                0xfc, 0x51, 0xcd, 0x8e, 0x62, 0x18, 0xa1, 0xa3, 0x8d, 0xa4, 0x7e, 0xd0, 0x02, 0x30,
                0xf0, 0x58, 0x08, 0x16, 0xed, 0x13, 0xba, 0x33, 0x03, 0xac, 0x5d, 0xeb, 0x91, 0x15,
                0x48, 0x90, 0x80, 0x25,
            ],
            b"\xaf\x82",
            [
                0x62, 0x91, 0xd6, 0x57, 0xde, 0xec, 0x24, 0x02, 0x48, 0x27, 0xe6, 0x9c, 0x3a, 0xbe,
                0x01, 0xa3, 0x0c, 0xe5, 0x48, 0xa2, 0x84, 0x74, 0x3a, 0x44, 0x5e, 0x36, 0x80, 0xd7,
                0xdb, 0x5a, 0xc3, 0xac, 0x18, 0xff, 0x9b, 0x53, 0x8d, 0x16, 0xf2, 0x90, 0xae, 0x67,
                0xf7, 0x60, 0x98, 0x4d, 0xc6, 0x59, 0x4a, 0x7c, 0x15, 0xe9, 0x71, 0x6e, 0xd2, 0x8d,
                0xc0, 0x27, 0xbe, 0xce, 0xea, 0x1e, 0xc4, 0x0a,
            ],
        ),
    ];
    let mut passed = 0;
    for (seed, expected_pk, message, expected_sig) in VECTORS {
        let kp = KeyPair::from_seed(Seed::new(seed));
        if *kp.pk != expected_pk {
            continue;
        }
        let signature = kp.sk.sign(message, None);
        if *signature == expected_sig && kp.pk.verify(message, &signature).is_ok() {
            passed += 1;
        }
    }
    passed
}

/// Number of words in the aerospace table (the browser builds autocomplete
/// from the same table rather than shipping a second copy).
#[no_mangle]
pub extern "C" fn mission_word_count() -> u32 {
    odd_identity::wordlist::WORDS.len() as u32
}

#[no_mangle]
pub extern "C" fn mission_word_len(index: u32) -> u32 {
    odd_identity::wordlist::WORDS
        .get(index as usize)
        .map_or(0, |w| w.len() as u32)
}

#[no_mangle]
pub extern "C" fn mission_word_byte(index: u32, position: u32) -> f64 {
    odd_identity::wordlist::WORDS
        .get(index as usize)
        .and_then(|w| w.as_bytes().get(position as usize))
        .map_or(f64::NAN, |b| *b as f64)
}

/// The byte channel as text, for bridges that need a callsign or a payload.
pub(crate) fn input_bytes() -> Vec<u8> {
    INPUT.lock().unwrap().clone()
}

pub(crate) fn input_text() -> String {
    absorbed_text().unwrap_or_default()
}

/// The active license, so the recorder bridge can sign without the secret
/// ever crossing the WASM boundary.
pub(crate) fn active_license() -> Option<License> {
    LICENSE.lock().unwrap().clone()
}

/// Install a license directly. Native tests use this instead of the entropy
/// export so a test never has to move secret bytes through the boundary.
#[cfg(test)]
pub(crate) fn set_license_for_test(license: License) {
    *LICENSE.lock().unwrap() = Some(license);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn absorb(bytes: &[u8]) {
        mission_input_reset();
        for byte in bytes {
            assert_eq!(mission_input_absorb(*byte as f64), 1);
        }
    }

    #[test]
    fn entropy_words_pack_big_endian_and_roundtrip() {
        assert_eq!(
            mission_identity_from_entropy(
                0x0011_2233 as f64,
                0x4455_6677 as f64,
                0x8899_aabb_u32 as f64,
                0xccdd_eeff_u32 as f64
            ),
            1
        );
        assert_eq!(mission_identity_ready(), 1);
        let len = mission_identity_string_len(2);
        let key: String = (0..len)
            .map(|i| mission_identity_string_byte(2, i) as u8 as char)
            .collect();
        assert_eq!(key, "ODD-0011-2233-4455-6677-8899-AABB-CCDD-EEFF");
        // Restoring from that key must reach the same public key.
        let original: Vec<u8> = (0..32).map(|i| mission_identity_pubkey(i) as u8).collect();
        absorb(key.as_bytes());
        assert_eq!(mission_identity_from_key(), 1);
        let restored: Vec<u8> = (0..32).map(|i| mission_identity_pubkey(i) as u8).collect();
        assert_eq!(original, restored);
    }

    #[test]
    fn sign_and_verify_through_the_byte_channel() {
        assert_eq!(mission_identity_from_entropy(1.0, 2.0, 3.0, 4.0), 1);
        let public: Vec<u8> = (0..32).map(|i| mission_identity_pubkey(i) as u8).collect();
        absorb(b"flight data recorder canonical bytes");
        assert_eq!(mission_identity_sign(), 1);
        let signature: Vec<u8> = (0..mission_signature_len())
            .map(|i| mission_signature_byte(i) as u8)
            .collect();
        assert_eq!(signature.len(), 64);
        mission_verify_reset();
        for byte in &public {
            assert_eq!(mission_verify_absorb(0, *byte as f64), 1);
        }
        for byte in &signature {
            assert_eq!(mission_verify_absorb(1, *byte as f64), 1);
        }
        assert_eq!(mission_verify(), 1);
        // One flipped message bit must fail.
        absorb(b"flight data recorder canonical byteS");
        assert_eq!(mission_verify(), 0);
    }

    #[test]
    fn rfc8032_vectors_pass_in_this_build() {
        assert_eq!(mission_ed25519_selftest(), 3);
    }

    #[test]
    fn rejects_bad_input_and_absent_license() {
        mission_identity_clear();
        assert_eq!(mission_identity_ready(), 0);
        assert!(mission_identity_pubkey(0).is_nan());
        assert_eq!(mission_identity_string_len(0), 0);
        absorb(b"hello");
        assert_eq!(mission_identity_sign(), 0);
        assert_eq!(mission_input_absorb(-1.0), 0);
        assert_eq!(mission_input_absorb(256.0), 0);
        assert_eq!(mission_input_absorb(1.5), 0);
        assert_eq!(mission_input_absorb(f64::NAN), 0);
        assert_eq!(mission_identity_from_entropy(-1.0, 0.0, 0.0, 0.0), 0);
        assert_eq!(mission_verify_absorb(9, 0.0), 0);
        absorb(b"not a license");
        assert_eq!(mission_identity_mnemonic_valid(), 0);
        assert_eq!(mission_identity_from_mnemonic(), 0);
        assert_eq!(mission_identity_key_valid(), 0);
        assert_eq!(mission_word_count(), 2048);
        assert!(mission_word_len(2048) == 0);
        assert!(mission_word_byte(0, 999).is_nan());
    }
}
