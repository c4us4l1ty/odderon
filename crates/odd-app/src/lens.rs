//! Visual Physics Lens domain model: acronym glossary and hover projections.
//! Rendering lives in the browser; this module defines the deterministic
//! physical quantities a projection must display so the UI cannot silently
//! drop an explanation.

use odd_astro::{elements::Elements, maneuver::TnwBasis};
use odd_core::frames::State;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Projection {
    Tangential,
    Normal,
    Radial,
    ThrustToWeight,
    SpecificImpulse,
    AscendingNode,
    ArgumentOfPeriapsis,
    SphereOfInfluence,
    Eccentricity,
    GravityLoss,
    ManeuverFrame,
    SolarPressure,
    LowEarthOrbit,
    CausalGraph,
    DesignReview,
}

impl Projection {
    /// Stable browser art key for this projection. The match is exhaustive so
    /// a new variant must declare its demonstration; the browser reads this
    /// through the bridge instead of duplicating the declaration order in a
    /// JavaScript array, where an added variant would silently fall back to
    /// the tangential demo.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Tangential => "t",
            Self::Normal => "w",
            // The in-plane handle and the eccentricity vector deliberately
            // share one demonstration: a sideways push rotates that vector.
            Self::Radial | Self::Eccentricity => "n",
            Self::ThrustToWeight => "twr",
            Self::SpecificImpulse => "isp",
            Self::AscendingNode => "raan",
            Self::ArgumentOfPeriapsis => "aop",
            Self::SphereOfInfluence => "soi",
            Self::GravityLoss => "gravity-loss",
            Self::ManeuverFrame => "tnw",
            Self::SolarPressure => "srp",
            Self::LowEarthOrbit => "leo",
            Self::CausalGraph => "dag",
            Self::DesignReview => "pdr",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Acronym {
    pub term: &'static str,
    pub expansion: &'static str,
    pub plain: &'static str,
    pub projection: Projection,
}

/// Every acronym that may appear in the mission UI must have a dynamic
/// visual hover projection. The browser snapshot test audits rendered text
/// against this registry.
pub const ACRONYMS: &[Acronym] = &[
    Acronym {
        term: "TNW",
        expansion: "Tangential, in-plane normal, orbit normal",
        plain: "Three perpendicular push directions: along motion, sideways in the orbital plane, and out of the plane.",
        projection: Projection::ManeuverFrame,
    },
    Acronym {
        term: "TWR",
        expansion: "Thrust-to-weight ratio",
        plain: "Engine thrust divided by vehicle weight. Below one, the engine cannot lift the vehicle against gravity.",
        projection: Projection::ThrustToWeight,
    },
    Acronym {
        term: "ISP",
        expansion: "Specific impulse",
        plain: "How many seconds one kilogram of propellant can produce one kilogram-force of thrust; higher means less fuel for the same push.",
        projection: Projection::SpecificImpulse,
    },
    Acronym {
        term: "Isp",
        expansion: "Specific impulse",
        plain: "How many seconds one kilogram of propellant can produce one kilogram-force of thrust; higher means less fuel for the same push.",
        projection: Projection::SpecificImpulse,
    },
    Acronym {
        term: "RAAN",
        expansion: "Right ascension of the ascending node",
        plain: "The angle around the equator where the orbit crosses from south to north.",
        projection: Projection::AscendingNode,
    },
    Acronym {
        term: "AOP",
        expansion: "Argument of periapsis",
        plain: "The angle in the orbital plane from the ascending node to the closest point.",
        projection: Projection::ArgumentOfPeriapsis,
    },
    Acronym {
        term: "SOI",
        expansion: "Sphere of influence",
        plain: "The region where a smaller body's gravity dominates the patched-conic preview.",
        projection: Projection::SphereOfInfluence,
    },
    Acronym {
        term: "J2",
        expansion: "Earth's second zonal harmonic",
        plain: "The equatorial bulge term that twists orbital planes and apsides.",
        projection: Projection::AscendingNode,
    },
    Acronym {
        term: "SRP",
        expansion: "Solar radiation pressure",
        plain: "Sunlight momentum pushing on the spacecraft area.",
        projection: Projection::SolarPressure,
    },
    Acronym {
        term: "LEO",
        expansion: "Low Earth orbit",
        plain: "A close Earth parking orbit, here a few hundred kilometers up.",
        projection: Projection::LowEarthOrbit,
    },
    Acronym {
        term: "TLI",
        expansion: "Trans-lunar injection",
        plain: "The burn that raises apoapsis toward the Moon's distance.",
        projection: Projection::Tangential,
    },
    Acronym {
        term: "DAG",
        expansion: "Directed acyclic graph",
        plain: "A cause-and-effect chain with no loops.",
        projection: Projection::CausalGraph,
    },
    Acronym {
        term: "PDR",
        expansion: "Preliminary design review",
        plain: "An engineering dossier comparing prediction with modeled flight.",
        projection: Projection::DesignReview,
    },
];

pub fn find(term: &str) -> Option<&'static Acronym> {
    ACRONYMS.iter().find(|a| a.term == term)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TangentialProjection {
    pub kinetic_energy_change_j: f64,
    pub apoapsis_before_m: f64,
    pub apoapsis_after_m: f64,
    pub periapsis_before_m: f64,
    pub periapsis_after_m: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NormalProjection {
    pub torque_n_m: [f64; 3],
    pub plane_tilt_rad: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadialProjection {
    pub eccentricity_vector_before: [f64; 3],
    pub eccentricity_vector_after: [f64; 3],
    pub rotation_rad: f64,
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norm(a: [f64; 3]) -> f64 {
    libm::sqrt(dot(a, a))
}

fn apsides(state: State, mu: f64) -> Option<(f64, f64)> {
    let el = Elements::from_state(state, mu).ok()?;
    let a = el.semi_major_axis_m()?;
    if el.eccentricity >= 1.0 {
        return None;
    }
    Some((a * (1.0 + el.eccentricity), a * (1.0 - el.eccentricity)))
}

/// Live quantities for hovering the tangential handle.
pub fn tangential(state: State, mu: f64, mass_kg: f64, dv_t: f64) -> Option<TangentialProjection> {
    let basis = TnwBasis::from_state(state).ok()?;
    let dv = basis.inertial_delta_v([dv_t, 0.0, 0.0]).ok()?;
    let after = State {
        position_m: state.position_m,
        velocity_m_s: std::array::from_fn(|i| state.velocity_m_s[i] + dv[i]),
    };
    let before = apsides(state, mu);
    let after_apsides = apsides(after, mu);
    let dv2 = dot(dv, dv);
    let ke = 0.5 * mass_kg * (2.0 * dot(state.velocity_m_s, dv) + dv2);
    Some(TangentialProjection {
        kinetic_energy_change_j: ke,
        apoapsis_before_m: before.map_or(f64::NAN, |a| a.0),
        apoapsis_after_m: after_apsides.map_or(f64::NAN, |a| a.0),
        periapsis_before_m: before.map_or(f64::NAN, |a| a.1),
        periapsis_after_m: after_apsides.map_or(f64::NAN, |a| a.1),
    })
}

/// Torque and plane tilt for hovering the orbit-normal handle.
pub fn normal(state: State, thrust_n: f64, dv_w: f64) -> Option<NormalProjection> {
    let basis = TnwBasis::from_state(state).ok()?;
    let sign = if dv_w >= 0.0 { 1.0 } else { -1.0 };
    let force = basis.normal.map(|x| sign * thrust_n * x);
    let torque = cross(state.position_m, force);
    let dv = basis.inertial_delta_v([0.0, 0.0, dv_w]).ok()?;
    let h0 = cross(state.position_m, state.velocity_m_s);
    let v1 = std::array::from_fn(|i| state.velocity_m_s[i] + dv[i]);
    let h1 = cross(state.position_m, v1);
    let c = dot(h0, h1) / (norm(h0) * norm(h1));
    Some(NormalProjection {
        torque_n_m: torque,
        plane_tilt_rad: libm::acos(c.clamp(-1.0, 1.0)),
    })
}

/// Eccentricity-vector rotation for hovering the in-plane (radial at apsis) handle.
pub fn radial(state: State, mu: f64, dv_n: f64) -> Option<RadialProjection> {
    let basis = TnwBasis::from_state(state).ok()?;
    let dv = basis.inertial_delta_v([0.0, dv_n, 0.0]).ok()?;
    let ecc = |s: State| -> Option<[f64; 3]> {
        let h = cross(s.position_m, s.velocity_m_s);
        let vxh = cross(s.velocity_m_s, h);
        let r = norm(s.position_m);
        Some(std::array::from_fn(|i| vxh[i] / mu - s.position_m[i] / r))
    };
    let before = ecc(state)?;
    let after = ecc(State {
        position_m: state.position_m,
        velocity_m_s: std::array::from_fn(|i| state.velocity_m_s[i] + dv[i]),
    })?;
    let n0 = norm(before);
    let n1 = norm(after);
    if n0 <= 1e-12 || n1 <= 1e-12 {
        return None;
    }
    let c = dot(before, after) / (n0 * n1);
    Some(RadialProjection {
        eccentricity_vector_before: before,
        eccentricity_vector_after: after,
        rotation_rad: libm::acos(c.clamp(-1.0, 1.0)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forbidden_acronyms_have_projections() {
        for term in ["RAAN", "AOP", "TWR", "ISP", "Isp", "TNW", "SOI"] {
            assert!(find(term).is_some(), "{term} missing hover projection");
        }
    }

    #[test]
    fn every_acronym_declares_a_browser_art_key() {
        // Keys the lens renderer actually implements. A key outside this set
        // would fall through to the generic finite-burn drawing.
        const ART: &[&str] = &[
            "t",
            "w",
            "n",
            "twr",
            "isp",
            "raan",
            "aop",
            "soi",
            "gravity-loss",
            "tnw",
            "srp",
            "leo",
            "dag",
            "pdr",
        ];
        for a in ACRONYMS {
            let key = a.projection.key();
            assert!(
                ART.contains(&key),
                "{} maps to unrendered key {key}",
                a.term
            );
        }
        // The reverse direction is not required: some art is reached only from
        // a canvas handle ("w", "time") or a causal demo ("impact", "propellant",
        // "periapsis", "apoapsis", "split-kick"), never from an acronym.
    }

    #[test]
    fn projection_keys_are_stable_and_match_the_renderer() {
        use Projection::*;
        for (projection, key) in [
            (Tangential, "t"),
            (Normal, "w"),
            (Radial, "n"),
            (ThrustToWeight, "twr"),
            (SpecificImpulse, "isp"),
            (AscendingNode, "raan"),
            (ArgumentOfPeriapsis, "aop"),
            (SphereOfInfluence, "soi"),
            (Eccentricity, "n"),
            (GravityLoss, "gravity-loss"),
            (ManeuverFrame, "tnw"),
            (SolarPressure, "srp"),
            (LowEarthOrbit, "leo"),
            (CausalGraph, "dag"),
            (DesignReview, "pdr"),
        ] {
            assert_eq!(projection.key(), key, "{projection:?} art key drifted");
        }
    }

    #[test]
    fn tangential_prograde_raises_opposite_apsis_at_periapsis() {
        let r = 6_578_137.0;
        let mu = 3.986_004_418e14;
        let state = State {
            position_m: [r, 0.0, 0.0],
            velocity_m_s: [0.0, libm::sqrt(mu / r), 0.0],
        };
        let p = tangential(state, mu, 1000.0, 100.0).unwrap();
        assert!(p.kinetic_energy_change_j > 0.0);
        assert!(p.apoapsis_after_m > p.apoapsis_before_m);
    }

    #[test]
    fn tangential_reports_energy_even_when_result_is_unbound() {
        let r = 6_578_137.0;
        let mu = 3.986_004_418e14;
        let state = State {
            position_m: [r, 0.0, 0.0],
            velocity_m_s: [0.0, libm::sqrt(mu / r), 0.0],
        };
        let p = tangential(state, mu, 1000.0, 4000.0).unwrap();
        assert!(p.kinetic_energy_change_j > 0.0);
        assert!(p.apoapsis_after_m.is_nan());
    }

    #[test]
    fn radial_rejects_undefined_circular_eccentricity_direction() {
        let r = 6_578_137.0;
        let mu = 3.986_004_418e14;
        let state = State {
            position_m: [r, 0.0, 0.0],
            velocity_m_s: [0.0, libm::sqrt(mu / r), 0.0],
        };
        assert!(radial(state, mu, 10.0).is_none());
    }

    #[test]
    fn normal_torque_is_r_cross_f_and_tilts_plane() {
        let state = State {
            position_m: [7_000_000.0, 0.0, 0.0],
            velocity_m_s: [0.0, 7_500.0, 0.0],
        };
        let p = normal(state, 1000.0, 100.0).unwrap();
        assert!(p.torque_n_m[1].abs() > 0.0);
        assert!(p.plane_tilt_rad > 0.0);
    }
}
