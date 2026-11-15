//! Adaptive Dormand–Prince 5(4), seven stages, forward in SI seconds.
//!
//! Force callbacks provide acceleration and mass rate in an inertial frame.
//! Time events split steps and stages on the terminating endpoint use its left
//! limit. State events are bracketed on accepted steps and localized by
//! bisection with fresh Runge–Kutta trial steps, not by extrapolating a spline.
//! Event value functions must remain finite on the short trial interval used
//! for bracketing; callers that stop at a physical surface must not interpret
//! trial stages beyond that surface as a continued flight.
use crate::spline::{Keyframe, TrajectorySpline};

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub relative_tolerance: f64,
    pub absolute_tolerance: f64,
    pub initial_step: f64,
    pub max_step: f64,
    pub max_steps: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntegrationError {
    InvalidInput,
    ForceFailure,
    StepUnderflow,
    StepLimit,
    NumericalRange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventDirection {
    Descending,
    Ascending,
}

/// A zero crossing of a scalar state function. The callback receives time,
/// position, velocity and mass in the integrator's common inertial frame.
pub struct StateEvent<'a> {
    pub name: &'static str,
    pub direction: EventDirection,
    /// Terminal events truncate the returned trajectory at the crossing.
    pub terminal: bool,
    pub value: &'a dyn Fn(f64, [f64; 3], [f64; 3], f64) -> f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EventRecord {
    pub name: &'static str,
    pub time: f64,
    pub state: Keyframe,
}

#[derive(Clone, Debug)]
pub struct Integrated {
    pub trajectory: TrajectorySpline,
    /// Non-terminal crossings, in chronological order.
    pub events: Vec<EventRecord>,
    /// The terminal crossing, if one occurred before the requested end.
    pub stopped: Option<EventRecord>,
}

/// Integrate until `end`, recording each accepted step in the returned spline.
/// Time events must be finite and strictly ordered; events outside the interval
/// are ignored. This compatibility entry point has no state-triggered events.
pub fn integrate<F>(
    initial: Keyframe,
    end: f64,
    events: &[f64],
    options: Options,
    force: F,
) -> Result<TrajectorySpline, IntegrationError>
where
    F: FnMut(f64, [f64; 3], [f64; 3], f64) -> Result<([f64; 3], f64), IntegrationError>,
{
    integrate_with_events(initial, end, events, &[], options, force).map(|result| result.trajectory)
}

/// Forward integration with time events and scalar state-crossing events.
/// Crossing detection brackets a sign change between accepted endpoints; a
/// step that crosses and returns without an endpoint sign change is not seen.
/// Callers must bound `max_step` for the geometry they need to resolve.
pub fn integrate_with_events<F>(
    initial: Keyframe,
    end: f64,
    events: &[f64],
    state_events: &[StateEvent<'_>],
    options: Options,
    mut force: F,
) -> Result<Integrated, IntegrationError>
where
    F: FnMut(f64, [f64; 3], [f64; 3], f64) -> Result<([f64; 3], f64), IntegrationError>,
{
    let Options {
        relative_tolerance: rtol,
        absolute_tolerance: atol,
        initial_step,
        max_step,
        max_steps,
    } = options;
    if !end.is_finite()
        || end < initial.time
        || !rtol.is_finite()
        || rtol <= 0.0
        || !atol.is_finite()
        || atol <= 0.0
        || !initial_step.is_finite()
        || initial_step <= 0.0
        || !max_step.is_finite()
        || max_step <= 0.0
        || max_steps == 0
        || events.iter().any(|x| !x.is_finite())
        || events.windows(2).any(|w| w[0] >= w[1])
    {
        return Err(IntegrationError::InvalidInput);
    }
    let mut out = TrajectorySpline::new();
    out.push(initial)
        .map_err(|_| IntegrationError::InvalidInput)?;
    let mut t = initial.time;
    let mut y = [
        initial.position[0],
        initial.position[1],
        initial.position[2],
        initial.velocity[0],
        initial.velocity[1],
        initial.velocity[2],
        initial.mass,
    ];
    let value = |event: &StateEvent<'_>, time: f64, z: [f64; 7]| -> Result<f64, IntegrationError> {
        let v = (event.value)(time, [z[0], z[1], z[2]], [z[3], z[4], z[5]], z[6]);
        if v.is_finite() {
            Ok(v)
        } else {
            Err(IntegrationError::ForceFailure)
        }
    };
    let mut values = state_events
        .iter()
        .map(|event| value(event, t, y))
        .collect::<Result<Vec<_>, _>>()?;
    let mut armed = vec![true; state_events.len()];
    // A terminal event is also detected when the initial state is already on
    // or inside its surface; do not manufacture a later crossing from NaNs.
    for (event, g) in state_events.iter().zip(&values) {
        if event.terminal && *g <= 0.0 {
            return Ok(Integrated {
                trajectory: out,
                events: Vec::new(),
                stopped: Some(EventRecord {
                    name: event.name,
                    time: t,
                    state: initial,
                }),
            });
        }
    }
    let mut records = Vec::new();
    let mut h = initial_step.min(max_step);
    let mut event = events.partition_point(|x| *x <= t);
    let mut attempts = 0;

    // One Dormand–Prince trial from `start`, using the represented interval.
    // `boundary` and `at_event` reproduce the left-limit convention at time
    // events. State-event bisection calls this with ordinary interior steps.
    let mut step = |start: f64,
                    z0: [f64; 7],
                    width: f64,
                    boundary: f64,
                    at_event: bool|
     -> Result<([f64; 7], f64), IntegrationError> {
        if width <= 0.0 || !width.is_finite() {
            return Err(IntegrationError::StepUnderflow);
        }
        let stage_end = if at_event {
            next_down(boundary)
        } else {
            start + width
        };
        let mut k = [[0.0; 7]; 7];
        for i in 0..7 {
            let mut z = z0;
            for (j, coeff) in A[i].iter().enumerate().take(i) {
                for n in 0..7 {
                    z[n] += width * coeff * k[j][n];
                }
            }
            if z[6] <= 0.0 || !z.iter().all(|x| x.is_finite()) {
                return Err(IntegrationError::NumericalRange);
            }
            let st = if C[i] == 1.0 {
                stage_end
            } else if at_event {
                (start + C[i] * width).min(stage_end)
            } else {
                start + C[i] * width
            };
            let (acc, mass_rate) = force(st, [z[0], z[1], z[2]], [z[3], z[4], z[5]], z[6])?;
            k[i] = [z[3], z[4], z[5], acc[0], acc[1], acc[2], mass_rate];
            if !k[i].iter().all(|x| x.is_finite()) {
                return Err(IntegrationError::ForceFailure);
            }
        }
        let mut next = z0;
        let mut err = 0.0_f64;
        for n in 0..7 {
            let mut delta = 0.0;
            for i in 0..7 {
                next[n] += width * B5[i] * k[i][n];
                delta += width * (B5[i] - B4[i]) * k[i][n];
            }
            if !next[n].is_finite() {
                return Err(IntegrationError::NumericalRange);
            }
            let scale = atol + rtol * z0[n].abs().max(next[n].abs());
            err = err.max((delta / scale).abs());
        }
        if !err.is_finite() {
            return Err(IntegrationError::NumericalRange);
        }
        if next[6] <= 0.0 {
            return Err(IntegrationError::NumericalRange);
        }
        Ok((next, err))
    };

    while t < end {
        attempts += 1;
        if attempts > max_steps {
            return Err(IntegrationError::StepLimit);
        }
        while event < events.len() && events[event] <= t {
            event += 1;
        }
        let boundary = if event < events.len() {
            events[event].min(end)
        } else {
            end
        };
        let remaining = boundary - t;
        h = h.min(max_step).min(remaining);
        let mut next_t = if h == remaining {
            boundary
        } else {
            (t + h).min(boundary)
        };
        if next_t - t > max_step {
            next_t = next_down(next_t);
        }
        if next_t <= t {
            return Err(IntegrationError::StepUnderflow);
        }
        h = next_t - t;
        let landed = next_t == boundary;
        let at_event = landed && event < events.len() && boundary == events[event];
        let (next, err) = step(t, y, h, boundary, at_event)?;
        if err <= 1.0 {
            let next_values = state_events
                .iter()
                .map(|event| value(event, next_t, next))
                .collect::<Result<Vec<_>, _>>()?;
            let mut candidate: Option<(f64, [f64; 7], usize)> = None;
            for (i, event_spec) in state_events.iter().enumerate() {
                let g0 = values[i];
                let g1 = next_values[i];
                let crossed = match event_spec.direction {
                    EventDirection::Descending => g0 > 0.0 && g1 <= 0.0,
                    EventDirection::Ascending => g0 < 0.0 && g1 >= 0.0,
                };
                if !armed[i] {
                    let restored = match event_spec.direction {
                        EventDirection::Descending => g1 > 0.0,
                        EventDirection::Ascending => g1 < 0.0,
                    };
                    if restored {
                        armed[i] = true;
                        values[i] = g1;
                    }
                    continue;
                }
                if !crossed {
                    continue;
                }
                let (mut lo, mut hi) = (t, next_t);
                for _ in 0..48 {
                    let mid = lo + (hi - lo) * 0.5;
                    if mid <= lo || mid >= hi {
                        break;
                    }
                    let (z, _) = step(t, y, mid - t, boundary, false)?;
                    let g = value(event_spec, mid, z)?;
                    let positive = match event_spec.direction {
                        EventDirection::Descending => g > 0.0,
                        EventDirection::Ascending => g < 0.0,
                    };
                    if positive {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                let root = lo + (hi - lo) * 0.5;
                if root <= t {
                    return Err(IntegrationError::StepUnderflow);
                }
                let (z, _) = step(t, y, root - t, boundary, false)?;
                if candidate.as_ref().is_none_or(|(time, _, _)| root < *time) {
                    candidate = Some((root, z, i));
                }
            }
            if let Some((root, z, i)) = candidate {
                let event_spec = &state_events[i];
                let state = Keyframe {
                    time: root,
                    position: [z[0], z[1], z[2]],
                    velocity: [z[3], z[4], z[5]],
                    mass: z[6],
                };
                out.push(state)
                    .map_err(|_| IntegrationError::NumericalRange)?;
                let record = EventRecord {
                    name: event_spec.name,
                    time: root,
                    state,
                };
                if event_spec.terminal {
                    return Ok(Integrated {
                        trajectory: out,
                        events: records,
                        stopped: Some(record),
                    });
                }
                records.push(record);
                armed[i] = false;
                y = z;
                t = root;
                values = state_events
                    .iter()
                    .map(|event_spec| value(event_spec, t, y))
                    .collect::<Result<Vec<_>, _>>()?;
                h = initial_step.min(max_step);
                continue;
            }
            values = next_values;
            y = next;
            t = next_t;
            out.push(Keyframe {
                time: t,
                position: [y[0], y[1], y[2]],
                velocity: [y[3], y[4], y[5]],
                mass: y[6],
            })
            .map_err(|_| IntegrationError::NumericalRange)?;
        }
        let factor = if err == 0.0 {
            5.0
        } else {
            (0.9 * libm::pow(err, -0.2)).clamp(0.2, 5.0)
        };
        h *= factor;
    }
    Ok(Integrated {
        trajectory: out,
        events: records,
        stopped: None,
    })
}

fn next_down(x: f64) -> f64 {
    if x == 0.0 {
        -f64::from_bits(1)
    } else if x > 0.0 {
        f64::from_bits(x.to_bits() - 1)
    } else {
        f64::from_bits(x.to_bits() + 1)
    }
}

const C: [f64; 7] = [0.0, 1.0 / 5.0, 3.0 / 10.0, 4.0 / 5.0, 8.0 / 9.0, 1.0, 1.0];
const A: [[f64; 7]; 7] = [
    [0.0; 7],
    [1.0 / 5.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [3.0 / 40.0, 9.0 / 40.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [44.0 / 45.0, -56.0 / 15.0, 32.0 / 9.0, 0.0, 0.0, 0.0, 0.0],
    [
        19372.0 / 6561.0,
        -25360.0 / 2187.0,
        64448.0 / 6561.0,
        -212.0 / 729.0,
        0.0,
        0.0,
        0.0,
    ],
    [
        9017.0 / 3168.0,
        -355.0 / 33.0,
        46732.0 / 5247.0,
        49.0 / 176.0,
        -5103.0 / 18656.0,
        0.0,
        0.0,
    ],
    [
        35.0 / 384.0,
        0.0,
        500.0 / 1113.0,
        125.0 / 192.0,
        -2187.0 / 6784.0,
        11.0 / 84.0,
        0.0,
    ],
];
const B5: [f64; 7] = [
    35.0 / 384.0,
    0.0,
    500.0 / 1113.0,
    125.0 / 192.0,
    -2187.0 / 6784.0,
    11.0 / 84.0,
    0.0,
];
const B4: [f64; 7] = [
    5179.0 / 57600.0,
    0.0,
    7571.0 / 16695.0,
    393.0 / 640.0,
    -92097.0 / 339200.0,
    187.0 / 2100.0,
    1.0 / 40.0,
];

#[cfg(test)]
mod tests {
    use super::*;
    fn options() -> Options {
        Options {
            relative_tolerance: 1e-12,
            absolute_tolerance: 1e-12,
            initial_step: 10.0,
            max_step: 120.0,
            max_steps: 1_000_000,
        }
    }
    #[test]
    fn oracle_two_body_energy_conservation() {
        let mu = 3.986_004_418e14_f64;
        let rp = 6_678_000.0;
        let e = 0.7;
        let v = libm::sqrt(mu * (1.0 + e) / rp);
        let a = rp / (1.0 - e);
        let period = 2.0 * std::f64::consts::PI * libm::sqrt(a * a * a / mu);
        let initial = Keyframe {
            time: 0.0,
            position: [rp, 0.0, 0.0],
            velocity: [0.0, v, 0.0],
            mass: 1000.0,
        };
        let trajectory = integrate(initial, 10.0 * period, &[], options(), |_, r, _, _| {
            let d = libm::sqrt(r.iter().map(|x| x * x).sum::<f64>());
            let f = -mu / (d * d * d);
            Ok((r.map(|x| f * x), 0.0))
        })
        .unwrap();
        let energy = |k: Keyframe| {
            let r = libm::sqrt(k.position.iter().map(|x| x * x).sum::<f64>());
            k.velocity.iter().map(|x| x * x).sum::<f64>() * 0.5 - mu / r
        };
        let angular = |k: Keyframe| k.position[0] * k.velocity[1] - k.position[1] * k.velocity[0];
        for index in 0..trajectory.len() {
            let s = trajectory.keyframe(index).unwrap();
            assert!((energy(s) / energy(initial) - 1.0).abs() < 1e-9);
            assert!((angular(s) / angular(initial) - 1.0).abs() < 1e-9);
        }
    }
    #[test]
    fn rounded_step_landing_uses_actual_interval_and_left_limit() {
        let t = 4_503_599_627_370_496.0;
        let boundary = t + 1.0;
        let initial = Keyframe {
            time: t,
            position: [0.0; 3],
            velocity: [0.0; 3],
            mass: 1.0,
        };
        let mut o = options();
        o.initial_step = 0.75;
        o.max_step = 1.0;
        let out = integrate(initial, boundary, &[boundary], o, |time, _, _, _| {
            Ok((
                if time < boundary {
                    [1.0, 0.0, 0.0]
                } else {
                    [0.0; 3]
                },
                0.0,
            ))
        })
        .unwrap();
        let end = out.sample(boundary).unwrap();
        assert!((end.velocity[0] - 1.0).abs() < 1e-12);
        assert!((end.position[0] - 0.5).abs() < 1e-12);
    }
    #[test]
    fn rounded_interior_steps_integrate_the_represented_interval() {
        let t = 281_474_976_710_656.0;
        let initial = Keyframe {
            time: t,
            position: [0.0; 3],
            velocity: [0.0; 3],
            mass: 1.0,
        };
        let mut o = options();
        o.initial_step = 0.1;
        o.max_step = 0.1;
        let out = integrate(initial, t + 2.0, &[], o, |_, _, _, _| {
            Ok(([1.0, 0.0, 0.0], 0.0))
        })
        .unwrap();
        let end = out.sample(t + 2.0).unwrap();
        assert!((end.velocity[0] - 2.0).abs() < 1e-12);
        assert!((end.position[0] - 2.0).abs() < 1e-12);
        for i in 1..out.len() {
            assert!(
                out.keyframe(i).unwrap().time - out.keyframe(i - 1).unwrap().time <= o.max_step
            );
        }
    }
    #[test]
    fn event_splits_force_and_mass_rate() {
        let initial = Keyframe {
            time: 0.0,
            position: [1.0, 0.0, 0.0],
            velocity: [0.0; 3],
            mass: 100.0,
        };
        let out = integrate(initial, 10.0, &[2.0, 4.0], options(), |t, _, _, _| {
            Ok((
                if (2.0..4.0).contains(&t) {
                    [1.0, 0.0, 0.0]
                } else {
                    [0.0; 3]
                },
                if (2.0..4.0).contains(&t) { -1.0 } else { 0.0 },
            ))
        })
        .unwrap();
        let state = out.sample(10.0).unwrap();
        assert!((state.velocity[0] - 2.0).abs() < 1e-10);
        assert!((state.position[0] - 15.0).abs() < 1e-9);
        assert!((state.mass - 98.0).abs() < 1e-10);
        assert_eq!(out.sample(2.0).unwrap().time, 2.0);
        assert_eq!(out.sample(4.0).unwrap().time, 4.0);
    }
    #[test]
    fn terminal_state_event_is_localized_and_truncates() {
        let initial = Keyframe {
            time: 0.0,
            position: [10.0, 0.0, 0.0],
            velocity: [-1.0, 0.0, 0.0],
            mass: 1.0,
        };
        let surface = |_: f64, r: [f64; 3], _: [f64; 3], _: f64| r[0] - 3.5;
        let events = [StateEvent {
            name: "surface",
            direction: EventDirection::Descending,
            terminal: true,
            value: &surface,
        }];
        let out = integrate_with_events(initial, 20.0, &[], &events, options(), |_, _, _, _| {
            Ok(([0.0; 3], 0.0))
        })
        .unwrap();
        let stop = out.stopped.unwrap();
        assert_eq!(stop.name, "surface");
        assert!((stop.time - 6.5).abs() < 1e-10);
        assert!((stop.state.position[0] - 3.5).abs() < 1e-10);
        assert_eq!(
            out.trajectory.sample(20.0),
            Err(crate::spline::SplineError::OutsideCoverage)
        );
    }
    #[test]
    fn nonterminal_entry_event_rearms_only_after_leaving() {
        let initial = Keyframe {
            time: 0.0,
            position: [0.0, 0.0, 0.0],
            velocity: [1.0, 0.0, 0.0],
            mass: 1.0,
        };
        let entry = |_: f64, r: [f64; 3], _: [f64; 3], _: f64| 5.0 - r[0];
        let events = [StateEvent {
            name: "entry",
            direction: EventDirection::Descending,
            terminal: false,
            value: &entry,
        }];
        let out = integrate_with_events(initial, 20.0, &[], &events, options(), |_, _, _, _| {
            Ok(([0.0; 3], 0.0))
        })
        .unwrap();
        assert!(out.stopped.is_none());
        assert_eq!(out.events.len(), 1);
        assert!((out.events[0].time - 5.0).abs() < 1e-10);
        assert!((out.trajectory.sample(20.0).unwrap().position[0] - 20.0).abs() < 1e-10);
    }
}
