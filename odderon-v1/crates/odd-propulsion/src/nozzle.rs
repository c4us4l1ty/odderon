//! Quasi-one-dimensional isentropic nozzle with Summerfield separation.
//!
//! The flow is treated as a perfect gas with the chamber's frozen `γ`, which
//! is the standard preliminary-design model. Everything here is closed form
//! or a bracketed iteration with a fixed convergence rule, so the same
//! `EngineSpec` produces the same bits natively and in wasm32.
use crate::thermochem::{ChamberThermo, G0};
use crate::PropulsionError;

/// Reference conical half-angle a "100% length" bell replaces.
const REFERENCE_CONE_RAD: f64 = 0.261_799_387_799_149_4; // 15 degrees
/// Rao parabolic bells start steep. The opening angle is capped so the wall
/// stays monotonic for the requested length.
const MAX_INITIAL_WALL_RAD: f64 = 0.523_598_775_598_298_8; // 30 degrees
/// A parabola's momentum-averaged exit angle is well below its mean cone
/// angle; 0.6 reproduces Rao's divergence-efficiency tables for 60–100% bells.
const BELL_ANGLE_FACTOR: f64 = 0.6;
/// Summerfield criterion: the wall flow separates when the local static
/// pressure falls below roughly 0.38 of ambient.
///
/// Summerfield's data is from **conical** nozzles. A parabolic bell turns the
/// flow back toward axial, and its boundary layer negotiates a much larger
/// pressure rise before detaching: an RS-25 (`Pe/Pamb = 0.22`) runs full at
/// sea level even though the bare criterion calls it separated. The
/// threshold is therefore scaled by the local wall angle relative to the 15°
/// reference cone, capped at 1. In the steep part of a bell — where
/// separation actually happens — the factor is 1 and this is exactly
/// `0.38 Pamb`; only near-axial exit sections get the relief.
pub const SUMMERFIELD_RATIO: f64 = 0.38;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NozzleGeometry {
    pub r_throat_m: f64,
    pub r_exit_m: f64,
    /// Bell length as a fraction of the equivalent 15° cone.
    pub length_fraction: f64,
}

impl NozzleGeometry {
    pub fn validate(self) -> Result<(), PropulsionError> {
        if !self.r_throat_m.is_finite() || !(0.005..=1.0).contains(&self.r_throat_m) {
            return Err(PropulsionError::GeometryOutOfRange);
        }
        if !self.r_exit_m.is_finite() || self.r_exit_m < self.r_throat_m {
            return Err(PropulsionError::GeometryOutOfRange);
        }
        if !self.length_fraction.is_finite() || !(0.4..=1.2).contains(&self.length_fraction) {
            return Err(PropulsionError::GeometryOutOfRange);
        }
        let ratio = self.expansion_ratio();
        if !(1.0..=250.0).contains(&ratio) || !ratio.is_finite() {
            return Err(PropulsionError::GeometryOutOfRange);
        }
        Ok(())
    }
    pub fn expansion_ratio(self) -> f64 {
        (self.r_exit_m / self.r_throat_m) * (self.r_exit_m / self.r_throat_m)
    }
    pub fn throat_area_m2(self) -> f64 {
        core::f64::consts::PI * self.r_throat_m * self.r_throat_m
    }
    pub fn exit_area_m2(self) -> f64 {
        self.expansion_ratio() * self.throat_area_m2()
    }
    pub fn length_m(self) -> f64 {
        self.length_fraction * (self.r_exit_m - self.r_throat_m) / libm::tan(REFERENCE_CONE_RAD)
    }
    /// Normalized initial slope of the parabolic wall, capped so `radius_at`
    /// is monotonic on [0, 1] for every legal length fraction.
    fn wall_slope(self) -> f64 {
        let rise = self.r_exit_m - self.r_throat_m;
        if rise <= 0.0 {
            return 1.0;
        }
        let wanted = self.length_m() * libm::tan(MAX_INITIAL_WALL_RAD) / rise;
        libm::fmin(wanted, 1.9)
    }
    /// Wall radius at normalized axial station `x/L`.
    pub fn radius_at(self, x_norm: f64) -> f64 {
        let a = self.wall_slope();
        let s = x_norm.clamp(0.0, 1.0);
        self.r_throat_m + (self.r_exit_m - self.r_throat_m) * (a * s + (1.0 - a) * s * s)
    }
    /// Inverse of `radius_at`, by the stable quadratic root.
    pub fn station_for_radius(self, radius_m: f64) -> f64 {
        let rise = self.r_exit_m - self.r_throat_m;
        if rise <= 0.0 {
            return 0.0;
        }
        let q = ((radius_m - self.r_throat_m) / rise).clamp(0.0, 1.0);
        let a = self.wall_slope();
        let disc = a * a + 4.0 * (1.0 - a) * q;
        if disc <= 0.0 {
            return 1.0;
        }
        (2.0 * q / (a + libm::sqrt(disc))).clamp(0.0, 1.0)
    }
    /// Local wall half-angle at a normalized station, radians.
    pub fn wall_angle_rad(self, x_norm: f64) -> f64 {
        let rise = self.r_exit_m - self.r_throat_m;
        let length = self.length_m();
        if length <= 0.0 || rise <= 0.0 {
            return 0.0;
        }
        let a = self.wall_slope();
        let s = x_norm.clamp(0.0, 1.0);
        libm::atan((rise / length) * (a + 2.0 * (1.0 - a) * s))
    }
    /// Rao bell divergence factor: the axial fraction of exit momentum.
    pub fn divergence_factor(self) -> f64 {
        let rise = self.r_exit_m - self.r_throat_m;
        let length = self.length_m();
        if length <= 0.0 || rise <= 0.0 {
            return 1.0;
        }
        let effective = BELL_ANGLE_FACTOR * libm::atan(rise / length);
        0.5 * (1.0 + libm::cos(effective))
    }
}

/// Supersonic area–Mach relation, `A/A*` for `M >= 1`.
pub fn area_ratio(mach: f64, gamma: f64) -> f64 {
    let term = (2.0 / (gamma + 1.0)) * (1.0 + 0.5 * (gamma - 1.0) * mach * mach);
    libm::pow(term, (gamma + 1.0) / (2.0 * (gamma - 1.0))) / mach
}

/// Invert the supersonic branch by damped Newton iteration with the analytic
/// derivative, falling back to bisection whenever a step leaves the bracket.
/// Converges to `|ΔM| < 1e-12`; the iteration count is bounded so a bad input
/// cannot spin.
pub fn mach_at_area(target: f64, gamma: f64) -> Result<f64, PropulsionError> {
    if !target.is_finite() || target < 1.0 || !(1.0..2.0).contains(&gamma) {
        return Err(PropulsionError::NumericalRange);
    }
    if target == 1.0 {
        return Ok(1.0);
    }
    let b = 0.5 * (gamma - 1.0);
    let e = (gamma + 1.0) / (2.0 * (gamma - 1.0));
    let mut lo = 1.0;
    let mut hi = 80.0;
    let mut mach = 2.0;
    for _ in 0..120 {
        let area = area_ratio(mach, gamma);
        let residual = area - target;
        if residual > 0.0 {
            hi = mach;
        } else {
            lo = mach;
        }
        let dlog = -1.0 / mach + e * (2.0 * b * mach) / (1.0 + b * mach * mach);
        let slope = area * dlog;
        let mut next = if slope > 0.0 {
            mach - residual / slope
        } else {
            f64::NAN
        };
        if !next.is_finite() || next <= lo || next >= hi {
            next = 0.5 * (lo + hi);
        }
        let step = libm::fabs(next - mach);
        mach = next;
        if step < 1e-13 {
            break;
        }
    }
    if !mach.is_finite() || mach < 1.0 {
        return Err(PropulsionError::NumericalRange);
    }
    Ok(mach)
}

/// One axial station of the isentropic solution.
#[derive(Clone, Copy, Debug)]
pub struct Station {
    pub x_norm: f64,
    pub radius_m: f64,
    pub area_ratio: f64,
    pub mach: f64,
    pub pressure_pa: f64,
    pub temperature_k: f64,
    pub density_kg_m3: f64,
    pub velocity_m_s: f64,
}

pub fn state_at_mach(mach: f64, thermo: &ChamberThermo) -> (f64, f64, f64, f64) {
    let b = 0.5 * (thermo.gamma - 1.0);
    let ratio = 1.0 / (1.0 + b * mach * mach);
    let temperature = thermo.temperature_k * ratio;
    let pressure = thermo.p_c_pa * libm::pow(ratio, thermo.gamma / (thermo.gamma - 1.0));
    let density = pressure / (thermo.gas_constant_j_kg_k * temperature);
    let velocity = mach * libm::sqrt(thermo.gamma * thermo.gas_constant_j_kg_k * temperature);
    (pressure, temperature, density, velocity)
}

/// Sample the divergent section for the CAD/Schlieren view. Station 0 is the
/// throat, the last station is the exit plane.
pub fn stations(
    geometry: NozzleGeometry,
    thermo: &ChamberThermo,
    count: usize,
) -> Result<Vec<Station>, PropulsionError> {
    geometry.validate()?;
    let count = count.clamp(2, 512);
    let mut out = Vec::with_capacity(count);
    for index in 0..count {
        let x_norm = index as f64 / (count - 1) as f64;
        let radius_m = geometry.radius_at(x_norm);
        let ratio = (radius_m / geometry.r_throat_m) * (radius_m / geometry.r_throat_m);
        let mach = mach_at_area(ratio.max(1.0), thermo.gamma)?;
        let (pressure_pa, temperature_k, density_kg_m3, velocity_m_s) = state_at_mach(mach, thermo);
        out.push(Station {
            x_norm,
            radius_m,
            area_ratio: ratio,
            mach,
            pressure_pa,
            temperature_k,
            density_kg_m3,
            velocity_m_s,
        });
    }
    Ok(out)
}

/// Ideal characteristic velocity `c*` of the chamber gas, m/s.
pub fn ideal_c_star(thermo: &ChamberThermo) -> f64 {
    let g = thermo.gamma;
    let numerator = libm::sqrt(g * thermo.gas_constant_j_kg_k * thermo.temperature_k);
    let denominator = g * libm::pow(2.0 / (g + 1.0), (g + 1.0) / (2.0 * (g - 1.0)));
    numerator / denominator
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlowSeparation {
    /// Axial station of the separation line, normalized by bell length.
    pub x_norm: f64,
    pub area_ratio_sep: f64,
    /// Static side-load estimate, kN. An order-of-magnitude design warning,
    /// not a transient aeroelastic prediction.
    pub side_load_kn: f64,
}

/// Contour-corrected Summerfield wall pressure at which the boundary layer
/// detaches at `x_norm`. Shared by the separation solver and the plume model
/// so the drawn detachment line and the computed one cannot disagree.
pub fn separation_pressure(geometry: NozzleGeometry, x_norm: f64, ambient_pa: f64) -> f64 {
    let angle = geometry.wall_angle_rad(x_norm);
    let relief = libm::fmin(1.0, libm::tan(angle) / libm::tan(REFERENCE_CONE_RAD));
    SUMMERFIELD_RATIO * ambient_pa * relief
}

/// Summerfield separation with the contour correction described above.
/// Returns the detachment line, or `None` when the bell flows full.
pub fn separation(
    geometry: NozzleGeometry,
    thermo: &ChamberThermo,
    ambient_pa: f64,
    _exit_mach: f64,
    exit_pressure_pa: f64,
) -> Option<FlowSeparation> {
    if ambient_pa <= 0.0 {
        return None;
    }
    if exit_pressure_pa >= separation_pressure(geometry, 1.0, ambient_pa) {
        return None;
    }
    // Wall pressure falls monotonically along the divergent section while
    // the threshold can only fall by a factor of a few, so the difference
    // has a single root. Bisect it on the axial coordinate.
    let pressure_at = |x_norm: f64| -> f64 {
        let radius = geometry.radius_at(x_norm);
        let ratio = (radius / geometry.r_throat_m) * (radius / geometry.r_throat_m);
        let mach = mach_at_area(ratio.max(1.0), thermo.gamma).unwrap_or(1.0);
        state_at_mach(mach, thermo).0
    };
    let mut lo = 0.0;
    let mut hi = 1.0;
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if mid <= lo || mid >= hi {
            break;
        }
        if pressure_at(mid) > separation_pressure(geometry, mid, ambient_pa) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let x_norm = 0.5 * (lo + hi);
    let radius = geometry.radius_at(x_norm);
    let area_ratio_sep = (radius / geometry.r_throat_m) * (radius / geometry.r_throat_m);
    let area = area_ratio_sep * geometry.throat_area_m2();
    let side_load_kn = 0.05 * (ambient_pa - pressure_at(x_norm)) * area / 1000.0;
    Some(FlowSeparation {
        x_norm,
        area_ratio_sep,
        side_load_kn,
    })
}

/// Axial thrust and specific impulse at an ambient pressure.
///
/// Attached flow uses the whole bell. Separated flow is truncated at the
/// separation plane: momentum and pressure thrust are evaluated there, and
/// the downstream wall is assumed to recover to ambient and contribute no
/// net axial force. That is the standard engineering treatment and it is why
/// an over-expanded vacuum bell loses thrust at sea level instead of gaining
/// it from its extra area.
pub fn performance(
    geometry: NozzleGeometry,
    thermo: &ChamberThermo,
    mass_flow_kg_s: f64,
    exit: &Station,
    ambient_pa: f64,
    separated: Option<FlowSeparation>,
) -> (f64, f64) {
    let (area, velocity, pressure, divergence) = match separated {
        Some(sep) => {
            let radius = geometry.r_throat_m * libm::sqrt(sep.area_ratio_sep);
            let truncated = NozzleGeometry {
                r_throat_m: geometry.r_throat_m,
                r_exit_m: radius,
                length_fraction: 1.0,
            };
            let mach = area_ratio_inverse(sep.area_ratio_sep, thermo.gamma);
            let (pressure, _t, _d, velocity) = state_at_mach(mach, thermo);
            (
                sep.area_ratio_sep * geometry.throat_area_m2(),
                velocity,
                pressure,
                truncated.divergence_factor(),
            )
        }
        None => (
            geometry.exit_area_m2(),
            exit.velocity_m_s,
            exit.pressure_pa,
            geometry.divergence_factor(),
        ),
    };
    let momentum = thermo.nozzle_efficiency * divergence * mass_flow_kg_s * velocity;
    let thrust = momentum + (pressure - ambient_pa) * area;
    let isp = thrust / (mass_flow_kg_s * G0);
    (thrust, isp)
}

fn area_ratio_inverse(ratio: f64, gamma: f64) -> f64 {
    mach_at_area(ratio.max(1.0), gamma).unwrap_or(1.0)
}

/// US Standard Atmosphere 1976 static pressure, Pa, to 100 km.
///
/// The layer table is the same one `odd-astro`'s drag model uses; a test pins
/// `pressure / (R T)` against that crate's density so the two cannot drift.
pub fn ambient_pressure(altitude_m: f64) -> f64 {
    if !altitude_m.is_finite() || altitude_m <= 0.0 {
        return 101_325.0;
    }
    const H: [f64; 7] = [0.0, 11e3, 20e3, 32e3, 47e3, 51e3, 71e3];
    const T: [f64; 7] = [288.15, 216.65, 216.65, 228.65, 270.65, 270.65, 214.65];
    const P: [f64; 7] = [
        101_325.0, 22_632.06, 5474.889, 868.0187, 110.9063, 66.938_87, 3.956_420,
    ];
    const L: [f64; 7] = [-0.0065, 0.0, 0.001, 0.0028, 0.0, -0.0028, -0.002];
    let height = libm::fmin(altitude_m, 100e3);
    let h = 6_356_766.0 * height / (6_356_766.0 + height);
    let mut i = 0;
    for (j, &base) in H.iter().enumerate().skip(1) {
        if h >= base {
            i = j;
        }
    }
    let dt = h - H[i];
    let temp = T[i] + L[i] * dt;
    let pressure = if L[i] == 0.0 {
        P[i] * libm::exp(-G0 * dt / (287.05287 * T[i]))
    } else {
        P[i] * libm::pow(T[i] / temp, G0 / (287.05287 * L[i]))
    };
    if height >= 100e3 {
        // The table ends at 86 km; above it the residual is already below a
        // hundredth of a pascal and the test stand reads it as vacuum.
        return 0.0;
    }
    pressure
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermochem::{chamber, Propellant};

    fn thermo() -> ChamberThermo {
        chamber(Propellant::LoxLh2, 6.0, 20.64e6).unwrap()
    }

    #[test]
    fn area_mach_round_trips_and_starts_at_the_throat() {
        let t = thermo();
        assert_eq!(mach_at_area(1.0, t.gamma).unwrap(), 1.0);
        for ratio in [1.5, 4.0, 16.0, 69.0, 200.0] {
            let mach = mach_at_area(ratio, t.gamma).unwrap();
            assert!(mach > 1.0);
            assert!(
                libm::fabs(area_ratio(mach, t.gamma) - ratio) < 1e-9,
                "{ratio} -> {mach}"
            );
        }
    }

    #[test]
    fn wall_is_monotonic_and_invertible_for_every_legal_length() {
        for fraction in [0.4, 0.6, 0.8, 1.0, 1.2] {
            let g = NozzleGeometry {
                r_throat_m: 0.05,
                r_exit_m: 0.45,
                length_fraction: fraction,
            };
            let mut previous = g.r_throat_m;
            for index in 0..=100 {
                let x = index as f64 / 100.0;
                let radius = g.radius_at(x);
                assert!(radius >= previous - 1e-12, "{fraction} {x}");
                assert!((g.station_for_radius(radius) - x).abs() < 1e-9);
                previous = radius;
            }
        }
    }

    #[test]
    fn standard_pressure_matches_the_reference_layers() {
        assert!((ambient_pressure(0.0) - 101_325.0).abs() < 1e-9);
        // The table is geopotential; 11 km geometric is 10.98 km geopotential.
        assert!((ambient_pressure(11_000.0) - 22_632.06).abs() < 100.0);
        assert!(ambient_pressure(35_000.0) < 700.0);
        assert_eq!(ambient_pressure(100_000.0), 0.0);
    }
}
