//! Exact finite-burn loss bookkeeping for constant-inertial-thrust arcs.
//!
//! During a burn in a point-mass gravity field the speed derivative is
//!
//! ```text
//! d|v|/dt = (F/m) (u · v̂) + g · v̂
//! ```
//!
//! where `u` is the fixed thrust direction and `g = -μ r / |r|³`. Integrating
//! over the arc gives the identity
//!
//! ```text
//! Δv_ideal - (|v_end| - |v_start|) = L_steer + L_gravity
//! L_steer   = ∫ (F/m) (1 - u · v̂) dt
//! L_gravity = ∫ -(g · v̂) dt
//! ```
//!
//! `L_steer` is the cosine/steering loss caused by the velocity vector rotating
//! under gravity while the thrust direction stays fixed. For a short burn
//! centered on periapsis it reduces to `Δv μ T² / (24 r³)`, the estimate quoted
//! in the work-package card. `L_gravity` is the classical flight-path-angle
//! gravity loss and can be negative on a descending arc. Neither is the whole
//! apoapsis shortfall: the impulsive and finite trajectories also differ in
//! direction and radius at the comparison epoch.
//!
//! The integrals are evaluated with five-point Gauss–Legendre quadrature on
//! every spline segment inside each burn. The spline is the adaptive
//! Dormand–Prince solution itself, not a separate coarse approximation.

use crate::{
    force_model::ForceModel,
    integrator::{IntegrationError, Options},
    perturb::burn::Burn,
    spline::{Keyframe, TrajectorySpline},
};

/// Five-point Gauss–Legendre nodes and weights on [-1, 1].
const NODES: [f64; 5] = [
    -0.906_179_845_938_664,
    -0.538_469_310_105_683,
    0.0,
    0.538_469_310_105_683,
    0.906_179_845_938_664,
];
const WEIGHTS: [f64; 5] = [
    0.236_926_885_056_189,
    0.478_628_670_499_366,
    0.568_888_888_888_889,
    0.478_628_670_499_366,
    0.236_926_885_056_189,
];

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BurnLoss {
    /// Rocket-equation velocity increment actually commanded, m/s.
    pub ideal_delta_v_m_s: f64,
    /// ∫ (F/m)(1 - u·v̂) dt, m/s. Non-negative.
    pub steering_loss_m_s: f64,
    /// ∫ -(g·v̂) dt, m/s. Signed; negative while descending toward periapsis.
    pub gravity_loss_m_s: f64,
    /// ideal_delta_v - (speed_end - speed_start), m/s.
    pub speed_deficit_m_s: f64,
    /// Residual of the exact identity above, m/s; a numerical-quality gauge.
    pub identity_residual_m_s: f64,
    /// Total powered time, s.
    pub burn_duration_s: f64,
    /// Total swept central angle over all burns, rad (sum of absolute turns).
    pub sweep_rad: f64,
}

impl BurnLoss {
    /// Small-burn centered-periapsis estimate `Δv μ T² / (24 r³)`.
    /// Valid only while the swept angle is small; not used as the oracle for
    /// long trans-lunar burns.
    pub fn centered_periapsis_estimate(
        delta_v_m_s: f64,
        mu: f64,
        radius_m: f64,
        duration_s: f64,
    ) -> f64 {
        delta_v_m_s * mu * duration_s * duration_s / (24.0 * radius_m * radius_m * radius_m)
    }

    /// Total finite-burn speed penalty relative to the commanded rocket Δv.
    pub fn total_loss_m_s(&self) -> f64 {
        self.steering_loss_m_s + self.gravity_loss_m_s
    }
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norm(a: [f64; 3]) -> f64 {
    libm::sqrt(dot(a, a))
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Integrate the exact loss identity over one burn using the cached trajectory.
/// The trajectory must have been produced with point gravity plus this burn;
/// with additional perturbations the identity residual measures their work.
pub fn measure_burn(
    trajectory: &TrajectorySpline,
    burn: &Burn,
    mu: f64,
) -> Result<BurnLoss, IntegrationError> {
    if !mu.is_finite() || mu <= 0.0 || burn.end <= burn.start {
        return Err(IntegrationError::InvalidInput);
    }
    let force = norm(burn.force_newtons);
    if force <= 0.0 || !force.is_finite() {
        return Err(IntegrationError::InvalidInput);
    }
    let u = burn.force_newtons.map(|x| x / force);
    let start_state = trajectory
        .sample(burn.start)
        .map_err(|_| IntegrationError::NumericalRange)?;
    let end_state = trajectory
        .sample(burn.end)
        .map_err(|_| IntegrationError::NumericalRange)?;
    let ideal =
        burn.specific_impulse_seconds * 9.80665 * libm::log(start_state.mass / end_state.mass);
    if !ideal.is_finite() || ideal < 0.0 {
        return Err(IntegrationError::NumericalRange);
    }
    let mut steering = 0.0;
    let mut gravity = 0.0;
    let mut sweep = 0.0;
    for i in 1..trajectory.len() {
        let k0 = trajectory
            .keyframe(i - 1)
            .ok_or(IntegrationError::NumericalRange)?;
        let k1 = trajectory
            .keyframe(i)
            .ok_or(IntegrationError::NumericalRange)?;
        let lo = k0.time.max(burn.start);
        let hi = k1.time.min(burn.end);
        if hi <= lo {
            continue;
        }
        let p0 = trajectory
            .sample(lo)
            .map_err(|_| IntegrationError::NumericalRange)?
            .position;
        let p1 = trajectory
            .sample(hi)
            .map_err(|_| IntegrationError::NumericalRange)?
            .position;
        sweep += libm::fabs(libm::atan2(norm(cross(p0, p1)), dot(p0, p1)));
        let mid = lo + (hi - lo) * 0.5;
        let half = (hi - lo) * 0.5;
        for (node, weight) in NODES.iter().zip(WEIGHTS) {
            let t = mid + half * node;
            let s = trajectory
                .sample(t)
                .map_err(|_| IntegrationError::NumericalRange)?;
            let speed = norm(s.velocity);
            let radius = norm(s.position);
            if speed <= 0.0 || radius <= 0.0 {
                return Err(IntegrationError::ForceFailure);
            }
            let vhat = s.velocity.map(|x| x / speed);
            let (acc, _) = burn
                .acceleration_and_mass_rate(t, s.mass)
                .map_err(|_| IntegrationError::ForceFailure)?;
            let thrust_accel = norm(acc);
            let g = s.position.map(|x| -mu * x / (radius * radius * radius));
            steering += half * weight * thrust_accel * (1.0 - dot(u, vhat));
            gravity += half * weight * -dot(g, vhat);
        }
    }
    let speed_gain = norm(end_state.velocity) - norm(start_state.velocity);
    let deficit = ideal - speed_gain;
    Ok(BurnLoss {
        ideal_delta_v_m_s: ideal,
        steering_loss_m_s: steering,
        gravity_loss_m_s: gravity,
        speed_deficit_m_s: deficit,
        identity_residual_m_s: deficit - steering - gravity,
        burn_duration_s: burn.end - burn.start,
        sweep_rad: sweep,
    })
}

/// Measure every burn in a chronological schedule and sum the losses.
pub fn measure_burns(
    trajectory: &TrajectorySpline,
    burns: &[Burn],
    mu: f64,
) -> Result<BurnLoss, IntegrationError> {
    let mut total = BurnLoss::default();
    for burn in burns {
        let part = measure_burn(trajectory, burn, mu)?;
        total.ideal_delta_v_m_s += part.ideal_delta_v_m_s;
        total.steering_loss_m_s += part.steering_loss_m_s;
        total.gravity_loss_m_s += part.gravity_loss_m_s;
        total.speed_deficit_m_s += part.speed_deficit_m_s;
        total.identity_residual_m_s += part.identity_residual_m_s;
        total.burn_duration_s += part.burn_duration_s;
        total.sweep_rad += part.sweep_rad;
    }
    Ok(total)
}

/// Run the plain two-body + finite-burn model and measure its exact losses.
/// Perturbations are deliberately excluded so the finite-burn penalty is
/// isolated from J2, third bodies, drag and radiation pressure.
pub fn isolate_finite_burn(
    initial: Keyframe,
    end: f64,
    options: Options,
    mu: f64,
    burns: &[Burn],
) -> Result<(TrajectorySpline, BurnLoss), IntegrationError> {
    let model = ForceModel {
        mu,
        zonal: None,
        drag: None,
        radiation: None,
        third_body: false,
        burn: None,
    };
    let trajectory = model.propagate_with_burns(initial, end, options, burns, |_| {
        Err(IntegrationError::ForceFailure)
    })?;
    let loss = measure_burns(&trajectory, burns, mu)?;
    Ok((trajectory, loss))
}
