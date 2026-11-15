//! oracle_identity_roundtrip_and_rfc8032 (native half).
//!
//! The Flight License must not invent a signature scheme. It derives an
//! Ed25519 seed and then uses ordinary RFC 8032 Ed25519, so the standard
//! published vectors have to reproduce byte for byte. The wasm32 half of this
//! gate runs the same vectors inside Chromium via tests/browser/observatory.mjs.
use ed25519_compact::{KeyPair, PublicKey, Seed, Signature};
use odd_identity::{
    entropy_from_flight_key, entropy_from_mnemonic, flight_key, keypair, mnemonic, seed, verify,
    License, LicenseError, ENTROPY_BYTES, WORD_COUNT,
};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

/// RFC 8032 section 7.1: seed, public key, message, signature.
const VECTORS: &[(&str, &str, &str, &str)] = &[
    (
        "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
        "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
        "",
        "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
    ),
    (
        "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb",
        "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
        "72",
        "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00",
    ),
    (
        "c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7",
        "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025",
        "af82",
        "6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac18ff9b538d16f290ae67f760984dc6594a7c15e9716ed28dc027beceea1ec40a",
    ),
];

#[test]
fn oracle_identity_rfc8032_vectors_reproduce_exactly() {
    for (i, (seed_hex, pk_hex, msg_hex, sig_hex)) in VECTORS.iter().enumerate() {
        let raw = unhex(seed_hex);
        let kp = KeyPair::from_seed(Seed::from_slice(&raw).unwrap());
        assert_eq!(hex(kp.pk.as_ref()), *pk_hex, "vector {i} public key");
        let message = unhex(msg_hex);
        let signature = kp.sk.sign(&message, None);
        assert_eq!(hex(signature.as_ref()), *sig_hex, "vector {i} signature");
        let expected = Signature::from_slice(&unhex(sig_hex)).unwrap();
        let pk = PublicKey::from_slice(&unhex(pk_hex)).unwrap();
        assert!(pk.verify(&message, &expected).is_ok(), "vector {i} verify");
    }
}

#[test]
fn derived_seed_signs_identically_to_a_plain_ed25519_key() {
    // The derivation must only choose the seed; signing stays standard.
    let entropy: [u8; ENTROPY_BYTES] = core::array::from_fn(|i| (i as u8) * 7 + 3);
    let license = License::from_entropy(entropy);
    let direct = KeyPair::from_seed(Seed::new(seed(&entropy)));
    let message = b"odderon flight data recorder";
    assert_eq!(license.public_key().to_vec(), direct.pk.to_vec());
    assert_eq!(
        license.sign(message).to_vec(),
        direct.sk.sign(message, None).to_vec()
    );
    assert_eq!(keypair(&entropy).pk.to_vec(), direct.pk.to_vec());
}

#[test]
fn oracle_identity_roundtrip_entropy_words_key() {
    // Exercise the full 132-bit path across many entropies, including the
    // all-zero and all-ones extremes where bit packing is easiest to get wrong.
    let mut cases: Vec<[u8; ENTROPY_BYTES]> = vec![[0u8; ENTROPY_BYTES], [0xff; ENTROPY_BYTES]];
    for k in 0..64u8 {
        cases.push(core::array::from_fn(|i| {
            k.wrapping_mul(31).wrapping_add(i as u8)
        }));
    }
    for entropy in cases {
        let words = mnemonic(&entropy);
        assert_eq!(words.len(), WORD_COUNT);
        let text = words.join(" ");
        assert_eq!(entropy_from_mnemonic(&text).unwrap(), entropy);
        let key = flight_key(&entropy);
        assert_eq!(entropy_from_flight_key(&key).unwrap(), entropy);
        // The two representations must describe the same identity.
        let a = License::from_mnemonic(&text).unwrap();
        let b = License::from_flight_key(&key).unwrap();
        assert_eq!(a.public_key(), b.public_key());
        assert_eq!(a.public_id(), b.public_id());
    }
}

#[test]
fn a_single_bit_flip_in_any_word_is_rejected() {
    let entropy: [u8; ENTROPY_BYTES] = core::array::from_fn(|i| i as u8 * 13);
    let words = mnemonic(&entropy);
    // Swapping any one word changes the payload; the 4-bit checksum catches
    // 15 of 16 such substitutions, so assert the aggregate detection rate is
    // exactly what a 4-bit checksum promises rather than claiming perfection.
    let mut detected = 0;
    let mut trials = 0;
    for position in 0..WORD_COUNT {
        for replacement in 0..2048usize {
            let original = odd_identity::wordlist::index_of(words[position]).unwrap();
            if replacement == original {
                continue;
            }
            let mut altered = words;
            altered[position] = odd_identity::wordlist::WORDS[replacement];
            trials += 1;
            match entropy_from_mnemonic(&altered.join(" ")) {
                Err(LicenseError::Checksum) => detected += 1,
                Err(other) => panic!("unexpected error {other:?}"),
                Ok(_) => {}
            }
        }
    }
    let rate = detected as f64 / trials as f64;
    assert!(
        (rate - 15.0 / 16.0).abs() < 0.01,
        "4-bit checksum should catch ~93.75% of substitutions, caught {rate:.4}"
    );
}

#[test]
fn signature_verification_rejects_a_flipped_bit() {
    let license = License::from_entropy([42u8; ENTROPY_BYTES]);
    let message = b"58-field debrief report";
    let signature = license.sign(message);
    assert!(verify(&license.public_key(), message, &signature).is_ok());
    for byte in [0usize, 31, 63] {
        let mut bad = signature;
        bad[byte] ^= 0x01;
        assert_eq!(
            verify(&license.public_key(), message, &bad),
            Err(LicenseError::Signature)
        );
    }
    let mut bad_key = license.public_key();
    bad_key[0] ^= 0x01;
    assert_eq!(
        verify(&bad_key, message, &signature),
        Err(LicenseError::Signature)
    );
}

#[test]
fn every_word_is_reachable_by_index_and_by_prefix() {
    // Bijectivity is the property the 11-bit codec depends on: if any index
    // were unreachable or any prefix ambiguous, two licenses could collide.
    for (index, word) in odd_identity::wordlist::WORDS.iter().enumerate() {
        assert_eq!(
            odd_identity::wordlist::index_of(word),
            Some(index),
            "{word}"
        );
        assert_eq!(
            odd_identity::wordlist::from_prefix(&word[..4]),
            Some(*word),
            "{word}"
        );
        assert_eq!(
            odd_identity::wordlist::from_prefix(word),
            Some(*word),
            "{word}"
        );
    }
    assert_eq!(
        odd_identity::wordlist::from_prefix("abc"),
        None,
        "too short to be unique"
    );
    assert_eq!(odd_identity::wordlist::index_of("notaword"), None);
}

#[test]
fn mnemonic_input_tolerates_case_and_whitespace_but_not_word_count() {
    let entropy: [u8; ENTROPY_BYTES] = core::array::from_fn(|i| (i as u8).wrapping_mul(17));
    let words = mnemonic(&entropy);
    assert_eq!(
        entropy_from_mnemonic(&words.join(" ").to_uppercase()).unwrap(),
        entropy
    );
    let messy = format!(
        "\t{}\n\r  {}  ",
        words[..6].join("\t"),
        words[6..].join("\n")
    );
    assert_eq!(entropy_from_mnemonic(&messy).unwrap(), entropy);
    assert!(matches!(
        entropy_from_mnemonic(&format!("{} {}", words.join(" "), words[0])),
        Err(LicenseError::WordCount)
    ));
    assert!(matches!(
        entropy_from_mnemonic(&words[..11].join(" ")),
        Err(LicenseError::WordCount)
    ));
    assert!(matches!(
        entropy_from_mnemonic(""),
        Err(LicenseError::WordCount)
    ));
}
