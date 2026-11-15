//! The test-stand pressure model and the flight drag model must describe the
//! same atmosphere. They are separate implementations on purpose (the
//! propulsion crate has no dependency on the astrodynamics crate), so this
//! test is what keeps them from drifting apart.
#[test]
fn pressure_over_rt_matches_the_drag_density() {
    for step in 0..=100 {
        let altitude = step as f64 * 800.0;
        let pressure = odd_propulsion::nozzle::ambient_pressure(altitude);
        let density = odd_astro::perturb::drag::earth_density(altitude).unwrap();
        // Recover temperature from the same standard layers the drag model
        // uses, then compare rho = p / (R T).
        let h = 6_356_766.0 * altitude / (6_356_766.0 + altitude);
        let temperature = if h < 11e3 {
            288.15 - 0.0065 * h
        } else if h < 20e3 {
            216.65
        } else if h < 32e3 {
            216.65 + 0.001 * (h - 20e3)
        } else if h < 47e3 {
            228.65 + 0.0028 * (h - 32e3)
        } else if h < 51e3 {
            270.65
        } else if h < 71e3 {
            270.65 - 0.0028 * (h - 51e3)
        } else {
            214.65 - 0.002 * (h - 71e3)
        };
        let implied = pressure / (287.05287 * temperature);
        let relative = (implied - density).abs() / density.max(1e-30);
        assert!(relative < 1e-12, "{altitude} m: {implied} vs {density}");
    }
}
