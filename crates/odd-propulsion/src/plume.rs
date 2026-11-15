//! Free-jet structure and the Schlieren density field.
//!
//! This is a reduced-order reconstruction, not a CFD solution. It places the
//! features a Schlieren photograph of a rocket jet actually shows — the lip
//! expansion fan, the shock-cell train, the Mach disk of a strongly
//! underexpanded jet, and the detachment line inside an overexpanded bell —
//! using closed-form gas dynamics, then samples a continuous density field so
//! the browser can paint `|grad rho|` without inventing any physics.
//!
//! Sources for the two empirical constants, both standard and both cited in
//! `docs/propulsion.md`:
//! * Prandtl–Pack shock-cell spacing `L = 1.306 D sqrt(M^2 - 1)`.
//! * Ashkenas–Sherman Mach-disk standoff `x_M / D = 0.67 sqrt(Pe / Pamb)`.
use crate::nozzle::{self, NozzleGeometry};
use crate::thermochem::ChamberThermo;
use crate::{EngineSolution, EngineSpec, FlowSeparation};
use libm::{atan, cos, exp, fabs, pow, sqrt, tan};

/// Prandtl–Pack first shock-cell length coefficient.
const PACK_COEFFICIENT: f64 = 1.306;
/// Ashkenas–Sherman Mach-disk standoff coefficient.
const MACH_DISK_COEFFICIENT: f64 = 0.67;
/// Axial mixing length in jet radii. Beyond a few radii the core is entrained.
const MIXING_RADII: f64 = 14.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlumeRegime {
    /// Exit pressure matches ambient: no fan, no cells.
    Adapted,
    /// Exit pressure above ambient: the lip fans outward.
    Underexpanded,
    /// Exit pressure below ambient but the bell still flows full.
    Overexpanded,
    /// Summerfield separation: the jet detaches inside the bell.
    Separated,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plume {
    pub regime: PlumeRegime,
    pub ambient_pa: f64,
    pub pressure_ratio: f64,
    /// Prandtl–Meyer expansion half-angle at the lip, radians. Positive only
    /// for an underexpanded jet; an overexpanded or detached jet is compressed
    /// instead, and carries `lip_shock_rad`.
    pub lip_turn_rad: f64,
    /// Oblique-shock wave angle at the lip, radians. NaN when there is no
    /// lip shock. The detachment line inside a separated bell uses the same
    /// relation, measured from the local Mach there.
    pub lip_shock_rad: f64,
    /// Inward flow deflection behind that shock, radians. Zero without one.
    pub lip_deflection_rad: f64,
    /// Fully expanded jet Mach number after the lip fan.
    pub jet_mach: f64,
    /// First shock-cell length, m. Zero when adapted.
    pub cell_length_m: f64,
    /// Mach-disk standoff from the exit, m. NaN unless strongly underexpanded.
    pub mach_disk_m: f64,
    /// Axial start of the free jet, m from the throat.
    pub origin_x_m: f64,
    /// Jet radius at that origin, m.
    pub origin_radius_m: f64,
    /// Static pressure at the origin, Pa.
    pub origin_pressure_pa: f64,
    pub origin_density_kg_m3: f64,
    pub ambient_density_kg_m3: f64,
    geometry: NozzleGeometry,
    gas_constant: f64,
    gamma: f64,
    total_temperature_k: f64,
    chamber_density_kg_m3: f64,
}

/// Prandtl–Meyer function, radians.
pub fn prandtl_meyer(mach: f64, gamma: f64) -> f64 {
    if mach <= 1.0 {
        return 0.0;
    }
    let root = sqrt(mach * mach - 1.0);
    let k = sqrt((gamma + 1.0) / (gamma - 1.0));
    k * atan(root / k) - atan(root)
}

/// Invert the Prandtl–Meyer function by bisection. Monotone in Mach.
pub fn mach_from_prandtl_meyer(nu: f64, gamma: f64) -> f64 {
    let ceiling = prandtl_meyer(1.0e4, gamma);
    if nu <= 0.0 || nu.is_nan() {
        return 1.0;
    }
    if nu >= ceiling {
        return 1.0e4;
    }
    let (mut lo, mut hi) = (1.0, 100.0);
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if prandtl_meyer(mid, gamma) < nu {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Reconstruct the free jet leaving a solved engine.
pub fn analyze(
    spec: &EngineSpec,
    solution: &EngineSolution,
    thermo: &ChamberThermo,
    ambient_pa: f64,
) -> Plume {
    let geometry = spec.geometry();
    let gamma = thermo.gamma;
    let gas_constant = thermo.gas_constant_j_kg_k;
    let ambient = if ambient_pa > 0.0 { ambient_pa } else { 0.0 };
    // Outside the atmosphere there is nothing to refract against; the jet
    // expands freely and the Schlieren shows only the core boundary.
    let reference = if ambient > 1.0 { ambient } else { 1.0 };
    let ratio = solution.exit_pressure_pa / reference;
    let separation: Option<FlowSeparation> = solution.separation;
    let (origin_x_m, origin_radius_m, origin_pressure_pa) = match separation {
        Some(sep) => (
            sep.x_norm * geometry.length_m(),
            geometry.radius_at(sep.x_norm),
            // The detached shear layer sits near the Summerfield pressure.
            nozzle::separation_pressure(geometry, sep.x_norm, ambient),
        ),
        None => (
            geometry.length_m(),
            geometry.r_exit_m,
            solution.exit_pressure_pa,
        ),
    };
    let origin_mach = if separation.is_some() {
        nozzle::mach_at_area(sep_area(geometry, origin_radius_m), gamma).unwrap_or(1.0)
    } else {
        solution.exit_mach
    };
    let regime = if separation.is_some() {
        PlumeRegime::Separated
    } else if ambient <= 1.0 || fabs(ratio - 1.0) < 0.02 {
        PlumeRegime::Adapted
    } else if ratio > 1.0 {
        PlumeRegime::Underexpanded
    } else {
        PlumeRegime::Overexpanded
    };
    // Expansion to ambient through the lip fan. For an overexpanded jet the
    // same relation gives the (smaller) Mach behind the oblique lip shock.
    let jet_mach = if regime == PlumeRegime::Adapted {
        origin_mach
    } else {
        let b = 0.5 * (gamma - 1.0);
        let total = origin_pressure_pa / reference;
        let squared = (2.0 / (gamma - 1.0)) * (pow(total, b / gamma) - 1.0);
        sqrt(squared.max(0.0) + origin_mach * origin_mach).max(1.0)
    };
    let expanding = regime == PlumeRegime::Underexpanded;
    let lip_turn_rad = if expanding {
        (prandtl_meyer(jet_mach, gamma) - prandtl_meyer(origin_mach, gamma)).max(0.0)
    } else {
        0.0
    };
    // Overexpanded or detached: ambient compresses the jet through an oblique
    // shock instead of a fan. Solve the pressure ratio for the wave angle.
    let (lip_shock_rad, lip_deflection_rad) =
        if !expanding && regime != PlumeRegime::Adapted && origin_mach > 1.0 {
            oblique_shock(origin_mach, gamma, reference / origin_pressure_pa.max(1e-9))
                .unwrap_or((f64::NAN, 0.0))
        } else {
            (f64::NAN, 0.0)
        };
    let cell_length_m = if regime == PlumeRegime::Adapted {
        0.0
    } else {
        PACK_COEFFICIENT * 2.0 * origin_radius_m * sqrt((jet_mach * jet_mach - 1.0).max(0.0))
    };
    // A Mach disk only forms once the jet is strongly underexpanded; below
    // about two-to-one the cell train stays a regular reflection pattern.
    let mach_disk_m = if regime == PlumeRegime::Underexpanded && ratio > 2.0 {
        MACH_DISK_COEFFICIENT * 2.0 * origin_radius_m * sqrt(ratio)
    } else {
        f64::NAN
    };
    let origin_temperature =
        thermo.temperature_k / (1.0 + 0.5 * (gamma - 1.0) * origin_mach * origin_mach);
    Plume {
        regime,
        ambient_pa: ambient,
        pressure_ratio: ratio,
        lip_turn_rad,
        lip_shock_rad,
        lip_deflection_rad,
        jet_mach,
        cell_length_m,
        mach_disk_m,
        origin_x_m,
        origin_radius_m,
        origin_pressure_pa,
        origin_density_kg_m3: origin_pressure_pa / (gas_constant * origin_temperature),
        // Ambient air, not exhaust gas: 287.05 J/kg/K at the local static
        // temperature the atmosphere model implies.
        ambient_density_kg_m3: ambient / (287.05287 * ambient_temperature(ambient)),
        geometry,
        gas_constant,
        gamma,
        total_temperature_k: thermo.temperature_k,
        chamber_density_kg_m3: thermo.p_c_pa / (gas_constant * thermo.temperature_k),
    }
}

fn sep_area(geometry: NozzleGeometry, radius_m: f64) -> f64 {
    let ratio = radius_m / geometry.r_throat_m;
    (ratio * ratio).max(1.0)
}

/// Oblique shock that raises static pressure by `pressure_ratio`. Returns the
/// wave angle beta and the flow deflection theta, both radians, or `None` when
/// the required compression is beyond a detached shock.
pub fn oblique_shock(mach: f64, gamma: f64, pressure_ratio: f64) -> Option<(f64, f64)> {
    if mach <= 1.0 || pressure_ratio <= 1.0 || mach.is_nan() || pressure_ratio.is_nan() {
        return None;
    }
    let ratio_at = |beta: f64| {
        let normal = mach * libm::sin(beta);
        1.0 + 2.0 * gamma / (gamma + 1.0) * (normal * normal - 1.0)
    };
    if ratio_at(core::f64::consts::FRAC_PI_2) < pressure_ratio {
        return None; // Normal shock cannot do it: the shock detaches.
    }
    let (mut lo, mut hi) = (libm::asin(1.0 / mach), core::f64::consts::FRAC_PI_2);
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if ratio_at(mid) < pressure_ratio {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let beta = 0.5 * (lo + hi);
    let m2 = mach * mach;
    let numerator = 2.0 * (m2 * libm::sin(beta) * libm::sin(beta) - 1.0) / libm::tan(beta);
    let denominator = m2 * (gamma + libm::cos(2.0 * beta)) + 2.0;
    Some((beta, libm::atan(numerator / denominator)))
}

/// Ambient static temperature consistent with the pressure layers, K.
fn ambient_temperature(pressure_pa: f64) -> f64 {
    // Invert the pressure ladder coarsely; the Schlieren background only needs
    // the right order of magnitude for air density contrast.
    let ladder = [
        (101_325.0, 288.15),
        (22_632.0, 216.65),
        (5_474.9, 216.65),
        (868.0, 228.65),
        (110.9, 270.65),
        (66.9, 270.65),
        (3.96, 214.65),
    ];
    let mut temperature = 190.0;
    for (p, t) in ladder {
        if pressure_pa <= p {
            temperature = t;
        }
    }
    temperature
}

impl Plume {
    /// Density at an axial/radial point, kg/m^3. `x` is metres from the throat
    /// and `r` metres off the axis. Continuous except across the features a
    /// Schlieren image is meant to reveal.
    pub fn density_at(&self, x: f64, r: f64) -> f64 {
        let r = fabs(r);
        if x <= 0.0 {
            // Chamber: stagnant gas up to the contraction wall.
            let chamber_radius = self.geometry.r_throat_m * sqrt(3.0);
            return if r <= chamber_radius {
                self.chamber_density_kg_m3
            } else {
                self.ambient_density_kg_m3
            };
        }
        if x < self.origin_x_m {
            let x_norm = (x / self.geometry.length_m()).clamp(0.0, 1.0);
            let wall = self.geometry.radius_at(x_norm);
            if r > wall {
                return self.ambient_density_kg_m3;
            }
            let mach = nozzle::mach_at_area(sep_area(self.geometry, wall), self.gamma)
                .unwrap_or(1.0)
                .max(1.0);
            // Isentropic from chamber conditions: exact for the quasi-1D
            // core, and it makes the wall gradient continuous with the
            // chamber instead of jumping at the throat.
            return self.chamber_density_kg_m3
                * pow(
                    1.0 / (1.0 + 0.5 * (self.gamma - 1.0) * mach * mach),
                    1.0 / (self.gamma - 1.0),
                );
        }
        let s = x - self.origin_x_m;
        let boundary = self.boundary_radius(s);
        if r > boundary {
            return self.ambient_density_kg_m3;
        }
        let jet_temperature = self.total_temperature_k
            / (1.0 + 0.5 * (self.gamma - 1.0) * self.jet_mach * self.jet_mach);
        let base = if self.ambient_pa > 1.0 {
            self.ambient_pa
        } else {
            // Vacuum: the core keeps expanding, so density falls with area.
            self.origin_pressure_pa * pow(self.origin_radius_m / boundary.max(1e-9), 2.0)
        };
        let decay = exp(-s / (MIXING_RADII * self.origin_radius_m).max(1e-9));
        // Standing shock-cell wave: pressure oscillates about ambient while
        // the boundary breathes in antiphase. Reduced order, not CFD.
        let phase = if self.cell_length_m > 1e-9 {
            cos(2.0 * core::f64::consts::PI * s / self.cell_length_m)
        } else {
            0.0
        };
        let amplitude = ((self.origin_pressure_pa - base) / base.max(1e-9)).clamp(-0.8, 0.8);
        let radial = cos(0.5 * core::f64::consts::PI * (r / boundary.max(1e-9)).clamp(0.0, 1.0));
        let pressure = base * (1.0 + amplitude * decay * phase * (0.4 + 0.6 * radial));
        let core = pressure / (self.gas_constant * jet_temperature);
        // Shear layer blends the core into the ambient over a tenth of a radius.
        let edge = (boundary - r) / (0.1 * boundary).max(1e-9);
        let blend = edge.clamp(0.0, 1.0);
        self.ambient_density_kg_m3 + (core - self.ambient_density_kg_m3) * blend
    }

    /// Free-jet shear-layer radius at `s` metres past the origin.
    pub fn boundary_radius(&self, s: f64) -> f64 {
        // Underexpanded jets fan outward; compressed jets neck down toward a
        // throat and then re-expand, which is what draws the diamond train.
        let spread = if self.lip_deflection_rad > 0.0 {
            -tan(self.lip_deflection_rad.min(0.5)) * s
        } else {
            tan(self.lip_turn_rad.min(0.6)) * s
        };
        let floor = 0.35 * self.origin_radius_m;
        let growth = (self.origin_radius_m + spread.min(3.0 * self.origin_radius_m)).max(floor);
        if self.cell_length_m <= 1e-9 {
            return growth;
        }
        let decay = exp(-s / (MIXING_RADII * self.origin_radius_m).max(1e-9));
        let breath = 0.12 * decay * cos(2.0 * core::f64::consts::PI * s / self.cell_length_m);
        growth * (1.0 + breath)
    }

    /// Sample `|grad rho|` on a regular grid, normalized to [0, 1].
    /// Row-major, `width * height` values, `r` running from `-r_max` to
    /// `+r_max` so the image is symmetric about the axis.
    pub fn schlieren(
        &self,
        width: usize,
        height: usize,
        x_min: f64,
        x_max: f64,
        r_max: f64,
    ) -> Vec<f64> {
        let width = width.clamp(2, 1024);
        let height = height.clamp(2, 1024);
        let dx = (x_max - x_min) / width as f64;
        let dr = 2.0 * r_max / height as f64;
        let hx = dx * 0.5;
        let hr = dr * 0.5;
        let mut out = vec![0.0; width * height];
        let mut peak: f64 = 0.0;
        for row in 0..height {
            let r = -r_max + (row as f64 + 0.5) * dr;
            for col in 0..width {
                let x = x_min + (col as f64 + 0.5) * dx;
                let gx = (self.density_at(x + hx, r) - self.density_at(x - hx, r)) / (2.0 * hx);
                let gy = (self.density_at(x, r + hr) - self.density_at(x, r - hr)) / (2.0 * hr);
                // Schlieren with a vertical knife edge is dominated by the
                // radial gradient; keep a little axial term so the fan and
                // the cell train both read.
                let magnitude = sqrt(0.25 * gx * gx + gy * gy);
                let magnitude = if magnitude.is_finite() {
                    magnitude
                } else {
                    0.0
                };
                out[row * width + col] = magnitude;
                if magnitude > peak {
                    peak = magnitude;
                }
            }
        }
        // Compress the dynamic range: the wall discontinuity is orders above
        // the plume, and a linear map would hide every plume feature.
        if peak > 0.0 {
            let scale = libm::log1p(peak);
            for value in &mut out {
                *value = libm::log1p(*value) / scale;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{solve_engine, thermochem, EngineSpec, Propellant};

    fn spec() -> EngineSpec {
        EngineSpec {
            propellant: Propellant::LoxRp1,
            of_ratio: 2.36,
            p_c_pa: 5.0e6,
            r_throat_m: 0.05,
            r_exit_m: 0.05 * 8.944_271_91,
            length_fraction: 0.8,
        }
    }

    fn plume_at(ambient: f64) -> Plume {
        let s = spec();
        let solution = solve_engine(&s, ambient).expect("design must solve");
        let thermo = thermochem::chamber(s.propellant, s.of_ratio, s.p_c_pa).expect("thermo");
        analyze(&s, &solution, &thermo, ambient)
    }

    #[test]
    fn prandtl_meyer_round_trips_and_starts_at_sonic() {
        assert_eq!(prandtl_meyer(1.0, 1.22), 0.0);
        for mach in [1.5, 2.0, 3.4, 6.0] {
            let nu = prandtl_meyer(mach, 1.22);
            assert!((mach_from_prandtl_meyer(nu, 1.22) - mach).abs() < 1e-6);
        }
        assert!(prandtl_meyer(4.0, 1.22) > prandtl_meyer(2.0, 1.22));
    }

    #[test]
    fn vacuum_is_adapted_with_no_cells() {
        let p = plume_at(0.0);
        assert_eq!(p.regime, PlumeRegime::Adapted);
        assert_eq!(p.cell_length_m, 0.0);
        assert!(p.mach_disk_m.is_nan());
    }

    #[test]
    fn sea_level_on_a_big_bell_detaches_at_the_summerfield_line() {
        let p = plume_at(101_325.0);
        assert_eq!(p.regime, PlumeRegime::Separated);
        let solution = solve_engine(&spec(), 101_325.0).expect("solve");
        let sep = solution.separation.expect("must separate");
        let expected = sep.x_norm * spec().geometry().length_m();
        assert!(
            (p.origin_x_m - expected).abs() < 1e-12,
            "detach exactly at x_sep"
        );
        assert!(p.origin_x_m < spec().geometry().length_m());
        // A detached jet is overexpanded at the detachment line, so ambient
        // compresses it: a shock, not a Prandtl-Meyer fan.
        assert_eq!(p.lip_turn_rad, 0.0);
        assert!(p.lip_shock_rad.is_finite() && p.lip_shock_rad > 0.0);
        assert!(p.lip_deflection_rad > 0.0);
    }

    #[test]
    fn a_strongly_underexpanded_jet_shows_cells_and_a_mach_disk() {
        // Small bell at sea level: exit pressure well above ambient.
        let s = EngineSpec {
            r_exit_m: 0.05 * 2.0,
            ..spec()
        };
        let solution = solve_engine(&s, 101_325.0).expect("solve");
        let thermo = thermochem::chamber(s.propellant, s.of_ratio, s.p_c_pa).expect("thermo");
        let p = analyze(&s, &solution, &thermo, 101_325.0);
        assert_eq!(p.regime, PlumeRegime::Underexpanded);
        assert!(p.pressure_ratio > 2.0);
        assert!(p.cell_length_m > 0.0);
        assert!(p.mach_disk_m > 0.0);
        assert!(p.lip_turn_rad > 0.0);
    }

    #[test]
    fn the_schlieren_field_is_finite_bounded_and_non_empty() {
        let p = plume_at(101_325.0);
        let field = p.schlieren(64, 32, -0.05, 0.9, 0.6);
        assert_eq!(field.len(), 64 * 32);
        assert!(field
            .iter()
            .all(|v| v.is_finite() && *v >= 0.0 && *v <= 1.0));
        assert!(
            field.iter().any(|v| *v > 0.1),
            "the image must show structure"
        );
        // Symmetric about the axis.
        for row in 0..32 {
            for col in 0..64 {
                let a = field[row * 64 + col];
                let b = field[(31 - row) * 64 + col];
                assert!((a - b).abs() < 1e-9);
            }
        }
    }
}
