//! 3-tier hierarchical coordinate frames solving the floating-point Kraken across 100,000+ ly.
//! Tier 1: GalacticSector in i128 nanometers relative to the Solar System Barycenter (ICRS).
//! Tier 2: BarycentricPos in f64 meters relative to the active system/planet center.
//! Tier 3: Vessel/Room local f32/f64 coordinates centered on the active interior.

use crate::trig::dsqrt;
use crate::vec::Vec3d;

pub const LIGHT_SPEED_MPS: f64 = 299_792_458.0;
/// 1 Julian light-year in exact integer nanometers: 9,460,730,472,580,800,000,000,000 nm.
pub const LIGHT_YEAR_NM: i128 = 9_460_730_472_580_800_000_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GalacticSector {
    pub x_nm: i128,
    pub y_nm: i128,
    pub z_nm: i128,
}

impl GalacticSector {
    pub const SOLAR_BARYCENTER: Self = Self {
        x_nm: 0,
        y_nm: 0,
        z_nm: 0,
    };

    pub const fn from_light_years(lx: i64, ly: i64, lz: i64) -> Self {
        Self {
            x_nm: (lx as i128) * LIGHT_YEAR_NM,
            y_nm: (ly as i128) * LIGHT_YEAR_NM,
            z_nm: (lz as i128) * LIGHT_YEAR_NM,
        }
    }

    /// Exact integer nanometer displacement before conversion to meters for light-time delay.
    pub fn light_delay_seconds(&self, other: &Self) -> f64 {
        let dx_m = ((other.x_nm - self.x_nm) as f64) * 1e-9;
        let dy_m = ((other.y_nm - self.y_nm) as f64) * 1e-9;
        let dz_m = ((other.z_nm - self.z_nm) as f64) * 1e-9;
        dsqrt(dx_m * dx_m + dy_m * dy_m + dz_m * dz_m) / LIGHT_SPEED_MPS
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BarycentricPos {
    pub sector: GalacticSector,
    pub local_m: Vec3d,
}

impl BarycentricPos {
    /// Uchinoura Space Center coastal town on Earth (31.2510° N, 131.0791° E, +94 m MSL).
    pub const fn earth_uchinoura_hab() -> Self {
        Self {
            sector: GalacticSector::SOLAR_BARYCENTER,
            // ECEF meters for 31.2510 N, 131.0791 E
            local_m: Vec3d::new(-3_584_118.4, 4_111_295.2, 3_289_441.8),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn galactic_scale_spans_8000_ly_with_nanometer_precision() {
        let earth = GalacticSector::SOLAR_BARYCENTER;
        let far_star = GalacticSector::from_light_years(4000, -4000, 2000);
        let shifted = GalacticSector {
            x_nm: far_star.x_nm + 1, // 1 nanometer step at >6,000 ly!
            ..far_star
        };
        assert_eq!(shifted.x_nm - far_star.x_nm, 1);
        let delay_yr = earth.light_delay_seconds(&far_star) / (365.25 * 86400.0);
        assert!((delay_yr - 6000.0).abs() < 0.05);
    }
}
