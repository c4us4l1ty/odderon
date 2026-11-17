//! Reproducible full-force tug study. Run with `cargo run --release -p odd-sim --example tug_study`.
//! Equal requested delta-v; every later node targets the preceding full-force
//! numerical periapsis passage. Design study, not a general optimizer.
use odd_astro::{
    elements::Elements,
    force_model::{Environment, FlightLimits, ForceModel, Radiation, ThirdBody, Zonal},
    integrator::{IntegrationError, Options},
    perturb::drag::DragParameters,
    spline::Keyframe,
};
use odd_core::{epoch::Epoch, frames::State};
use odd_ephem::{Body, Ephemeris};
use odd_sim::{
    planner::{ManeuverNode, Plan},
    targeting::next_radial_turn,
    Engine,
};
const MU: f64 = 3.986_004_418e14;
const R: f64 = 6_378_137.0;
fn radial(s: Keyframe) -> f64 {
    s.position.iter().zip(s.velocity).map(|(r, v)| r * v).sum()
}
fn elements(s: Keyframe) -> Elements {
    Elements::from_state(
        State {
            position_m: s.position,
            velocity_m_s: s.velocity,
        },
        MU,
    )
    .unwrap()
}
fn next_apo(path: &odd_astro::spline::TrajectorySpline, after: f64) -> Option<f64> {
    let mut bracket = None;
    for i in 1..path.len() {
        let a = path.keyframe(i - 1).unwrap();
        let b = path.keyframe(i).unwrap();
        if a.time >= after && radial(a) > 0. && radial(b) <= 0. {
            bracket = Some((a.time, b.time));
            break;
        }
    }
    let (mut lo, mut hi) = bracket?;
    for _ in 0..48 {
        let mid = lo + (hi - lo) * 0.5;
        if mid <= lo || mid >= hi {
            break;
        }
        if radial(path.sample(mid).unwrap()) > 0. {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some(lo + (hi - lo) * 0.5)
}
fn main() {
    let eph = Ephemeris::bundled().unwrap();
    let env = |t: f64| -> Result<Environment, IntegrationError> {
        let epoch = Epoch::new(820_497_600, t).map_err(|_| IntegrationError::ForceFailure)?;
        let p = |b| {
            eph.state(b, Body::Earth, epoch)
                .map(|s| s.state.position_m)
                .map_err(|_| IntegrationError::ForceFailure)
        };
        Ok(Environment {
            sun: ThirdBody {
                position: p(Body::Sun)?,
                mu: 1.327_124_400_412_794_2e20,
                radius: 695_700_000.0,
            },
            companion: ThirdBody {
                position: p(Body::Moon)?,
                mu: 4.902_800_118e12,
                radius: 1_737_400.0,
            },
        })
    };
    let model = ForceModel {
        mu: MU,
        zonal: Some(Zonal {
            pole: [0., 0., 1.],
            radius: R,
            j2: 1.082_626_68e-3,
        }),
        drag: Some(DragParameters {
            pole: [0., 0., 1.],
            omega: [0., 0., 7.292115e-5],
            equatorial_radius: R,
            polar_radius: 6_356_752.314,
            cd: 2.2,
            area: 4.,
            mass: 18000.,
        }),
        radiation: Some(Radiation {
            cr: 1.3,
            area: 4.,
            central_radius: R,
        }),
        third_body: true,
        burn: None,
    };
    let limits = FlightLimits {
        pole: [0., 0., 1.],
        equatorial_radius_m: R,
        polar_radius_m: 6_356_752.314,
        entry_altitude_m: Some(120_000.),
    };
    let opts = Options {
        relative_tolerance: 1e-10,
        absolute_tolerance: 1e-10,
        initial_step: 10.,
        max_step: 120.,
        max_steps: 1_000_000,
    };
    let engine = Engine {
        thrust_newtons: 18000.,
        specific_impulse_seconds: 450.,
        stage: 0,
        engine: 0,
    };
    let r = R + 200_000.;
    let initial = Keyframe {
        time: 0.,
        position: [r, 0., 0.],
        velocity: [0., (MU / r).sqrt(), 0.],
        mass: 18000.,
    };
    for kicks in [2, 3] {
        for total in [3120., 3200., 3240., 3280., 3320., 3360., 3380., 3400.] {
            let mut nodes = vec![ManeuverNode {
                epoch_us: 1_600_000_000,
                tnw_m_s: [total / kicks as f64, 0., 0.],
            }];
            let mut guidance = vec![None];
            let mut failed = false;
            for _ in 1..kicks {
                let plan = Plan::compile_guided(
                    initial,
                    MU,
                    600_000_000_000,
                    5200.,
                    engine,
                    &nodes,
                    &guidance,
                    None,
                )
                .unwrap();
                let burns = plan.timeline.burns(&[engine]).unwrap();
                let after = burns.last().unwrap().end;
                match next_radial_turn(
                    initial,
                    after,
                    after + 200_000.,
                    true,
                    &model,
                    &burns,
                    limits,
                    opts,
                    env,
                ) {
                    Ok(guide) => {
                        nodes.push(ManeuverNode {
                            epoch_us: (guide.time * 1e6).round() as i64,
                            tnw_m_s: [total / kicks as f64, 0., 0.],
                        });
                        guidance.push(Some(State {
                            position_m: guide.position,
                            velocity_m_s: guide.velocity,
                        }));
                    }
                    Err(e) => {
                        println!("kicks={kicks} dv={total}: targeting {e:?}");
                        failed = true;
                        break;
                    }
                }
            }
            if failed {
                continue;
            }
            let plan = Plan::compile_guided(
                initial,
                MU,
                600_000_000_000,
                5200.,
                engine,
                &nodes,
                &guidance,
                None,
            )
            .unwrap();
            let burns = plan.timeline.burns(&[engine]).unwrap();
            let out = model
                .propagate_with_limits(initial, 600_000., opts, &burns, limits, env)
                .unwrap();
            if let Some(stop) = out.stopped {
                println!(
                    "kicks={kicks} dv={total}: impact t={:.3} r={:.3}",
                    stop.time,
                    stop.state
                        .position
                        .iter()
                        .map(|x| x * x)
                        .sum::<f64>()
                        .sqrt()
                        / 1e3
                );
                continue;
            }
            let Some(at) = next_apo(&out.trajectory, burns.last().unwrap().end) else {
                println!("kicks={kicks} dv={total}: no apoapsis");
                continue;
            };
            let s = out.trajectory.sample(at).unwrap();
            let el = elements(s);
            let a = el.semi_major_axis_m().unwrap();
            let times = nodes
                .iter()
                .map(|n| n.epoch_us as f64 / 1e6)
                .collect::<Vec<_>>();
            println!("kicks={kicks} dv={total:.3} nodes={times:?} apo_t={at:.3} ra={:.3} km rp={:.3} km ecc={:.8} prop={:.6}",a*(1.+el.eccentricity)/1e3,(a*(1.-el.eccentricity)-R)/1e3,el.eccentricity,plan.commanded_propellant_kg);
        }
    }
}
