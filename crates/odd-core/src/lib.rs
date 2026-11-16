//! Foundational simulation types. Distances are in meters and times in seconds.
//!
//! This crate does not yet model celestial ephemerides or orientations over time.
//! No floating point rendering coordinates should be fed back into simulation state.

pub mod canon;
pub mod consts;
pub mod epoch;
pub mod fmt;
pub mod frames;
pub mod physics_revision;

pub use physics_revision::PhysicsRevision;
pub mod units;
pub mod utc;
