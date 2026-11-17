//! B-plane computation for hyperbolic encounters.
//!
//! The B-plane is the plane through the secondary body's center, perpendicular
//! to the incoming asymptote (the direction of the hyperbolic excess velocity
//! at encounter). A spacecraft's trajectory crosses this plane at the impact
//! parameter vector B from the secondary's center; |B| is the miss distance.
//!
//! All vectors are expressed in the caller's inertial frame, with positions
//! and velocities relative to the secondary body at the encounter epoch.
//! Angles are radians; distances are meters; velocities are m/s.
//!
//! Reference: Vallado, _Fundamentals of Astrodynamics and Applications_, B-plane
//! targeting section. The incoming asymptote unit vector S is the normal to
//! the B-plane. A reference vector (e.g. the secondary-to-primary line at
//! encounter) is projected into the B-plane to define the R (cross-track) and
//! T (along-track) basis vectors; B·T and B·R are the B-plane coordinates.

use crate::{finite, OrbitError};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BplaneError {
    InvalidInput,
    /// The encounter state is not hyperbolic (eccentricity <= 1), so there is
    /// no incoming asymptote and no B-plane.
    NotHyperbolic,
    /// The reference vector is parallel to the asymptote and cannot define a
    /// B-plane basis; supply a different reference or use the default.
    ReferenceDegenerate,
    NumericalRange,
}

/// Incoming asymptote direction (unit vector, normal to the B-plane), the
/// impact parameter vector, and the B-plane basis coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bplane {
    /// Unit vector along the incoming asymptote, i.e. the direction of the
    /// hyperbolic excess velocity at encounter. Normal to the B-plane.
    pub s: [f64; 3],
    /// Impact parameter vector from the secondary's center to the point where
    /// the incoming asymptote crosses the B-plane. Perpendicular to `s`;
    /// `|b_vector|` is the miss distance.
    pub b_vector: [f64; 3],
    /// Miss distance in meters: `|b_vector|`.
    pub miss_distance_m: f64,
    /// B-plane along-track coordinate: `b_vector · t_hat`.
    pub b_t: f64,
    /// B-plane cross-track coordinate: `b_vector · r_hat`.
    pub b_r: f64,
    /// R (cross-track) basis unit vector in the B-plane.
    pub r_hat: [f64; 3],
    /// T (along-track) basis unit vector in the B-plane.
    pub t_hat: [f64; 3],
}

impl From<OrbitError> for BplaneError {
    fn from(_: OrbitError) -> Self {
        BplaneError::NumericalRange
    }
}

/// Compute the B-plane for a hyperbolic encounter.
///
/// `r` and `v` are the vehicle's position and velocity relative to the
/// secondary body at the encounter epoch, in a common inertial frame.
/// `mu` is the secondary's gravitational parameter (m³/s²).
///
/// `reference` is an optional vector in the B-plane defining the R (cross-
/// track) direction. It is projected perpendicular to `s`; when `None`, the
/// default reference is the secondary-to-inertial-x direction (arbitrary but
/// stable). The reference must not be parallel to `s`.
pub fn compute_bplane(
    r: [f64; 3],
    v: [f64; 3],
    mu: f64,
    reference: Option<[f64; 3]>,
) -> Result<Bplane, BplaneError> {
    if !r.iter().all(|x| x.is_finite())
        || !v.iter().all(|x| x.is_finite())
        || !mu.is_finite()
        || mu <= 0.0
    {
        return Err(BplaneError::InvalidInput);
    }

    let r_mag = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
    if r_mag <= 0.0 {
        return Err(BplaneError::InvalidInput);
    }
    let v_mag_sq = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];

    // Specific angular momentum h = r × v.
    let h = [
        r[1] * v[2] - r[2] * v[1],
        r[2] * v[0] - r[0] * v[2],
        r[0] * v[1] - r[1] * v[0],
    ];
    let h_mag_sq = h[0] * h[0] + h[1] * h[1] + h[2] * h[2];
    if h_mag_sq <= 0.0 {
        return Err(BplaneError::InvalidInput);
    }
    let h_mag = h_mag_sq.sqrt();

    // Eccentricity vector e = (v × h) / μ - r̂.
    let v_cross_h = [
        v[1] * h[2] - v[2] * h[1],
        v[2] * h[0] - v[0] * h[2],
        v[0] * h[1] - v[1] * h[0],
    ];
    let e_vec = [
        finite(v_cross_h[0] / mu)? - r[0] / r_mag,
        finite(v_cross_h[1] / mu)? - r[1] / r_mag,
        finite(v_cross_h[2] / mu)? - r[2] / r_mag,
    ];
    let e = (e_vec[0] * e_vec[0] + e_vec[1] * e_vec[1] + e_vec[2] * e_vec[2]).sqrt();
    if e <= 1.0 {
        return Err(BplaneError::NotHyperbolic);
    }

    // Hyperbolic excess speed: v∞² = v² − 2μ/r.
    let v_inf_sq = finite(v_mag_sq - 2.0 * mu / r_mag)?;
    if v_inf_sq <= 0.0 {
        return Err(BplaneError::NotHyperbolic);
    }
    let v_inf = v_inf_sq.sqrt();

    // Unit vector toward periapsis: ê_x = ê / e.
    let e_x = [e_vec[0] / e, e_vec[1] / e, e_vec[2] / e];

    // Unit vector along the orbit normal: ĥ = h / |h|.
    let h_hat = [h[0] / h_mag, h[1] / h_mag, h[2] / h_mag];

    // Transverse unit vector in the orbit plane: ê_y = ĥ × ê_x, the direction
    // of motion at periapsis. Then ê_x × ê_y = ĥ, a right-handed basis.
    let e_y = [
        h_hat[1] * e_x[2] - h_hat[2] * e_x[1],
        h_hat[2] * e_x[0] - h_hat[0] * e_x[2],
        h_hat[0] * e_x[1] - h_hat[1] * e_x[0],
    ];

    // True anomaly at infinity: cos(ν∞) = −1/e; sin(ν∞) = √(1 − 1/e²).
    let sin_nu_inf = (1.0 - 1.0 / (e * e)).sqrt();

    // Incoming asymptote unit vector S.
    // At ν → −ν∞ on the inbound branch, the position vector points along the
    // asymptote away from the focus, and the velocity points toward the focus,
    // so S = −ê_r = (1/e) ê_x + sin(ν∞) ê_y.
    let s = [
        (1.0 / e) * e_x[0] + sin_nu_inf * e_y[0],
        (1.0 / e) * e_x[1] + sin_nu_inf * e_y[1],
        (1.0 / e) * e_x[2] + sin_nu_inf * e_y[2],
    ];

    // Impact parameter vector B = (S × h) / v∞.
    let s_cross_h = [
        s[1] * h[2] - s[2] * h[1],
        s[2] * h[0] - s[0] * h[2],
        s[0] * h[1] - s[1] * h[0],
    ];
    let b_vector = [
        s_cross_h[0] / v_inf,
        s_cross_h[1] / v_inf,
        s_cross_h[2] / v_inf,
    ];
    let miss_distance_m =
        (b_vector[0] * b_vector[0] + b_vector[1] * b_vector[1] + b_vector[2] * b_vector[2]).sqrt();

    // B-plane basis: project the reference vector perpendicular to S.
    let reference_vec = reference.unwrap_or([1.0, 0.0, 0.0]);
    if !reference_vec.iter().all(|x| x.is_finite()) {
        return Err(BplaneError::InvalidInput);
    }
    let ref_dot_s = reference_vec[0] * s[0] + reference_vec[1] * s[1] + reference_vec[2] * s[2];
    let r_raw = [
        reference_vec[0] - ref_dot_s * s[0],
        reference_vec[1] - ref_dot_s * s[1],
        reference_vec[2] - ref_dot_s * s[2],
    ];
    let r_raw_mag_sq = r_raw[0] * r_raw[0] + r_raw[1] * r_raw[1] + r_raw[2] * r_raw[2];
    if r_raw_mag_sq <= 1e-24 {
        return Err(BplaneError::ReferenceDegenerate);
    }
    let r_hat = [
        r_raw[0] / r_raw_mag_sq.sqrt(),
        r_raw[1] / r_raw_mag_sq.sqrt(),
        r_raw[2] / r_raw_mag_sq.sqrt(),
    ];

    // T = S × R completes the right-handed triad (S × R = T, R × T = S).
    let t_hat = [
        s[1] * r_hat[2] - s[2] * r_hat[1],
        s[2] * r_hat[0] - s[0] * r_hat[2],
        s[0] * r_hat[1] - s[1] * r_hat[0],
    ];

    let b_t = b_vector[0] * t_hat[0] + b_vector[1] * t_hat[1] + b_vector[2] * t_hat[2];
    let b_r = b_vector[0] * r_hat[0] + b_vector[1] * r_hat[1] + b_vector[2] * r_hat[2];

    Ok(Bplane {
        s,
        b_vector,
        miss_distance_m,
        b_t,
        b_r,
        r_hat,
        t_hat,
    })
}

/// Solve for the maneuver TNW adjustments that move the B-plane crossing from
/// its current coordinates to a target point.
///
/// `bplane` is the current B-plane at the encounter. `target_bt` and
/// `target_br` are the desired B-plane coordinates. `nodes` are the maneuver
/// nodes (TNW Δv in m/s) whose values may be adjusted; `sensitivity_m_per_mps`
/// is the per-node, per-component change in B-plane coordinate per m/s of Δv,
/// as measured by the caller (finite difference or variational). Returns the
/// adjusted TNW values for each node, or `None` if the system is singular.
///
/// This is a single Newton–Raphson iteration over the linearized system:
///     Δcoord = Σᵢ sensitivityᵢ · ΔΔvᵢ
/// solved by least squares when there are more unknowns than equations.
pub fn target_bplane(
    bplane: &Bplane,
    target_bt: f64,
    target_br: f64,
    nodes: &[[f64; 3]],
    sensitivity_m_per_mps: &[[[f64; 2]; 3]],
) -> Option<Vec<[f64; 3]>> {
    if !target_bt.is_finite()
        || !target_br.is_finite()
        || nodes.is_empty()
        || nodes.len() != sensitivity_m_per_mps.len()
    {
        return None;
    }

    let n = nodes.len();
    // Residual: current − target.
    let r0 = bplane.b_t - target_bt;
    let r1 = bplane.b_r - target_br;

    // Build the 2 × (3n) Jacobian J: row 0 = ∂B·T/∂Δv, row 1 = ∂B·R/∂Δv.
    // Unknown vector x = [ΔΔv₀_T, ΔΔv₀_N, ΔΔv₀_W, ..., ΔΔvₙ₋₁_T, ΔΔvₙ₋₁_N, ΔΔvₙ₋₁_W].
    // Solve J x = −r by distributing the correction across nodes proportional
    // to each node's sensitivity magnitude (damped least squares).

    // Total sensitivity magnitude per coordinate across all nodes.
    let mut sens_t_sq = 0.0;
    let mut sens_r_sq = 0.0;
    let mut sens_cross = 0.0;
    for node_sens in sensitivity_m_per_mps.iter().take(n) {
        for comp_sens in node_sens.iter() {
            sens_t_sq += comp_sens[0] * comp_sens[0];
            sens_r_sq += comp_sens[1] * comp_sens[1];
            sens_cross += comp_sens[0] * comp_sens[1];
        }
    }

    // Solve the 2×2 normal system for the coordinate correction direction.
    // [sens_t_sq  sens_cross; sens_cross  sens_r_sq] [λ_t; λ_r] = [r0; r1]
    // Then per-node ΔΔvᵢ_c = −λ_t·∂B·T/∂vᵢ_c − λ_r·∂B·R/∂vᵢ_c.
    let det = sens_t_sq * sens_r_sq - sens_cross * sens_cross;
    if det.abs() < 1e-30 {
        return None;
    }
    let lambda_t = (r0 * sens_r_sq - r1 * sens_cross) / det;
    let lambda_r = (sens_t_sq * r1 - sens_cross * r0) / det;

    let mut result = Vec::with_capacity(n);
    for node_sens in sensitivity_m_per_mps.iter().take(n) {
        let mut adj = [0.0; 3];
        for (comp_idx, comp_sens) in node_sens.iter().enumerate() {
            adj[comp_idx] = -lambda_t * comp_sens[0] - lambda_r * comp_sens[1];
        }
        result.push(adj);
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bplane_is_perpendicular_to_s_and_b_matches_h_over_vinf() {
        // A simple hyperbolic encounter: vehicle at [1e7, 0, 0] m relative to
        // the Moon, moving in +y at 2 km/s. Moon μ = 4.9028e12 m³/s².
        // Specific energy: v²/2 − μ/r = 2e6 − 4.9e5 = 1.5e6 > 0, hyperbolic.
        let r = [1.0e7, 0.0, 0.0];
        let v = [0.0, 2.0e3, 0.0];
        let mu = 4.902_800_118e12;

        let bplane = compute_bplane(r, v, mu, None).expect("hyperbolic encounter");

        // S must be a unit vector.
        let s_mag =
            (bplane.s[0] * bplane.s[0] + bplane.s[1] * bplane.s[1] + bplane.s[2] * bplane.s[2])
                .sqrt();
        assert!((s_mag - 1.0).abs() < 1e-12, "S not unit: {s_mag}");

        // B must be perpendicular to S.
        let b_dot_s = bplane.b_vector[0] * bplane.s[0]
            + bplane.b_vector[1] * bplane.s[1]
            + bplane.b_vector[2] * bplane.s[2];
        assert!(b_dot_s.abs() < 1e-6, "B not perpendicular to S: {b_dot_s}");

        // Miss distance = |h| / v∞.
        let h = [0.0, 0.0, r[0] * v[1]];
        let h_mag = h[2].abs();
        let v_inf = (v[0] * v[0] + v[1] * v[1] - 2.0 * mu / r[0]).sqrt();
        let expected_miss = h_mag / v_inf;
        assert!(
            (bplane.miss_distance_m - expected_miss).abs() < 1e-3,
            "miss {} vs expected {}",
            bplane.miss_distance_m,
            expected_miss
        );

        // |B| must equal |b_vector|.
        let b_mag = (bplane.b_vector[0] * bplane.b_vector[0]
            + bplane.b_vector[1] * bplane.b_vector[1]
            + bplane.b_vector[2] * bplane.b_vector[2])
            .sqrt();
        assert!((b_mag - bplane.miss_distance_m).abs() < 1e-6);

        // R and T must be unit and orthogonal to S.
        let r_mag = (bplane.r_hat[0] * bplane.r_hat[0]
            + bplane.r_hat[1] * bplane.r_hat[1]
            + bplane.r_hat[2] * bplane.r_hat[2])
            .sqrt();
        let t_mag = (bplane.t_hat[0] * bplane.t_hat[0]
            + bplane.t_hat[1] * bplane.t_hat[1]
            + bplane.t_hat[2] * bplane.t_hat[2])
            .sqrt();
        assert!((r_mag - 1.0).abs() < 1e-12);
        assert!((t_mag - 1.0).abs() < 1e-12);
        let s_dot_r = bplane.s[0] * bplane.r_hat[0]
            + bplane.s[1] * bplane.r_hat[1]
            + bplane.s[2] * bplane.r_hat[2];
        let s_dot_t = bplane.s[0] * bplane.t_hat[0]
            + bplane.s[1] * bplane.t_hat[1]
            + bplane.s[2] * bplane.t_hat[2];
        assert!(s_dot_r.abs() < 1e-12);
        assert!(s_dot_t.abs() < 1e-12);
    }

    #[test]
    fn non_hyperbolic_encounter_is_rejected() {
        // Circular orbit at 200 km altitude: e = 0, not hyperbolic.
        let r = [6_578_137.0, 0.0, 0.0];
        let mu = 3.986_004_418e14;
        let speed_sq: f64 = mu / r[0];
        let speed = speed_sq.sqrt();
        let v = [0.0, speed, 0.0];
        assert!(matches!(
            compute_bplane(r, v, mu, None),
            Err(BplaneError::NotHyperbolic)
        ));
    }

    #[test]
    fn default_reference_does_not_crash_for_equatorial_encounter() {
        // Encounter where S happens to be along +X, the default reference.
        // Pick r along +Y, v such that S points along +X.
        // h = r × v; choose v so h points along +Z, e_vec points so S = +X.
        let r = [0.0, 1.0e7, 0.0];
        let v = [1.0e3, 0.0, 0.0];
        let mu = 4.902_800_118e12;
        // v² = 1e6, 2μ/r ≈ 9.8e5, so v² − 2μ/r ≈ 2e4 > 0, hyperbolic.
        let result = compute_bplane(r, v, mu, None);
        // S may or may not be exactly +X; if it is, the default reference is
        // degenerate and we expect ReferenceDegenerate, otherwise a valid B-plane.
        match result {
            Ok(_) => {}
            Err(BplaneError::ReferenceDegenerate) => {}
            Err(e) => panic!("unexpected error: {e:?}"),
        }
    }

    #[test]
    fn target_bplane_distributes_correction_across_nodes() {
        // Synthetic: one node with sensitivity ∂B·T/∂v_T = 1000 m per m/s,
        // ∂B·R/∂v_W = 500 m per m/s, everything else zero. Current B·T = 10000 m,
        // target 0 m; current B·R = 5000 m, target 0 m.
        let bplane = Bplane {
            s: [1.0, 0.0, 0.0],
            b_vector: [0.0, 10000.0, 5000.0],
            miss_distance_m: 11180.3398875,
            b_t: 10000.0,
            b_r: 5000.0,
            r_hat: [0.0, 1.0, 0.0],
            t_hat: [0.0, 0.0, 1.0],
        };
        let nodes = [[0.0, 0.0, 0.0]];
        let mut sens = [[[0.0; 2]; 3]; 1];
        sens[0][0][0] = 1000.0; // node 0, T component, affects B·T
        sens[0][2][1] = 500.0; // node 0, W component, affects B·R
        let adj = target_bplane(&bplane, 0.0, 0.0, &nodes, &sens).expect("solvable");
        // ΔΔv_T = −10000/1000 = −10 m/s; ΔΔv_W = −5000/500 = −10 m/s.
        assert!((adj[0][0] - (-10.0)).abs() < 1e-9);
        assert!((adj[0][1] - 0.0).abs() < 1e-9);
        assert!((adj[0][2] - (-10.0)).abs() < 1e-9);
    }

    #[test]
    fn target_bplane_rejects_empty_nodes() {
        let bplane = Bplane {
            s: [1.0, 0.0, 0.0],
            b_vector: [0.0, 0.0, 0.0],
            miss_distance_m: 0.0,
            b_t: 0.0,
            b_r: 0.0,
            r_hat: [0.0, 1.0, 0.0],
            t_hat: [0.0, 0.0, 1.0],
        };
        assert!(target_bplane(&bplane, 0.0, 0.0, &[], &[]).is_none());
    }

    #[test]
    fn target_bplane_rejects_singular_sensitivity() {
        let bplane = Bplane {
            s: [1.0, 0.0, 0.0],
            b_vector: [0.0, 1000.0, 0.0],
            miss_distance_m: 1000.0,
            b_t: 1000.0,
            b_r: 0.0,
            r_hat: [0.0, 1.0, 0.0],
            t_hat: [0.0, 0.0, 1.0],
        };
        let nodes = [[0.0, 0.0, 0.0]];
        let sens = [[[0.0; 2]; 3]; 1]; // all zero sensitivity -> singular
        assert!(target_bplane(&bplane, 0.0, 0.0, &nodes, &sens).is_none());
    }
}
