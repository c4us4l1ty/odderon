//! Exact Shapley attribution of a scalar mission-metric error.
//!
//! Four physical players are toggled independently:
//!
//! 0. finite-burn arc (absent = instantaneous impulses with the same propellant)
//! 1. Earth J2 zonal gravity
//! 2. Sun/Moon third-body gravity
//! 3. atmospheric drag + solar radiation pressure
//!
//! The value of a coalition is the chosen metric at a common epoch. The empty
//! coalition is the Level-1 impulsive, unperturbed reference. Exact Shapley
//! values average each player's marginal contribution over all 2⁴ coalitions,
//! so the signed contributions sum to `full - reference` with no residual
//! other than floating-point roundoff. This is deliberately not the older
//! leave-one-out [`crate::attribution`] sensitivity recipe.
//!
//! All coalitions are propagated with the same integrator options, force
//! assembly, environment callback and flight limits. If any coalition impacts
//! before the requested epoch, every outcome is resampled at the earliest
//! impact and `truncated` is set; callers must not present that as a successful
//! late-epoch prediction comparison.

use crate::{
    force_model::{FlightLimits, ForceModel},
    integrator::{IntegrationError, Options},
    perturb::burn::Burn,
    spline::Keyframe,
};

pub const PLAYERS: usize = 4;
pub const COALITIONS: usize = 1 << PLAYERS;
pub const PLAYER_NAMES: [&str; PLAYERS] = [
    "Finite-burn arc",
    "Earth equatorial bulge",
    "Moon and Sun gravity",
    "Atmospheric drag and sunlight pressure",
];

#[derive(Clone, Debug)]
pub struct ShapleyAttribution {
    /// Signed metric contributions in the metric's own units.
    pub contributions: [f64; PLAYERS],
    /// Absolute contributions normalized to sum to one, or None when all zero.
    pub shares: Option<[f64; PLAYERS]>,
    /// full metric minus empty-coalition metric.
    pub total_error: f64,
    pub reference_metric: f64,
    pub full_metric: f64,
    /// Sum of contributions minus total error; a numerical-quality gauge.
    pub efficiency_residual: f64,
    pub coalition_metrics: [f64; COALITIONS],
    pub common_end_s: f64,
    pub requested_end_s: f64,
    pub truncated: bool,
}

fn model_for_coalition(full: ForceModel, mask: usize) -> ForceModel {
    let mut m = full;
    m.burn = None;
    if mask & 2 == 0 {
        m.zonal = None;
    }
    if mask & 4 == 0 {
        m.third_body = false;
    }
    if mask & 8 == 0 {
        m.drag = None;
        m.radiation = None;
    }
    m
}

/// Propagate an impulsive command sequence with a possibly perturbed model.
/// Returns the reached state and the earliest impact time, if any.
fn propagate_impulses<F>(
    model: ForceModel,
    initial: Keyframe,
    end: f64,
    options: Options,
    impulses: &[crate::attribution::Impulse],
    limits: FlightLimits,
    environment: &mut F,
) -> Result<(Keyframe, Option<f64>), IntegrationError>
where
    F: FnMut(f64) -> Result<crate::force_model::Environment, IntegrationError>,
{
    let mut state = initial;
    for impulse in impulses {
        if impulse.time > end {
            break;
        }
        let integrated = model.propagate_with_limits(
            state,
            impulse.time,
            options,
            &[],
            limits,
            &mut *environment,
        )?;
        if let Some(stop) = integrated.stopped {
            return Ok((stop.state, Some(stop.time)));
        }
        state = integrated
            .trajectory
            .sample(impulse.time)
            .map_err(|_| IntegrationError::NumericalRange)?;
        for i in 0..3 {
            state.velocity[i] += impulse.delta_v_m_s[i];
        }
        state.mass -= impulse.propellant_kg;
        if state.mass <= 0.0
            || !state.mass.is_finite()
            || !state.velocity.iter().all(|x| x.is_finite())
        {
            return Err(IntegrationError::NumericalRange);
        }
    }
    let integrated =
        model.propagate_with_limits(state, end, options, &[], limits, &mut *environment)?;
    if let Some(stop) = integrated.stopped {
        return Ok((stop.state, Some(stop.time)));
    }
    Ok((
        integrated
            .trajectory
            .sample(end)
            .map_err(|_| IntegrationError::NumericalRange)?,
        None,
    ))
}

/// Exact four-player Shapley attribution with terminal-impact handling.
#[allow(clippy::too_many_arguments)]
pub fn attribute_shapley<F, M>(
    initial: Keyframe,
    end: f64,
    options: Options,
    full_model: ForceModel,
    burns: &[Burn],
    impulses: &[crate::attribution::Impulse],
    limits: FlightLimits,
    mut environment: F,
    metric: M,
) -> Result<ShapleyAttribution, IntegrationError>
where
    F: FnMut(f64) -> Result<crate::force_model::Environment, IntegrationError>,
    M: Fn(Keyframe) -> Result<f64, IntegrationError>,
{
    if !end.is_finite()
        || end < initial.time
        || impulses.iter().any(|b| {
            !b.time.is_finite()
                || b.time < initial.time
                || b.time > end
                || !b.propellant_kg.is_finite()
                || b.propellant_kg < 0.0
                || !b.delta_v_m_s.iter().all(|x| x.is_finite())
        })
        || impulses.windows(2).any(|w| w[0].time >= w[1].time)
    {
        return Err(IntegrationError::InvalidInput);
    }

    let run = |mask: usize,
               until: f64,
               environment: &mut F|
     -> Result<(Keyframe, Option<f64>), IntegrationError> {
        let model = model_for_coalition(full_model, mask);
        if mask & 1 == 0 {
            propagate_impulses(
                model,
                initial,
                until,
                options,
                impulses,
                limits,
                environment,
            )
        } else {
            let integrated =
                model.propagate_with_limits(initial, until, options, burns, limits, environment)?;
            if let Some(stop) = integrated.stopped {
                return Ok((stop.state, Some(stop.time)));
            }
            Ok((
                integrated
                    .trajectory
                    .sample(until)
                    .map_err(|_| IntegrationError::NumericalRange)?,
                None,
            ))
        }
    };

    let mut states = [initial; COALITIONS];
    let mut stops: [Option<f64>; COALITIONS] = [None; COALITIONS];
    let mut common = end;
    for mask in 0..COALITIONS {
        let (state, stop) = run(mask, end, &mut environment)?;
        states[mask] = state;
        stops[mask] = stop;
        if let Some(t) = stop {
            common = common.min(t);
        }
    }
    let truncated = common < end;
    if truncated {
        for mask in 0..COALITIONS {
            if stops[mask].is_none_or(|t| t > common) {
                states[mask] = run(mask, common, &mut environment)?.0;
            }
        }
    }

    let mut coalition_metrics = [f64::NAN; COALITIONS];
    for mask in 0..COALITIONS {
        coalition_metrics[mask] = metric(states[mask])?;
        if !coalition_metrics[mask].is_finite() {
            return Err(IntegrationError::ForceFailure);
        }
    }

    // Shapley weight for a coalition of size k among n players:
    // k! (n-k-1)! / n!
    let weights = [1.0 / 4.0, 1.0 / 12.0, 1.0 / 12.0, 1.0 / 4.0];
    let mut contributions = [0.0; PLAYERS];
    for (player, contribution) in contributions.iter_mut().enumerate() {
        let bit = 1 << player;
        for mask in 0..COALITIONS {
            if mask & bit != 0 {
                continue;
            }
            let k = (mask as u32).count_ones() as usize;
            let marginal = coalition_metrics[mask | bit] - coalition_metrics[mask];
            *contribution += weights[k] * marginal;
        }
    }
    let reference_metric = coalition_metrics[0];
    let full_metric = coalition_metrics[COALITIONS - 1];
    let total_error = full_metric - reference_metric;
    let sum: f64 = contributions.iter().sum();
    let efficiency_residual = sum - total_error;
    let scale = contributions.iter().fold(0.0_f64, |a, x| a.max(x.abs()));
    let shares = if scale == 0.0 {
        None
    } else {
        let abs = contributions.map(|x| x.abs() / scale);
        let total: f64 = abs.iter().sum();
        Some(abs.map(|x| x / total))
    };
    Ok(ShapleyAttribution {
        contributions,
        shares,
        total_error,
        reference_metric,
        full_metric,
        efficiency_residual,
        coalition_metrics,
        common_end_s: common,
        requested_end_s: end,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapley_weights_sum_to_one_for_each_player() {
        for player in 0..PLAYERS {
            let bit = 1 << player;
            let mut sum = 0.0;
            for mask in 0..COALITIONS {
                if mask & bit == 0 {
                    let k = (mask as u32).count_ones() as usize;
                    sum += weights_for_test(k);
                }
            }
            assert!((sum - 1.0).abs() < 1e-15);
        }
    }
    fn weights_for_test(k: usize) -> f64 {
        [1.0 / 4.0, 1.0 / 12.0, 1.0 / 12.0, 1.0 / 4.0][k]
    }

    #[test]
    fn additive_game_has_no_interaction_residual() {
        // v(S) = 3*finite + 5*j2 - 2*third + 7*drag_srp.
        let value = |mask: usize| -> f64 {
            3.0 * ((mask & 1) != 0) as u8 as f64 + 5.0 * ((mask & 2) != 0) as u8 as f64
                - 2.0 * ((mask & 4) != 0) as u8 as f64
                + 7.0 * ((mask & 8) != 0) as u8 as f64
        };
        let mut metrics = [0.0; COALITIONS];
        for (mask, metric) in metrics.iter_mut().enumerate() {
            *metric = value(mask);
        }
        let weights = [1.0 / 4.0, 1.0 / 12.0, 1.0 / 12.0, 1.0 / 4.0];
        let mut phi = [0.0; PLAYERS];
        for (player, contribution) in phi.iter_mut().enumerate() {
            let bit = 1 << player;
            for mask in 0..COALITIONS {
                if mask & bit == 0 {
                    let k = (mask as u32).count_ones() as usize;
                    *contribution += weights[k] * (metrics[mask | bit] - metrics[mask]);
                }
            }
        }
        for (actual, expected) in phi.iter().zip([3.0, 5.0, -2.0, 7.0]) {
            assert!((actual - expected).abs() < 1e-14);
        }
        assert!((phi.iter().sum::<f64>() - (value(15) - value(0))).abs() < 1e-14);
    }
}
