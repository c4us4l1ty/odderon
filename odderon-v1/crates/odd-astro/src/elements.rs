//! Osculating two-body elements in a caller-supplied inertial frame.
//!
//! The semilatus rectum is used as the size parameter so parabolic orbits
//! remain representable. Classical angles are retained even near singularities
//! for reconstruction; `special_angles` provides better-conditioned combined
//! angles for interpretation. A radial trajectory has no orbital plane.
use crate::conic_denominator;
use odd_core::frames::State;
use std::f64::consts::{PI, TAU};

/// Threshold for reporting the usual singular-angle alternatives.
pub const SINGULAR_TOLERANCE: f64 = 1e-8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementsError {
    InvalidInput,
    RadialTrajectory,
    /// Posigrade equinoctial chart is singular for exactly retrograde equatorial motion.
    EquinoctialSingularity,
    OutsidePhysicalBranch,
    NumericalRange,
}

/// Classical osculating elements, with SI length and radian angles.
/// Derived true anomaly is signed in [-π, π], preserving small inbound angles.
/// `p` is positive for ellipses, parabolas, and hyperbolas. Inclination is
/// in [0, π]. Angles are measured in the caller's inertial reference frame.
/// For an exactly circular orbit ω is set to zero. For an exactly equatorial
/// orbit Ω is set to zero; for retrograde equatorial orbits ω and ν increase
/// in the direction of motion (opposite to inertial longitude). Near-singular
/// classical angles may be poorly conditioned, but are retained for round trips.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Elements {
    pub semi_latus_rectum_m: f64,
    pub eccentricity: f64,
    pub inclination_rad: f64,
    pub ascending_node_rad: f64,
    pub argument_of_periapsis_rad: f64,
    pub true_anomaly_rad: f64,
}

/// Direction of motion in the reference plane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquatorialDirection {
    Prograde,
    Retrograde,
}

impl EquatorialDirection {
    fn inclination(self) -> f64 {
        match self {
            Self::Prograde => 0.0,
            Self::Retrograde => PI,
        }
    }
    fn sign(self) -> f64 {
        match self {
            Self::Prograde => 1.0,
            Self::Retrograde => -1.0,
        }
    }
}

/// Well-conditioned combined angles for nearly singular configurations.
/// Longitudes are measured in the inertial xy-plane; argument of latitude is
/// measured from the ascending node in the direction of motion. For near (not
/// exactly) singular cases, they are limiting combinations of classical angles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpecialAngles {
    General,
    CircularEquatorial { true_longitude_rad: f64 },
    CircularInclined { argument_of_latitude_rad: f64 },
    EccentricEquatorial { longitude_of_periapsis_rad: f64 },
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(v: [f64; 3]) -> f64 {
    libm::hypot(libm::hypot(v[0], v[1]), v[2])
}
fn signed_angle(a: [f64; 3], b: [f64; 3], normal: [f64; 3]) -> f64 {
    libm::atan2(dot(cross(a, b), normal), dot(a, b))
}
fn valid_mu(mu: f64) -> bool {
    mu.is_finite() && mu > 0.0
}

impl Elements {
    /// Construct an exactly circular equatorial orbit using the inertial
    /// true longitude. The radius is measured from the central body.
    pub fn circular_equatorial(
        radius_m: f64,
        direction: EquatorialDirection,
        true_longitude_rad: f64,
    ) -> Result<Self, ElementsError> {
        if !radius_m.is_finite() || radius_m <= 0.0 || !true_longitude_rad.is_finite() {
            return Err(ElementsError::InvalidInput);
        }
        Ok(Self {
            semi_latus_rectum_m: radius_m,
            eccentricity: 0.0,
            inclination_rad: direction.inclination(),
            ascending_node_rad: 0.0,
            argument_of_periapsis_rad: 0.0,
            true_anomaly_rad: direction.sign() * true_longitude_rad,
        })
    }

    /// Construct an exactly circular inclined orbit from the argument of
    /// latitude measured from the ascending node in the direction of motion.
    pub fn circular_inclined(
        radius_m: f64,
        inclination_rad: f64,
        ascending_node_rad: f64,
        argument_of_latitude_rad: f64,
    ) -> Result<Self, ElementsError> {
        if !radius_m.is_finite()
            || radius_m <= 0.0
            || !inclination_rad.is_finite()
            || inclination_rad <= 0.0
            || inclination_rad >= PI
            || !ascending_node_rad.is_finite()
            || !argument_of_latitude_rad.is_finite()
        {
            return Err(ElementsError::InvalidInput);
        }
        Ok(Self {
            semi_latus_rectum_m: radius_m,
            eccentricity: 0.0,
            inclination_rad,
            ascending_node_rad,
            argument_of_periapsis_rad: 0.0,
            true_anomaly_rad: argument_of_latitude_rad,
        })
    }

    /// Construct an exactly equatorial noncircular conic using the inertial
    /// longitude of periapsis and true anomaly in the direction of motion.
    /// p is the semilatus rectum, so this also supports open conics.
    pub fn eccentric_equatorial(
        semi_latus_rectum_m: f64,
        eccentricity: f64,
        direction: EquatorialDirection,
        longitude_of_periapsis_rad: f64,
        true_anomaly_rad: f64,
    ) -> Result<Self, ElementsError> {
        if !semi_latus_rectum_m.is_finite()
            || semi_latus_rectum_m <= 0.0
            || !eccentricity.is_finite()
            || eccentricity <= 0.0
            || !longitude_of_periapsis_rad.is_finite()
            || !true_anomaly_rad.is_finite()
        {
            return Err(ElementsError::InvalidInput);
        }
        let denominator = conic_denominator(eccentricity, true_anomaly_rad, true_anomaly_rad.cos());
        if !denominator.is_finite() {
            return Err(ElementsError::NumericalRange);
        }
        if denominator <= 0.0 {
            return Err(ElementsError::OutsidePhysicalBranch);
        }
        Ok(Self {
            semi_latus_rectum_m,
            eccentricity,
            inclination_rad: direction.inclination(),
            ascending_node_rad: 0.0,
            argument_of_periapsis_rad: direction.sign() * longitude_of_periapsis_rad,
            true_anomaly_rad,
        })
    }

    /// Construct from the usual signed semi-major axis and five classical
    /// elements. Ellipses require a > 0 and e < 1; hyperbolas require a < 0
    /// and e > 1. Parabolas have no finite a and must be specified with p.
    pub fn from_classical(
        semi_major_axis_m: f64,
        eccentricity: f64,
        inclination_rad: f64,
        ascending_node_rad: f64,
        argument_of_periapsis_rad: f64,
        true_anomaly_rad: f64,
    ) -> Result<Self, ElementsError> {
        let a = semi_major_axis_m;
        let e = eccentricity;
        if !a.is_finite()
            || a == 0.0
            || !e.is_finite()
            || e < 0.0
            || e == 1.0
            || (a > 0.0) != (e < 1.0)
            || !inclination_rad.is_finite()
            || !(0.0..=PI).contains(&inclination_rad)
            || ![
                ascending_node_rad,
                argument_of_periapsis_rad,
                true_anomaly_rad,
            ]
            .iter()
            .all(|v| v.is_finite())
        {
            return Err(ElementsError::InvalidInput);
        }
        let p = (a * (1.0 - e)) * (1.0 + e);
        if !p.is_finite() || p <= 0.0 {
            return Err(ElementsError::NumericalRange);
        }
        let denominator = conic_denominator(e, true_anomaly_rad, true_anomaly_rad.cos());
        if !denominator.is_finite() {
            return Err(ElementsError::NumericalRange);
        }
        if denominator <= 0.0 {
            return Err(ElementsError::OutsidePhysicalBranch);
        }
        Ok(Self {
            semi_latus_rectum_m: p,
            eccentricity: e,
            inclination_rad,
            ascending_node_rad,
            argument_of_periapsis_rad,
            true_anomaly_rad,
        })
    }
    /// Derive osculating elements from an inertial central-body-relative state.
    pub fn from_state(state: State, mu_m3_s2: f64) -> Result<Self, ElementsError> {
        if !valid_mu(mu_m3_s2)
            || !state
                .position_m
                .iter()
                .chain(&state.velocity_m_s)
                .all(|x| x.is_finite())
        {
            return Err(ElementsError::InvalidInput);
        }
        let r = state.position_m;
        let v = state.velocity_m_s;
        let radius = norm(r);
        if radius == 0.0 {
            return Err(ElementsError::InvalidInput);
        }
        if !radius.is_finite() {
            return Err(ElementsError::NumericalRange);
        }
        let h = cross(r, v);
        let hmag = norm(h);
        if hmag == 0.0 {
            return Err(ElementsError::RadialTrajectory);
        }
        if !hmag.is_finite() {
            return Err(ElementsError::NumericalRange);
        }
        let hhat = h.map(|x| x / hmag);
        let p = (hmag / mu_m3_s2) * hmag;
        let v_cross_h = cross(v, h);
        let evec = std::array::from_fn(|j| v_cross_h[j] / mu_m3_s2 - r[j] / radius);
        let e = norm(evec);
        if !p.is_finite() || p <= 0.0 || !e.is_finite() {
            return Err(ElementsError::NumericalRange);
        }
        let node = [-hhat[1], hhat[0], 0.0];
        let nmag = norm(node);
        let inclination = libm::atan2(nmag, hhat[2]);
        let raan = if nmag > 0.0 {
            libm::atan2(node[1], node[0]).rem_euclid(TAU)
        } else {
            0.0
        };
        let reference = if nmag > 0.0 {
            node.map(|x| x / nmag)
        } else {
            [1.0, 0.0, 0.0]
        };
        let (argument, anomaly) = if e > 0.0 {
            let ehat = evec.map(|x| x / e);
            (
                signed_angle(reference, ehat, hhat).rem_euclid(TAU),
                signed_angle(ehat, r.map(|x| x / radius), hhat),
            )
        } else {
            (0.0, signed_angle(reference, r.map(|x| x / radius), hhat))
        };
        let result = Self {
            semi_latus_rectum_m: p,
            eccentricity: e,
            inclination_rad: inclination,
            ascending_node_rad: raan,
            argument_of_periapsis_rad: argument,
            true_anomaly_rad: anomaly,
        };
        if [raan, inclination, argument, anomaly]
            .iter()
            .all(|x| x.is_finite())
        {
            Ok(result)
        } else {
            Err(ElementsError::NumericalRange)
        }
    }

    /// Recover an inertial state for the physically accessible conic branch.
    pub fn to_state(self, mu_m3_s2: f64) -> Result<State, ElementsError> {
        let p = self.semi_latus_rectum_m;
        let e = self.eccentricity;
        let i = self.inclination_rad;
        if !valid_mu(mu_m3_s2)
            || !p.is_finite()
            || p <= 0.0
            || !e.is_finite()
            || e < 0.0
            || !i.is_finite()
            || !(0.0..=PI).contains(&i)
            || ![
                self.ascending_node_rad,
                self.argument_of_periapsis_rad,
                self.true_anomaly_rad,
            ]
            .iter()
            .all(|x| x.is_finite())
        {
            return Err(ElementsError::InvalidInput);
        }
        let (sn, cn) = self.true_anomaly_rad.sin_cos();
        let denominator = conic_denominator(e, self.true_anomaly_rad, cn);
        if !denominator.is_finite() {
            return Err(ElementsError::NumericalRange);
        }
        if denominator <= 0.0 {
            return Err(ElementsError::OutsidePhysicalBranch);
        }
        let radius = p / denominator;
        let speed = (mu_m3_s2 / p).sqrt();
        let (so, co) = self.ascending_node_rad.sin_cos();
        let (sw, cw) = self.argument_of_periapsis_rad.sin_cos();
        let (si, ci) = i.sin_cos();
        let peri = [co * cw - so * sw * ci, so * cw + co * sw * ci, sw * si];
        let transverse = [-co * sw - so * cw * ci, -so * sw + co * cw * ci, cw * si];
        let position_m = std::array::from_fn(|j| radius * (cn * peri[j] + sn * transverse[j]));
        let velocity_m_s =
            std::array::from_fn(|j| speed * (-sn * peri[j] + (e + cn) * transverse[j]));
        if radius <= 0.0
            || speed <= 0.0
            || !position_m
                .iter()
                .chain(&velocity_m_s)
                .all(|x| x.is_finite())
        {
            return Err(ElementsError::NumericalRange);
        }
        Ok(State {
            position_m,
            velocity_m_s,
        })
    }

    /// Signed semi-major axis (negative for hyperbolas), or None for an
    /// exactly parabolic orbit or an unrepresentable result.
    pub fn semi_major_axis_m(self) -> Option<f64> {
        let e = self.eccentricity;
        if !self.semi_latus_rectum_m.is_finite()
            || self.semi_latus_rectum_m <= 0.0
            || !e.is_finite()
            || e < 0.0
            || e == 1.0
        {
            return None;
        }
        let a = self.semi_latus_rectum_m / ((1.0 - e) * (1.0 + e));
        if a.is_finite() && a != 0.0 {
            Some(a)
        } else {
            None
        }
    }

    /// Identify the conventional singular-angle alternative without discarding
    /// near-singular information needed to reconstruct the original state.
    pub fn special_angles(self) -> Result<SpecialAngles, ElementsError> {
        let e = self.eccentricity;
        let i = self.inclination_rad;
        if !e.is_finite()
            || e < 0.0
            || !i.is_finite()
            || !(0.0..=PI).contains(&i)
            || ![
                self.ascending_node_rad,
                self.argument_of_periapsis_rad,
                self.true_anomaly_rad,
            ]
            .iter()
            .all(|v| v.is_finite())
        {
            return Err(ElementsError::InvalidInput);
        }
        let circular = e < SINGULAR_TOLERANCE;
        let equatorial = i.sin().abs() < SINGULAR_TOLERANCE;
        let direction = if i > PI / 2.0 { -1.0 } else { 1.0 };
        let longitude = |a: f64| {
            (self.ascending_node_rad.rem_euclid(TAU) + direction * a.rem_euclid(TAU))
                .rem_euclid(TAU)
        };
        Ok(match (circular, equatorial) {
            (true, true) => SpecialAngles::CircularEquatorial {
                true_longitude_rad: longitude(
                    (self.argument_of_periapsis_rad.rem_euclid(TAU)
                        + self.true_anomaly_rad.rem_euclid(TAU))
                    .rem_euclid(TAU),
                ),
            },
            (true, false) => SpecialAngles::CircularInclined {
                argument_of_latitude_rad: (self.argument_of_periapsis_rad.rem_euclid(TAU)
                    + self.true_anomaly_rad.rem_euclid(TAU))
                .rem_euclid(TAU),
            },
            (false, true) => SpecialAngles::EccentricEquatorial {
                longitude_of_periapsis_rad: longitude(self.argument_of_periapsis_rad),
            },
            (false, false) => SpecialAngles::General,
        })
    }
}

/// Posigrade modified equinoctial elements (Walker et al.). SI meters/radians.
/// This chart is nonsingular at zero eccentricity and inclination but cannot
/// represent an exactly retrograde equatorial orbit (inclination π).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModifiedEquinoctialElements {
    pub p: f64,
    pub f: f64,
    pub g: f64,
    pub h: f64,
    pub k: f64,
    pub l: f64,
}

impl ModifiedEquinoctialElements {
    fn basis(h: f64, k: f64) -> Result<([f64; 3], [f64; 3]), ElementsError> {
        let hh = h * h;
        let kk = k * k;
        let s = 1.0 + hh + kk;
        if !s.is_finite() {
            return Err(ElementsError::NumericalRange);
        }
        Ok((
            [(1.0 + hh - kk) / s, 2.0 * h * k / s, -2.0 * k / s],
            [2.0 * h * k / s, (1.0 - hh + kk) / s, 2.0 * h / s],
        ))
    }

    /// Convert directly from a Cartesian state, without singular classical angles.
    pub fn from_state(state: State, mu: f64) -> Result<Self, ElementsError> {
        if !valid_mu(mu)
            || !state
                .position_m
                .iter()
                .chain(&state.velocity_m_s)
                .all(|v| v.is_finite())
        {
            return Err(ElementsError::InvalidInput);
        }
        let r = state.position_m;
        let v = state.velocity_m_s;
        let radius = norm(r);
        if radius == 0.0 {
            return Err(ElementsError::InvalidInput);
        }
        if !radius.is_finite() {
            return Err(ElementsError::NumericalRange);
        }
        let angular = cross(r, v);
        let angular_mag = norm(angular);
        if angular_mag == 0.0 {
            return Err(ElementsError::RadialTrajectory);
        }
        if !angular_mag.is_finite() {
            return Err(ElementsError::NumericalRange);
        }
        let n = angular.map(|x| x / angular_mag);
        let transverse = n[0].hypot(n[1]);
        if transverse == 0.0 && n[2] < 0.0 {
            return Err(ElementsError::EquinoctialSingularity);
        }
        // Avoid cancellation in 1+n_z near i=π.
        let factor = if n[2] >= 0.0 {
            1.0 / (1.0 + n[2])
        } else {
            (1.0 - n[2]) / transverse / transverse
        };
        let h = -n[1] * factor;
        let k = n[0] * factor;
        let (fhat, ghat) = Self::basis(h, k)?;
        let evec = cross(v, angular);
        let eccentricity = std::array::from_fn(|j| evec[j] / mu - r[j] / radius);
        let p = (angular_mag / mu) * angular_mag;
        let f = dot(eccentricity, fhat);
        let g = dot(eccentricity, ghat);
        let l = libm::atan2(dot(r, ghat), dot(r, fhat));
        if ![p, f, g, h, k, l].iter().all(|x| x.is_finite()) || p <= 0.0 {
            return Err(ElementsError::NumericalRange);
        }
        Ok(Self { p, f, g, h, k, l })
    }

    /// Convert directly to Cartesian state. Open conics must be on their
    /// physically reachable branch; true longitude is periodic.
    pub fn to_state(self, mu: f64) -> Result<State, ElementsError> {
        if !valid_mu(mu)
            || ![self.p, self.f, self.g, self.h, self.k, self.l]
                .iter()
                .all(|v| v.is_finite())
            || self.p <= 0.0
        {
            return Err(ElementsError::InvalidInput);
        }
        let (fhat, ghat) = Self::basis(self.h, self.k)?;
        let (sl, cl) = self.l.sin_cos();
        let eccentricity = self.f.hypot(self.g);
        if !eccentricity.is_finite() {
            return Err(ElementsError::NumericalRange);
        }
        let radial_e = self.f * cl + self.g * sl;
        // Use the already-reduced sin/cos of L. Subtracting the periapsis
        // angle from a huge unwrapped L can erase it and change the phase.
        // Near a parabolic far side, evaluate 1 + e cos(ν) via
        // 1+cos(ν) = sin²(ν)/(1-cos(ν)) to avoid cancellation.
        let cos_nu = if eccentricity > 0.0 {
            radial_e / eccentricity
        } else {
            1.0
        };
        let mut w = if cos_nu < -0.9 && radial_e < -0.9 && eccentricity < 2.0 {
            let sin_nu = (self.f / eccentricity) * sl - (self.g / eccentricity) * cl;
            (1.0 - eccentricity) + eccentricity * (sin_nu * sin_nu / (1.0 - cos_nu))
        } else {
            1.0 + radial_e
        };
        // Match the classical chart's convention for the exact binary64 π
        // parabolic asymptote, without subtracting angles at large L.
        if eccentricity == 1.0 && self.l.abs() <= 2.0 * TAU {
            let nu = self.l - libm::atan2(self.g, self.f);
            if (nu % TAU).abs() == PI {
                w = 0.0;
            }
        }
        if !w.is_finite() {
            return Err(ElementsError::NumericalRange);
        }
        if w <= 0.0 {
            return Err(ElementsError::OutsidePhysicalBranch);
        }
        let radius = self.p / w;
        let speed = (mu / self.p).sqrt();
        let position_m = std::array::from_fn(|j| radius * (cl * fhat[j] + sl * ghat[j]));
        // Preserve nearly cancelling velocity components at principal axes.
        let f_plus_cos = if cl < -0.5 && self.f > 0.5 {
            (self.f - 1.0) + sl * sl / (1.0 - cl)
        } else if cl > 0.5 && self.f < -0.5 {
            (self.f + 1.0) - sl * sl / (1.0 + cl)
        } else {
            self.f + cl
        };
        let g_plus_sin = if sl > 0.5 && self.g < -0.5 {
            (self.g + 1.0) - cl * cl / (1.0 + sl)
        } else if sl < -0.5 && self.g > 0.5 {
            (self.g - 1.0) + cl * cl / (1.0 - sl)
        } else {
            self.g + sl
        };
        let velocity_m_s =
            std::array::from_fn(|j| speed * (-g_plus_sin * fhat[j] + f_plus_cos * ghat[j]));
        if radius <= 0.0
            || speed <= 0.0
            || !position_m
                .iter()
                .chain(&velocity_m_s)
                .all(|v| v.is_finite())
        {
            return Err(ElementsError::NumericalRange);
        }
        Ok(State {
            position_m,
            velocity_m_s,
        })
    }
}
