#![no_std]

pub mod frames;
pub mod hash;
pub mod provenance;
pub mod solar;
pub mod trig;
pub mod vec;

pub use frames::{BarycentricPos, GalacticSector, LIGHT_SPEED_MPS, LIGHT_YEAR_NM};
pub use hash::DetHasher;
pub use provenance::{EpistemicSource, ProvGraph, ProvId, ProvNode};
pub use solar::{SolarState, sun_state_uchinoura};
pub use trig::{dasin, datan2, dcos, dexp, dln, dsin, dsqrt, dtan, clamp_f32, clamp_f64, PI, TAU, HALF_PI};
pub use vec::{Aabb3, Vec3, Vec3d};
