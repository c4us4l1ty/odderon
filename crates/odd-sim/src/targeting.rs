//! Numerical periapsis/apoapsis passage targeting for finite-burn planning.
//! Uses the supplied force model and commanded burns, not an ideal conic's
//! period. This is a forward search over a bounded window, not an optimizer.
use odd_astro::{
    force_model::{Environment, FlightLimits, ForceModel},
    integrator::{IntegrationError, Options},
    perturb::burn::Burn,
    spline::Keyframe,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetError {
    InvalidInput,
    Impact,
    NotFound,
    Integration(IntegrationError),
}
impl From<IntegrationError> for TargetError {
    fn from(value: IntegrationError) -> Self {
        Self::Integration(value)
    }
}

fn radial(state: Keyframe) -> f64 {
    state
        .position
        .iter()
        .zip(state.velocity)
        .map(|(r, v)| r * v)
        .sum()
}

/// Find the first radial-velocity turn after `after_s` and before `end_s`.
/// `periapsis=true` searches inward-to-outward (negative to positive r·v);
/// false searches the apoapsis turn. The returned state is sampled from the
/// accepted Hermite cache after bracketing; the turn time is refined by
/// bisection on that cache. Impact truncation is an error, never a fake node.
#[allow(clippy::too_many_arguments)]
pub fn next_radial_turn<F>(
    initial: Keyframe,
    after_s: f64,
    end_s: f64,
    periapsis: bool,
    model: &ForceModel,
    burns: &[Burn],
    limits: FlightLimits,
    options: Options,
    mut environment: F,
) -> Result<Keyframe, TargetError>
where
    F: FnMut(f64) -> Result<Environment, IntegrationError>,
{
    if !after_s.is_finite() || !end_s.is_finite() || end_s <= after_s || initial.time > after_s {
        return Err(TargetError::InvalidInput);
    }
    let integrated =
        model.propagate_with_limits(initial, end_s, options, burns, limits, &mut environment)?;
    if integrated.stopped.is_some() {
        return Err(TargetError::Impact);
    }
    let path = &integrated.trajectory;
    let mut bracket = None;
    for i in 1..path.len() {
        let a = path.keyframe(i - 1).ok_or(TargetError::NotFound)?;
        let b = path.keyframe(i).ok_or(TargetError::NotFound)?;
        if a.time < after_s {
            continue;
        }
        let ra = radial(a);
        let rb = radial(b);
        let crossed = if periapsis {
            ra < 0.0 && rb >= 0.0
        } else {
            ra > 0.0 && rb <= 0.0
        };
        if crossed {
            bracket = Some((a.time, b.time));
            break;
        }
    }
    let (mut lo, mut hi) = bracket.ok_or(TargetError::NotFound)?;
    for _ in 0..48 {
        let mid = lo + (hi - lo) * 0.5;
        if mid <= lo || mid >= hi {
            break;
        }
        let r = radial(path.sample(mid).map_err(|_| TargetError::NotFound)?);
        let before = if periapsis { r < 0.0 } else { r > 0.0 };
        if before {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    path.sample(lo + (hi - lo) * 0.5)
        .map_err(|_| TargetError::NotFound)
}
