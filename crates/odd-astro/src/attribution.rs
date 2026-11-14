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
