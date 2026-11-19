//! Force model and the fixed-step RK4 integrator used while the engine burns.
//!
//! Two-body gravity with an optional J2 oblateness term and an exponential
//! drag model that only bites below 200 km. The constants match
//! `src/core/orbit.ts`.

use crate::elements::Vec3;
use crate::{EARTH_J2, EARTH_RADIUS};

/// Mass used for the drag model, kg. Also the vessel's wet mass in v1.
const DRAG_AREA: f64 = 0.6 * 8.0;
const DRAG_MASS: f64 = 4200.0;

pub fn accel(r: Vec3, v: Vec3, thrust: Vec3, j2: bool, mu: f64) -> Vec3 {
    let rm = r.len();
    let mut out = r.scale(-mu / (rm * rm * rm));
    if j2 {
        let r2 = rm * rm;
        let k = 1.5 * EARTH_J2 * mu * EARTH_RADIUS * EARTH_RADIUS / (r2 * r2 * rm);
        let z2r2 = 5.0 * r.z * r.z / r2;
        out = out.add(Vec3::new(
            k * r.x * (z2r2 - 1.0),
            k * r.y * (z2r2 - 1.0),
            k * r.z * (z2r2 - 3.0),
        ));
    }
    let alt = rm - EARTH_RADIUS;
    if alt < 200_000.0 && alt > 0.0 {
        let rho = 1.225 * (-alt / 8500.0).exp();
        let speed = v.len();
        let drag = -0.5 * rho * speed * speed * DRAG_AREA / DRAG_MASS;
        out = out.add(v.scale(drag / speed.max(1e-6)));
    }
    out.add(thrust)
}

/// One RK4 step of the force model.
pub fn step_rk4(r: Vec3, v: Vec3, dt: f64, thrust: Vec3, j2: bool, mu: f64) -> (Vec3, Vec3) {
    let a1 = accel(r, v, thrust, j2, mu);
    let r2 = r.add_scaled(v, dt / 2.0);
    let v2 = v.add_scaled(a1, dt / 2.0);
    let a2 = accel(r2, v2, thrust, j2, mu);
    let r3 = r.add_scaled(v2, dt / 2.0);
    let v3 = v.add_scaled(a2, dt / 2.0);
    let a3 = accel(r3, v3, thrust, j2, mu);
    let r4 = r.add_scaled(v3, dt);
    let v4 = v.add_scaled(a3, dt);
    let a4 = accel(r4, v4, thrust, j2, mu);

    let dr = v
        .add_scaled(v2, 2.0)
        .add_scaled(v3, 2.0)
        .add(v4)
        .scale(dt / 6.0);
    let dv = a1
        .add_scaled(a2, 2.0)
        .add_scaled(a3, 2.0)
        .add(a4)
        .scale(dt / 6.0);
    (r.add(dr), v.add(dv))
}

/// Specific orbital energy, J/kg.
pub fn specific_energy(r: Vec3, v: Vec3, mu: f64) -> f64 {
    v.dot(v) / 2.0 - mu / r.len()
}
