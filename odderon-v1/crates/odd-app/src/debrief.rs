//! Prediction-vs-reality debrief domain model.
//! SI units internally; the browser formats display conversions.

use odd_astro::shapley::{ShapleyAttribution, PLAYERS, PLAYER_NAMES};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MetricComparison {
    pub label: &'static str,
    pub predicted: f64,
    pub actual: f64,
}

impl MetricComparison {
    pub fn error(&self) -> f64 {
        self.actual - self.predicted
    }
}

#[derive(Clone, Debug)]
pub struct AttributionRow {
    pub name: &'static str,
    pub share: Option<f64>,
    pub signed_contribution: f64,
    pub explanation: &'static str,
}

pub fn attribution_rows(s: &ShapleyAttribution) -> Vec<AttributionRow> {
    let explanations = [
        "Thrust was spread over time instead of applied at a single point; steering and gravity losses reduced the useful speed change.",
        "Earth's equatorial bulge twisted the orbital plane and apsides away from the point-mass prediction.",
        "Moon and Sun gravity pulled the spacecraft off the Earth-only conic.",
        "Atmospheric drag removed energy and sunlight pressure pushed on the spacecraft area.",
    ];
    (0..PLAYERS)
        .map(|i| AttributionRow {
            name: PLAYER_NAMES[i],
            share: s.shares.map(|sh| sh[i]),
            signed_contribution: s.contributions[i],
            explanation: explanations[i],
        })
        .collect()
}

/// Standard Elo update with a fixed K factor. Returns (player, mission).
pub fn elo_update(
    player: f64,
    mission: f64,
    score: f64,
    k_player: f64,
    k_mission: f64,
) -> (f64, f64) {
    let expected = 1.0 / (1.0 + libm::pow(10.0, (mission - player) / 400.0));
    (
        player + k_player * (score - expected),
        mission + k_mission * (expected - score),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elo_matches_hand_calculation() {
        let (p, m) = elo_update(1420.0, 1500.0, 1.0, 40.0, 8.0);
        let e = 1.0 / (1.0 + 10f64.powf((1500.0 - 1420.0) / 400.0));
        assert!((p - (1420.0 + 40.0 * (1.0 - e))).abs() < 1e-12);
        assert!((m - (1500.0 + 8.0 * (e - 1.0))).abs() < 1e-12);
    }

    #[test]
    fn comparison_error_is_signed() {
        let c = MetricComparison {
            label: "apoapsis",
            predicted: 100.0,
            actual: 80.0,
        };
        assert_eq!(c.error(), -20.0);
    }
}
