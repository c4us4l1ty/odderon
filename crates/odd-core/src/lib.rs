//! Foundational simulation types. Distances are in meters and times in seconds.
//!
//! This crate does not yet model celestial ephemerides, orientations over time, or UTC.
//! No floating point rendering coordinates should be fed back into simulation state.

pub mod canon;
pub mod consts;
pub mod epoch;
pub mod frames;
pub mod units;
pub mod utc;
