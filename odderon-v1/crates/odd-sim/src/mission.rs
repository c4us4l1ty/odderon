//! Locked prediction and point-Earth orbit-rescue scoring primitives.
//! A lock owns its plan and freezes the player's inputs; it does not sign a
//! replay, supervise fuel valves, or supply a target-encounter geometry.
use crate::planner::Plan;
use odd_astro::{elements::Elements, integrator::IntegrationError, spline::Keyframe};
use odd_core::frames::State;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PredictionBaseline {
    pub delta_v_m_s: f64,
    pub apoapsis_altitude_m: f64,
    pub periapsis_altitude_m: f64,
    pub remaining_propellant_kg: f64,
    pub time_of_flight_s: f64,
    /// None when there is no defined target position or encounter epoch.
    pub target_miss_distance_m: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionError {
    InvalidPrediction,
    InvalidOrbit,
    OutsideFuelBudget,
}

#[derive(Clone, Debug)]
pub struct LockedMission {
    plan: Plan,
    prediction: PredictionBaseline,
    dry_mass_kg: f64,
}
impl LockedMission {
    /// Consume the plan, freezing it and the player's prediction together.
    pub fn new(
        plan: Plan,
        prediction: PredictionBaseline,
        dry_mass_kg: f64,
    ) -> Result<Self, MissionError> {
        if !dry_mass_kg.is_finite()
            || dry_mass_kg <= 0.0
            || !prediction.delta_v_m_s.is_finite()
            || prediction.delta_v_m_s < 0.0
            || !prediction.apoapsis_altitude_m.is_finite()
            || !prediction.periapsis_altitude_m.is_finite()
            || !prediction.remaining_propellant_kg.is_finite()
            || prediction.remaining_propellant_kg < 0.0
            || !prediction.time_of_flight_s.is_finite()
            || prediction.time_of_flight_s <= 0.0
            || prediction
                .target_miss_distance_m
                .is_some_and(|v| !v.is_finite() || v < 0.0)
        {
            return Err(MissionError::InvalidPrediction);
        }
        Ok(Self {
            plan,
            prediction,
            dry_mass_kg,
        })
    }
    pub fn plan(&self) -> &Plan {
        &self.plan
    }
    pub fn prediction(&self) -> PredictionBaseline {
        self.prediction
    }
    pub fn dry_mass_kg(&self) -> f64 {
        self.dry_mass_kg
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Apsides {
    pub apoapsis_altitude_m: f64,
    pub periapsis_altitude_m: f64,
    pub eccentricity: f64,
}
/// Osculating two-body apsides at this state, not future measured extrema.
pub fn apsides(state: Keyframe, mu: f64, body_radius_m: f64) -> Result<Apsides, MissionError> {
    if !body_radius_m.is_finite() || body_radius_m <= 0.0 {
        return Err(MissionError::InvalidOrbit);
    }
    let el = Elements::from_state(
        State {
            position_m: state.position,
            velocity_m_s: state.velocity,
        },
        mu,
    )
    .map_err(|_| MissionError::InvalidOrbit)?;
    if el.eccentricity >= 1.0 {
        return Err(MissionError::InvalidOrbit);
    }
    let a = el.semi_major_axis_m().ok_or(MissionError::InvalidOrbit)?;
    Ok(Apsides {
        apoapsis_altitude_m: a * (1.0 + el.eccentricity) - body_radius_m,
        periapsis_altitude_m: a * (1.0 - el.eccentricity) - body_radius_m,
        eccentricity: el.eccentricity,
    })
}

/// Explicitly local scoring rule for the 550 km circular-orbit objective.
/// The score is zero if the hard deadline, eccentricity, or fuel conditions
/// fail. For an achieved target, score runs from 0.8 at the tolerance edge to
/// 1.0 at the center; outside tolerance it is below 0.8. Not an Elo update.
pub fn score_rescue(
    state: Keyframe,
    dry_mass_kg: f64,
    time_s: f64,
    mu: f64,
    radius_m: f64,
) -> Result<f64, MissionError> {
    if !dry_mass_kg.is_finite()
        || dry_mass_kg <= 0.0
        || !time_s.is_finite()
        || time_s < 0.0
        || !state.mass.is_finite()
    {
        return Err(MissionError::InvalidOrbit);
    }
    if state.mass < dry_mass_kg {
        return Err(MissionError::OutsideFuelBudget);
    }
    let result = apsides(state, mu, radius_m)?;
    if time_s > 10_800.0 || result.eccentricity >= 0.005 {
        return Ok(0.0);
    }
    let error = (result.apoapsis_altitude_m - 550_000.0)
        .abs()
        .max((result.periapsis_altitude_m - 550_000.0).abs());
    Ok(if error <= 10_000.0 {
        1.0 - 0.2 * error / 10_000.0
    } else {
        (0.8 * 10_000.0 / error).max(0.0)
    })
}

pub fn metric_apoapsis(state: Keyframe, mu: f64, radius_m: f64) -> Result<f64, IntegrationError> {
    Ok(apsides(state, mu, radius_m)
        .map_err(|_| IntegrationError::ForceFailure)?
        .apoapsis_altitude_m)
}
