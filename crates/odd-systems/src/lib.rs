//! Coupled spacecraft subsystems: cryogenic tank thermal state, boil-off and
//! the electrical bus that active cooling draws from.
//!
//! # Laws honoured
//!
//! * **SI base units** throughout — K, W, J, kg, Pa, m².
//! * **No-Euler**: the tank ODE steps through `odd_astro::dp45`, the same
//!   Dormand–Prince 5(4) tableau and controller the orbital integrator uses.
//!   There is no second integrator to drift.
//! * **No wall clock**: `integrate_tank` advances on the flight's own time
//!   axis, and `integrate_along` samples position from the cached trajectory
//!   spline. Nothing polls, sleeps or reads `Instant::now`.
//! * **No magic lookup**: every watt and every kg/s is derived from geometry,
//!   fluid constants and the real sun/planet vectors. The signed record will
//!   carry the *design*, never a boil-off rate.
//!
//! # Scope
//!
//! A lumped single-node liquid model with a saturation-pinned venting regime.
//! It does not resolve stratification, ullage thermodynamics beyond the
//! ideal-gas relief estimate, slosh, or transient chilldown of the shell.

pub mod bus;
pub mod coupled;
pub mod environment;
pub mod fluid;
pub mod stage;
pub mod thermal;

pub use bus::{BusSpec, PowerBudget};
pub use coupled::{allocate_bus, integrate_pair, BusAllocation, TankPair};
pub use environment::ThermalEnvironment;
pub use fluid::{Fluid, FluidProperties};
pub use stage::{needs_thermal_model, StageThermal, StageThermalHistory};
pub use thermal::{
    cryocooler_point, heat_balance, ideal_epsilon_for_layers, integrate_along, integrate_tank,
    CryocoolerPoint, HeatBalance, ShellMaterial, TankHistory, TankSpec, TankState, ThermalClock,
    ThermalContext, VentEvent,
};
