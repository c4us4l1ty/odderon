use odd_astro::{
    force_model::{ForceModel, Zonal},
    integrator::{IntegrationError, Options},
    perturb::{burn::Burn, drag::DragParameters},
    spline::Keyframe,
};
const MU: f64 = 3.986_004_418e14;
const R: f64 = 6_378_137.0;
fn options() -> Options {
    Options {
        relative_tolerance: 1e-11,
        absolute_tolerance: 1e-11,
        initial_step: 10.0,
        max_step: 120.0,
        max_steps: 1_000_000,
    }
}
fn model() -> ForceModel {
    ForceModel {
        mu: MU,
        zonal: None,
        drag: None,
        radiation: None,
        third_body: false,
        burn: None,
    }
}
fn unavailable(_: f64) -> Result<odd_astro::force_model::Environment, IntegrationError> {
    Err(IntegrationError::ForceFailure)
}
fn norm(r: [f64; 3]) -> f64 {
    libm::sqrt(r.iter().map(|x| x * x).sum())
}
fn elements(s: Keyframe) -> (f64, f64, f64, f64) {
    let r = norm(s.position);
    let v2 = s.velocity.iter().map(|x| x * x).sum::<f64>();
    let a = 1.0 / (2.0 / r - v2 / MU);
    let p = s.position;
    let v = s.velocity;
    let h = [
        p[1] * v[2] - p[2] * v[1],
        p[2] * v[0] - p[0] * v[2],
        p[0] * v[1] - p[1] * v[0],
    ];
    let hmag = norm(h);
    let ecc = libm::sqrt((1.0 - hmag * hmag / (MU * a)).max(0.0));
    (a, ecc, h[2] / hmag, libm::atan2(h[0], -h[1]))
}
#[test]
fn oracle_sso_j2_precession() {
    let a = 7_178_137.0;
    let e = 0.001;
    let inc = 98.60 * std::f64::consts::PI / 180.0;
    let rp = a * (1.0 - e);
    let speed = libm::sqrt(MU * (1.0 + e) / rp);
    let start = Keyframe {
        time: 0.0,
        position: [rp, 0.0, 0.0],
        velocity: [0.0, speed * libm::cos(inc), speed * libm::sin(inc)],
        mass: 500.0,
    };
    let mut force = model();
    force.zonal = Some(Zonal {
        pole: [0.0, 0.0, 1.0],
        radius: R,
        j2: 1.082_626_68e-3,
    });
    let end = 10.0 * 86400.0;
    let spline = force.propagate(start, end, options(), unavailable).unwrap();
    let mut sx = 0.0;
    let mut sy = 0.0;
    let mut sxx = 0.0;
    let mut sxy = 0.0;
    let mut predicted = 0.0;
    let count = 501;
    for i in 0..count {
        let t = end * (i as f64) / (count - 1) as f64;
        let (a, e, c, node) = elements(spline.sample(t).unwrap());
        sx += t;
        sy += node;
        sxx += t * t;
        sxy += t * node;
        let p = a * (1.0 - e * e);
        predicted += -1.5 * 1.082_626_68e-3 * (R / p) * (R / p) * libm::sqrt(MU / (a * a * a)) * c;
    }
    let measured = (count as f64 * sxy - sx * sy) / (count as f64 * sxx - sx * sx);
    predicted /= count as f64;
    assert!(
        (measured / predicted - 1.0).abs() < 0.0025,
        "measured {measured}, predicted {predicted}"
    );
}
#[test]
fn oracle_leo_drag_decay() {
    let a = R + 300_000.0;
    let speed = libm::sqrt(MU / a);
    let start = Keyframe {
        time: 0.0,
        position: [a, 0.0, 0.0],
        velocity: [0.0, speed, 0.0],
        mass: 500.0,
    };
    let drag = DragParameters {
        pole: [0.0, 0.0, 1.0],
        omega: [0.0, 0.0, 7.292115e-5],
        equatorial_radius: R,
        polar_radius: 6_356_752.314,
        cd: 2.2,
        area: 4.0,
        mass: 500.0,
    };
    let mut force = model();
    force.drag = Some(drag);
    let trajectory = force
        .propagate(start, 5.0 * 86400.0, options(), unavailable)
        .unwrap();
    let mut previous = a;
    for day in 1..=5 {
        let axis = elements(trajectory.sample(day as f64 * 86400.0).unwrap()).0;
        assert!(axis < previous, "{axis} {previous}");
        previous = axis;
    }
    let measured = previous - a;
    // Integrate the independently derived circular-orbit energy balance:
    // dε/dt = -0.5 ρ Cd A/m * v * (v-ωa)^2, da/dt=2a²/μ dε/dt.
    // A fixed initial density is inadequate here: decay changes the density.
    let rate = |axis:f64| {
        let v=libm::sqrt(MU/axis);
        let rho=odd_astro::perturb::drag::earth_density(axis-R).unwrap();
        -axis*axis/MU * rho*drag.cd*drag.area/drag.mass*v*(v-drag.omega[2]*axis).powi(2)
    };
    let mut secular=a;
    for _ in 0..(5*1440) { let k=rate(secular); secular+=60.0*rate(secular+30.0*k); }
    let expected = secular-a;
    assert!(
        (measured / expected - 1.0).abs() < 0.05,
        "measured {measured}, expected {expected}"
    );
}
#[test]
fn oracle_finite_burn_gravity_loss() {
    let r = R + 200_000.0;
    let speed = libm::sqrt(MU / r);
    let m0 = 1000.0;
    let isp = 450.0;
    let dv = 3150.0;
    let g = 9.80665;
    let final_mass = m0 * libm::exp(-dv / (isp * g));
    let duration = (m0 - final_mass) * isp * g / 1500.0;
    let initial = Keyframe {
        time: 0.0,
        position: [r, 0.0, 0.0],
        velocity: [0.0, speed, 0.0],
        mass: m0,
    };
    let mut force = model();
    force.burn = Some(Burn {
        start: 0.0,
        end: duration,
        force_newtons: [0.0, 1500.0, 0.0],
        specific_impulse_seconds: isp,
    });
    let spline = force
        .propagate(initial, duration, options(), unavailable)
        .unwrap();
    let final_state = spline.sample(duration).unwrap();
    assert!(
        (final_state.mass - final_mass).abs() < 1e-10,
        "{} {}",
        final_state.mass,
        final_mass
    );
    let (a, e, _, _) = elements(final_state);
    let finite_apo = a * (1.0 + e);
    let impulsive = Keyframe {
        velocity: [0.0, speed + dv, 0.0],
        ..initial
    };
    let (a, e, _, _) = elements(impulsive);
    let ideal_apo = a * (1.0 + e);
    assert!(
        a > 0.0 && finite_apo > 0.0 && finite_apo < ideal_apo,
        "finite {finite_apo}, impulsive {ideal_apo}"
    );
}
