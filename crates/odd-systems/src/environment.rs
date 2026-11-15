//! The radiative environment a tank actually sees, from real geometry.
//!
//! Nothing here is averaged over an orbit. Solar flux uses the instantaneous
//! heliocentric distance and the **same** conical-shadow illumination function
//! the solar-radiation-pressure perturbation uses, so a tank in eclipse and a
//! spacecraft losing SRP agree by construction. Earth IR uses the visible
//! solid angle of a 255 K disc from the tank's current position.

use odd_astro::perturb::srp;
use odd_core::consts::{ASTRONOMICAL_UNIT_M, STEFAN_BOLTZMANN_W_M2_K4};

/// Total solar irradiance at 1 AU, W/m² (SORCE/TIM consensus, Kopp & Lean 2011).
pub const SOLAR_CONSTANT_W_M2: f64 = 1_361.0;
/// Earth's effective radiating temperature, K (planetary energy balance).
pub const EARTH_IR_TEMPERATURE_K: f64 = 255.0;
/// Earth's mean radius for the IR disc, m.
pub const EARTH_RADIUS_M: f64 = 6_371_000.0;
/// Solar radius, m (IAU 2015 nominal).
pub const SUN_RADIUS_M: f64 = 695_700_000.0;
/// Deep-space sink temperature, K.
pub const SPACE_TEMPERATURE_K: f64 = 3.0;

/// Where the tank is, relative to the bodies that heat it. All vectors are
/// **from the tank** to the body, in metres, in one inertial frame.
#[derive(Clone, Copy, Debug)]
pub struct ThermalEnvironment {
    /// Tank-to-Sun vector, m.
    pub sun: [f64; 3],
    /// Tank-to-Earth vector, m. Zero when the tank is not near Earth.
    pub earth: [f64; 3],
    /// Tank-to-Moon vector, m; used only as a second occulter.
    pub moon: [f64; 3],
}

fn norm(v: [f64; 3]) -> f64 {
    libm::sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2])
}

impl ThermalEnvironment {
    /// Fraction of the solar disc visible, in `[0, 1]`. Earth and Moon shadows
    /// are unioned by the shared two-occulter helper, never double-counted.
    pub fn illumination(&self) -> f64 {
        let earth = norm(self.earth);
        let moon = norm(self.moon);
        // A zero vector means that body is not part of this scenario at all
        // (deep space), which is full sun. A non-zero vector inside the body
        // is undefined geometry, and the conservative answer is shadow.
        let earth_present = earth > 0.0;
        let moon_present = moon > 0.0;
        if (earth_present && earth <= EARTH_RADIUS_M) || (moon_present && moon <= 1_737_400.0) {
            return 0.0;
        }
        let result = if earth_present && moon_present {
            srp::illumination_two(
                self.sun,
                SUN_RADIUS_M,
                self.earth,
                EARTH_RADIUS_M,
                self.moon,
                1_737_400.0,
            )
        } else if earth_present {
            srp::illumination(self.sun, SUN_RADIUS_M, self.earth, EARTH_RADIUS_M)
        } else if moon_present {
            srp::illumination(self.sun, SUN_RADIUS_M, self.moon, 1_737_400.0)
        } else {
            Ok(1.0)
        };
        result.unwrap_or(0.0).clamp(0.0, 1.0)
    }

    /// Solar irradiance at the tank's heliocentric distance, W/m², before
    /// shadowing. Falls off as `1/r²` from the 1 AU reference.
    pub fn solar_flux_w_m2(&self) -> f64 {
        let d = norm(self.sun);
        if !d.is_finite() || d <= 0.0 {
            return 0.0;
        }
        let ratio = ASTRONOMICAL_UNIT_M / d;
        SOLAR_CONSTANT_W_M2 * ratio * ratio
    }

    /// Earth IR irradiance on a surface facing the planet, W/m².
    ///
    /// The planet radiates `σT⁴` from its surface; at distance `d` the disc
    /// subtends a half-angle `asin(R/d)`, and the flux received by a flat
    /// element normal to the line of sight is `σT⁴·sin²(α)`. That closed form
    /// is the exact view factor for a sphere-to-differential-element, so no
    /// solid-angle quadrature is needed.
    pub fn earth_ir_flux_w_m2(&self) -> f64 {
        let d = norm(self.earth);
        if !d.is_finite() || d <= EARTH_RADIUS_M {
            return 0.0;
        }
        let sin_alpha = EARTH_RADIUS_M / d;
        let emitted = STEFAN_BOLTZMANN_W_M2_K4 * EARTH_IR_TEMPERATURE_K.powi(4);
        emitted * sin_alpha * sin_alpha
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solar_flux_is_the_solar_constant_at_one_au() {
        let env = ThermalEnvironment {
            sun: [ASTRONOMICAL_UNIT_M, 0.0, 0.0],
            earth: [0.0; 3],
            moon: [0.0; 3],
        };
        assert!((env.solar_flux_w_m2() - 1_361.0).abs() < 1e-9);
        // Inverse square: twice the distance, a quarter the flux.
        let far = ThermalEnvironment {
            sun: [2.0 * ASTRONOMICAL_UNIT_M, 0.0, 0.0],
            ..env
        };
        assert!((far.solar_flux_w_m2() - 1_361.0 / 4.0).abs() < 1e-9);
    }

    #[test]
    fn earth_ir_matches_the_view_factor_at_leo_and_falls_off() {
        // 400 km altitude, looking straight down.
        let d = EARTH_RADIUS_M + 400_000.0;
        let env = ThermalEnvironment {
            sun: [ASTRONOMICAL_UNIT_M, 0.0, 0.0],
            earth: [d, 0.0, 0.0],
            moon: [0.0; 3],
        };
        let sin_alpha = EARTH_RADIUS_M / d;
        let expected = STEFAN_BOLTZMANN_W_M2_K4 * 255.0_f64.powi(4) * sin_alpha * sin_alpha;
        assert!((env.earth_ir_flux_w_m2() - expected).abs() < 1e-9);
        // Roughly 216 W/m² at 400 km — the published planet-shine order.
        assert!((200.0..240.0).contains(&expected), "{expected}");
        let far = ThermalEnvironment {
            earth: [10.0 * d, 0.0, 0.0],
            ..env
        };
        assert!(far.earth_ir_flux_w_m2() < expected / 90.0);
    }

    #[test]
    fn eclipse_removes_the_sun_and_leaving_it_restores_the_flux() {
        // Sun along +x, Earth directly between tank and Sun.
        let shadowed = ThermalEnvironment {
            sun: [ASTRONOMICAL_UNIT_M, 0.0, 0.0],
            earth: [EARTH_RADIUS_M + 400_000.0, 0.0, 0.0],
            moon: [0.0; 3],
        };
        assert!(shadowed.illumination() < 1e-9, "must be in umbra");
        // Same orbit, a quarter turn later: Earth is off to the side.
        let lit = ThermalEnvironment {
            earth: [0.0, EARTH_RADIUS_M + 400_000.0, 0.0],
            ..shadowed
        };
        assert!(
            (lit.illumination() - 1.0).abs() < 1e-9,
            "must be in full sun"
        );
    }
}
