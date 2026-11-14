use odd_core::canon::{canonical_bytes, canonical_hash};
use odd_core::units::{Kilograms, Meters, Seconds};
use serde::Serialize;

#[derive(Serialize)]
struct Fixture {
    length: Meters,
    elapsed: Seconds,
    dry_mass: Kilograms,
}

#[test]
fn typed_units_and_scalar_operations() {
    let mut distance = Meters::from(42.0);
    distance += Meters(3.0);
    assert_eq!(f64::from(distance / 3.0), 15.0);
    assert_eq!(Seconds(2.0) * 3.0 - Seconds(1.0), Seconds(5.0));
}

#[test]
fn encoding_stable_bit_sensitive_and_versioned() {
    let first = Fixture {
        length: Meters(1.0),
        elapsed: Seconds(2.0),
        dry_mass: Kilograms(3.0),
    };
    let altered = Fixture {
        length: Meters(f64::from_bits(1.0f64.to_bits() + 1)),
        ..first
    };
    assert_eq!(canonical_bytes(&first).unwrap().len(), 24);
    assert_eq!(
        canonical_hash(1, &first).unwrap(),
        canonical_hash(1, &first).unwrap()
    );
    assert_ne!(
        canonical_hash(1, &first).unwrap(),
        canonical_hash(1, &altered).unwrap()
    );
    assert_ne!(
        canonical_hash(1, &first).unwrap(),
        canonical_hash(2, &first).unwrap()
    );
    // Independent SHA-256 test vector: version bytes 0,0 plus postcard empty tuple.
    assert_eq!(
        canonical_hash(0, &()).unwrap(),
        [
            0x96, 0xa2, 0x96, 0xd2, 0x24, 0xf2, 0x85, 0xc6, 0x7b, 0xee, 0x93, 0xc3, 0x0f, 0x8a,
            0x30, 0x91, 0x57, 0xf0, 0xda, 0xa3, 0x5d, 0xc5, 0xb8, 0x7e, 0x41, 0x0b, 0x78, 0x63,
            0x0a, 0x09, 0xcf, 0xc7
        ]
    );
}
