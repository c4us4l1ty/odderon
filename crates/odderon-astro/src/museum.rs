//! The museum gallery layout: where each archive exhibit sits.
//!
//! The museum (`GAME-DESIGN-BIBLE.md` §11.4) is the archive made physical — a gallery that grows as
//! the career grows. Every vehicle, wreck, report, and message the player produced is an exhibit,
//! placed in a deterministic layout that a verifier can re-derive from the same archive.
//!
//! ## What the kernel owns, and what the page owns
//!
//! The kernel owns the **layout**: the position, scale, and rotation of each exhibit. This is a
//! pure function of the archive entries — the same entries produce the same gallery on every
//! platform, every run. That is the P4 "someone else can verify your work" claim, made
//! architectural rather than aspirational: a stranger who loads the same archive and calls
//! [`museum_layout`] walks the same aisles.
//!
//! The page owns the **rendering**: the parametric geometry of each exhibit kind (a rocket stack, a
//! scorched stand, a report on a desk). Geometry is `f(parameters)` per `GRAPHICS.md` R1, so the
//! page draws from the kernel's scale and rotation without inventing a placement rule of its own.
//!
//! ## Why a spiral
//!
//! The layout is a **sunflower spiral** (Vogel's model): exhibit `i` sits at angle
//! `i * golden_angle` and radius `base + i * radial_step`, where the golden angle
//! `2π / φ² ≈ 137.5°` distributes points evenly without clumping. The spiral grows outward as the
//! archive grows, so the gallery physically expands with the career — the building on the edge of
//! town getting bigger one exhibit at a time, which is the visual claim of §11.4.
//!
//! The spiral is deterministic and grows naturally. It is not physics; it is the kind of
//! placement rule that *must* be reproducible to be verifiable, which is why it lives in the
//! kernel rather than in a page-side loop.
//!
//! ## What this module is not
//!
//! It does not render. It does not know about pixels, shaders, or the page. It returns numbers —
//! positions in gallery metres, scales, rotations — and the page turns those numbers into
//! geometry. The kernel stays `forbid(unsafe_code)` and pure, the same as every other module.

use odderon_dmath::TAU;

/// The golden ratio, `(1 + √5) / 2`.
const GOLDEN_RATIO: f64 = 1.618_033_988_749_895;

/// The golden angle in radians: `2π / φ²`. The sunflower spiral's step — two exhibits never
/// clump, and the spiral grows evenly in every direction.
const GOLDEN_ANGLE: f64 = TAU / (GOLDEN_RATIO * GOLDEN_RATIO);

/// The base radius of the gallery, metres. The first exhibit sits at this radius from the centre.
const BASE_RADIUS_M: f64 = 4.0;

/// How much further each successive exhibit sits from the centre, metres. The gallery expands
/// outward as the archive grows.
const RADIAL_STEP_M: f64 = 0.8;

/// The minimum display scale for any exhibit. A reading that collapses to zero still occupies a
/// visible pedestal, not nothing.
const SCALE_MIN: f64 = 0.3;

/// The maximum display scale for any exhibit. A very large reading is clamped, so one exhibit
/// cannot dominate the gallery and hide its neighbours.
const SCALE_MAX: f64 = 3.0;

/// The display scale an exhibit gets when its size reading is absent or non-finite.
const SCALE_DEFAULT: f64 = 1.0;

/// The display scale applied to a reading that is present but non-positive, clamped to
/// [`SCALE_MIN`]. A size of `0` means "unknown", not "invisible".
const SCALE_FLOOR: f64 = SCALE_MIN;

/// The kind of exhibit an archive entry is. The code is the stable identifier the ABI carries;
/// the names are what the page reads to choose its geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExhibitKind {
    /// A vehicle the player authored and flew.
    Vehicle,
    /// A wreck — a failed cryo load, kept on the stand.
    Wreck,
    /// A report the player exported (Akashi).
    Report,
    /// A comms message the player sent.
    Comms,
    /// A message from the causal mail.
    Mail,
    /// A scored mission.
    Mission,
    /// A self-explanation the protégé briefing produced.
    Briefing,
}

impl ExhibitKind {
    /// The stable code carried across the ABI. Stable because a verifier keys on it and a
    /// renamed code is a different gallery.
    pub fn code(self) -> u8 {
        match self {
            ExhibitKind::Vehicle => 0,
            ExhibitKind::Wreck => 1,
            ExhibitKind::Report => 2,
            ExhibitKind::Comms => 3,
            ExhibitKind::Mail => 4,
            ExhibitKind::Mission => 5,
            ExhibitKind::Briefing => 6,
        }
    }

    /// Reconstruct from a code. `None` for a code this kernel does not know — a future exhibit
    /// kind added by an older page must not silently become a `Vehicle`.
    pub fn from_code(code: u8) -> Option<ExhibitKind> {
        match code {
            0 => Some(ExhibitKind::Vehicle),
            1 => Some(ExhibitKind::Wreck),
            2 => Some(ExhibitKind::Report),
            3 => Some(ExhibitKind::Comms),
            4 => Some(ExhibitKind::Mail),
            5 => Some(ExhibitKind::Mission),
            6 => Some(ExhibitKind::Briefing),
            _ => None,
        }
    }
}

/// A compact description of an archive entry, for the museum layout.
///
/// The kernel needs three things per entry to place it: what kind it is, a reading that
/// determines how large it displays, and a reading that determines how it is rotated. The page
/// maps each archive entry to this descriptor; the kernel never sees the archive's full structure.
#[derive(Clone, Copy, Debug)]
pub struct EntryDesc {
    /// The exhibit kind, as [`ExhibitKind::code`].
    pub kind: u8,
    /// A reading that determines the display scale. For a vehicle this is its apogee in km; for
    /// a wreck, the failure time in seconds; for a report or message, the entry id. Non-finite
    /// or non-positive means "unknown", and the exhibit gets [`SCALE_DEFAULT`] (clamped to
    /// [`SCALE_MIN`]).
    pub size: f64,
    /// A reading that determines the display rotation in radians. For a vehicle this is its
    /// launch azimuth; for other kinds, `0.0`. Non-finite means `0.0`.
    pub orient: f64,
    /// The archive sequence number. Used as the spiral index, so the layout is a function of the
    /// entry's position in the archive, not of the order the page happens to call with.
    pub id: u32,
}

/// A placed exhibit in the gallery.
#[derive(Clone, Copy, Debug)]
pub struct Exhibit {
    /// Gallery position, metres from the centre. `x` is east, `z` is south.
    pub x: f64,
    /// Gallery position, metres from the centre.
    pub z: f64,
    /// Display scale, dimensionless. Multiplied by the page's base geometry size.
    pub scale: f64,
    /// Display rotation, radians. The page applies this to the exhibit's geometry.
    pub rotation: f64,
    /// The exhibit kind, as [`ExhibitKind::code`].
    pub kind: u8,
    /// The archive sequence number this exhibit was placed for.
    pub id: u32,
}

/// Compute the display scale from a size reading.
///
/// A non-finite reading means "unknown", and the exhibit gets [`SCALE_DEFAULT`]. A finite reading
/// is mapped through a square-root curve — a 400 km apogee and a 4000 km apogee differ by a factor
/// of ten in the reading but only by √10 ≈ 3.16 in scale, which keeps large and small exhibits
/// visible in the same gallery without one dominating. The result is clamped to
/// [`SCALE_MIN`]..[`SCALE_MAX`].
fn scale_from_size(size: f64) -> f64 {
    if !size.is_finite() {
        return SCALE_DEFAULT;
    }
    if size <= 0.0 {
        return SCALE_FLOOR;
    }
    // sqrt(size) / sqrt(reference) maps a reading of `reference` to a scale of 1.0.
    // The reference is 100.0: a 100 km apogee or a 100 s failure time displays at scale 1.
    let ref_size = 100.0;
    let raw = odderon_dmath::sqrt(size / ref_size);
    if raw < SCALE_MIN {
        SCALE_MIN
    } else if raw > SCALE_MAX {
        SCALE_MAX
    } else {
        raw
    }
}

/// Compute the gallery layout for `entries`, writing exhibits into `out`.
///
/// Returns the number of exhibits written, which is `min(entries.len(), out.len())`. If `out` is
/// shorter than `entries`, the first `out.len()` exhibits are written and the rest are dropped —
/// the gallery is truncated, not shifted, so a verifier who reads the first N exhibits gets the
/// same N exhibits a full-size call would produce.
///
/// The layout is a pure function of `entries`: the same entries, in the same order, produce the
/// same gallery on every platform, every run.
pub fn museum_layout(entries: &[EntryDesc], out: &mut [Exhibit]) -> usize {
    let n = if entries.len() < out.len() { entries.len() } else { out.len() };
    let mut i = 0;
    while i < n {
        let e = entries[i];
        // Spiral position: angle = id * golden_angle, radius = base + id * radial_step.
        // The id is the archive sequence number, so the layout is stable across reordering of
        // the call: the same entry always lands at the same angle and radius.
        let id_f = e.id as f64;
        let angle = id_f * GOLDEN_ANGLE;
        let radius = BASE_RADIUS_M + id_f * RADIAL_STEP_M;
        let (sin_a, cos_a) = odderon_dmath::sin_cos(angle);
        out[i] = Exhibit {
            x: radius * cos_a,
            z: radius * sin_a,
            scale: scale_from_size(e.size),
            rotation: if e.orient.is_finite() { e.orient } else { 0.0 },
            kind: e.kind,
            id: e.id,
        };
        i += 1;
    }
    n
}

/// The deterministic design hash of the museum layout's constants.
///
/// Folds in the golden angle, the base radius, the radial step, and the scale bounds, so a
/// replay bundle or a verifier can detect a stale gallery layout. A pure function: the same
/// kernel produces the same hash on every platform, every run.
pub fn museum_design_hash() -> u64 {
    // FNV-1a, the same hash the page's archive uses, so a verifier who recomputes the hash
    // from the same constants gets the same number.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let constants: [f64; 6] = [
        GOLDEN_ANGLE,
        BASE_RADIUS_M,
        RADIAL_STEP_M,
        SCALE_MIN,
        SCALE_MAX,
        SCALE_DEFAULT,
    ];
    for c in constants {
        // Fold the bits of each constant. `(c as f64).to_bits()` is the IEEE-754 bit pattern,
        // which is deterministic on every platform — the same value has the same bits natively
        // and in WASM.
        let bits = (c as f64).to_bits();
        let mut b: u64 = bits;
        let mut shift = 0;
        while shift < 64 {
            h ^= b & 0xff;
            h = h.wrapping_mul(0x0100_0000_01b3);
            b >>= 8;
            shift += 8;
        }
    }
    // Fold the kind count so a new exhibit kind changes the hash.
    h ^= 7u64;
    h = h.wrapping_mul(0x0100_0000_01b3);
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build an `EntryDesc` for a vehicle at a given id and apogee.
    fn vehicle(id: u32, apogee_km: f64) -> EntryDesc {
        EntryDesc {
            kind: ExhibitKind::Vehicle.code(),
            size: apogee_km,
            orient: 0.0,
            id,
        }
    }

    #[test]
    fn layout_is_deterministic() {
        let entries = [vehicle(0, 100.0), vehicle(1, 200.0), vehicle(2, 300.0)];
        let mut a = [Exhibit { x: 0.0, z: 0.0, scale: 0.0, rotation: 0.0, kind: 0, id: 0 }; 3];
        let mut b = [Exhibit { x: 0.0, z: 0.0, scale: 0.0, rotation: 0.0, kind: 0, id: 0 }; 3];
        let n_a = museum_layout(&entries, &mut a);
        let n_b = museum_layout(&entries, &mut b);
        assert_eq!(n_a, 3);
        assert_eq!(n_b, 3);
        for i in 0..3 {
            assert_eq!(a[i].x, b[i].x, "x mismatch at {i}");
            assert_eq!(a[i].z, b[i].z, "z mismatch at {i}");
            assert_eq!(a[i].scale, b[i].scale, "scale mismatch at {i}");
            assert_eq!(a[i].rotation, b[i].rotation, "rotation mismatch at {i}");
            assert_eq!(a[i].kind, b[i].kind);
            assert_eq!(a[i].id, b[i].id);
        }
    }

    #[test]
    fn layout_uses_id_as_spiral_index() {
        // The same entry at a different id lands at a different position.
        let mut a = [Exhibit { x: 0.0, z: 0.0, scale: 0.0, rotation: 0.0, kind: 0, id: 0 }; 1];
        let mut b = [Exhibit { x: 0.0, z: 0.0, scale: 0.0, rotation: 0.0, kind: 0, id: 0 }; 1];
        museum_layout(&[vehicle(0, 100.0)], &mut a);
        museum_layout(&[vehicle(7, 100.0)], &mut b);
        assert_ne!(a[0].x, b[0].x, "different id must land at different x");
        assert_ne!(a[0].z, b[0].z, "different id must land at different z");
        // Same id, same position.
        let mut c = [Exhibit { x: 0.0, z: 0.0, scale: 0.0, rotation: 0.0, kind: 0, id: 0 }; 1];
        museum_layout(&[vehicle(7, 100.0)], &mut c);
        assert_eq!(b[0].x, c[0].x);
        assert_eq!(b[0].z, c[0].z);
    }

    #[test]
    fn spiral_grows_outward() {
        // Later exhibits sit further from the centre.
        let entries = [vehicle(0, 100.0), vehicle(1, 100.0), vehicle(2, 100.0), vehicle(3, 100.0)];
        let mut out = [Exhibit { x: 0.0, z: 0.0, scale: 0.0, rotation: 0.0, kind: 0, id: 0 }; 4];
        museum_layout(&entries, &mut out);
        let r0 = odderon_dmath::sqrt(out[0].x * out[0].x + out[0].z * out[0].z);
        let r1 = odderon_dmath::sqrt(out[1].x * out[1].x + out[1].z * out[1].z);
        let r2 = odderon_dmath::sqrt(out[2].x * out[2].x + out[2].z * out[2].z);
        let r3 = odderon_dmath::sqrt(out[3].x * out[3].x + out[3].z * out[3].z);
        assert!(r1 > r0, "radius must grow: {r0} -> {r1}");
        assert!(r2 > r1, "radius must grow: {r1} -> {r2}");
        assert!(r3 > r2, "radius must grow: {r2} -> {r3}");
    }

    #[test]
    fn scale_maps_size_through_sqrt_curve() {
        // A reading of 100 maps to scale 1.0 (the reference).
        assert!(odderon_dmath::fabs(scale_from_size(100.0) - 1.0) < 1e-12);
        // A reading of 400 maps to sqrt(4) = 2.0.
        assert!(odderon_dmath::fabs(scale_from_size(400.0) - 2.0) < 1e-12);
        // A reading of 0 maps to the floor.
        assert_eq!(scale_from_size(0.0), SCALE_FLOOR);
        // A negative reading maps to the floor.
        assert_eq!(scale_from_size(-5.0), SCALE_FLOOR);
        // A non-finite reading maps to the default.
        assert_eq!(scale_from_size(f64::NAN), SCALE_DEFAULT);
        assert_eq!(scale_from_size(f64::INFINITY), SCALE_DEFAULT);
    }

    #[test]
    fn scale_is_clamped() {
        // A very large reading clamps to SCALE_MAX.
        assert_eq!(scale_from_size(1.0e9), SCALE_MAX);
        // A very small positive reading clamps to SCALE_MIN.
        assert_eq!(scale_from_size(1.0e-6), SCALE_MIN);
    }

    #[test]
    fn rotation_is_orientation_or_zero() {
        let mut out = [Exhibit { x: 0.0, z: 0.0, scale: 0.0, rotation: 0.0, kind: 0, id: 0 }; 2];
        let entries = [
            EntryDesc { kind: 0, size: 100.0, orient: 1.5, id: 0 },
            EntryDesc { kind: 0, size: 100.0, orient: f64::NAN, id: 1 },
        ];
        museum_layout(&entries, &mut out);
        assert!(odderon_dmath::fabs(out[0].rotation - 1.5) < 1e-12, "finite orient must be used");
        assert_eq!(out[1].rotation, 0.0, "non-finite orient must be zero");
    }

    #[test]
    fn truncation_keeps_leading_exhibits() {
        // A short output buffer writes the first N exhibits, not a shifted subset.
        let entries = [vehicle(0, 100.0), vehicle(1, 200.0), vehicle(2, 300.0)];
        let mut out = [Exhibit { x: 0.0, z: 0.0, scale: 0.0, rotation: 0.0, kind: 0, id: 0 }; 2];
        let n = museum_layout(&entries, &mut out);
        assert_eq!(n, 2);
        assert_eq!(out[0].id, 0);
        assert_eq!(out[1].id, 1);
    }

    #[test]
    fn empty_archive_gives_empty_gallery() {
        let mut out = [Exhibit { x: 0.0, z: 0.0, scale: 0.0, rotation: 0.0, kind: 0, id: 0 }; 4];
        let n = museum_layout(&[], &mut out);
        assert_eq!(n, 0);
    }

    #[test]
    fn exhibit_kind_codes_are_stable() {
        // The codes are the ABI contract. A renamed code is a different gallery.
        assert_eq!(ExhibitKind::Vehicle.code(), 0);
        assert_eq!(ExhibitKind::Wreck.code(), 1);
        assert_eq!(ExhibitKind::Report.code(), 2);
        assert_eq!(ExhibitKind::Comms.code(), 3);
        assert_eq!(ExhibitKind::Mail.code(), 4);
        assert_eq!(ExhibitKind::Mission.code(), 5);
        assert_eq!(ExhibitKind::Briefing.code(), 6);
    }

    #[test]
    fn exhibit_kind_round_trips() {
        for &code in &[0u8, 1, 2, 3, 4, 5, 6] {
            let kind = ExhibitKind::from_code(code).unwrap();
            assert_eq!(kind.code(), code);
        }
        assert!(ExhibitKind::from_code(7).is_none());
        assert!(ExhibitKind::from_code(200).is_none());
    }

    #[test]
    fn design_hash_is_deterministic_and_nonzero() {
        let h1 = museum_design_hash();
        let h2 = museum_design_hash();
        assert_eq!(h1, h2, "design hash must be deterministic");
        assert_ne!(h1, 0, "design hash must be nonzero");
    }

    #[test]
    fn layout_exhibits_carry_their_kind_and_id() {
        let entries = [
            EntryDesc { kind: ExhibitKind::Vehicle.code(), size: 100.0, orient: 0.0, id: 3 },
            EntryDesc { kind: ExhibitKind::Wreck.code(), size: 50.0, orient: 0.0, id: 4 },
        ];
        let mut out = [Exhibit { x: 0.0, z: 0.0, scale: 0.0, rotation: 0.0, kind: 0, id: 0 }; 2];
        museum_layout(&entries, &mut out);
        assert_eq!(out[0].kind, ExhibitKind::Vehicle.code());
        assert_eq!(out[0].id, 3);
        assert_eq!(out[1].kind, ExhibitKind::Wreck.code());
        assert_eq!(out[1].id, 4);
    }

    #[test]
    fn spiral_points_are_not_collinear() {
        // The golden angle ensures no two exhibits sit on the same ray from the centre.
        let entries = [vehicle(0, 100.0), vehicle(1, 100.0), vehicle(2, 100.0)];
        let mut out = [Exhibit { x: 0.0, z: 0.0, scale: 0.0, rotation: 0.0, kind: 0, id: 0 }; 3];
        museum_layout(&entries, &mut out);
        // Cross products of consecutive position vectors must not all be zero.
        let cross01 = out[0].x * out[1].z - out[0].z * out[1].x;
        let cross12 = out[1].x * out[2].z - out[1].z * out[2].x;
        assert!(odderon_dmath::fabs(cross01) > 1e-9, "exhibits 0 and 1 are collinear");
        assert!(odderon_dmath::fabs(cross12) > 1e-9, "exhibits 1 and 2 are collinear");
    }
}
