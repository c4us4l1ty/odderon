//! Selected reference body parameters. They are constants/models, not exact
//! measured or immutable properties, and do not imply a shape or gravity field.
use crate::Body;

/// DE440 mass parameter and NAIF pck00011 reference radii in SI units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyPhysical {
    pub gravitational_parameter_m3_s2: f64,
    /// Triaxial reference ellipsoid radii, not a topographic surface.
    pub radii_m: [f64; 3],
}

/// Source: Park et al. (2021), DE440 table 2; radii: NAIF pck00011 (2022).
/// The Sun's 695700 km is a nominal solar radius; lunar radii are spherical.
/// Barycenters are mathematical points and have no physical body parameters.
pub fn physical(body: Body) -> Option<BodyPhysical> {
    let (mu, radii) = match body {
        Body::Sun => (1.327_124_400_412_794_2e20, [695_700_000.0; 3]),
        Body::Earth => (3.986_004_355_07e14, [6_378_136.6, 6_378_136.6, 6_356_751.9]),
        Body::Moon => (4.902_800_118e12, [1_737_400.0; 3]),
        _ => return None,
    };
    Some(BodyPhysical {
        gravitational_parameter_m3_s2: mu,
        radii_m: radii,
    })
}
