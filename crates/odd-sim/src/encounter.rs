//! Bounded lunar sphere-of-influence search and a Level-1 patched-conic preview.
//! The SOI is a Laplace approximation, not a physical gravity discontinuity.
//! Full-force flight still uses continuous third-body gravity; this module only
//! finds/preview a patch. It does not target a B-plane or guarantee capture.
use crate::planner::{kepler_coast, Plan, PlanError};
use odd_astro::{
    patched_conics::{patch_to_primary, patch_to_secondary},
    spline::{Keyframe, TrajectorySpline},
};
use odd_core::frames::State;

#[derive(Clone, Copy, Debug)]
pub struct Encounter {
    pub time_s: f64,
    pub vehicle_primary: State,
    pub secondary: State,
    pub soi_radius_m: f64,
}

fn relative_distance(vehicle: State, secondary: State) -> f64 {
    let d = std::array::from_fn::<_, 3, _>(|i| vehicle.position_m[i] - secondary.position_m[i]);
    libm::hypot(libm::hypot(d[0], d[1]), d[2])
}

/// First downward crossing of the secondary SOI in an accepted numerical path.
pub fn find_entry<F>(
    path: &TrajectorySpline,
    soi_radius_m: f64,
    mut secondary: F,
) -> Result<Option<Encounter>, PlanError>
where
    F: FnMut(f64) -> Result<State, PlanError>,
{
    if !soi_radius_m.is_finite() || soi_radius_m <= 0.0 {
        return Err(PlanError::InvalidInput);
    }
    let mut bracket = None;
    for i in 1..path.len() {
        let a = path.keyframe(i - 1).ok_or(PlanError::NumericalRange)?;
        let b = path.keyframe(i).ok_or(PlanError::NumericalRange)?;
        let sa = secondary(a.time)?;
        let sb = secondary(b.time)?;
        let da = relative_distance(
            State {
                position_m: a.position,
                velocity_m_s: a.velocity,
            },
            sa,
        ) - soi_radius_m;
        let db = relative_distance(
            State {
                position_m: b.position,
                velocity_m_s: b.velocity,
            },
            sb,
        ) - soi_radius_m;
        if da > 0.0 && db <= 0.0 {
            bracket = Some((a.time, b.time));
            break;
        }
    }
    let Some((mut lo, mut hi)) = bracket else {
        return Ok(None);
    };
    for _ in 0..48 {
        let mid = lo + (hi - lo) * 0.5;
        if mid <= lo || mid >= hi {
            break;
        }
        let s = path.sample(mid).map_err(|_| PlanError::NumericalRange)?;
        let sec = secondary(mid)?;
        let d = relative_distance(
            State {
                position_m: s.position,
                velocity_m_s: s.velocity,
            },
            sec,
        ) - soi_radius_m;
        if d > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let time_s = lo + (hi - lo) * 0.5;
    let s = path.sample(time_s).map_err(|_| PlanError::NumericalRange)?;
    Ok(Some(Encounter {
        time_s,
        vehicle_primary: State {
            position_m: s.position,
            velocity_m_s: s.velocity,
        },
        secondary: secondary(time_s)?,
        soi_radius_m,
    }))
}

/// A Level-1 Earth→Moon→Earth patched preview after the first SOI entry.
#[derive(Clone, Debug)]
pub struct LunarPatch {
    pub entry: Encounter,
    moon_initial: State,
    mu_secondary: f64,
    end_us: i64,
}

impl LunarPatch {
    /// Search the ideal plan at bounded intervals. `step_us` must resolve the
    /// encounter; the default caller uses 300 s. This is not a global optimizer.
    pub fn find<F>(
        plan: &Plan,
        mu_secondary: f64,
        soi_radius_m: f64,
        step_us: i64,
        mut secondary: F,
    ) -> Result<Option<Self>, PlanError>
    where
        F: FnMut(f64) -> Result<State, PlanError>,
    {
        if step_us <= 0
            || !mu_secondary.is_finite()
            || mu_secondary <= 0.0
            || !soi_radius_m.is_finite()
            || soi_radius_m <= 0.0
        {
            return Err(PlanError::InvalidInput);
        }
        let start_us = plan
            .impulses
            .last()
            .map_or(0, |i| (i.time * 1e6).round() as i64);
        let mut previous_time = start_us;
        let previous = plan.sample(previous_time)?;
        let mut previous_d = relative_distance(
            State {
                position_m: previous.position,
                velocity_m_s: previous.velocity,
            },
            secondary(previous.time)?,
        ) - soi_radius_m;
        let mut bracket = None;
        let mut time = start_us.saturating_add(step_us);
        while time <= plan.end_us {
            let s = plan.sample(time)?;
            let sec = secondary(s.time)?;
            let d = relative_distance(
                State {
                    position_m: s.position,
                    velocity_m_s: s.velocity,
                },
                sec,
            ) - soi_radius_m;
            if previous_d > 0.0 && d <= 0.0 {
                bracket = Some((previous_time, time));
                break;
            }
            previous_time = time;
            previous_d = d;
            time = time.saturating_add(step_us);
        }
        let Some((mut lo, mut hi)) = bracket else {
            return Ok(None);
        };
        for _ in 0..40 {
            let mid = lo + (hi - lo) / 2;
            if mid <= lo || mid >= hi {
                break;
            }
            let s = plan.sample(mid)?;
            let sec = secondary(s.time)?;
            let d = relative_distance(
                State {
                    position_m: s.position,
                    velocity_m_s: s.velocity,
                },
                sec,
            ) - soi_radius_m;
            if d > 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let entry_us = lo + (hi - lo) / 2;
        let s = plan.sample(entry_us)?;
        let sec = secondary(s.time)?;
        let vehicle = State {
            position_m: s.position,
            velocity_m_s: s.velocity,
        };
        let moon_initial =
            patch_to_secondary(vehicle, sec).map_err(|_| PlanError::NumericalRange)?;
        Ok(Some(Self {
            entry: Encounter {
                time_s: entry_us as f64 / 1e6,
                vehicle_primary: vehicle,
                secondary: sec,
                soi_radius_m,
            },
            moon_initial,
            mu_secondary,
            end_us: plan.end_us,
        }))
    }

    /// Earth-centered display state. Before entry this is the plan's Earth
    /// conic; after entry it is a Moon-centered conic translated back to Earth.
    pub fn sample<F>(
        &self,
        plan: &Plan,
        time_us: i64,
        mut secondary: F,
    ) -> Result<Keyframe, PlanError>
    where
        F: FnMut(f64) -> Result<State, PlanError>,
    {
        if time_us < 0 || time_us > self.end_us {
            return Err(PlanError::InvalidInput);
        }
        let base = plan.sample(time_us)?;
        if (time_us as f64 / 1e6) < self.entry.time_s {
            return Ok(base);
        }
        let moon = kepler_coast(
            self.moon_initial,
            self.mu_secondary,
            base.time - self.entry.time_s,
        )?;
        let sec = secondary(base.time)?;
        let earth = patch_to_primary(moon, sec).map_err(|_| PlanError::NumericalRange)?;
        Ok(Keyframe {
            time: base.time,
            position: earth.position_m,
            velocity: earth.velocity_m_s,
            mass: base.mass,
        })
    }
}
