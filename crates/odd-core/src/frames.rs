//! Translation-only local frames with shared inertial axes.
//!
//! A star's coarse catalog anchor is kept separate from local meter offsets.
//! Perform nearby calculations in their shared frame rather than subtracting
//! enormous global floating-point positions. This module does not yet provide
//! rotation, velocities, cross-system transforms, or ephemerides.

/// Coarse ICRS-aligned stellar anchor in integer milliparsecs relative to the
/// Solar System barycenter. This is catalog indexing, not a precise location;
/// one milliparsec is approximately 3.09e13 m. Catalog residuals must be
/// stored separately before using an anchor for astrometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GalacticAnchor {
    pub milliparsecs: [i64; 3],
}

/// An origin in its parent frame, in meters. Both frames have the same axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Translation {
    origin_m: [f64; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NonFiniteCoordinate;

impl Translation {
    pub fn new(origin_m: [f64; 3]) -> Result<Self, NonFiniteCoordinate> {
        if origin_m.iter().all(|x| x.is_finite()) {
            Ok(Self { origin_m })
        } else {
            Err(NonFiniteCoordinate)
        }
    }

    /// Convert a child-relative position to its parent frame in meters.
    pub fn to_parent(self, child_m: [f64; 3]) -> Result<[f64; 3], NonFiniteCoordinate> {
        self.apply(child_m, false)
    }

    /// Convert a parent-relative position to this frame in meters.
    pub fn from_parent(self, parent_m: [f64; 3]) -> Result<[f64; 3], NonFiniteCoordinate> {
        self.apply(parent_m, true)
    }

    fn apply(self, position: [f64; 3], subtract: bool) -> Result<[f64; 3], NonFiniteCoordinate> {
        let mut result = [0.0; 3];
        for i in 0..3 {
            result[i] = if subtract {
                position[i] - self.origin_m[i]
            } else {
                position[i] + self.origin_m[i]
            };
        }
        if result.iter().all(|x| x.is_finite()) {
            Ok(result)
        } else {
            Err(NonFiniteCoordinate)
        }
    }
}

/// Convert a camera-relative simulation position to a lossy f32 render position.
/// Never use this output to reconstruct simulation coordinates.
pub fn render_position(camera_relative_m: [f64; 3]) -> Result<[f32; 3], NonFiniteCoordinate> {
    let result = camera_relative_m.map(|x| x as f32);
    if result.iter().all(|x| x.is_finite()) {
        Ok(result)
    } else {
        Err(NonFiniteCoordinate)
    }
}
