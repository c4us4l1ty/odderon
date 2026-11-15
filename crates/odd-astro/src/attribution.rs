//! Counterfactual sensitivity summaries, not additive causal decomposition.
//! Input outcomes must come from consistently configured independent runs.
//! The finite-only comparison has a different reference from leave-one-out
//! comparisons, so normalized shares describe the specified sensitivity recipe.

#[derive(Clone, Copy, Debug)]
pub struct Outcome {
    pub position_m: [f64; 3],
    /// Signed metric selected by the caller, e.g. apoapsis radius or unwrapped
    /// node longitude, using identical units and epochs in every run.
    pub metric: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Counterfactuals {
    pub impulsive: Outcome,
    pub finite_only: Outcome,
    pub full: Outcome,
    pub no_j2: Outcome,
    pub no_third_body: Outcome,
    pub no_drag: Outcome,
    pub no_srp: Outcome,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sensitivity {
    /// Order: finite burn, J2, third bodies, drag, solar radiation pressure.
    pub position_distances_m: [f64; 5],
    pub signed_metric_deltas: [f64; 5],
    /// Normalized position-distance magnitudes, or None when all are zero.
    pub position_shares: Option<[f64; 5]>,
    /// Normalized absolute changes of the chosen metric, or None when all zero.
    pub metric_shares: Option<[f64; 5]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttributionError {
    InvalidOutcome,
    NumericalRange,
}

fn shares(values: [f64; 5]) -> Result<Option<[f64; 5]>, AttributionError> {
    let scale = values.iter().fold(0.0_f64, |a, x| a.max(x.abs()));
    if !scale.is_finite() {
        return Err(AttributionError::NumericalRange);
    }
    if scale == 0.0 {
        return Ok(None);
    }
    let v = values.map(|x| x.abs() / scale);
    let sum: f64 = v.iter().sum();
    Ok(Some(v.map(|x| x / sum)))
}

impl Counterfactuals {
    pub fn sensitivities(self) -> Result<Sensitivity, AttributionError> {
        let pairs = [
            (self.finite_only, self.impulsive),
            (self.full, self.no_j2),
            (self.full, self.no_third_body),
            (self.full, self.no_drag),
            (self.full, self.no_srp),
        ];
        if pairs.iter().any(|(a, b)| {
            !a.metric.is_finite()
                || !b.metric.is_finite()
                || !a
                    .position_m
                    .iter()
                    .chain(&b.position_m)
                    .all(|x| x.is_finite())
        }) {
            return Err(AttributionError::InvalidOutcome);
        }
        let mut distances = [0.0; 5];
        let mut metrics = [0.0; 5];
        for (i, (a, b)) in pairs.iter().enumerate() {
            let d = std::array::from_fn::<_, 3, _>(|j| a.position_m[j] - b.position_m[j]);
            distances[i] = libm::hypot(libm::hypot(d[0], d[1]), d[2]);
            metrics[i] = a.metric - b.metric;
        }
        let position_shares = shares(distances)?;
        let metric_shares = shares(metrics)?;
        Ok(Sensitivity {
            position_distances_m: distances,
            signed_metric_deltas: metrics,
            position_shares,
            metric_shares,
        })
    }
}

/// An impulsive baseline command in inertial axes at a known relative time.
/// Fuel is removed instantaneously for bookkeeping; no dry-mass model is implied.
#[derive(Clone, Copy, Debug)]
pub struct Impulse {
    pub time: f64,
    pub delta_v_m_s: [f64; 3],
    pub propellant_kg: f64,
}

/// Run the seven specified models and evaluate the same metric at the same
/// final epoch. Finite burn directions and timings are supplied explicitly:
/// this routine does not invent steering, events, encounters or dry-mass limits.
/// The environment callback must be deterministic and stateless across runs.
#[allow(clippy::too_many_arguments)] // Explicitly separates both models and their commands.
pub fn propagate_counterfactuals<F, M>(
    initial: crate::spline::Keyframe,
    end: f64,
    options: crate::integrator::Options,
    full_model: crate::force_model::ForceModel,
    burns: &[crate::perturb::burn::Burn],
    impulses: &[Impulse],
    mut environment: F,
    metric: M,
) -> Result<Counterfactuals, crate::integrator::IntegrationError>
where
    F: FnMut(f64) -> Result<crate::force_model::Environment, crate::integrator::IntegrationError>,
    M: Fn(crate::spline::Keyframe) -> Result<f64, crate::integrator::IntegrationError>,
{
    use crate::integrator::IntegrationError;
    if impulses.iter().any(|b| {
        !b.time.is_finite()
            || b.time < initial.time
            || b.time > end
            || !b.propellant_kg.is_finite()
            || b.propellant_kg < 0.0
            || !b.delta_v_m_s.iter().all(|x| x.is_finite())
    }) || impulses.windows(2).any(|w| w[0].time >= w[1].time)
    {
        return Err(IntegrationError::InvalidInput);
    }
    let mut plain = full_model;
    plain.zonal = None;
    plain.drag = None;
    plain.radiation = None;
    plain.third_body = false;
    plain.burn = None;
    let mut state = initial;
    for impulse in impulses {
        state = plain
            .propagate_with_burns(state, impulse.time, options, &[], &mut environment)?
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
    state = plain
        .propagate_with_burns(state, end, options, &[], &mut environment)?
        .sample(end)
        .map_err(|_| IntegrationError::NumericalRange)?;
    let outcome = |s: crate::spline::Keyframe| -> Result<Outcome, IntegrationError> {
        Ok(Outcome {
            position_m: s.position,
            metric: metric(s)?,
        })
    };
    let impulsive = outcome(state)?;
    let mut run = |model: crate::force_model::ForceModel| -> Result<Outcome, IntegrationError> {
        let state = model
            .propagate_with_burns(initial, end, options, burns, &mut environment)?
            .sample(end)
            .map_err(|_| IntegrationError::NumericalRange)?;
        outcome(state)
    };
    let finite_only = run(plain)?;
    let full = run(full_model)?;
    let mut without = full_model;
    without.zonal = None;
    let no_j2 = run(without)?;
    without = full_model;
    without.third_body = false;
    let no_third_body = run(without)?;
    without = full_model;
    without.drag = None;
    let no_drag = run(without)?;
    without = full_model;
    without.radiation = None;
    let no_srp = run(without)?;
    Ok(Counterfactuals {
        impulsive,
        finite_only,
        full,
        no_j2,
        no_third_body,
        no_drag,
        no_srp,
    })
}

/// Counterfactuals evaluated at a common, physically reached epoch.
/// If any run impacts before `end`, all outcomes are sampled at the earliest
/// impact and `truncated` is true. Callers must not present a truncated target
/// debrief as a successful prediction comparison.
#[derive(Clone, Debug)]
pub struct CheckedCounterfactuals {
    pub counterfactuals: Counterfactuals,
    pub common_end_s: f64,
    pub requested_end_s: f64,
    pub truncated: bool,
}

/// Seven checked propagations with terminal impact handling. The impulsive
/// baseline may be re-propagated once to the common epoch if a finite run
/// impacts earlier; this avoids comparing states at different times.
#[allow(clippy::too_many_arguments)]
pub fn propagate_counterfactuals_checked<F, M>(
    initial: crate::spline::Keyframe,
    end: f64,
    options: crate::integrator::Options,
    full_model: crate::force_model::ForceModel,
    burns: &[crate::perturb::burn::Burn],
    impulses: &[Impulse],
    limits: crate::force_model::FlightLimits,
    mut environment: F,
    metric: M,
) -> Result<CheckedCounterfactuals, crate::integrator::IntegrationError>
where
    F: FnMut(f64) -> Result<crate::force_model::Environment, crate::integrator::IntegrationError>,
    M: Fn(crate::spline::Keyframe) -> Result<f64, crate::integrator::IntegrationError>,
{
    use crate::integrator::IntegrationError;
    if impulses.iter().any(|b| {
        !b.time.is_finite()
            || b.time < initial.time
            || b.time > end
            || !b.propellant_kg.is_finite()
            || b.propellant_kg < 0.0
            || !b.delta_v_m_s.iter().all(|x| x.is_finite())
    }) || impulses.windows(2).any(|w| w[0].time >= w[1].time)
    {
        return Err(IntegrationError::InvalidInput);
    }
    let mut plain = full_model;
    plain.zonal = None;
    plain.drag = None;
    plain.radiation = None;
    plain.third_body = false;
    plain.burn = None;
    let baseline = |until: f64,
                    environment: &mut F|
     -> Result<(crate::spline::Keyframe, Option<f64>), IntegrationError> {
        let mut state = initial;
        for impulse in impulses {
            if impulse.time > until {
                break;
            }
            let integrated = plain.propagate_with_limits(
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
            plain.propagate_with_limits(state, until, options, &[], limits, &mut *environment)?;
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
    };
    let (mut baseline_state, baseline_stop) = baseline(end, &mut environment)?;
    let mut common = baseline_stop.unwrap_or(end);
    let run = |model: crate::force_model::ForceModel,
               environment: &mut F|
     -> Result<(crate::spline::Keyframe, Option<f64>), IntegrationError> {
        let integrated =
            model.propagate_with_limits(initial, end, options, burns, limits, &mut *environment)?;
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
    };
    let mut finite_only = run(plain, &mut environment)?;
    let mut full = run(full_model, &mut environment)?;
    let mut without = full_model;
    without.zonal = None;
    let mut no_j2 = run(without, &mut environment)?;
    without = full_model;
    without.third_body = false;
    let mut no_third_body = run(without, &mut environment)?;
    without = full_model;
    without.drag = None;
    let mut no_drag = run(without, &mut environment)?;
    without = full_model;
    without.radiation = None;
    let mut no_srp = run(without, &mut environment)?;
    for stop in [
        finite_only.1,
        full.1,
        no_j2.1,
        no_third_body.1,
        no_drag.1,
        no_srp.1,
    ]
    .into_iter()
    .flatten()
    {
        common = common.min(stop);
    }
    let truncated = common < end;
    if truncated {
        if baseline_stop.is_none_or(|t| t > common) {
            baseline_state = baseline(common, &mut environment)?.0;
        }
        let resample = |pair: &mut (crate::spline::Keyframe, Option<f64>),
                        model: crate::force_model::ForceModel,
                        environment: &mut F|
         -> Result<(), IntegrationError> {
            if pair.1.is_none_or(|t| t > common) {
                let integrated = model.propagate_with_limits(
                    initial,
                    common,
                    options,
                    burns,
                    limits,
                    &mut *environment,
                )?;
                pair.0 = integrated
                    .trajectory
                    .sample(common)
                    .map_err(|_| IntegrationError::NumericalRange)?;
            }
            Ok(())
        };
        resample(&mut finite_only, plain, &mut environment)?;
        resample(&mut full, full_model, &mut environment)?;
        without = full_model;
        without.zonal = None;
        resample(&mut no_j2, without, &mut environment)?;
        without = full_model;
        without.third_body = false;
        resample(&mut no_third_body, without, &mut environment)?;
        without = full_model;
        without.drag = None;
        resample(&mut no_drag, without, &mut environment)?;
        without = full_model;
        without.radiation = None;
        resample(&mut no_srp, without, &mut environment)?;
    }
    let outcome = |s: crate::spline::Keyframe| -> Result<Outcome, IntegrationError> {
        Ok(Outcome {
            position_m: s.position,
            metric: metric(s)?,
        })
    };
    Ok(CheckedCounterfactuals {
        counterfactuals: Counterfactuals {
            impulsive: outcome(baseline_state)?,
            finite_only: outcome(finite_only.0)?,
            full: outcome(full.0)?,
            no_j2: outcome(no_j2.0)?,
            no_third_body: outcome(no_third_body.0)?,
            no_drag: outcome(no_drag.0)?,
            no_srp: outcome(no_srp.0)?,
        },
        common_end_s: common,
        requested_end_s: end,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metric_specific_shares_and_zero_case() {
        let o = |x, metric| Outcome {
            position_m: [x, 0.0, 0.0],
            metric,
        };
        let cases = Counterfactuals {
            impulsive: o(0.0, 0.0),
            finite_only: o(1.0, 0.0),
            full: o(10.0, 10.0),
            no_j2: o(8.0, 0.5),
            no_third_body: o(7.0, 9.7),
            no_drag: o(6.0, 9.9),
            no_srp: o(5.0, 9.9),
        };
        let out = cases.sensitivities().unwrap();
        assert!((out.position_shares.unwrap().iter().sum::<f64>() - 1.0).abs() < 1e-15);
        assert!(out.metric_shares.unwrap()[1] > 0.9);
        assert!((out.metric_shares.unwrap().iter().sum::<f64>() - 1.0).abs() < 1e-15);
        let zero = Counterfactuals {
            impulsive: o(0.0, 0.0),
            finite_only: o(0.0, 0.0),
            full: o(0.0, 0.0),
            no_j2: o(0.0, 0.0),
            no_third_body: o(0.0, 0.0),
            no_drag: o(0.0, 0.0),
            no_srp: o(0.0, 0.0),
        }
        .sensitivities()
        .unwrap();
        assert_eq!(zero.position_shares, None);
        assert_eq!(zero.metric_shares, None);
    }
}
