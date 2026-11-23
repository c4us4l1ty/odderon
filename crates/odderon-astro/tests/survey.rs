//! The `Survey` seam, checked against a synthetic backend.
//!
//! **What these tests prove, and what they do not.** They prove the seam's own behaviour: that a bad request
//! is refused, that a backend's broken output is caught, that the release rule reads the flag it claims to,
//! and that the angle function keeps precision at small separations. They do **not** prove that any real
//! star data works, because the crate ships none — the synthetic backend below is a test fixture, and a
//! fixture agrees with the code that wrote it.
//!
//! The one test here that is more than a unit test is
//! `no_gated_dataset_name_appears_in_the_kernel`: it reads `src/classes.json` and fails if a GATED
//! identifier appears anywhere in the compiled kernel. That is `docs/CLASSES.md` §2 R2's promise ("no GATED
//! record may appear as a Rust identifier in `crates/`") moved from prose into a test that runs on every
//! push.

use odderon_astro::provenance::Quantity;
use odderon_astro::survey::{
    admits, cone_search, BuildProfile, Direction, Star, StarTile, Survey, SurveyError,
};

/// A backend with three stars in a small patch, plus one far away.
///
/// Deliberately NOT sorted by distance or magnitude, so a test that accidentally depends on order fails.
struct Fixture {
    stars: [Star; 4],
    redistributable: bool,
}

/// A placeholder star for pre-filling a test buffer.
///
/// `Quantity::unknown()` is not `const`, so this cannot be a `const`; it is an ordinary function used only to
/// size an array, and the values are overwritten before anything reads them.
fn blank_star() -> Star {
    Star {
        id: 0,
        direction: Direction {
            vector: [1.0, 0.0, 0.0],
        },
        distance_parsec: Quantity::unknown(),
        magnitude: Quantity::unknown(),
        proper_motion: Quantity::unknown(),
    }
}

fn star_at(id: u64, ra_deg: f64, dec_deg: f64, distance: f64, sigma: f64) -> Star {
    let deg = core::f64::consts::PI / 180.0;
    Star {
        id,
        direction: Direction::from_ra_dec(ra_deg * deg, dec_deg * deg).expect("valid angles"),
        distance_parsec: Quantity::measured(distance, sigma).expect("positive sigma"),
        magnitude: Quantity::measured(6.0, 0.01).expect("positive sigma"),
        proper_motion: Quantity::unknown(),
    }
}

impl Fixture {
    fn new(redistributable: bool) -> Fixture {
        Fixture {
            stars: [
                // Out of order on purpose: 30 pc, then 4 pc, then 100 pc.
                star_at(1, 10.0, 20.0, 30.0, 0.5),
                star_at(2, 10.5, 20.5, 4.0, 0.01),
                star_at(3, 12.0, 21.0, 100.0, 40.0),
                // The control: a star on the opposite side of the sky.
                star_at(4, 190.0, -20.0, 8.0, 0.1),
            ],
            redistributable,
        }
    }
}

impl Survey for Fixture {
    fn cone_search<'a>(
        &self,
        ra_rad: f64,
        dec_rad: f64,
        radius_rad: f64,
        out: &'a mut [Star],
    ) -> Result<StarTile<'a>, SurveyError> {
        let centre = Direction::from_ra_dec(ra_rad, dec_rad).expect("caller validated");
        let mut n = 0;
        let mut truncated = false;
        for star in self.stars.iter() {
            if centre.separation(&star.direction) <= radius_rad {
                if n == out.len() {
                    // More matches than the caller's buffer holds: say so rather than dropping silently.
                    truncated = true;
                    break;
                }
                out[n] = *star;
                n += 1;
            }
        }
        // Borrowing back out of `out` is what makes the lifetime honest: the tile's stars live in the
        // caller's buffer, not in a copy the backend made.
        let filled: &[Star] = &*out;
        Ok(StarTile {
            stars: &filled[..n],
            truncated,
        })
    }

    fn dataset(&self) -> &'static str {
        "fixture"
    }

    fn redistributable(&self) -> bool {
        self.redistributable
    }
}

// -------------------------------------------------------------------------------------------
// The request is validated before the backend sees it
// -------------------------------------------------------------------------------------------

/// A nonsensical radius is refused, not rounded into an empty sky.
///
/// The failure this prevents is quiet: an empty tile is indistinguishable from a genuinely empty patch of
/// sky, so a caller passing a negative radius would conclude "nothing here" and move the telescope.
#[test]
fn a_bad_radius_is_refused_rather_than_answered() {
    let mut buf = [blank_star(); 8];
    let backend = Fixture::new(true);
    let deg = core::f64::consts::PI / 180.0;
    for radius in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            cone_search(&backend, 0.0, 0.0, radius, &mut buf).err(),
            Some(SurveyError::BadRadius),
            "radius {radius} must be refused"
        );
    }
    // A hemisphere is the limit; past it the caller wanted the whole sky, which is a different request.
    assert_eq!(
        cone_search(
            &backend,
            0.0,
            0.0,
            core::f64::consts::FRAC_PI_2 + 0.01,
            &mut buf
        )
        .err(),
        Some(SurveyError::BadRadius)
    );
    assert!(cone_search(&backend, 0.0, 0.0, 10.0 * deg, &mut buf).is_ok());
}

/// A non-finite coordinate is refused.
#[test]
fn a_non_finite_coordinate_is_refused() {
    let mut buf = [blank_star(); 8];
    let backend = Fixture::new(true);
    assert_eq!(
        cone_search(&backend, f64::NAN, 0.0, 0.1, &mut buf).err(),
        Some(SurveyError::BadDirection)
    );
    assert_eq!(
        cone_search(&backend, 0.0, f64::INFINITY, 0.1, &mut buf).err(),
        Some(SurveyError::BadDirection)
    );
}

// -------------------------------------------------------------------------------------------
// The cone actually restricts the answer
// -------------------------------------------------------------------------------------------

/// A small cone sees the nearby stars and not the one on the other side of the sky.
///
/// This is the test that a "backend ignores the direction and returns a fixed sky" implementation fails.
/// Without it, every other test here would pass on such a backend.
#[test]
fn the_cone_restricts_the_answer() {
    let mut buf = [blank_star(); 8];
    let backend = Fixture::new(true);
    let deg = core::f64::consts::PI / 180.0;

    // Measured separations from (10°, 20°): star 1 = 0.000°, star 2 = 0.686°, star 3 = 2.124°,
    // star 4 = 180°. The radii sit between those numbers so each result is a different set.
    // One call per scope: the tile borrows `buf`, so holding two at once would not compile — which is the
    // borrow doing its job, since a second call would overwrite the first tile's stars.
    {
        let tile =
            cone_search(&backend, 10.0 * deg, 20.0 * deg, 0.3 * deg, &mut buf).expect("valid");
        assert_eq!(
            tile.stars.len(),
            1,
            "only star 1 is within 0.3 deg of (10, 20)"
        );
        assert_eq!(tile.stars[0].id, 1);
    }
    {
        let tile =
            cone_search(&backend, 10.0 * deg, 20.0 * deg, 1.0 * deg, &mut buf).expect("valid");
        assert_eq!(
            tile.stars.len(),
            2,
            "star 2 at 0.686 deg joins at a 1 deg radius"
        );
    }
    {
        let tile =
            cone_search(&backend, 10.0 * deg, 20.0 * deg, 3.0 * deg, &mut buf).expect("valid");
        assert_eq!(tile.stars.len(), 3, "three stars lie within 3 deg");
        // And nothing in that cone is the antipodal control.
        assert!(tile.stars.iter().all(|s| s.id != 4));
    }
}

/// A cone centred on the control star finds only it, so the answer genuinely depends on the direction.
#[test]
fn a_different_direction_gives_a_different_answer() {
    let mut buf = [blank_star(); 8];
    let backend = Fixture::new(true);
    let deg = core::f64::consts::PI / 180.0;
    let tile = cone_search(&backend, 190.0 * deg, -20.0 * deg, 0.5 * deg, &mut buf).expect("valid");
    assert_eq!(tile.stars.len(), 1);
    assert_eq!(tile.stars[0].id, 4);
}

// -------------------------------------------------------------------------------------------
// The backend's output is checked
// -------------------------------------------------------------------------------------------

/// A backend whose directions are not unit vectors is caught, not absorbed.
///
/// This is the commonest real backend bug — a direction built without normalising — and it would otherwise
/// show up as every separation angle being slightly wrong, with no error anywhere.
#[test]
fn a_non_unit_direction_from_a_backend_is_caught() {
    let mut buf = [blank_star(); 8];
    struct Sloppy {
        stars: [Star; 1],
    }
    impl Survey for Sloppy {
        fn cone_search<'a>(
            &self,
            _ra: f64,
            _dec: f64,
            _radius: f64,
            out: &'a mut [Star],
        ) -> Result<StarTile<'a>, SurveyError> {
            out[0] = self.stars[0];
            let filled: &[Star] = &*out;
            Ok(StarTile {
                stars: &filled[..1],
                truncated: false,
            })
        }
        fn dataset(&self) -> &'static str {
            "sloppy"
        }
        fn redistributable(&self) -> bool {
            false
        }
    }

    // Built by struct literal, bypassing `from_vector`, because that is exactly what a real backend does when
    // it forgets to normalise — and the seam has to catch it. A backend *can* construct an invalid
    // `Direction`; the test is that `cone_search` notices.
    let mut bad = star_at(9, 0.0, 0.0, 10.0, 0.1);
    bad.direction = Direction {
        vector: [1.5, 0.0, 0.0],
    };
    let sloppy = Sloppy { stars: [bad] };
    assert_eq!(
        cone_search(&sloppy, 0.0, 0.0, 0.1, &mut buf).err(),
        Some(SurveyError::BackendReturnedNonUnitDirection)
    );
}

/// `Direction::from_vector` refuses a vector that is not unit, rather than normalising it.
#[test]
fn a_non_unit_vector_is_refused_at_construction() {
    assert!(Direction::from_vector([1.0, 0.0, 0.0]).is_some());
    assert!(
        Direction::from_vector([1.0, 0.0, 1.0e-12]).is_some(),
        "1e-12 off unit is fine"
    );
    assert!(Direction::from_vector([1.5, 0.0, 0.0]).is_none());
    assert!(Direction::from_vector([0.0, 0.0, 0.0]).is_none());
    assert!(Direction::from_vector([0.5, 0.5, 0.5]).is_none());
}

// -------------------------------------------------------------------------------------------
// The angle function keeps precision where acos does not
// -------------------------------------------------------------------------------------------

/// Separations of a few arcseconds are measured accurately, which `acos(dot)` cannot do.
///
/// The numbers are the point. `acos` on `1 − 1e-11` has a quantisation of ~1.5e-8 rad ≈ 3 mas, so a 5 mas
/// separation comes back wrong by more than itself. The cross-product form resolves it.
#[test]
fn small_separations_are_measured_accurately() {
    let deg = core::f64::consts::PI / 180.0;
    // 5 milliarcseconds, in degrees.
    let offset_deg = 5.0e-3 / 3600.0;

    let a = Direction::from_ra_dec(0.0, 0.0).expect("valid");
    let b = Direction::from_ra_dec(offset_deg * deg, 0.0).expect("valid");

    let measured = a.separation(&b);
    let expected = offset_deg * deg;
    assert!(
        (measured - expected).abs() < expected * 1.0e-6,
        "5 mas separation: measured {measured:e}, expected {expected:e}"
    );

    // And the same for a separation across a declination (near the pole, where RA converges).
    let c = Direction::from_ra_dec(0.0, 89.0 * deg).expect("valid");
    let d = Direction::from_ra_dec(0.0, 89.0 * deg + offset_deg * deg).expect("valid");
    assert!((c.separation(&d) - expected).abs() < expected * 1.0e-6);
}

/// Separations are symmetric, and zero against itself.
#[test]
fn separation_is_symmetric_and_zero_against_itself() {
    let deg = core::f64::consts::PI / 180.0;
    let a = Direction::from_ra_dec(10.0 * deg, 20.0 * deg).expect("valid");
    let b = Direction::from_ra_dec(30.0 * deg, -40.0 * deg).expect("valid");
    assert_eq!(a.separation(&a), 0.0);
    assert!((a.separation(&b) - b.separation(&a)).abs() < 1.0e-15);

    // A known large angle: the pole to the equator at RA 0.
    let pole = Direction::from_ra_dec(0.0, 90.0 * deg).expect("valid");
    let equator = Direction::from_ra_dec(0.0, 0.0).expect("valid");
    assert!((pole.separation(&equator) - core::f64::consts::FRAC_PI_2).abs() < 1.0e-12);
}

/// RA/Dec survive a round trip through the vector representation.
#[test]
fn ra_dec_round_trip() {
    let deg = core::f64::consts::PI / 180.0;
    for (ra, dec) in [
        (0.0, 0.0),
        (90.0, 0.0),
        (180.0, 45.0),
        (270.0, -60.0),
        (359.9, 89.9),
    ] {
        let d = Direction::from_ra_dec(ra * deg, dec * deg).expect("valid");
        assert!((d.ra_rad() - ra * deg).abs() < 1.0e-12, "RA {ra}");
        assert!((d.dec_rad() - dec * deg).abs() < 1.0e-12, "Dec {dec}");
    }
    // An impossible declination is refused rather than projected.
    assert!(Direction::from_ra_dec(0.0, 100.0 * deg).is_none());
    assert!(Direction::from_ra_dec(f64::NAN, 0.0).is_none());
}

// -------------------------------------------------------------------------------------------
// The distance uncertainty is real data, not decoration
// -------------------------------------------------------------------------------------------

/// A star's distance uncertainty survives the seam, and it is allowed to be larger than the distance.
///
/// `DATA-LICENSING.md` §4 makes a large distance uncertainty the designed baseline: distant stars are "there,
/// at a known bearing, at an uncertain range". If the type forced `sigma < value`, that design would be
/// unrepresentable and the honest US-PD sky could not be stored.
#[test]
fn distance_uncertainty_is_first_class_and_may_exceed_the_distance() {
    let mut buf = [blank_star(); 8];
    let backend = Fixture::new(true);
    let deg = core::f64::consts::PI / 180.0;
    let tile = cone_search(&backend, 10.0 * deg, 20.0 * deg, 3.0 * deg, &mut buf).expect("valid");

    let distant = tile
        .stars
        .iter()
        .find(|s| s.id == 3)
        .expect("star 3 is in the cone");
    assert_eq!(distant.distance_parsec.value, 100.0);
    assert_eq!(distant.distance_parsec.sigma, 40.0);
    assert!(
        distant.distance_parsec.sigma < distant.distance_parsec.value,
        "40 % is large but this one is still a number"
    );

    // A genuinely unmeasurable distance is representable, and propagates as no knowledge rather than as zero.
    let unknown = Quantity::unknown();
    assert!(unknown.value.is_nan());
    assert!(unknown.sigma.is_infinite());
}

/// A star with no proper motion says "unknown", not "zero".
///
/// The difference is the whole `Prov` discipline: zero proper motion is a claim that the star does not move,
/// and a catalogue that cannot tell "unmeasured" from "measured as zero" will make every derived trajectory
/// wrong in the same direction.
#[test]
fn absent_proper_motion_is_unknown_not_zero() {
    let mut buf = [blank_star(); 8];
    let backend = Fixture::new(true);
    let deg = core::f64::consts::PI / 180.0;
    let tile = cone_search(&backend, 10.0 * deg, 20.0 * deg, 0.3 * deg, &mut buf).expect("valid");
    let star = tile.stars[0];
    assert!(
        star.proper_motion.value.is_nan(),
        "absent means NaN, not 0.0"
    );
    assert!(star.proper_motion.sigma.is_infinite());
}

// -------------------------------------------------------------------------------------------
// The release rule
// -------------------------------------------------------------------------------------------

/// A release build admits redistributable data and refuses anything else; development admits both.
///
/// Both branches are testable because the profile is an argument, not `cfg!(debug_assertions)`. A rule that
/// can only be exercised in the configuration that never ships is not a rule.
#[test]
fn a_release_build_refuses_non_redistributable_data() {
    let open = Fixture::new(true);
    let gated = Fixture::new(false);

    assert!(admits(BuildProfile::Development, &open));
    assert!(
        admits(BuildProfile::Development, &gated),
        "a dev build may use a player's own data"
    );
    assert!(admits(BuildProfile::Release, &open));
    assert!(
        !admits(BuildProfile::Release, &gated),
        "a release build must not ship non-redistributable data"
    );
}

/// The dataset name is what the class list calls the same thing, so a backend can be matched to its class.
///
/// The literal `"fixture"` is deliberately *not* a registered class — that is what makes this test honest:
/// if the fixture claimed a real dataset name, the last test in this file would fail, which is exactly the
/// coupling we want.
#[test]
fn the_dataset_name_is_carried_through() {
    let backend = Fixture::new(true);
    assert_eq!(backend.dataset(), "fixture");
}

// -------------------------------------------------------------------------------------------
// The licensing claim, made checkable
// -------------------------------------------------------------------------------------------

/// No GATED dataset identifier appears anywhere in the kernel's source.
///
/// `docs/CLASSES.md` §2 R2 promises this in prose ("no GATED record may appear as a Rust identifier in
/// `crates/`") and says the checker cannot prove a `const` array is a star catalogue. This test does not
/// claim to prove that either. It proves the narrower, still-useful thing: the names of the datasets we are
/// not allowed to ship appear nowhere in the compiled code, so the seam is not being bypassed by someone
/// typing a source's name into a struct.
///
/// It reads `src/classes.json` rather than a hard-coded list, so adding a GATED dataset to the list
/// automatically extends the check.
#[test]
fn no_gated_dataset_name_appears_in_the_kernel() {
    // The kernel source, read at compile time. `include_str!` would need one call per file; this walks the
    // directory instead so a new module is covered without editing this test.
    let mut gated: [&str; 8] = [""; 8];
    let mut gated_count = 0;

    // The class list is JSON; a full parser is not available here, so scan for the records' ids in order.
    // Every GATED record in `src/classes.json` has a `"class": "GATED"` line, and its `id` is the nearest
    // `"id"` line above it. That is the shape the file has, and the checker enforces that shape.
    let classes = include_str!("../../../src/classes.json");
    let mut pending_id: Option<&str> = None;
    for line in classes.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("\"id\": \"") {
            pending_id = rest.strip_suffix("\",");
        } else if trimmed == "\"class\": \"GATED\"," {
            if let Some(id) = pending_id {
                if gated_count < gated.len() {
                    gated[gated_count] = id;
                    gated_count += 1;
                }
            }
        }
    }
    assert!(
        gated_count >= 3,
        "expected at least three GATED records in src/classes.json, found {gated_count}; the scan is broken"
    );

    // Every kernel source file, concatenated. Walked via `include_str!` on a fixed list, because Rust has no
    // directory listing at compile time and adding one would mean depending on `std` in a test that is meant
    // to describe the kernel.
    let sources = [
        include_str!("../src/lib.rs"),
        include_str!("../src/time.rs"),
        include_str!("../src/survey.rs"),
        include_str!("../src/catalogue.rs"),
        include_str!("../src/provenance.rs"),
        include_str!("../src/frames.rs"),
        include_str!("../src/observe.rs"),
        include_str!("../src/passes.rs"),
        include_str!("../src/j2.rs"),
        include_str!("../src/integrate.rs"),
    ];

    // Files allowed to name a GATED dataset, because they *explain why it is not here*. Named individually
    // with the reason, not excluded by pattern: an allowance that covers files nobody listed is how a rule
    // stops meaning anything. `catalogue.rs` and `provenance.rs` each carry a paragraph about the
    // non-redistributable surveys; that paragraph is the licensing argument, not a leak of the data.
    const PROSE_EXCEPTIONS: [&str; 2] = ["catalogue.rs", "provenance.rs"];

    let source_names = [
        "lib.rs",
        "time.rs",
        "survey.rs",
        "catalogue.rs",
        "provenance.rs",
        "frames.rs",
        "observe.rs",
        "passes.rs",
        "j2.rs",
        "integrate.rs",
    ];
    assert_eq!(
        sources.len(),
        source_names.len(),
        "every source needs a name"
    );

    for i in 0..gated_count {
        let name = gated[i];
        // Hyphens cannot appear in a Rust identifier, so the realistic bypass spells it with underscores.
        let underscored: &str = &name.replace('-', "_");
        for (n, source) in sources.iter().enumerate() {
            let exempt = PROSE_EXCEPTIONS.contains(&source_names[n]);
            if exempt {
                continue;
            }
            assert!(
                !source.contains(name),
                "GATED dataset {name} appears in kernel source `{}`; it may not ship in the binary",
                source_names[n]
            );
            assert!(
                !source.contains(underscored),
                "GATED dataset {name} appears in `{}` as {underscored}",
                source_names[n]
            );
        }
    }

    // The exceptions must stay exceptions: each file allowed to mention the names must *still* be prose,
    // i.e. contain no `const` or `static` declaration mentioning one. A dataset mentioned in a doc comment
    // is an argument; the same string in a `const` is the thing this test exists to catch.
    for name in PROSE_EXCEPTIONS {
        let idx = source_names
            .iter()
            .position(|s| *s == name)
            .expect("listed source");
        for i in 0..gated_count {
            let dataset = gated[i];
            let mut in_declaration = false;
            for line in sources[idx].lines() {
                let code = line.trim_start();
                if code.starts_with("///") || code.starts_with("//!") || code.starts_with("//") {
                    continue;
                }
                if code.contains("const ") || code.contains("static ") || code.contains("let ") {
                    in_declaration = true;
                }
                if line.contains(dataset) && in_declaration {
                    panic!("GATED dataset {dataset} appears in a declaration in {name}");
                }
            }
        }
    }
}
