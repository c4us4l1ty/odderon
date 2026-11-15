//! Names and units for the 58-field debrief report.
//!
//! The order is fixed by odd-view's REPORT_LEN contract and is part of the
//! signed payload, so a field index can never be silently repurposed: the
//! schema version would have to change. Undefined values are NaN.

pub const NAMES: [(&str, &str); 58] = [
    ("Final spacecraft mass", "kg"),
    ("Apoapsis altitude", "m"),
    ("Periapsis altitude", "m"),
    ("Eccentricity", ""),
    ("Mission score", ""),
    ("Commanded delta-v", "m/s"),
    ("Finite-burn share", ""),
    ("J2 share", ""),
    ("Sun and Moon share", ""),
    ("Drag and SRP share", ""),
    ("Finite-burn contribution", "metric"),
    ("J2 contribution", "metric"),
    ("Sun and Moon contribution", "metric"),
    ("Drag and SRP contribution", "metric"),
    ("Total attribution error", "metric"),
    ("Reference metric", "metric"),
    ("Full-physics metric", "metric"),
    ("Efficiency residual", "metric"),
    ("Common attribution epoch", "s"),
    ("Attribution truncated", ""),
    ("Ideal delta-v", "m/s"),
    ("Steering loss", "m/s"),
    ("Gravity loss", "m/s"),
    ("Speed deficit", "m/s"),
    ("Identity residual", "m/s"),
    ("Burn duration", "s"),
    ("Inclination", "rad"),
    ("Right ascension of the ascending node", "rad"),
    ("Node-rotation metric", "rad"),
    ("Evaluation epoch", "s"),
    ("Impacted", ""),
    ("Atmospheric entry epoch", "s"),
    ("Lunar sphere-of-influence entry", "s"),
    ("Lunar sphere-of-influence radius", "m"),
    ("Prediction subscore", ""),
    ("Objective subscore", ""),
    ("Efficiency subscore", ""),
    ("Common attribution epoch (duplicate)", "s"),
    ("Attribution truncated (duplicate)", ""),
    ("Reserved", ""),
    ("Target radius miss", "m"),
    ("Coalition 0000 metric", "metric"),
    ("Coalition 0001 metric", "metric"),
    ("Coalition 0010 metric", "metric"),
    ("Coalition 0011 metric", "metric"),
    ("Coalition 0100 metric", "metric"),
    ("Coalition 0101 metric", "metric"),
    ("Coalition 0110 metric", "metric"),
    ("Coalition 0111 metric", "metric"),
    ("Coalition 1000 metric", "metric"),
    ("Coalition 1001 metric", "metric"),
    ("Coalition 1010 metric", "metric"),
    ("Coalition 1011 metric", "metric"),
    ("Coalition 1100 metric", "metric"),
    ("Coalition 1101 metric", "metric"),
    ("Coalition 1110 metric", "metric"),
    ("Coalition 1111 metric", "metric"),
    ("Burn sweep angle", "rad"),
];

/// Shapley player names in bit order (bit 0 first).
pub const PLAYERS: [&str; 4] = [
    "Finite burn",
    "Earth oblateness (J2)",
    "Sun and Moon gravity",
    "Drag and solar radiation pressure",
];

/// Coalition label from its bitmask, e.g. mask 5 -> "finite + third body".
pub fn coalition_label(mask: usize) -> String {
    const SHORT: [&str; 4] = ["finite", "J2", "third body", "drag+SRP"];
    let parts: Vec<&str> = (0..4)
        .filter(|bit| mask & (1 << bit) != 0)
        .map(|bit| SHORT[bit])
        .collect();
    if parts.is_empty() {
        "none (two-body impulsive)".to_string()
    } else {
        parts.join(" + ")
    }
}

pub fn name(index: usize) -> &'static str {
    NAMES.get(index).map_or("Unknown field", |entry| entry.0)
}

pub fn unit(index: usize) -> &'static str {
    NAMES.get(index).map_or("", |entry| entry.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_report_field_is_named() {
        assert_eq!(NAMES.len(), 58);
        for (index, (name, _)) in NAMES.iter().enumerate() {
            assert!(!name.is_empty(), "field {index}");
            assert_ne!(*name, "Unknown field");
        }
        assert_eq!(coalition_label(0), "none (two-body impulsive)");
        assert_eq!(coalition_label(5), "finite + third body");
        assert_eq!(coalition_label(15), "finite + J2 + third body + drag+SRP");
    }
}
