//! Multi-node two-body preview and quantized finite-burn compilation.
//! Times are integer microseconds relative to the initial state. The preview
//! switches conics at each impulse; it does not perform encounter finding,
//! model dry-mass failures in flight, or predict lunar gravity assists.
use crate::{compile_burn, CommandError, CommandTimeline, Engine};
use odd_astro::{
    anomaly::{elliptic_from_mean, elliptic_from_true, elliptic_mean, hyperbolic_from_mean, hyperbolic_from_true, hyperbolic_mean, true_from_elliptic, true_from_hyperbolic},
    elements::Elements,
    maneuver::{apply_impulse, TnwBasis},
    spline::Keyframe,
};
use odd_core::frames::State;

#[derive(Clone, Copy, Debug)]
pub struct ManeuverNode {
    pub epoch_us: i64,
    /// Tangential, in-plane (W × T), and orbit-normal delta-v in m/s.
    pub tnw_m_s: [f64; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanError {
    InvalidInput,
    UnsupportedConic,
    NumericalRange,
    PropellantDeficit,
    BurnOutsidePlan,
    Command(CommandError),
}
impl From<CommandError> for PlanError {
    fn from(value: CommandError) -> Self { Self::Command(value) }
}

#[derive(Clone, Copy, Debug)]
struct Segment {
    epoch_us: i64,
    state: State,
    mass_kg: f64,
}

/// A compiled preview, with pre-burn inertial impulses for shadow runs.
#[derive(Clone, Debug)]
pub struct Plan {
    segments: Vec<Segment>,
    pub timeline: CommandTimeline,
    pub impulses: Vec<odd_astro::attribution::Impulse>,
    pub commanded_propellant_kg: f64,
    pub requested_delta_v_m_s: f64,
    pub end_us: i64,
    mu: f64,
}

/// Propagate a non-parabolic conic analytically. Backward time is allowed,
/// but the caller must ensure the time interval remains physically meaningful.
pub fn kepler_coast(state: State, mu: f64, dt: f64) -> Result<State, PlanError> {
    if !dt.is_finite() || !mu.is_finite() || mu <= 0.0 { return Err(PlanError::InvalidInput); }
    let mut el = Elements::from_state(state, mu).map_err(|_| PlanError::InvalidInput)?;
    if dt == 0.0 { return Ok(state); }
    let e = el.eccentricity;
    let a = el.semi_major_axis_m().ok_or(PlanError::UnsupportedConic)?;
    let rate = libm::sqrt(mu / a.abs()) / a.abs();
    if !rate.is_finite() || rate <= 0.0 { return Err(PlanError::NumericalRange); }
    el.true_anomaly_rad = if e < 1.0 {
        let ea = elliptic_from_true(e, el.true_anomaly_rad).map_err(|_| PlanError::NumericalRange)?;
        let mean = elliptic_mean(e, ea).map_err(|_| PlanError::NumericalRange)? + rate*dt;
        let next = elliptic_from_mean(e, mean).map_err(|_| PlanError::NumericalRange)?;
        true_from_elliptic(e, next).map_err(|_| PlanError::NumericalRange)?
    } else if e > 1.0 {
        let ha = hyperbolic_from_true(e, el.true_anomaly_rad).map_err(|_| PlanError::NumericalRange)?;
        let mean = hyperbolic_mean(e, ha).map_err(|_| PlanError::NumericalRange)? + rate*dt;
        let next = hyperbolic_from_mean(e, mean).map_err(|_| PlanError::NumericalRange)?;
        true_from_hyperbolic(e, next).map_err(|_| PlanError::NumericalRange)?
    } else { return Err(PlanError::UnsupportedConic); };
    el.to_state(mu).map_err(|_| PlanError::NumericalRange)
}

impl Plan {
    /// Build a preview with a fixed engine and explicit dry mass. Each
    /// command's orientation is frozen in the inertial frame at its ideal
    /// node state; the finite trajectory can therefore diverge from the ideal.
    /// Burn intervals must fit inside [0, end_us] and must not overlap.
    pub fn compile(initial: Keyframe, mu: f64, end_us: i64, dry_mass_kg: f64, engine: Engine, nodes: &[ManeuverNode]) -> Result<Self, PlanError> {
        if initial.time != 0.0 || end_us <= 0 || end_us > (1_i64 << 53)
            || !initial.mass.is_finite() || !dry_mass_kg.is_finite() || dry_mass_kg <= 0.0 || initial.mass < dry_mass_kg
            || nodes.iter().any(|n| n.epoch_us <= 0 || n.epoch_us > end_us)
            || nodes.windows(2).any(|n| n[0].epoch_us >= n[1].epoch_us) { return Err(PlanError::InvalidInput); }
        let initial_state = State { position_m: initial.position, velocity_m_s: initial.velocity };
        Elements::from_state(initial_state, mu).map_err(|_| PlanError::InvalidInput)?;
        let mut segments = vec![Segment { epoch_us: 0, state: initial_state, mass_kg: initial.mass }];
        let mut timeline = CommandTimeline::default();
        let mut impulses = Vec::with_capacity(nodes.len());
        let mut propellant = 0.0;
        let mut total_dv = 0.0;
        for node in nodes {
            let previous = *segments.last().ok_or(PlanError::NumericalRange)?;
            let pre = kepler_coast(previous.state, mu, (node.epoch_us - previous.epoch_us) as f64 / 1e6)?;
            let burn = compile_burn(pre, node.tnw_m_s, node.epoch_us, previous.mass_kg, engine)?;
            let crate::CommandKind::Burn { dur_us, .. } = burn.command.kind;
            let burn_end = burn.command.t_us.checked_add(dur_us).ok_or(PlanError::NumericalRange)?;
            if burn.command.t_us < 0 || burn_end > end_us { return Err(PlanError::BurnOutsidePlan); }
            let mass = previous.mass_kg - burn.commanded_propellant_kg;
            if mass < dry_mass_kg { return Err(PlanError::PropellantDeficit); }
            timeline.insert(burn.command)?;
            let dv = TnwBasis::from_state(pre).map_err(|_| PlanError::InvalidInput)?.inertial_delta_v(node.tnw_m_s).map_err(|_| PlanError::NumericalRange)?;
            let post = apply_impulse(pre, node.tnw_m_s).map_err(|_| PlanError::NumericalRange)?;
            impulses.push(odd_astro::attribution::Impulse { time: node.epoch_us as f64 / 1e6, delta_v_m_s: dv, propellant_kg: burn.commanded_propellant_kg });
            propellant += burn.commanded_propellant_kg;
            total_dv += libm::hypot(libm::hypot(dv[0],dv[1]),dv[2]);
            if !propellant.is_finite() || !total_dv.is_finite() { return Err(PlanError::NumericalRange); }
            segments.push(Segment { epoch_us: node.epoch_us, state: post, mass_kg: mass });
        }
        Ok(Self { segments, timeline, impulses, commanded_propellant_kg: propellant, requested_delta_v_m_s: total_dv, end_us, mu })
    }

    /// Right-continuous ideal state: at a node's epoch, the impulse is applied.
    pub fn sample(&self, time_us: i64) -> Result<Keyframe, PlanError> {
        if time_us < 0 || time_us > self.end_us { return Err(PlanError::InvalidInput); }
        let i = self.segments.partition_point(|s| s.epoch_us <= time_us) - 1;
        let segment = self.segments[i];
        let state = kepler_coast(segment.state, self.mu, (time_us-segment.epoch_us) as f64 / 1e6)?;
        Ok(Keyframe { time: time_us as f64 / 1e6, position: state.position_m, velocity: state.velocity_m_s, mass: segment.mass_kg })
    }
}
