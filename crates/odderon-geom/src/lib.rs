//! Modular Zero-Asset Parametric Geometry Engine for Odderon.
//!
//! Architecture:
//! - `primitives`: Smooth-shaded rounded boxes, U-frames, cylinders, lathes, catenaries, Kawara tile roofs.
//! - `props`: Reusable local-space 3D props placed anywhere via `Pose { pos, yaw, scale }`.
//! - `hab`: Modular Apartment #204 assembly (reusable hab components for 1A & 1B+).
//! - `town`: Parameterized 2-story Japanese houses, utility poles, streetscape & distant launch complex.

pub mod hab;
pub mod modules;
pub mod primitives;
pub mod props;
pub mod rooms;
pub mod town;

use odderon_dmath::{dcos, dsin, Vec3};

// Material IDs interpreted analytically by the WebGL2 stylized physically-based shader:
pub const MAT_MATTE: f32 = 0.0;
pub const MAT_WOOD: f32 = 1.0;
pub const MAT_TATAMI: f32 = 2.0;
pub const MAT_CORRUGATED: f32 = 3.0;
pub const MAT_ALLOY: f32 = 4.0;
pub const MAT_SHOJI: f32 = 5.0;
pub const MAT_EMISSIVE_WARM: f32 = 6.0;
pub const MAT_OCEAN: f32 = 7.0;
pub const MAT_CRT: f32 = 8.0;
pub const MAT_PLUME: f32 = 9.0;
pub const MAT_EIDOS: f32 = 10.0;
pub const MAT_CERAMIC: f32 = 11.0;
pub const MAT_FOLIAGE: f32 = 12.0;
pub const MAT_KAWARA: f32 = 13.0;

/// Interleaved 10-float vertex: pos(3), normal(3), rgb(3), mat_id(1).
pub const FLOATS_PER_VERTEX: usize = 10;

///Rigid + uniform-scale placement transform so every prop/building is authored once in local space!
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub pos: Vec3,
    pub cos_y: f32,
    pub sin_y: f32,
    pub scale: f32,
}

impl Pose {
    pub const IDENTITY: Self = Self {
        pos: Vec3::new(0.0, 0.0, 0.0),
        cos_y: 1.0,
        sin_y: 0.0,
        scale: 1.0,
    };

    #[inline]
    pub fn at(x: f32, y: f32, z: f32) -> Self {
        Self {
            pos: Vec3::new(x, y, z),
            cos_y: 1.0,
            sin_y: 0.0,
            scale: 1.0,
        }
    }

    #[inline]
    pub fn at_yaw(x: f32, y: f32, z: f32, yaw_rad: f32) -> Self {
        Self {
            pos: Vec3::new(x, y, z),
            cos_y: dcos(yaw_rad as f64) as f32,
            sin_y: dsin(yaw_rad as f64) as f32,
            scale: 1.0,
        }
    }

    #[inline]
    pub fn with_scale(mut self, s: f32) -> Self {
        self.scale = s;
        self
    }

    #[inline]
    pub fn transform_point(&self, p: Vec3) -> Vec3 {
        let sx = p.x * self.scale;
        let sy = p.y * self.scale;
        let sz = p.z * self.scale;
        Vec3::new(
            self.pos.x + sx * self.cos_y + sz * self.sin_y,
            self.pos.y + sy,
            self.pos.z - sx * self.sin_y + sz * self.cos_y,
        )
    }

    #[inline]
    pub fn transform_normal(&self, n: Vec3) -> Vec3 {
        Vec3::new(
            n.x * self.cos_y + n.z * self.sin_y,
            n.y,
            -n.x * self.sin_y + n.z * self.cos_y,
        )
        .normalized()
    }

    #[inline]
    pub fn compose(&self, child: Pose) -> Pose {
        let pos = self.transform_point(child.pos);
        let cos_y = self.cos_y * child.cos_y - self.sin_y * child.sin_y;
        let sin_y = self.sin_y * child.cos_y + self.cos_y * child.sin_y;
        Pose {
            pos,
            cos_y,
            sin_y,
            scale: self.scale * child.scale,
        }
    }
}

pub struct MeshBuffer {
    pub verts: Vec<f32>,
    pub indices: Vec<u32>,
    pose_stack: Vec<Pose>,
    cur_pose: Pose,
}

impl Default for MeshBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl MeshBuffer {
    pub fn new() -> Self {
        Self {
            verts: Vec::with_capacity(65536),
            indices: Vec::with_capacity(98304),
            pose_stack: Vec::with_capacity(8),
            cur_pose: Pose::IDENTITY,
        }
    }

    pub fn clear(&mut self) {
        self.verts.clear();
        self.indices.clear();
        self.pose_stack.clear();
        self.cur_pose = Pose::IDENTITY;
    }

    /// Executes `f` inside a local `Pose` coordinate frame, restoring parent pose automatically.
    #[inline]
    pub fn with_pose(&mut self, local_pose: Pose, f: impl FnOnce(&mut Self)) {
        self.pose_stack.push(self.cur_pose);
        self.cur_pose = self.cur_pose.compose(local_pose);
        f(self);
        self.cur_pose = self.pose_stack.pop().unwrap_or(Pose::IDENTITY);
    }

    #[inline]
    pub fn push_vert(&mut self, p_local: Vec3, n_local: Vec3, col: [f32; 3], mat: f32) -> u32 {
        let p = self.cur_pose.transform_point(p_local);
        let n = self.cur_pose.transform_normal(n_local);
        let idx = (self.verts.len() / FLOATS_PER_VERTEX) as u32;
        self.verts.extend_from_slice(&[
            p.x, p.y, p.z, n.x, n.y, n.z, col[0], col[1], col[2], mat,
        ]);
        idx
    }

    #[inline]
    pub fn push_quad(&mut self, p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, col: [f32; 3], mat: f32) {
        let n = p1.sub(p0).cross(p2.sub(p0)).normalized();
        let i0 = self.push_vert(p0, n, col, mat);
        let i1 = self.push_vert(p1, n, col, mat);
        let i2 = self.push_vert(p2, n, col, mat);
        let i3 = self.push_vert(p3, n, col, mat);
        self.indices.extend_from_slice(&[i0, i1, i2, i0, i2, i3]);
    }
}

/// Dynamic parameter snapshot passed when rebuilding the small moving-parts buffer.
#[derive(Clone, Copy, Debug)]
pub struct DynamicSceneParams {
    pub shoji_closed: f32,
    pub window_open: f32,
    pub window_rattle_m: f32,
    pub fan_angle_rad: f32,
    pub lamp_on: bool,
    pub main_breaker_on: bool,
    pub breaker_tripped: bool,
    pub ac_switch_on: bool,
    pub fan_switch_on: bool,
    pub kettle_switch_on: bool,
    pub toilet_door_open: f32,
    pub entry_door_open: f32,
    pub shuttle_x: f32,
    pub shuttle_door_open: f32,
    pub room_temp_c: f32,
    pub bus_power_w: f32,
    pub bus_voltage_v: f32,
    pub gauge_marks: [f32; 3],
    pub plume_intensity: f32,
    pub plume_age_s: f32,
    pub shockwave_radius_m: f32,
}

/// Center positions of the 3 wall-mounted analog gauges on the Top-Right Hab Control Panel.
pub const GAUGE_CENTERS: [Vec3; 3] = [
    Vec3::new(-1.58, 1.58, 1.954),
    Vec3::new(-1.88, 1.58, 1.954),
    Vec3::new(-2.18, 1.58, 1.954),
];

/// Switch X coordinates along bottom of the Top-Right Hab Control Panel (`y = 1.18, z = 1.954`).
pub const PANEL_SWITCH_XS: [f32; 5] = [-1.50, -1.69, -1.88, -2.07, -2.26];

/// Distant Uchinoura Pad-1 visual placement in exterior world coords (`z = 380m` framed in right-side sneak-view gap!).
pub const EXTERIOR_PAD_POS: Vec3 = Vec3::new(-58.0, 17.8, 380.0);

/// Builds the entire static Apartment + Exterior Japanese Street & Distant Pad in ONE draw call!
pub fn build_static_world(buf: &mut MeshBuffer) {
    buf.clear();
    hab::build_hab_204_static(buf);
    town::build_uchinoura_street_static(buf);
}

/// Rebuilds only the dynamic moving elements in ONE small draw call.
pub fn build_dynamic_parts(buf: &mut MeshBuffer, p: &DynamicSceneParams) {
    buf.clear();
    hab::build_hab_204_dynamic(buf, p);
    town::build_distant_pad_plume(buf, p);
}

/// Builds 3D Eidos AR Visor Causal Sensitivity Flow Lines.
pub fn build_eidos_vectors(buf: &mut MeshBuffer, shoji_closed: f32, ac_active: bool, kettle_active: bool) {
    buf.clear();
    let sun_beam_strength = 1.0 - 0.82 * shoji_closed;
    if sun_beam_strength > 0.05 {
        buf.add_beam(
            Vec3::new(0.0, 1.62, 1.94),
            Vec3::new(0.0, 0.88, 0.88),
            0.008 + 0.010 * sun_beam_strength,
            [1.0, 0.78, 0.24],
            MAT_EIDOS,
        );
    }
    if ac_active {
        buf.add_beam(
            Vec3::new(-0.15, 1.15, 0.88),
            Vec3::new(-1.88, 2.08, 1.72),
            0.010,
            [0.22, 0.88, 0.98],
            MAT_EIDOS,
        );
        buf.add_beam(
            Vec3::new(-1.92, 2.18, 1.94),
            Vec3::new(-1.92, 0.45, 2.38),
            0.010,
            [0.98, 0.45, 0.24],
            MAT_EIDOS,
        );
    }
    buf.add_beam(
        Vec3::new(-1.69, 1.28, 1.93),
        Vec3::new(-1.69, 2.06, 1.80),
        0.007,
        [0.28, 0.96, 0.62],
        MAT_EIDOS,
    );
    if kettle_active {
        buf.add_beam(
            Vec3::new(-2.26, 1.28, 1.93),
            Vec3::new(1.62, 1.06, 1.66),
            0.008,
            [1.0, 0.52, 0.22],
            MAT_EIDOS,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_and_dynamic_meshes_stay_within_igpu_triangle_budget() {
        let mut static_buf = MeshBuffer::new();
        build_static_world(&mut static_buf);
        let static_tris = static_buf.indices.len() / 3;
        println!("[geom] static mesh triangles: {}", static_tris);
        // Budget note: 30k was set when a grass tuft was three flat spikes.
        // Summer-noon grass needs real tufts (5 tapered, bent, double-sided
        // blades each), and a dense verge is worth ~8k triangles. 64k static
        // triangles is ~0.1 ms of vertex work on a Gen9 iGPU and changes no
        // fill rate, because a blade is a sliver of screen. Fill, draws and
        // per-frame allocation remain the real gates (PERFORMANCE.md §3).
        assert!(
            static_tris > 1000 && static_tris < 64_000,
            "static tris: {}",
            static_tris
        );

        let mut dyn_buf = MeshBuffer::new();
        build_dynamic_parts(
            &mut dyn_buf,
            &DynamicSceneParams {
                shoji_closed: 0.25,
                window_open: 0.15,
                window_rattle_m: 0.0,
                fan_angle_rad: 0.5,
                lamp_on: true,
                main_breaker_on: true,
                breaker_tripped: false,
                ac_switch_on: true,
                fan_switch_on: true,
                kettle_switch_on: false,
                toilet_door_open: 0.5,
                entry_door_open: 0.5,
                shuttle_x: -6.2,
                shuttle_door_open: 0.8,
                room_temp_c: 26.8,
                bus_power_w: 580.0,
                bus_voltage_v: 98.9,
                gauge_marks: [25.0, -1.0, -1.0],
                plume_intensity: 0.8,
                plume_age_s: 3.2,
                shockwave_radius_m: 1100.0,
            },
        );
        let dyn_tris = dyn_buf.indices.len() / 3;
        assert!(dyn_tris > 50 && dyn_tris < 5_000, "dyn tris: {}", dyn_tris);

        for (label, buf) in [("static", &static_buf), ("dynamic", &dyn_buf)] {
            for tri in buf.indices.chunks_exact(3) {
                let get_vn = |idx: u32| -> (Vec3, Vec3) {
                    let b = (idx as usize) * FLOATS_PER_VERTEX;
                    (
                        Vec3::new(buf.verts[b], buf.verts[b + 1], buf.verts[b + 2]),
                        Vec3::new(buf.verts[b + 3], buf.verts[b + 4], buf.verts[b + 5]),
                    )
                };
                let (p0, n0) = get_vn(tri[0]);
                let (p1, n1) = get_vn(tri[1]);
                let (p2, n2) = get_vn(tri[2]);
                let face_cross = p1.sub(p0).cross(p2.sub(p0));
                if face_cross.dot(face_cross) > 1e-10 {
                    let n_avg = n0.add(n1).add(n2);
                    assert!(
                        face_cross.dot(n_avg) > 0.0,
                        "{} mesh has inverted triangle winding at {:?}",
                        label,
                        p0
                    );
                }
            }
        }
    }
}
