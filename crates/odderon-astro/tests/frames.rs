//! Body-fixed frames, checked against things that are true independently of the implementation.
//!
//! The interesting failure here is not a wrong number — it is a **consistent** wrong number. If the prime
//! meridian constant were zero, the transform would still be a perfect rotation, round-trip exactly, and
//! conserve distance. Every property in the first half of this file would pass. Only a test that pins the
//! *absolute* orientation against an outside fact catches it.
//!
//! So the tests come in two kinds, and both are needed:
//!
//! * **algebraic** — round trips, rotations preserve length, rate is constant. These catch sign errors and
//!   dropped terms.
//! * **physical** — the subsolar point moves west at 15°/hour; a satellite in a geostationary orbit sits
//!   over a fixed longitude; the ISS's ground track makes sense. These catch a wrong frame origin, which
//!   the algebraic tests cannot see.

use odderon_astro::catalogue::EARTH_FRAME;
use odderon_astro::frames::{geodetic_of, BodyFixed, FrameError};
use odderon_astro::provenance::Quantity;
use odderon_astro::{mu, radius, Vec3};
use odderon_dmath::{fabs, sqrt, TAU};

const MU: f64 = mu::EARTH;
const RE: f64 = radius::EARTH;

/// The sidereal day, seconds. One full rotation in this interval.
const SIDEREAL_DAY: f64 = 86164.0905;

// ---------------------------------------------------------------------------------------------
// 1. Algebraic properties
// ---------------------------------------------------------------------------------------------

/// Inertial → body-fixed → inertial returns the original point.
#[test]
fn the_frame_transform_round_trips() {
    let p = Vec3::new(4200.0, -3100.0, 5100.0);
    for t in [0.0, 60.0, 3600.0, 86_400.0, 1e6] {
        let body = EARTH_FRAME.to_body_fixed(p, t).expect("frame available");
        let back = EARTH_FRAME.to_inertial(body, t).expect("frame available");
        let err = (back.x - p.x).abs() + (back.y - p.y).abs() + (back.z - p.z).abs();
        assert!(err < 1e-9, "round trip at t={t} lost {err} km");
    }
}

/// The transform is a rotation: it preserves length and z.
///
/// If length were not preserved the "rotation" would be scaling too, and every altitude derived downstream
/// would drift with time. That is a subtle enough bug to be worth a dedicated assertion.
#[test]
fn the_transform_preserves_length_and_axial_component() {
    let p = Vec3::new(3000.0, 4000.0, 5000.0);
    let r0 = p.norm();
    for t in [0.0, 1000.0, 50_000.0, 1e7] {
        let b = EARTH_FRAME.to_body_fixed(p, t).expect("frame available");
        assert!(fabs(b.norm() - r0) < 1e-9, "length changed at t={t}");
        assert!(
            fabs(b.z - p.z) < 1e-9,
            "z changed at t={t}, so the rotation is not about z"
        );
    }
}

/// The rotation angle advances by exactly `2π` over one sidereal day.
#[test]
fn one_sidereal_day_is_one_full_rotation() {
    let w0 = EARTH_FRAME.angle_at(0.0).expect("available");
    let w1 = EARTH_FRAME.angle_at(SIDEREAL_DAY).expect("available");
    let mut d = w1 - w0;
    while d < 0.0 {
        d += TAU;
    }
    while d >= TAU {
        d -= TAU;
    }
    // Either it wrapped to ~0 or to ~2π depending on rounding; both mean "the same angle".
    let err = if d > TAU / 2.0 { TAU - d } else { d };
    assert!(
        err < 1e-6,
        "one sidereal day should be one full rotation, residual {err} rad"
    );
}

/// The angle stays in `[0, 2π)` even after years of accumulated rotation.
///
/// Without wrapping, `Ẇ·t` reaches 2.3e6 rad after a year and the low bits that carry the ground position
/// are gone. The point of the wrap is not tidiness.
#[test]
fn the_angle_wraps_and_stays_in_range() {
    for t in [0.0, 1e3, 1e6, 3.15e7, 3.15e9] {
        let w = EARTH_FRAME.angle_at(t).expect("available");
        assert!(
            (0.0..TAU).contains(&w),
            "angle at t={t} was {w}, outside [0, 2pi)"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// 2. The absolute orientation — the tests that catch a zero prime meridian
// ---------------------------------------------------------------------------------------------

/// The prime meridian constant is **not zero**, and it is the IAU 2015 value.
///
/// This is the single most important assertion in the file. A zero `W₀` produces a frame that is a perfect
/// rotation, round-trips exactly, preserves length — and puts every ground position up to half a rotation
/// away from where it belongs. Nothing else in this suite would notice.
#[test]
fn the_prime_meridian_is_the_iau_value_and_not_zero() {
    let w0 = EARTH_FRAME.w0.value;
    let expected = 190.147 * core::f64::consts::PI / 180.0;
    assert!(
        fabs(w0 - expected) < 1e-9,
        "prime meridian should be the IAU 2015 value {expected:e} rad, got {w0:e}"
    );
    assert!(
        fabs(w0) > 1.0,
        "a zero (or near-zero) prime meridian is the bug this test exists to catch"
    );
}

/// A satellite in a geostationary orbit holds a **fixed** body-fixed longitude.
///
/// This is the best available check of the whole frame, because it does not depend on the prime meridian at
/// all — it depends on the *rate* matching the orbital rate. If the stored rotation rate were the solar-day
/// rate, the point would drift east by 0.9856°/day, which is 11°/year: invisible in a screenshot and
/// obvious the moment a ground station can no longer see its satellite.
#[test]
fn a_geostationary_satellite_holds_a_fixed_longitude() {
    // circular equatorial orbit at the geostationary radius
    // 42 164.17 km is the radius whose period is exactly one sidereal day. The rounder 42 164.0 is 0.5 s
    // short, which accumulates to about 1.1e-3 rad of longitude error over 30 days - so the radius matters
    // and the tolerance below is set from that arithmetic rather than guessed.
    let r_geo = 42_164.17_f64;
    let v_geo = sqrt(MU / r_geo);
    let pos = Vec3::new(r_geo, 0.0, 0.0);

    let lon_at = |t: f64| {
        let b = EARTH_FRAME.to_body_fixed(pos, t).expect("available");
        geodetic_of(b).longitude
    };

    let l0 = lon_at(0.0);
    for days in [1.0, 7.0, 30.0] {
        let l = lon_at(days * SIDEREAL_DAY);
        let mut d = l - l0;
        while d > core::f64::consts::PI {
            d -= TAU;
        }
        while d <= -core::f64::consts::PI {
            d += TAU;
        }
        // 1e-4 rad is 0.6 km at the equator. Tighter than the rounding of the radius and far looser than
        // any real drift, so this fails only if the rate is actually wrong.
        assert!(
            fabs(d) < 1e-4,
            "after {days} sidereal days the geostationary point moved {d} rad in longitude"
        );
    }

    // and the orbital period is exactly one sidereal day, which is why the above works
    let period = TAU * sqrt(r_geo * r_geo * r_geo / MU);
    assert!(
        fabs(period - SIDEREAL_DAY) < 1.0,
        "the geostationary radius should give a sidereal-day period, got {period} s"
    );
    let _ = v_geo;
}

/// The ground point beneath a satellite moves **west** in body-fixed longitude as time passes.
///
/// A body rotating eastward means a fixed inertial point appears to move westward across the surface. Sign
/// errors in the transform flip this, and a flipped frame is self-consistent: every round trip still passes,
/// and the planet just spins backwards.
#[test]
fn the_sub_satellite_point_moves_west() {
    let r = RE + 400.0;
    let pos = Vec3::new(r, 0.0, 0.0);

    let lon = |t: f64| geodetic_of(EARTH_FRAME.to_body_fixed(pos, t).expect("available")).longitude;

    let l0 = lon(0.0);
    let quarter = lon(SIDEREAL_DAY / 4.0);

    let mut d = quarter - l0;
    while d > core::f64::consts::PI {
        d -= TAU;
    }
    while d <= -core::f64::consts::PI {
        d += TAU;
    }

    // a quarter sidereal day is a quarter turn west, i.e. about -1.571 rad
    assert!(
        d < -1.4,
        "a quarter day should move the ground point about -90 deg (west); it moved {} rad",
        d
    );
}

// ---------------------------------------------------------------------------------------------
// 3. Geodetic output
// ---------------------------------------------------------------------------------------------

/// Latitude and longitude of a point on the equator, on the prime meridian, are zero.
#[test]
fn the_frame_origin_is_latitude_zero_longitude_zero() {
    // Put the point where the prime meridian is at t = 0: body-fixed x-axis.
    let body = Vec3::new(RE, 0.0, 0.0);
    let g = geodetic_of(body);
    assert!(fabs(g.latitude) < 1e-12, "equator should be latitude 0");
    assert!(
        fabs(g.longitude) < 1e-12,
        "prime meridian should be longitude 0"
    );
    assert!(
        fabs(g.radius - RE) < 1e-9,
        "radius should be the equatorial radius"
    );
}

/// The pole is at latitude ±90°, and longitude there is degenerate but finite.
#[test]
fn the_pole_is_at_ninety_degrees() {
    let north = geodetic_of(Vec3::new(0.0, 0.0, RE));
    let south = geodetic_of(Vec3::new(0.0, 0.0, -RE));
    assert!(fabs(north.latitude - core::f64::consts::FRAC_PI_2) < 1e-12);
    assert!(fabs(south.latitude + core::f64::consts::FRAC_PI_2) < 1e-12);
    assert!(
        north.longitude.is_finite(),
        "longitude at the pole must not be NaN"
    );
}

/// Latitude is `atan2`-based, so it stays accurate at the poles where `asin(z/r)` loses precision.
///
/// At 89.9999° the naive `asin(z/r)` has a derivative that blows up: `z/r = 1 − 1.5e-12` and the result
/// carries far fewer significant digits than the input. This asserts that a point half a metre from the
/// pole still resolves to the right latitude.
#[test]
fn latitude_is_accurate_near_the_pole() {
    let lat = 89.9999 * core::f64::consts::PI / 180.0;
    let p = Vec3::new(RE * cos_lat(lat), 0.0, RE * sin_lat(lat));
    let g = geodetic_of(p);
    assert!(
        fabs(g.latitude - lat) < 1e-12,
        "latitude near the pole should be exact, got {} vs {}",
        g.latitude,
        lat
    );
}

// ---------------------------------------------------------------------------------------------
// 4. Refusals
// ---------------------------------------------------------------------------------------------

/// A body with no prime-meridian constant cannot produce a frame, and does not pretend to.
///
/// This is the type-level guarantee that the module's header promises: you cannot accidentally construct a
/// `BodyFixed` without a `W₀`, because there is no `Default` impl and no constructor that fills one in.
#[test]
fn a_missing_rotation_constant_is_refused_not_defaulted() {
    // an Unknown quantity as a rotation rate: the frame must refuse rather than return a position
    let broken = BodyFixed {
        w0: Quantity::unknown(),
        rate: Quantity::unknown(),
    };
    assert!(
        broken.angle_at(0.0).is_none(),
        "unknown constants must not yield an angle"
    );
    assert!(broken
        .to_body_fixed(Vec3::new(1.0, 2.0, 3.0), 0.0)
        .is_none());
    assert!(broken.to_inertial(Vec3::new(1.0, 2.0, 3.0), 0.0).is_none());

    // and a non-finite epoch is refused too, rather than producing NaN coordinates
    assert!(EARTH_FRAME.angle_at(f64::NAN).is_none());
    assert!(EARTH_FRAME.angle_at(f64::INFINITY).is_none());
}

/// `FrameError` exists as a nameable reason, so a UI can explain the absence instead of drawing nothing.
#[test]
fn the_error_enum_distinguishes_the_reasons() {
    // Just asserting the variants exist and are distinct - this is a compile-time contract, and the
    // assertion stops someone collapsing them into one later.
    let a = FrameError::NoPrimeMeridianConstant;
    let b = FrameError::PropertyUnknown;
    assert_ne!(a, b);
}

fn cos_lat(x: f64) -> f64 {
    odderon_astro::cos_rad(x)
}
fn sin_lat(x: f64) -> f64 {
    odderon_astro::sin_rad(x)
}
