//! Body-centered inertial trajectory keyframes and cubic Hermite interpolation.
//! Times are SI seconds in a caller-chosen common epoch. Position is in meters,
//! velocity in meters/second, and mass in kilograms. Mass is linearly interpolated;
//! no interpolation models a burn discontinuity unless its endpoints are stored.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyframe {
    pub time: f64,
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub mass: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplineError {
    InvalidKeyframe,
    OutsideCoverage,
    NumericalRange,
}

/// Structure of arrays; append in strictly increasing time order.
#[derive(Clone, Debug, Default)]
pub struct TrajectorySpline {
    times: Vec<f64>,
    positions: [Vec<f64>; 3],
    velocities: [Vec<f64>; 3],
    masses: Vec<f64>,
}

impl TrajectorySpline {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.times.len()
    }
    pub fn is_empty(&self) -> bool {
        self.times.is_empty()
    }

    /// Retrieve an accepted integration keyframe, without interpolation.
    pub fn keyframe(&self, index: usize) -> Option<Keyframe> {
        let &time = self.times.get(index)?;
        Some(Keyframe {
            time,
            position: std::array::from_fn(|i| self.positions[i][index]),
            velocity: std::array::from_fn(|i| self.velocities[i][index]),
            mass: self.masses[index],
        })
    }

    pub fn push(&mut self, frame: Keyframe) -> Result<(), SplineError> {
        if !frame.time.is_finite()
            || !frame.mass.is_finite()
            || frame.mass <= 0.0
            || !frame
                .position
                .iter()
                .chain(frame.velocity.iter())
                .all(|v| v.is_finite())
            || self
                .times
                .last()
                .is_some_and(|t| frame.time <= *t || !(frame.time - *t).is_finite())
        {
            return Err(SplineError::InvalidKeyframe);
        }
        self.times.push(frame.time);
        for i in 0..3 {
            self.positions[i].push(frame.position[i]);
            self.velocities[i].push(frame.velocity[i]);
        }
        self.masses.push(frame.mass);
        Ok(())
    }

    pub fn sample(&self, time: f64) -> Result<Keyframe, SplineError> {
        if !time.is_finite()
            || self.is_empty()
            || time < self.times[0]
            || time > self.times[self.len() - 1]
        {
            return Err(SplineError::OutsideCoverage);
        }
        let j = self.times.partition_point(|t| *t < time);
        if j < self.len() && self.times[j] == time {
            return Ok(Keyframe {
                time,
                position: std::array::from_fn(|i| self.positions[i][j]),
                velocity: std::array::from_fn(|i| self.velocities[i][j]),
                mass: self.masses[j],
            });
        }
        let i = j - 1;
        let dt = self.times[j] - self.times[i];
        let u = (time - self.times[i]) / dt;
        let u2 = u * u;
        let u3 = u2 * u;
        let (h00, h10, h01, h11) = (
            2.0 * u3 - 3.0 * u2 + 1.0,
            u3 - 2.0 * u2 + u,
            -2.0 * u3 + 3.0 * u2,
            u3 - u2,
        );
        let (d00, d10, d01, d11) = (
            (6.0 * u2 - 6.0 * u) / dt,
            3.0 * u2 - 4.0 * u + 1.0,
            (-6.0 * u2 + 6.0 * u) / dt,
            3.0 * u2 - 2.0 * u,
        );
        let position = std::array::from_fn(|k| {
            h00 * self.positions[k][i]
                + h10 * dt * self.velocities[k][i]
                + h01 * self.positions[k][j]
                + h11 * dt * self.velocities[k][j]
        });
        let velocity = std::array::from_fn(|k| {
            d00 * self.positions[k][i]
                + d10 * self.velocities[k][i]
                + d01 * self.positions[k][j]
                + d11 * self.velocities[k][j]
        });
        let mass = self.masses[i] + u * (self.masses[j] - self.masses[i]);
        if !position
            .iter()
            .chain(velocity.iter())
            .all(|x| x.is_finite())
            || !mass.is_finite()
        {
            return Err(SplineError::NumericalRange);
        }
        Ok(Keyframe {
            time,
            position,
            velocity,
            mass,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cubic_and_derivative_are_exact_at_interior_and_endpoints() {
        let mut spline = TrajectorySpline::new();
        for t in [0.0_f64, 2.0, 5.0] {
            spline
                .push(Keyframe {
                    time: t,
                    position: [t * t * t, 2.0 * t * t, 7.0],
                    velocity: [3.0 * t * t, 4.0 * t, 0.0],
                    mass: 10.0 - t,
                })
                .unwrap();
        }
        for t in [0.0_f64, 0.25, 2.0, 3.5, 5.0] {
            let s = spline.sample(t).unwrap();
            for (actual, expected) in s.position.into_iter().zip([t * t * t, 2.0 * t * t, 7.0]) {
                assert!((actual - expected).abs() < 1e-12);
            }
            for (actual, expected) in s.velocity.into_iter().zip([3.0 * t * t, 4.0 * t, 0.0]) {
                assert!((actual - expected).abs() < 1e-12);
            }
            assert!((s.mass - (10.0 - t)).abs() < 1e-12);
        }
        assert_eq!(spline.sample(-1.0), Err(SplineError::OutsideCoverage));
        assert_eq!(spline.sample(f64::NAN), Err(SplineError::OutsideCoverage));
        assert_eq!(
            spline.push(spline.sample(2.0).unwrap()),
            Err(SplineError::InvalidKeyframe)
        );
    }
}
