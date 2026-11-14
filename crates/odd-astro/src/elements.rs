//! Osculating two-body elements in a caller-supplied inertial frame.
//!
//! The semilatus rectum is used as the size parameter so parabolic orbits
//! remain representable. Classical angles are retained even near singularities
//! for reconstruction; `special_angles` provides better-conditioned combined
//! angles for interpretation. A radial trajectory has no orbital plane.
use odd_core::frames::State;
use std::f64::consts::{PI, TAU};

/// Threshold for reporting the usual singular-angle alternatives.
pub const SINGULAR_TOLERANCE: f64 = 1e-8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementsError {
    InvalidInput,
    RadialTrajectory,
    OutsidePhysicalBranch,
    NumericalRange,
}

/// Classical osculating elements, with SI length and radian angles.
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
    v[0].hypot(v[1]).hypot(v[2])
}
fn angle(a: [f64; 3], b: [f64; 3], normal: [f64; 3]) -> f64 {
    dot(cross(a, b), normal).atan2(dot(a, b)).rem_euclid(TAU)
}
fn valid_mu(mu: f64) -> bool {
    mu.is_finite() && mu > 0.0
}

impl Elements {
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
        let inclination = nmag.atan2(hhat[2]);
        let raan = if nmag > 0.0 {
            node[1].atan2(node[0]).rem_euclid(TAU)
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
                angle(reference, ehat, hhat),
                angle(ehat, r.map(|x| x / radius), hhat),
            )
        } else {
            (0.0, angle(reference, r.map(|x| x / radius), hhat))
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
        let denominator = 1.0 + e * cn;
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
