//! Aerospace Flight License: one 128-bit secret with three faces.
//!
//! A player identity is 16 bytes of entropy. Everything else is derived, so
//! the same secret always produces the same words, the same flight key and
//! the same signing keypair on every platform:
//!
//! ```text
//! entropy (128 bits)
//!   ├─ + 4-bit SHA-256 checksum → 132 bits → 12 × 11-bit word indices
//!   ├─ hex                      → ODD-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX
//!   └─ SHA-512(domain ‖ entropy)[..32] → Ed25519 seed → signing keypair
//! ```
//!
//! The mnemonic carries a checksum so a typo is caught before it silently
//! opens a different account. The hex key carries the bare 128-bit entropy;
//! its checksum is recomputed rather than transmitted.
//!
//! Randomness is the caller's job. The browser supplies `crypto.getRandomValues`
//! bytes, which keeps this crate free of `getrandom` and buildable for
//! `wasm32-unknown-unknown` without a JS shim.

pub mod wordlist;

use ed25519_compact::{KeyPair, PublicKey, SecretKey, Seed, Signature};
use sha2::{Digest, Sha256, Sha512};

/// Domain separator for key derivation. Changing it changes every identity,
/// so it is versioned and must never be reused for another purpose.
pub const KEY_DOMAIN: &[u8] = b"odderon-flight-license-ed25519-v1";
/// Bytes of underlying secret entropy.
pub const ENTROPY_BYTES: usize = 16;
/// Words in a spoken/written license (132 bits / 11 bits per word).
pub const WORD_COUNT: usize = 12;
/// Hyphen-separated hex groups in an `ODD-` flight key.
pub const KEY_GROUPS: usize = 8;
/// Bytes of public key folded into the public ID fingerprint.
pub const FINGERPRINT_BYTES: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LicenseError {
    /// Entropy or seed had the wrong length.
    Length,
    /// Mnemonic did not contain exactly twelve words.
    WordCount,
    /// A word is not in the table.
    UnknownWord,
    /// The mnemonic's 4-bit checksum does not match its entropy.
    Checksum,
    /// Flight key was not `ODD-` followed by eight 4-hex groups.
    KeyFormat,
    /// A public key or signature had the wrong length or encoding.
    Encoding,
    /// Signature did not verify against the public key and message.
    Signature,
    /// Callsign was empty, too long, or used unsupported characters.
    Callsign,
}

impl core::fmt::Display for LicenseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let text = match self {
            Self::Length => "wrong entropy length",
            Self::WordCount => "a flight license has exactly twelve words",
            Self::UnknownWord => "word is not in the aerospace table",
            Self::Checksum => "license checksum does not match",
            Self::KeyFormat => "expected ODD- followed by eight 4-character hex groups",
            Self::Encoding => "malformed key or signature encoding",
            Self::Signature => "signature does not verify",
            Self::Callsign => "callsign must be 1-24 letters, digits, spaces, - or _",
        };
        f.write_str(text)
    }
}

/// The 4-bit checksum: high nibble of SHA-256 over the entropy.
fn checksum_nibble(entropy: &[u8; ENTROPY_BYTES]) -> u8 {
    Sha256::digest(entropy)[0] >> 4
}

/// Encode 128-bit entropy as twelve aerospace words.
pub fn mnemonic(entropy: &[u8; ENTROPY_BYTES]) -> [&'static str; WORD_COUNT] {
    // 132 bits held in 17 bytes, left-aligned: entropy then the checksum
    // nibble in the high half of the final byte.
    let mut bits = [0u8; ENTROPY_BYTES + 1];
    bits[..ENTROPY_BYTES].copy_from_slice(entropy);
    bits[ENTROPY_BYTES] = checksum_nibble(entropy) << 4;
    core::array::from_fn(|i| {
        let bit = i * 11;
        let mut index = 0usize;
        for offset in 0..11 {
            let position = bit + offset;
            let byte = bits[position / 8];
            let value = (byte >> (7 - (position % 8))) & 1;
            index = (index << 1) | value as usize;
        }
        wordlist::WORDS[index]
    })
}

/// Recover entropy from twelve words, verifying the embedded checksum.
pub fn entropy_from_mnemonic(text: &str) -> Result<[u8; ENTROPY_BYTES], LicenseError> {
    let mut bits = [0u8; ENTROPY_BYTES + 1];
    let mut count = 0usize;
    for token in text.split_whitespace() {
        if count == WORD_COUNT {
            return Err(LicenseError::WordCount);
        }
        let index =
            wordlist::index_of(&token.to_ascii_lowercase()).ok_or(LicenseError::UnknownWord)?;
        let start = count * 11;
        for offset in 0..11 {
            let position = start + offset;
            let value = (index >> (10 - offset)) & 1;
            bits[position / 8] |= (value as u8) << (7 - (position % 8));
        }
        count += 1;
    }
    if count != WORD_COUNT {
        return Err(LicenseError::WordCount);
    }
    let entropy: [u8; ENTROPY_BYTES] = bits[..ENTROPY_BYTES]
        .try_into()
        .map_err(|_| LicenseError::Length)?;
    if bits[ENTROPY_BYTES] >> 4 != checksum_nibble(&entropy) {
        return Err(LicenseError::Checksum);
    }
    Ok(entropy)
}

/// Format entropy as a Mullvad-style `ODD-` flight key.
pub fn flight_key(entropy: &[u8; ENTROPY_BYTES]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(4 + KEY_GROUPS * 5 - 1);
    out.push_str("ODD-");
    for (i, byte) in entropy.iter().enumerate() {
        if i > 0 && i % 2 == 0 {
            out.push('-');
        }
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

/// Parse an `ODD-` flight key. Case and surrounding whitespace are ignored.
pub fn entropy_from_flight_key(text: &str) -> Result<[u8; ENTROPY_BYTES], LicenseError> {
    let trimmed = text.trim();
    let body = trimmed
        .strip_prefix("ODD-")
        .or_else(|| trimmed.strip_prefix("odd-"))
        .ok_or(LicenseError::KeyFormat)?;
    let groups: Vec<&str> = body.split('-').collect();
    if groups.len() != KEY_GROUPS {
        return Err(LicenseError::KeyFormat);
    }
    let mut entropy = [0u8; ENTROPY_BYTES];
    for (g, group) in groups.iter().enumerate() {
        if group.len() != 4 {
            return Err(LicenseError::KeyFormat);
        }
        for (i, ch) in group.chars().enumerate() {
            let digit = ch.to_digit(16).ok_or(LicenseError::KeyFormat)? as u8;
            entropy[g * 2 + i / 2] |= digit << if i % 2 == 0 { 4 } else { 0 };
        }
    }
    Ok(entropy)
}

/// Derive the Ed25519 signing keypair from the license entropy.
///
/// The seed is `SHA-512(KEY_DOMAIN ‖ entropy)[..32]`. Ed25519 then applies its
/// own SHA-512 expansion internally, so signatures are ordinary RFC 8032
/// signatures over the derived key — not a bespoke scheme.
pub fn keypair(entropy: &[u8; ENTROPY_BYTES]) -> KeyPair {
    let mut hasher = Sha512::new();
    hasher.update(KEY_DOMAIN);
    hasher.update(entropy);
    let expanded = hasher.finalize();
    let seed: [u8; 32] = expanded[..32]
        .try_into()
        .expect("SHA-512 produces 64 bytes");
    KeyPair::from_seed(Seed::new(seed))
}

/// The seed alone, for callers that persist or export it deliberately.
pub fn seed(entropy: &[u8; ENTROPY_BYTES]) -> [u8; 32] {
    let mut hasher = Sha512::new();
    hasher.update(KEY_DOMAIN);
    hasher.update(entropy);
    let expanded = hasher.finalize();
    expanded[..32]
        .try_into()
        .expect("SHA-512 produces 64 bytes")
}

/// RFC 4648 base32 (A-Z, 2-7), unpadded.
fn base32(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut out = String::new();
    let mut buffer = 0u32;
    let mut held = 0u32;
    for byte in bytes {
        buffer = (buffer << 8) | *byte as u32;
        held += 8;
        while held >= 5 {
            held -= 5;
            out.push(ALPHABET[((buffer >> held) & 31) as usize] as char);
        }
    }
    if held > 0 {
        out.push(ALPHABET[((buffer << (5 - held)) & 31) as usize] as char);
    }
    out
}

/// Public fingerprint: `ODD-ID-` plus base32 of the first ten pubkey bytes.
/// This is a display handle, not an authentication token; verification always
/// uses the full 32-byte public key.
pub fn public_id(public_key: &[u8; 32]) -> String {
    format!("ODD-ID-{}", base32(&public_key[..FINGERPRINT_BYTES]))
}

/// Callsigns are display names: 1-24 letters, digits, spaces, `-` or `_`.
pub fn validate_callsign(name: &str) -> Result<(), LicenseError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 24 {
        return Err(LicenseError::Callsign);
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '-' || c == '_')
    {
        return Err(LicenseError::Callsign);
    }
    Ok(())
}

/// A restored identity. Holds the secret, so it is never serialized wholesale
/// into a dossier; only the public halves leave the machine.
#[derive(Clone)]
pub struct License {
    entropy: [u8; ENTROPY_BYTES],
    keys: KeyPair,
}

impl License {
    pub fn from_entropy(entropy: [u8; ENTROPY_BYTES]) -> Self {
        let keys = keypair(&entropy);
        Self { entropy, keys }
    }

    pub fn from_mnemonic(text: &str) -> Result<Self, LicenseError> {
        Ok(Self::from_entropy(entropy_from_mnemonic(text)?))
    }

    pub fn from_flight_key(text: &str) -> Result<Self, LicenseError> {
        Ok(Self::from_entropy(entropy_from_flight_key(text)?))
    }

    pub fn entropy(&self) -> &[u8; ENTROPY_BYTES] {
        &self.entropy
    }

    pub fn mnemonic(&self) -> String {
        mnemonic(&self.entropy).join(" ")
    }

    pub fn flight_key(&self) -> String {
        flight_key(&self.entropy)
    }

    pub fn public_key(&self) -> [u8; 32] {
        let pk: PublicKey = self.keys.pk;
        *pk
    }

    pub fn public_id(&self) -> String {
        public_id(&self.public_key())
    }

    pub fn secret_key(&self) -> SecretKey {
        self.keys.sk.clone()
    }

    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        *self.keys.sk.sign(message, None)
    }
}

/// Verify a detached Ed25519 signature. Returns `Signature` on any failure so
/// callers cannot accidentally distinguish "malformed" from "wrong key" in a
/// way that leaks information.
pub fn verify(
    public_key: &[u8; 32],
    message: &[u8],
    signature: &[u8; 64],
) -> Result<(), LicenseError> {
    let pk = PublicKey::new(*public_key);
    let sig = Signature::new(*signature);
    pk.verify(message, &sig)
        .map_err(|_| LicenseError::Signature)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: [u8; ENTROPY_BYTES] = [
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xff,
    ];

    #[test]
    fn wordlist_is_sorted_lowercase_and_prefix_unique() {
        assert_eq!(wordlist::WORDS.len(), 2048);
        let mut prefixes = std::collections::BTreeSet::new();
        for pair in wordlist::WORDS.windows(2) {
            assert!(pair[0] < pair[1], "not strictly sorted: {pair:?}");
        }
        for word in wordlist::WORDS {
            assert!(
                (4..=10).contains(&word.len()),
                "{word} has length {}",
                word.len()
            );
            assert!(
                word.chars().all(|c| c.is_ascii_lowercase()),
                "{word} is not lowercase ASCII"
            );
            assert!(prefixes.insert(&word[..4]), "{word} collides on prefix");
        }
        assert_eq!(prefixes.len(), 2048);
    }

    #[test]
    fn binary_search_matches_linear_scan() {
        for (i, word) in wordlist::WORDS.iter().enumerate() {
            assert_eq!(wordlist::index_of(word), Some(i));
        }
        assert_eq!(wordlist::index_of("notaword"), None);
    }

    #[test]
    fn mnemonic_roundtrips_and_detects_corruption() {
        let words = mnemonic(&SAMPLE);
        let text = words.join(" ");
        assert_eq!(entropy_from_mnemonic(&text).unwrap(), SAMPLE);
        // Case and whitespace are forgiving.
        assert_eq!(
            entropy_from_mnemonic(&text.to_uppercase().replace(' ', "   ")).unwrap(),
            SAMPLE
        );
        // Substituting a different valid word must fail the checksum, not
        // silently open a different identity.
        let mut broken = words;
        broken[5] = wordlist::WORDS[(wordlist::index_of(broken[5]).unwrap() + 1) % 2048];
        assert_eq!(
            entropy_from_mnemonic(&broken.join(" ")),
            Err(LicenseError::Checksum)
        );
        assert_eq!(
            entropy_from_mnemonic(&words[..11].join(" ")),
            Err(LicenseError::WordCount)
        );
        let mut extra = text.clone();
        extra.push_str(" orbit");
        assert_eq!(entropy_from_mnemonic(&extra), Err(LicenseError::WordCount));
    }

    #[test]
    fn flight_key_roundtrips_and_rejects_malformed() {
        let key = flight_key(&SAMPLE);
        assert_eq!(key, "ODD-0011-2233-4455-6677-8899-AABB-CCDD-EEFF");
        assert_eq!(entropy_from_flight_key(&key).unwrap(), SAMPLE);
        assert_eq!(
            entropy_from_flight_key(&key.to_lowercase()).unwrap(),
            SAMPLE
        );
        for bad in [
            "ODD-0011-2233",
            "XXX-0011-2233-4455-6677-8899-AABB-CCDD-EEFF",
            "ODD-0011-2233-4455-6677-8899-AABB-CCDD-EEFG",
            "ODD-0011-2233-4455-6677-8899-AABB-CCDD-EEF",
        ] {
            assert_eq!(entropy_from_flight_key(bad), Err(LicenseError::KeyFormat));
        }
    }

    #[test]
    fn derivation_is_deterministic_and_domain_separated() {
        let a = License::from_entropy(SAMPLE);
        let b = License::from_entropy(SAMPLE);
        assert_eq!(a.public_key(), b.public_key());
        assert_eq!(a.mnemonic(), b.mnemonic());
        assert_eq!(a.flight_key(), b.flight_key());
        let mut other = SAMPLE;
        other[0] ^= 1;
        assert_ne!(a.public_key(), License::from_entropy(other).public_key());
        // The seed must actually depend on the domain string.
        let mut plain = Sha512::new();
        plain.update(SAMPLE);
        assert_ne!(&seed(&SAMPLE)[..], &plain.finalize()[..32]);
    }

    #[test]
    fn sign_verify_and_tamper_rejection() {
        let license = License::from_entropy(SAMPLE);
        let message = b"canonical flight data recorder bytes";
        let signature = license.sign(message);
        assert!(verify(&license.public_key(), message, &signature).is_ok());
        let mut tampered = signature;
        tampered[0] ^= 1;
        assert_eq!(
            verify(&license.public_key(), message, &tampered),
            Err(LicenseError::Signature)
        );
        assert_eq!(
            verify(&license.public_key(), b"different message", &signature),
            Err(LicenseError::Signature)
        );
    }

    #[test]
    fn public_id_is_stable_base32() {
        let id = public_id(&[0u8; 32]);
        assert_eq!(id, "ODD-ID-AAAAAAAAAAAAAAAA");
        let license = License::from_entropy(SAMPLE);
        assert!(license.public_id().starts_with("ODD-ID-"));
        assert_eq!(license.public_id().len(), 7 + 16);
    }

    #[test]
    fn callsign_rules() {
        for good in ["Ada", "Flight Director 1", "a_b-c", "A"] {
            assert!(validate_callsign(good).is_ok(), "{good}");
        }
        for bad in ["", "   ", "<script>", "a".repeat(25).as_str()] {
            assert_eq!(validate_callsign(bad), Err(LicenseError::Callsign));
        }
    }
}
