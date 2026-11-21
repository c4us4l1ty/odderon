//! Zero-Asset Parametric Geometry Engine for Odderon (Checkpoint 1A v2 — Refined Room & Summer Coast).
//! Every wall rib, tatami mat, shoji lattice, L-galley sink/kettle, bonsai, CRT scope, VHF transceiver,
//! analog gauge, unit-bath interior, utility pole, catenary cable, pine tree, and rocket pad is pure math.

use odderon_dmath::{dcos, dexp, dsin, Vec3, PI, TAU};

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

/// Interleaved 10-float vertex: pos(3), normal(3), rgb(3), mat_id(1).
pub const FLOATS_PER_VERTEX: usize = 10;

pub struct MeshBuffer {
    pub verts: Vec<f32>,
    pub indices: Vec<u32>,
}

impl Default for MeshBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl MeshBuffer {
    pub fn new() -> Self {
        Self {
            verts: Vec::with_capacity(40960),
            indices: Vec::with_capacity(65536),
        }
    }

    pub fn clear(&mut self) {
        self.verts.clear();
        self.indices.clear();
    }

    #[inline]
    pub fn push_vert(&mut self, p: Vec3, n: Vec3, col: [f32; 3], mat: f32) -> u32 {
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

    /// Axis-aligned parametric box from `min` to `max`.
    pub fn add_box(&mut self, min: Vec3, max: Vec3, col: [f32; 3], mat: f32) {
        let (x0, y0, z0) = (min.x, min.y, min.z);
        let (x1, y1, z1) = (max.x, max.y, max.z);
        // +Z
        self.push_quad(
            Vec3::new(x0, y0, z1),
            Vec3::new(x1, y0, z1),
            Vec3::new(x1, y1, z1),
            Vec3::new(x0, y1, z1),
            col,
            mat,
        );
        // -Z
        self.push_quad(
            Vec3::new(x1, y0, z0),
            Vec3::new(x0, y0, z0),
            Vec3::new(x0, y1, z0),
            Vec3::new(x1, y1, z0),
            col,
            mat,
        );
        // +X
        self.push_quad(
            Vec3::new(x1, y0, z1),
            Vec3::new(x1, y0, z0),
            Vec3::new(x1, y1, z0),
            Vec3::new(x1, y1, z1),
            col,
            mat,
        );
        // -X
        self.push_quad(
            Vec3::new(x0, y0, z0),
            Vec3::new(x0, y0, z1),
            Vec3::new(x0, y1, z1),
            Vec3::new(x0, y1, z0),
            col,
            mat,
        );
        // +Y
        self.push_quad(
            Vec3::new(x0, y1, z1),
            Vec3::new(x1, y1, z1),
            Vec3::new(x1, y1, z0),
            Vec3::new(x0, y1, z0),
            col,
            mat,
        );
        // -Y
        self.push_quad(
            Vec3::new(x0, y0, z0),
            Vec3::new(x1, y0, z0),
            Vec3::new(x1, y0, z1),
            Vec3::new(x0, y0, z1),
            col,
            mat,
        );
    }

    /// Top-beveled / chamfered box for furniture cushions, electronics housings, AC units, and countertops.
    /// Catches crisp rim specular highlights without inflating triangle count (only 10 quads = 20 tris).
    pub fn add_bevel_box(&mut self, min: Vec3, max: Vec3, bev: f32, col: [f32; 3], mat: f32) {
        let b = bev
            .min((max.x - min.x) * 0.35)
            .min((max.y - min.y) * 0.45)
            .min((max.z - min.z) * 0.35);
        if b <= 1e-4 {
            self.add_box(min, max, col, mat);
            return;
        }
        let (x0, y0, z0) = (min.x, min.y, min.z);
        let (x1, y1, z1) = (max.x, max.y, max.z);
        let ym = y1 - b;
        let (ix0, ix1, iz0, iz1) = (x0 + b, x1 - b, z0 + b, z1 - b);

        // Lower vertical walls
        self.push_quad(Vec3::new(x0, y0, z1), Vec3::new(x1, y0, z1), Vec3::new(x1, ym, z1), Vec3::new(x0, ym, z1), col, mat);
        self.push_quad(Vec3::new(x1, y0, z0), Vec3::new(x0, y0, z0), Vec3::new(x0, ym, z0), Vec3::new(x1, ym, z0), col, mat);
        self.push_quad(Vec3::new(x1, y0, z1), Vec3::new(x1, y0, z0), Vec3::new(x1, ym, z0), Vec3::new(x1, ym, z1), col, mat);
        self.push_quad(Vec3::new(x0, y0, z0), Vec3::new(x0, y0, z1), Vec3::new(x0, ym, z1), Vec3::new(x0, ym, z0), col, mat);
        // Bottom cap
        self.push_quad(Vec3::new(x0, y0, z0), Vec3::new(x1, y0, z0), Vec3::new(x1, y0, z1), Vec3::new(x0, y0, z1), col, mat);
        // Top inset cap
        self.push_quad(Vec3::new(ix0, y1, iz1), Vec3::new(ix1, y1, iz1), Vec3::new(ix1, y1, iz0), Vec3::new(ix0, y1, iz0), col, mat);
        // 4 top 45-degree chamfer skirts
        self.push_quad(Vec3::new(x0, ym, z1), Vec3::new(x1, ym, z1), Vec3::new(ix1, y1, iz1), Vec3::new(ix0, y1, iz1), col, mat);
        self.push_quad(Vec3::new(x1, ym, z0), Vec3::new(x0, ym, z0), Vec3::new(ix0, y1, iz0), Vec3::new(ix1, y1, iz0), col, mat);
        self.push_quad(Vec3::new(x1, ym, z1), Vec3::new(x1, ym, z0), Vec3::new(ix1, y1, iz0), Vec3::new(ix1, y1, iz1), col, mat);
        self.push_quad(Vec3::new(x0, ym, z0), Vec3::new(x0, ym, z1), Vec3::new(ix0, y1, iz1), Vec3::new(ix0, y1, iz0), col, mat);
    }

    /// Oriented beam of square cross-section between `a` and `b`.
    pub fn add_beam(&mut self, a: Vec3, b: Vec3, radius: f32, col: [f32; 3], mat: f32) {
        let axis = b.sub(a);
        if axis.len() < 1e-5 {
            return;
        }
        let dir = axis.normalized();
        let up_ref = if dir.y.abs() < 0.92 {
            Vec3::new(0.0, 1.0, 0.0)
        } else {
            Vec3::new(1.0, 0.0, 0.0)
        };
        let u = dir.cross(up_ref).normalized().scale(radius);
        let v = dir.cross(u).normalized().scale(radius);

        let a0 = a.sub(u).sub(v);
        let a1 = a.add(u).sub(v);
        let a2 = a.add(u).add(v);
        let a3 = a.sub(u).add(v);

        let b0 = b.sub(u).sub(v);
        let b1 = b.add(u).sub(v);
        let b2 = b.add(u).add(v);
        let b3 = b.sub(u).add(v);

        self.push_quad(a0, a1, b1, b0, col, mat);
        self.push_quad(a1, a2, b2, b1, col, mat);
        self.push_quad(a2, a3, b3, b2, col, mat);
        self.push_quad(a3, a0, b0, b3, col, mat);
        self.push_quad(b0, b1, b2, b3, col, mat);
        self.push_quad(a3, a2, a1, a0, col, mat);
    }

    /// Smooth round cylinder or tapered cone between arbitrary 3D endpoints `a` and `b`!
    /// Ideal for lamp shades, knobs, pipes, faucets, chair legs, and handles.
    pub fn add_cylinder(
        &mut self,
        a: Vec3,
        b: Vec3,
        r_a: f32,
        r_b: f32,
        segs: usize,
        col: [f32; 3],
        mat: f32,
        cap_ends: bool,
    ) {
        let axis = b.sub(a);
        let len = axis.len();
        if len < 1e-5 || segs < 3 {
            return;
        }
        let dir = axis.scale(1.0 / len);
        let up_ref = if dir.y.abs() < 0.92 {
            Vec3::new(0.0, 1.0, 0.0)
        } else {
            Vec3::new(1.0, 0.0, 0.0)
        };
        let u = dir.cross(up_ref).normalized();
        let v = dir.cross(u).normalized();
        let slope = (r_a - r_b) / len;

        for i in 0..segs {
            let th0 = (i as f64) * TAU / (segs as f64);
            let th1 = ((i + 1) as f64) * TAU / (segs as f64);
            let (c0, s0) = (dcos(th0) as f32, dsin(th0) as f32);
            let (c1, s1) = (dcos(th1) as f32, dsin(th1) as f32);

            let rad0 = u.scale(c0).add(v.scale(s0));
            let rad1 = u.scale(c1).add(v.scale(s1));

            let p00 = a.add(rad0.scale(r_a));
            let p10 = a.add(rad1.scale(r_a));
            let p11 = b.add(rad1.scale(r_b));
            let p01 = b.add(rad0.scale(r_b));

            let n0 = rad0.add(dir.scale(slope)).normalized();
            let n1 = rad1.add(dir.scale(slope)).normalized();

            let v0 = self.push_vert(p00, n0, col, mat);
            let v1 = self.push_vert(p10, n1, col, mat);
            let v2 = self.push_vert(p11, n1, col, mat);
            let v3 = self.push_vert(p01, n0, col, mat);
            self.indices.extend_from_slice(&[v0, v1, v2, v0, v2, v3]);
        }

        if cap_ends {
            if r_a > 1e-4 {
                let na = dir.scale(-1.0);
                let ca = self.push_vert(a, na, col, mat);
                for i in 0..segs {
                    let th0 = (i as f64) * TAU / (segs as f64);
                    let th1 = ((i + 1) as f64) * TAU / (segs as f64);
                    let p0 = a.add(u.scale(r_a * dcos(th0) as f32)).add(v.scale(r_a * dsin(th0) as f32));
                    let p1 = a.add(u.scale(r_a * dcos(th1) as f32)).add(v.scale(r_a * dsin(th1) as f32));
                    let v0 = self.push_vert(p0, na, col, mat);
                    let v1 = self.push_vert(p1, na, col, mat);
                    self.indices.extend_from_slice(&[ca, v1, v0]);
                }
            }
            if r_b > 1e-4 {
                let nb = dir;
                let cb = self.push_vert(b, nb, col, mat);
                for i in 0..segs {
                    let th0 = (i as f64) * TAU / (segs as f64);
                    let th1 = ((i + 1) as f64) * TAU / (segs as f64);
                    let p0 = b.add(u.scale(r_b * dcos(th0) as f32)).add(v.scale(r_b * dsin(th0) as f32));
                    let p1 = b.add(u.scale(r_b * dcos(th1) as f32)).add(v.scale(r_b * dsin(th1) as f32));
                    let v0 = self.push_vert(p0, nb, col, mat);
                    let v1 = self.push_vert(p1, nb, col, mat);
                    self.indices.extend_from_slice(&[cb, v0, v1]);
                }
            }
        }
    }

    /// Parametric surface of revolution around vertical Y axis: `profile` is `[(y_local, radius)]`.
    pub fn add_lathe_y(
        &mut self,
        origin: Vec3,
        profile: &[(f32, f32)],
        radial_segs: usize,
        col: [f32; 3],
        mat: f32,
        cap_ends: bool,
    ) {
        if profile.len() < 2 || radial_segs < 3 {
            return;
        }
        let segs = radial_segs;
        for s in 0..(profile.len() - 1) {
            let (y0, r0) = profile[s];
            let (y1, r1) = profile[s + 1];
            let dy = y1 - y0;
            let dr = r1 - r0;
            for i in 0..segs {
                let th0 = (i as f64) * TAU / (segs as f64);
                let th1 = ((i + 1) as f64) * TAU / (segs as f64);
                let (c0, s0) = (dcos(th0) as f32, dsin(th0) as f32);
                let (c1, s1) = (dcos(th1) as f32, dsin(th1) as f32);

                let p00 = origin.add(Vec3::new(r0 * c0, y0, r0 * s0));
                let p10 = origin.add(Vec3::new(r0 * c1, y0, r0 * s1));
                let p11 = origin.add(Vec3::new(r1 * c1, y1, r1 * s1));
                let p01 = origin.add(Vec3::new(r1 * c0, y1, r1 * s0));

                let n0 = Vec3::new(dy * c0, -dr, dy * s0).normalized();
                let n1 = Vec3::new(dy * c1, -dr, dy * s1).normalized();

                let v0 = self.push_vert(p00, n0, col, mat);
                let v1 = self.push_vert(p10, n1, col, mat);
                let v2 = self.push_vert(p11, n1, col, mat);
                let v3 = self.push_vert(p01, n0, col, mat);
                self.indices.extend_from_slice(&[v0, v3, v2, v0, v2, v1]);
            }
        }
        if cap_ends {
            let (yb, rb) = profile[0];
            if rb > 1e-4 {
                let nb = Vec3::new(0.0, -1.0, 0.0);
                let cb = self.push_vert(origin.add(Vec3::new(0.0, yb, 0.0)), nb, col, mat);
                for i in 0..segs {
                    let th0 = (i as f64) * TAU / (segs as f64);
                    let th1 = ((i + 1) as f64) * TAU / (segs as f64);
                    let p0 = origin.add(Vec3::new(rb * dcos(th0) as f32, yb, rb * dsin(th0) as f32));
                    let p1 = origin.add(Vec3::new(rb * dcos(th1) as f32, yb, rb * dsin(th1) as f32));
                    let v0 = self.push_vert(p0, nb, col, mat);
                    let v1 = self.push_vert(p1, nb, col, mat);
                    self.indices.extend_from_slice(&[cb, v0, v1]);
                }
            }
            let (yt, rt) = profile[profile.len() - 1];
            if rt > 1e-4 {
                let nt = Vec3::new(0.0, 1.0, 0.0);
                let ct = self.push_vert(origin.add(Vec3::new(0.0, yt, 0.0)), nt, col, mat);
                for i in 0..segs {
                    let th0 = (i as f64) * TAU / (segs as f64);
                    let th1 = ((i + 1) as f64) * TAU / (segs as f64);
                    let p0 = origin.add(Vec3::new(rt * dcos(th0) as f32, yt, rt * dsin(th0) as f32));
                    let p1 = origin.add(Vec3::new(rt * dcos(th1) as f32, yt, rt * dsin(th1) as f32));
                    let v0 = self.push_vert(p0, nt, col, mat);
                    let v1 = self.push_vert(p1, nt, col, mat);
                    self.indices.extend_from_slice(&[ct, v1, v0]);
                }
            }
        }
    }

    /// Circular disc facing `-Z` (for wall-mounted analog gauge faces on the `+Z` control panel).
    pub fn add_disc_neg_z(&mut self, center: Vec3, radius: f32, segs: usize, col: [f32; 3], mat: f32) {
        let n = Vec3::new(0.0, 0.0, -1.0);
        let c_idx = self.push_vert(center, n, col, mat);
        for i in 0..segs {
            let a0 = (i as f64) * TAU / (segs as f64);
            let a1 = ((i + 1) as f64) * TAU / (segs as f64);
            let p0 = center.add(Vec3::new(radius * dcos(a0) as f32, radius * dsin(a0) as f32, 0.0));
            let p1 = center.add(Vec3::new(radius * dcos(a1) as f32, radius * dsin(a1) as f32, 0.0));
            let v0 = self.push_vert(p0, n, col, mat);
            let v1 = self.push_vert(p1, n, col, mat);
            self.indices.extend_from_slice(&[c_idx, v1, v0]);
        }
    }

    /// Parametric catenary cable y = sag * (cosh(u) - 1) between `p0` and `p1`.
    pub fn add_catenary(&mut self, p0: Vec3, p1: Vec3, sag: f32, segs: usize, radius: f32, col: [f32; 3]) {
        let mut prev = p0;
        for i in 1..=segs {
            let t = (i as f32) / (segs as f32);
            let u = ((t - 0.5) * 2.0) as f64;
            let cosh_u = 0.5 * (dexp(u) + dexp(-u));
            let cosh_1 = 0.5 * (dexp(1.0) + dexp(-1.0));
            let dip = sag * ((cosh_1 - cosh_u) / (cosh_1 - 1.0)) as f32;
            let cur = Vec3::new(
                p0.x + (p1.x - p0.x) * t,
                p0.y + (p1.y - p0.y) * t - dip,
                p0.z + (p1.z - p0.z) * t,
            );
            self.add_beam(prev, cur, radius, col, MAT_ALLOY);
            prev = cur;
        }
    }

    /// Helper: Japanese coastal black pine / camphor tree at `base` with organic multi-tier canopy cushions.
    pub fn add_coastal_tree(&mut self, base: Vec3, scale: f32) {
        let bark = [0.28, 0.19, 0.13];
        let leaf_deep = [0.11, 0.34, 0.16];
        let leaf_mid = [0.16, 0.44, 0.20];
        let top1 = base.add(Vec3::new(0.18 * scale, 1.9 * scale, -0.10 * scale));
        let top2 = base.add(Vec3::new(-0.22 * scale, 3.1 * scale, 0.14 * scale));
        let branch_r = base.add(Vec3::new(0.85 * scale, 2.5 * scale, -0.15 * scale));
        self.add_cylinder(base, top1, 0.16 * scale, 0.11 * scale, 7, bark, MAT_WOOD, false);
        self.add_cylinder(top1, top2, 0.11 * scale, 0.06 * scale, 6, bark, MAT_WOOD, false);
        self.add_cylinder(top1, branch_r, 0.08 * scale, 0.04 * scale, 6, bark, MAT_WOOD, false);

        let puffs = [
            (top2.x, top2.y - 0.1 * scale, top2.z, 1.15 * scale, leaf_mid),
            (branch_r.x, branch_r.y - 0.05 * scale, branch_r.z, 0.92 * scale, leaf_deep),
            (base.x - 0.55 * scale, base.y + 2.35 * scale, base.z + 0.35 * scale, 0.88 * scale, leaf_deep),
            (base.x + 0.10 * scale, base.y + 3.45 * scale, base.z - 0.05 * scale, 0.82 * scale, leaf_mid),
        ];
        for &(cx, cy, cz, cr, col) in &puffs {
            self.add_lathe_y(
                Vec3::new(cx, cy, cz),
                &[
                    (0.0, cr * 0.45),
                    (0.28 * cr, cr),
                    (0.68 * cr, cr * 0.86),
                    (0.98 * cr, 0.08 * cr),
                ],
                9,
                col,
                MAT_FOLIAGE,
                true,
            );
        }
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
    pub room_temp_c: f32,
    pub bus_power_w: f32,
    pub bus_voltage_v: f32,
    /// Optional yellow China-marker values on gauges [temp_c, power_w, voltage_v] (<0 means unset).
    pub gauge_marks: [f32; 3],
    /// Rocket static-fire plume normalized intensity [0.0 .. 1.0] and age in seconds.
    pub plume_intensity: f32,
    pub plume_age_s: f32,
    /// Expanding shockwave radius in meters from coastal pad (0 when inactive).
    pub shockwave_radius_m: f32,
}

/// Center positions of the 3 wall-mounted analog gauges on the Top-Right Hab Control Panel (`z = 1.956`).
/// Note: When facing the `+Z` window, `-X` is Screen-Right (matching the user's floorplan top-right corner!).
/// Left-to-right on screen when facing the panel (`+X` to `-X`):
///   Gauge 0 (`x = -1.58`): Room Air Temp (15 C .. 40 C)
///   Gauge 1 (`x = -1.88`): Hab Bus Power (0 W .. 1800 W)
///   Gauge 2 (`x = -2.18`): Hab Bus Voltage (85 V .. 105 V)
pub const GAUGE_CENTERS: [Vec3; 3] = [
    Vec3::new(-1.58, 1.58, 1.954),
    Vec3::new(-1.88, 1.58, 1.954),
    Vec3::new(-2.18, 1.58, 1.954),
];

/// Switch X coordinates along bottom of the Top-Right Hab Control Panel (`y = 1.18, z = 1.954`).
pub const PANEL_SWITCH_XS: [f32; 5] = [-1.50, -1.69, -1.88, -2.07, -2.26];

/// Builds the entire static Apartment + Exterior Coastal Launch Town in ONE single draw-call mesh!
/// Room coordinates (`5.20 m` wide x `4.16 m` deep x `2.68 m` high):
///   `+Z` (`z = +2.04`) = Window wall (Top of floorplan)
///   `-Z` (`z = -2.08`) = Back wall   (Bottom of floorplan: Bed left, Small Shelf center, Toilet right)
///   `+X` (`x = +2.58`) = Left wall when facing window  (Top-Left L-Kitchen, Bottom-Left Bed)
///   `-X` (`x = -2.58`) = Right wall when facing window (Top-Right AC/Panel/Bonsai, Entry Door, Bottom-Right Toilet)
pub fn build_static_world(buf: &mut MeshBuffer) {
    buf.clear();

    // Palette tuned for Makoto Shinkai "Shadows of a Summer Noon" + Japantown cedar + Star Citizen alloy
    let shikkui_plaster = [0.87, 0.84, 0.78];
    let alloy_panel = [0.46, 0.50, 0.55];
    let alloy_mid = [0.33, 0.37, 0.42];
    let alloy_dark = [0.18, 0.21, 0.25];
    let chrome = [0.82, 0.85, 0.88];
    let brass = [0.84, 0.67, 0.30];
    let cedar_dark = [0.30, 0.18, 0.11];
    let walnut_warm = [0.42, 0.26, 0.16];
    let hinoki_light = [0.74, 0.56, 0.36];
    let floor_slate = [0.22, 0.24, 0.27];
    let ceramic_white = [0.94, 0.95, 0.94];

    // =========================================================================
    // 1. APARTMENT SHELL & MODULAR STAR CITIZEN + JAPANESE TIMBER ARCHITECTURE
    // =========================================================================
    // Sub-floor & ceiling structural slabs
    buf.add_box(Vec3::new(-2.64, -0.14, -2.14), Vec3::new(2.64, 0.0, 2.10), floor_slate, MAT_ALLOY);
    buf.add_box(Vec3::new(-2.64, 2.68, -2.14), Vec3::new(2.64, 2.82, 2.10), alloy_dark, MAT_ALLOY);

    // Recessed perimeter ceiling soffit border (gives realistic architectural ceiling depth!)
    buf.add_box(Vec3::new(-2.60, 2.56, -2.08), Vec3::new(2.60, 2.68, -1.86), alloy_mid, MAT_ALLOY);
    buf.add_box(Vec3::new(-2.60, 2.56, 1.84), Vec3::new(2.60, 2.68, 2.06), alloy_mid, MAT_ALLOY);
    buf.add_box(Vec3::new(-2.60, 2.56, -1.86), Vec3::new(-2.36, 2.68, 1.84), alloy_mid, MAT_ALLOY);
    buf.add_box(Vec3::new(2.36, 2.56, -1.86), Vec3::new(2.60, 2.68, 1.84), alloy_mid, MAT_ALLOY);

    // Outer walls: Back (-Z), Left (+X), Right (-X)
    buf.add_box(Vec3::new(-2.62, 0.0, -2.14), Vec3::new(2.62, 2.68, -2.04), shikkui_plaster, MAT_MATTE);
    buf.add_box(Vec3::new(2.54, 0.0, -2.10), Vec3::new(2.64, 2.68, 2.08), shikkui_plaster, MAT_MATTE);
    buf.add_box(Vec3::new(-2.64, 0.0, -2.10), Vec3::new(-2.54, 2.68, 2.08), shikkui_plaster, MAT_MATTE);

    // Dark cedar Japanese baseboards around room perimeter
    buf.add_box(Vec3::new(-2.54, 0.0, 1.98), Vec3::new(2.54, 0.07, 2.02), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(2.50, 0.0, -2.04), Vec3::new(2.55, 0.07, 2.00), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(-2.55, 0.0, -0.28), Vec3::new(-2.50, 0.07, 2.00), cedar_dark, MAT_WOOD);

    // Window Wall (+Z = 2.02): Centered Panoramic Opening x in [-1.26, +1.26], y in [0.76, 2.28]
    // Below window sill
    buf.add_box(Vec3::new(-2.58, 0.0, 2.00), Vec3::new(2.58, 0.76, 2.10), shikkui_plaster, MAT_MATTE);
    // Header above window
    buf.add_box(Vec3::new(-2.58, 2.28, 2.00), Vec3::new(2.58, 2.68, 2.10), shikkui_plaster, MAT_MATTE);
    // Top-Left Pier (`x in [+1.26, +2.58]`, holds L-Kitchen window return!)
    buf.add_box(Vec3::new(1.26, 0.76, 2.00), Vec3::new(2.58, 2.28, 2.10), shikkui_plaster, MAT_MATTE);
    // Top-Right Pier (`x in [-2.58, -1.26]`, holds Split AC + Analog Gauge/Breaker Panel + Bonsai below!)
    buf.add_box(Vec3::new(-2.58, 0.76, 2.00), Vec3::new(-1.26, 2.28, 2.10), shikkui_plaster, MAT_MATTE);

    // Deep Japanese Cedar Window Casing, Sill, Slide Tracks & Center Mullion
    buf.add_bevel_box(Vec3::new(-1.32, 0.72, 1.91), Vec3::new(1.32, 0.77, 2.12), 0.008, cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(-1.32, 2.26, 1.93), Vec3::new(1.32, 2.33, 2.12), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(-1.32, 0.76, 1.94), Vec3::new(-1.25, 2.28, 2.12), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(1.25, 0.76, 1.94), Vec3::new(1.32, 2.28, 2.12), cedar_dark, MAT_WOOD);
    // Wooden slide rail grooves on sill
    buf.add_box(Vec3::new(-1.25, 0.768, 1.965), Vec3::new(1.25, 0.776, 1.975), walnut_warm, MAT_WOOD);
    buf.add_box(Vec3::new(-1.25, 0.768, 2.035), Vec3::new(1.25, 0.776, 2.045), alloy_dark, MAT_ALLOY);
    // Center mullion post
    buf.add_box(Vec3::new(-0.025, 0.77, 2.01), Vec3::new(0.025, 2.27, 2.07), cedar_dark, MAT_WOOD);

    // Star Citizen Modular Structural Wall Columns & Chamfered Ceiling Struts
    for &z_rib in &[-0.30_f32, 1.32] {
        buf.add_box(Vec3::new(-2.55, 0.0, z_rib - 0.045), Vec3::new(-2.47, 2.58, z_rib + 0.045), alloy_panel, MAT_ALLOY);
        buf.add_box(Vec3::new(2.47, 0.0, z_rib - 0.045), Vec3::new(2.55, 2.58, z_rib + 0.045), alloy_panel, MAT_ALLOY);
        buf.add_box(Vec3::new(-2.50, 2.50, z_rib - 0.04), Vec3::new(2.50, 2.58, z_rib + 0.04), alloy_dark, MAT_ALLOY);
        buf.add_beam(Vec3::new(-2.48, 2.22, z_rib), Vec3::new(-2.16, 2.52, z_rib), 0.030, alloy_panel, MAT_ALLOY);
        buf.add_beam(Vec3::new(2.48, 2.22, z_rib), Vec3::new(2.16, 2.52, z_rib), 0.030, alloy_panel, MAT_ALLOY);
    }

    // Woven Japanese Tatami Living Zone across center of room (`x in [-1.52, +1.78]`, `z in [-0.42, +1.94]`)
    let tatami_straw = [0.77, 0.73, 0.46];
    let tatami_straw_alt = [0.73, 0.70, 0.44];
    let tatami_heri = [0.13, 0.17, 0.22];
    // Cedar border frame around flush tatami inlay
    buf.add_box(Vec3::new(-1.56, 0.0, -0.44), Vec3::new(1.80, 0.024, 1.96), cedar_dark, MAT_WOOD);
    let mats = [
        (-1.52_f32, -0.40_f32, -0.44_f32, 0.76_f32, tatami_straw),
        (-0.41, -0.40, 0.68, 0.76, tatami_straw_alt),
        (0.71, -0.40, 1.76, 0.76, tatami_straw),
        (-1.52, 0.79, -0.44, 1.92, tatami_straw_alt),
        (-0.41, 0.79, 0.68, 1.92, tatami_straw),
        (0.71, 0.79, 1.76, 1.92, tatami_straw_alt),
    ];
    for &(mx0, mz0, mx1, mz1, col) in &mats {
        buf.add_box(Vec3::new(mx0, 0.020, mz0), Vec3::new(mx1, 0.030, mz1), tatami_heri, MAT_MATTE);
        buf.add_box(
            Vec3::new(mx0 + 0.032, 0.022, mz0 + 0.032),
            Vec3::new(mx1 - 0.032, 0.034, mz1 - 0.032),
            col,
            MAT_TATAMI,
        );
    }

    // =========================================================================
    // 2. RIGHT WALL ENTRY DOOR (`x = -2.54`, `z in [0.24, 1.24]`) — Star Citizen Bulkhead
    // =========================================================================
    // Chamfered steel frame + recessed multi-panel door leaf + vertical grab bar + biometric pad
    buf.add_box(Vec3::new(-2.55, 0.0, 0.18), Vec3::new(-2.45, 2.20, 1.30), alloy_dark, MAT_ALLOY);
    buf.add_bevel_box(Vec3::new(-2.49, 0.03, 0.26), Vec3::new(-2.42, 2.12, 1.22), 0.015, alloy_panel, MAT_ALLOY);
    // Horizontal recessed armor panel seams on door
    for &dy in &[0.58_f32, 1.15, 1.70] {
        buf.add_box(Vec3::new(-2.425, dy, 0.30), Vec3::new(-2.415, dy + 0.018, 1.18), alloy_dark, MAT_ALLOY);
    }
    // Brushed cylindrical grab handle + emerald lock indicator
    buf.add_cylinder(Vec3::new(-2.38, 0.88, 1.08), Vec3::new(-2.38, 1.32, 1.08), 0.013, 0.013, 10, chrome, MAT_ALLOY, true);
    buf.add_cylinder(Vec3::new(-2.43, 0.92, 1.08), Vec3::new(-2.38, 0.92, 1.08), 0.010, 0.010, 8, alloy_dark, MAT_ALLOY, false);
    buf.add_cylinder(Vec3::new(-2.43, 1.28, 1.08), Vec3::new(-2.38, 1.28, 1.08), 0.010, 0.010, 8, alloy_dark, MAT_ALLOY, false);
    buf.add_box(Vec3::new(-2.45, 1.16, 1.33), Vec3::new(-2.43, 1.34, 1.42), alloy_dark, MAT_ALLOY);
    buf.add_box(Vec3::new(-2.428, 1.20, 1.35), Vec3::new(-2.422, 1.30, 1.40), [0.22, 0.95, 0.62], MAT_CRT);
    // Genkan dark slate threshold mat
    buf.add_bevel_box(Vec3::new(-2.42, 0.0, 0.24), Vec3::new(-1.82, 0.018, 1.24), 0.006, [0.15, 0.17, 0.19], MAT_ALLOY);

    // =========================================================================
    // 3. TOP-LEFT CORNER: L-SHAPED KITCHEN AREA (`x in [1.34, 2.54]`, `z in [0.16, 2.00]`)
    // =========================================================================
    // Base cabinets: Leg A along window wall (`x: 1.34..2.54, z: 1.38..2.00`) + Leg B along left wall (`x: 1.88..2.54, z: 0.16..1.40`)
    buf.add_box(Vec3::new(1.36, 0.0, 1.42), Vec3::new(2.54, 0.08, 2.00), [0.11, 0.12, 0.14], MAT_ALLOY); // toe-kick A
    buf.add_box(Vec3::new(1.92, 0.0, 0.18), Vec3::new(2.54, 0.08, 1.42), [0.11, 0.12, 0.14], MAT_ALLOY); // toe-kick B
    buf.add_box(Vec3::new(1.34, 0.08, 1.40), Vec3::new(2.54, 0.86, 2.00), alloy_dark, MAT_ALLOY);
    buf.add_box(Vec3::new(1.88, 0.08, 0.16), Vec3::new(2.54, 0.86, 1.40), alloy_dark, MAT_ALLOY);

    // Warm walnut cabinet door fronts + brushed pulls
    for &(x0, x1) in &[(1.37_f32, 1.84_f32)] {
        buf.add_bevel_box(Vec3::new(x0, 0.12, 1.382), Vec3::new(x1, 0.82, 1.40), 0.008, walnut_warm, MAT_WOOD);
        buf.add_box(Vec3::new(x0 + 0.14, 0.74, 1.368), Vec3::new(x1 - 0.14, 0.76, 1.382), chrome, MAT_ALLOY);
    }
    for &(z0, z1) in &[(0.20_f32, 0.76_f32), (0.80, 1.36)] {
        buf.add_bevel_box(Vec3::new(1.862, 0.12, z0), Vec3::new(1.88, 0.82, z1), 0.008, walnut_warm, MAT_WOOD);
        buf.add_box(Vec3::new(1.848, 0.74, z0 + 0.16), Vec3::new(1.862, 0.76, z1 - 0.16), chrome, MAT_ALLOY);
    }

    // Beveled Brushed-Stone / Stainless L-Countertop + Backsplash
    let counter_col = [0.78, 0.80, 0.81];
    buf.add_bevel_box(Vec3::new(1.32, 0.86, 1.36), Vec3::new(2.55, 0.91, 2.01), 0.010, counter_col, MAT_ALLOY);
    buf.add_bevel_box(Vec3::new(1.85, 0.86, 0.14), Vec3::new(2.55, 0.91, 1.38), 0.010, counter_col, MAT_ALLOY);
    // Backsplash lip
    buf.add_box(Vec3::new(1.34, 0.91, 1.96), Vec3::new(2.54, 1.04, 2.00), [0.66, 0.69, 0.72], MAT_CERAMIC);
    buf.add_box(Vec3::new(2.50, 0.91, 0.16), Vec3::new(2.54, 1.04, 1.98), [0.66, 0.69, 0.72], MAT_CERAMIC);

    // Recessed Stainless Sink Basin (`x in [2.00, 2.42], z in [0.34, 0.82]`) + Gooseneck Faucet!
    buf.add_box(Vec3::new(1.98, 0.908, 0.32), Vec3::new(2.44, 0.916, 0.84), [0.56, 0.59, 0.63], MAT_ALLOY);
    buf.add_box(Vec3::new(2.02, 0.912, 0.36), Vec3::new(2.40, 0.918, 0.80), [0.16, 0.19, 0.22], MAT_ALLOY);
    // Drain strainer
    buf.add_lathe_y(Vec3::new(2.22, 0.919, 0.58), &[(0.0, 0.032), (0.003, 0.032)], 10, chrome, MAT_ALLOY, true);
    // Curved Gooseneck Faucet + Mixer Lever
    buf.add_cylinder(Vec3::new(2.44, 0.91, 0.58), Vec3::new(2.44, 1.15, 0.58), 0.015, 0.013, 10, chrome, MAT_ALLOY, true);
    buf.add_cylinder(Vec3::new(2.44, 1.15, 0.58), Vec3::new(2.30, 1.21, 0.58), 0.012, 0.011, 10, chrome, MAT_ALLOY, true);
    buf.add_cylinder(Vec3::new(2.30, 1.21, 0.58), Vec3::new(2.25, 1.12, 0.58), 0.011, 0.010, 10, chrome, MAT_ALLOY, true);
    buf.add_cylinder(Vec3::new(2.44, 0.96, 0.64), Vec3::new(2.38, 1.01, 0.69), 0.006, 0.005, 8, chrome, MAT_ALLOY, true);

    // Flush Black Glass-Ceramic Induction Hob (`x in [1.98, 2.42], z in [0.96, 1.34]`)
    buf.add_bevel_box(Vec3::new(1.98, 0.908, 0.96), Vec3::new(2.42, 0.918, 1.34), 0.004, [0.09, 0.10, 0.12], MAT_ALLOY);
    buf.add_lathe_y(Vec3::new(2.20, 0.919, 1.15), &[(0.0, 0.10), (0.002, 0.105)], 16, [0.32, 0.35, 0.38], MAT_ALLOY, true);

    // Sculpted Parametric Stainless Electric Kettle (`TargetId::KettleBody`, `x = 1.62, y = 0.91, z = 1.66`)
    // Power base puck
    buf.add_lathe_y(
        Vec3::new(1.62, 0.91, 1.66),
        &[(0.0, 0.102), (0.022, 0.098), (0.026, 0.088)],
        16,
        [0.14, 0.15, 0.17],
        MAT_ALLOY,
        true,
    );
    // Brushed stainless Carafe body + domed lid + finial knob
    buf.add_lathe_y(
        Vec3::new(1.62, 0.935, 1.66),
        &[
            (0.0, 0.094),
            (0.03, 0.096),
            (0.13, 0.082),
            (0.19, 0.072),
            (0.205, 0.075),
            (0.215, 0.068),
            (0.228, 0.022),
        ],
        18,
        chrome,
        MAT_ALLOY,
        true,
    );
    buf.add_lathe_y(
        Vec3::new(1.62, 1.163, 1.66),
        &[(0.0, 0.012), (0.015, 0.020), (0.022, 0.016)],
        12,
        [0.14, 0.15, 0.17],
        MAT_MATTE,
        true,
    );
    // Ergonomic D-loop handle (top bridge, grip bar, bottom bridge) + tapered pouring spout
    buf.add_cylinder(Vec3::new(1.69, 1.11, 1.66), Vec3::new(1.755, 1.10, 1.66), 0.010, 0.010, 8, [0.14, 0.15, 0.17], MAT_MATTE, true);
    buf.add_cylinder(Vec3::new(1.755, 1.10, 1.66), Vec3::new(1.765, 0.97, 1.66), 0.011, 0.010, 8, [0.14, 0.15, 0.17], MAT_MATTE, true);
    buf.add_cylinder(Vec3::new(1.695, 0.97, 1.66), Vec3::new(1.765, 0.97, 1.66), 0.010, 0.010, 8, [0.14, 0.15, 0.17], MAT_MATTE, true);
    buf.add_cylinder(Vec3::new(1.55, 1.09, 1.66), Vec3::new(1.495, 1.155, 1.66), 0.018, 0.010, 10, chrome, MAT_ALLOY, false);

    // Two Lathed Japanese Yunomi Ceramic Tea Cups + Hinoki Cutting Board on Counter!
    buf.add_bevel_box(Vec3::new(1.94, 0.91, 1.54), Vec3::new(2.24, 0.928, 1.78), 0.005, hinoki_light, MAT_WOOD);
    for &(cx, cz, col) in &[(2.04_f32, 1.64_f32, [0.24, 0.38, 0.42]), (2.15, 1.69, [0.86, 0.83, 0.76])] {
        buf.add_lathe_y(
            Vec3::new(cx, 0.928, cz),
            &[(0.0, 0.026), (0.015, 0.032), (0.068, 0.035), (0.072, 0.033)],
            12,
            col,
            MAT_CERAMIC,
            true,
        );
    }

    // Overhead L-Shaped Galley Cabinets + Warm 2700K Under-Cabinet Task LED Strip
    buf.add_bevel_box(Vec3::new(1.36, 1.72, 1.60), Vec3::new(2.54, 2.46, 2.00), 0.012, walnut_warm, MAT_WOOD);
    buf.add_bevel_box(Vec3::new(2.12, 1.72, 0.18), Vec3::new(2.54, 2.46, 1.62), 0.012, walnut_warm, MAT_WOOD);
    buf.add_box(Vec3::new(1.42, 1.705, 1.66), Vec3::new(2.46, 1.72, 1.70), [1.0, 0.76, 0.40], MAT_EMISSIVE_WARM);
    buf.add_box(Vec3::new(2.16, 1.705, 0.26), Vec3::new(2.20, 1.72, 1.60), [1.0, 0.76, 0.40], MAT_EMISSIVE_WARM);

    // =========================================================================
    // 4. TOP-CENTER: ENGINEERING TABLE & HIGH-CRAFT DESK INSTRUMENTS (`x in [-1.18, +1.18]`)
    // =========================================================================
    // Beveled solid Hinoki wood desktop (`z in [1.26, 1.95]`, `y = 0.71..0.755`)
    buf.add_bevel_box(Vec3::new(-1.18, 0.71, 1.26), Vec3::new(1.18, 0.755, 1.95), 0.012, hinoki_light, MAT_WOOD);
    // Sleek dark-alloy sled legs + modesty crossbar + under-desk drawer unit on left side
    for &lx in &[-1.10_f32, 1.10] {
        buf.add_cylinder(Vec3::new(lx, 0.03, 1.32), Vec3::new(lx, 0.71, 1.32), 0.022, 0.022, 10, alloy_dark, MAT_ALLOY, true);
        buf.add_cylinder(Vec3::new(lx, 0.03, 1.88), Vec3::new(lx, 0.71, 1.88), 0.022, 0.022, 10, alloy_dark, MAT_ALLOY, true);
        buf.add_bevel_box(Vec3::new(lx - 0.03, 0.025, 1.28), Vec3::new(lx + 0.03, 0.055, 1.92), 0.008, alloy_dark, MAT_ALLOY);
    }
    buf.add_cylinder(Vec3::new(-1.10, 0.22, 1.82), Vec3::new(1.10, 0.22, 1.82), 0.016, 0.016, 8, alloy_dark, MAT_ALLOY, false);
    // Under-desk suspended 2-drawer pedestal (`x in [0.68, 1.06]`)
    buf.add_bevel_box(Vec3::new(0.68, 0.40, 1.34), Vec3::new(1.06, 0.71, 1.86), 0.008, alloy_mid, MAT_ALLOY);
    buf.add_box(Vec3::new(0.80, 0.62, 1.328), Vec3::new(0.94, 0.635, 1.34), chrome, MAT_ALLOY);
    buf.add_box(Vec3::new(0.80, 0.48, 1.328), Vec3::new(0.94, 0.495, 1.34), chrome, MAT_ALLOY);

    // Sculpted Ergonomic Task Chair centered at `(0.0, z = 0.84)` facing the window (`+Z`)!
    let ch_x = 0.0_f32;
    let ch_z = 0.84_f32;
    // 5-star caster hub + 5 angled legs + wheel casters
    buf.add_cylinder(Vec3::new(ch_x, 0.08, ch_z), Vec3::new(ch_x, 0.45, ch_z), 0.028, 0.024, 12, alloy_dark, MAT_ALLOY, true);
    for leg in 0..5 {
        let ang = (leg as f64) * TAU / 5.0;
        let (ca, sa) = (dcos(ang) as f32, dsin(ang) as f32);
        let tip = Vec3::new(ch_x + 0.28 * ca, 0.055, ch_z + 0.28 * sa);
        buf.add_cylinder(Vec3::new(ch_x, 0.095, ch_z), tip, 0.018, 0.013, 8, alloy_mid, MAT_ALLOY, true);
        buf.add_lathe_y(Vec3::new(tip.x, 0.025, tip.z), &[(0.0, 0.024), (0.032, 0.024)], 8, [0.10, 0.11, 0.12], MAT_ALLOY, true);
    }
    // Contoured upholstered seat cushion
    let fabric_charcoal = [0.19, 0.22, 0.26];
    buf.add_bevel_box(Vec3::new(ch_x - 0.24, 0.44, ch_z - 0.22), Vec3::new(ch_x + 0.24, 0.51, ch_z + 0.22), 0.024, fabric_charcoal, MAT_MATTE);
    // Curved spine post + sculpted lumbar & upper backrest (on `-Z` side so player sits facing `+Z`!)
    buf.add_cylinder(Vec3::new(ch_x, 0.46, ch_z - 0.18), Vec3::new(ch_x, 0.86, ch_z - 0.25), 0.020, 0.016, 8, alloy_dark, MAT_ALLOY, true);
    buf.add_bevel_box(Vec3::new(ch_x - 0.22, 0.56, ch_z - 0.27), Vec3::new(ch_x + 0.22, 0.94, ch_z - 0.20), 0.022, fabric_charcoal, MAT_MATTE);
    // Twin L-armrests
    for &side in &[-0.255_f32, 0.255] {
        buf.add_cylinder(Vec3::new(ch_x + side, 0.47, ch_z - 0.04), Vec3::new(ch_x + side, 0.65, ch_z - 0.04), 0.012, 0.012, 8, alloy_dark, MAT_ALLOY, false);
        buf.add_bevel_box(Vec3::new(ch_x + side - 0.032, 0.65, ch_z - 0.16), Vec3::new(ch_x + side + 0.032, 0.68, ch_z + 0.10), 0.008, [0.14, 0.16, 0.18], MAT_MATTE);
    }

    // --- DESK ITEM A (Screen-Left of Desk, `x in [0.54, 1.06]`, `z in [1.44, 1.84]`):
    //     RETRO-FUTURISTIC CRT VECTOR TELEMETRY OSCILLOSCOPE ---
    // Main beveled chassis + tapered rear CRT neck housing
    buf.add_bevel_box(Vec3::new(0.54, 0.755, 1.46), Vec3::new(1.06, 1.11, 1.80), 0.018, [0.34, 0.38, 0.40], MAT_ALLOY);
    buf.add_bevel_box(Vec3::new(0.62, 0.78, 1.79), Vec3::new(0.98, 1.06, 1.88), 0.025, alloy_dark, MAT_ALLOY);
    // Front glare hood bezel around CRT screen (facing `-Z` toward player!)
    buf.add_box(Vec3::new(0.68, 0.79, 1.425), Vec3::new(1.03, 1.07, 1.465), alloy_dark, MAT_ALLOY);
    // Glowing phosphor vector CRT screen (`x in [0.70, 1.01], y in [0.81, 1.05]`)
    buf.add_box(Vec3::new(0.70, 0.81, 1.436), Vec3::new(1.01, 1.05, 1.448), [0.14, 0.84, 0.76], MAT_CRT);
    // Left-hand instrument control column (`x in [0.56, 0.67]`): 3 lathed rotary knobs + BNC jacks + power LED!
    buf.add_box(Vec3::new(0.56, 0.79, 1.452), Vec3::new(0.665, 1.07, 1.462), [0.20, 0.23, 0.26], MAT_ALLOY);
    for &ky in &[1.01_f32, 0.93, 0.85] {
        buf.add_cylinder(
            Vec3::new(0.612, ky, 1.455),
            Vec3::new(0.612, ky, 1.428),
            0.020,
            0.017,
            12,
            alloy_dark,
            MAT_ALLOY,
            true,
        );
        buf.add_cylinder(
            Vec3::new(0.612, ky, 1.428),
            Vec3::new(0.612, ky, 1.422),
            0.008,
            0.008,
            8,
            chrome,
            MAT_ALLOY,
            true,
        );
    }
    // Top aluminum carry handle
    buf.add_cylinder(Vec3::new(0.64, 1.11, 1.56), Vec3::new(0.64, 1.145, 1.56), 0.008, 0.008, 8, chrome, MAT_ALLOY, false);
    buf.add_cylinder(Vec3::new(0.96, 1.11, 1.56), Vec3::new(0.96, 1.145, 1.56), 0.008, 0.008, 8, chrome, MAT_ALLOY, false);
    buf.add_cylinder(Vec3::new(0.64, 1.145, 1.56), Vec3::new(0.96, 1.145, 1.56), 0.009, 0.009, 8, chrome, MAT_ALLOY, true);

    // --- DESK ITEM B (Center of Desk, `x in [-0.14, 0.26]`, `z in [1.34, 1.68]`):
    //     PAD TECHNICIAN SHIFT CLIPBOARD (`TargetId::Clipboard`) + BRASS PEN ---
    buf.add_bevel_box(Vec3::new(-0.14, 0.756, 1.34), Vec3::new(0.24, 0.770, 1.68), 0.004, walnut_warm, MAT_WOOD);
    buf.add_box(Vec3::new(-0.11, 0.770, 1.37), Vec3::new(0.21, 0.776, 1.64), [0.96, 0.95, 0.90], MAT_MATTE);
    // Header accent stripe + yellow calibration sticky note + spring clip
    buf.add_box(Vec3::new(-0.09, 0.776, 1.58), Vec3::new(0.19, 0.778, 1.62), [0.22, 0.34, 0.46], MAT_MATTE);
    buf.add_box(Vec3::new(-0.09, 0.776, 1.39), Vec3::new(0.01, 0.779, 1.47), [0.98, 0.88, 0.34], MAT_MATTE);
    buf.add_bevel_box(Vec3::new(-0.01, 0.774, 1.62), Vec3::new(0.11, 0.792, 1.67), 0.004, chrome, MAT_ALLOY);
    // Brass drafting pen resting beside clipboard
    buf.add_cylinder(Vec3::new(-0.20, 0.763, 1.40), Vec3::new(-0.23, 0.763, 1.58), 0.006, 0.005, 8, brass, MAT_ALLOY, true);

    // --- DESK ITEM C (Center-Right of Desk, `x = -0.38, z = 1.72`):
    //     ARTICULATED ARCHITECT DESK LAMP (Lathed Base + Twin Boom Arms + Conical Bell Shade!) ---
    buf.add_lathe_y(
        Vec3::new(-0.38, 0.755, 1.72),
        &[(0.0, 0.088), (0.018, 0.084), (0.028, 0.024), (0.055, 0.018)],
        16,
        alloy_dark,
        MAT_ALLOY,
        true,
    );
    let elbow = Vec3::new(-0.42, 1.06, 1.76);
    let wrist = Vec3::new(-0.32, 1.22, 1.56);
    let shade_tip = Vec3::new(-0.26, 1.10, 1.46);
    // Twin parallel lower bars + upper arm + brass hinge knuckles
    buf.add_cylinder(Vec3::new(-0.37, 0.80, 1.72), elbow, 0.007, 0.007, 8, chrome, MAT_ALLOY, false);
    buf.add_cylinder(Vec3::new(-0.39, 0.80, 1.73), elbow.add(Vec3::new(-0.02, 0.0, 0.01)), 0.007, 0.007, 8, chrome, MAT_ALLOY, false);
    buf.add_cylinder(elbow, wrist, 0.008, 0.007, 8, chrome, MAT_ALLOY, true);
    buf.add_lathe_y(elbow.sub(Vec3::new(0.0, 0.015, 0.0)), &[(0.0, 0.018), (0.03, 0.018)], 10, brass, MAT_ALLOY, true);
    // Sculpted Conical Enamel Bell Shade (`wrist -> shade_tip`)!
    let lamp_enamel = [0.18, 0.36, 0.32];
    buf.add_cylinder(wrist, shade_tip, 0.022, 0.082, 16, lamp_enamel, MAT_ALLOY, true);

    // --- DESK ITEM D (Screen-Right of Desk, `x in [-1.06, -0.56]`, `z in [1.46, 1.80]`):
    //     COASTAL RANGE VHF PAD-1 TRANSCEIVER (`TargetId::VhfPadRadio`) ---
    buf.add_bevel_box(Vec3::new(-1.06, 0.755, 1.46), Vec3::new(-0.56, 0.94, 1.78), 0.014, [0.22, 0.26, 0.30], MAT_ALLOY);
    // Recessed dark front faceplate
    buf.add_box(Vec3::new(-1.03, 0.775, 1.452), Vec3::new(-0.59, 0.915, 1.462), [0.11, 0.13, 0.15], MAT_ALLOY);
    // Backlit cyan-amber frequency OLED window (`149.62 MHz`)
    buf.add_box(Vec3::new(-1.00, 0.855, 1.448), Vec3::new(-0.78, 0.898, 1.455), [0.16, 0.78, 0.72], MAT_CRT);
    // Knurled VFO tuning dial + squelch knob
    buf.add_cylinder(Vec3::new(-0.70, 0.84, 1.452), Vec3::new(-0.70, 0.84, 1.422), 0.030, 0.027, 14, alloy_panel, MAT_ALLOY, true);
    buf.add_cylinder(Vec3::new(-0.62, 0.87, 1.452), Vec3::new(-0.62, 0.87, 1.432), 0.014, 0.012, 10, chrome, MAT_ALLOY, true);
    // Speaker louvre slats under frequency window
    for s in 0..4 {
        let sy = 0.788 + (s as f32) * 0.014;
        buf.add_box(Vec3::new(-1.00, sy, 1.448), Vec3::new(-0.84, sy + 0.006, 1.454), alloy_mid, MAT_ALLOY);
    }
    // Guarded Red `PAD-1 STATIC FIRE` pushbutton (`x = -0.62, y = 0.805`)
    buf.add_box(Vec3::new(-0.65, 0.782, 1.442), Vec3::new(-0.59, 0.832, 1.455), brass, MAT_ALLOY);
    buf.add_cylinder(Vec3::new(-0.62, 0.807, 1.450), Vec3::new(-0.62, 0.807, 1.432), 0.018, 0.016, 12, [0.96, 0.20, 0.14], MAT_EMISSIVE_WARM, true);
    // BNC spring base + tall stainless whip antenna rising cleanly in front of right window edge
    buf.add_lathe_y(Vec3::new(-0.98, 0.94, 1.72), &[(0.0, 0.020), (0.045, 0.016), (0.08, 0.008)], 10, chrome, MAT_ALLOY, true);
    buf.add_cylinder(Vec3::new(-0.98, 1.02, 1.72), Vec3::new(-1.01, 1.62, 1.75), 0.005, 0.0025, 8, chrome, MAT_ALLOY, true);
    // Handheld PTT Mic resting on side cradle
    buf.add_bevel_box(Vec3::new(-0.555, 0.80, 1.54), Vec3::new(-0.515, 0.89, 1.62), 0.010, [0.14, 0.16, 0.18], MAT_MATTE);

    // =========================================================================
    // 5. TOP-RIGHT CORNER (`x in [-2.52, -1.34]`):
    //    SPLIT AC + WALL TELEMETRY PANEL + SMALL BONSAI TABLE (`image-3.png`!)
    // =========================================================================
    // A. Sculpted Split AC Evaporator Unit high on Top-Right Wall (`x in [-2.42, -1.38]`, `y in [2.04, 2.38]`, `z in [1.68, 2.00]`)
    buf.add_bevel_box(Vec3::new(-2.42, 2.06, 1.68), Vec3::new(-1.38, 2.38, 2.00), 0.032, [0.90, 0.91, 0.89], MAT_ALLOY);
    // Top intake vent ribs + front seam line + green power LED
    buf.add_box(Vec3::new(-2.36, 2.22, 1.674), Vec3::new(-1.44, 2.232, 1.682), alloy_panel, MAT_ALLOY);
    buf.add_box(Vec3::new(-1.48, 2.16, 1.672), Vec3::new(-1.45, 2.18, 1.682), [0.22, 0.92, 0.58], MAT_CRT);
    // Recessed cold-air discharge throat
    buf.add_box(Vec3::new(-2.36, 2.055, 1.685), Vec3::new(-1.44, 2.115, 1.76), alloy_dark, MAT_ALLOY);

    // B. Recessed Industrial Hab Analog Gauge & Breaker Panel (`x in [-2.38, -1.38]`, `y in [1.04, 1.82]`, `z in [1.955, 2.00]`)
    buf.add_bevel_box(Vec3::new(-2.38, 1.04, 1.958), Vec3::new(-1.38, 1.82, 2.00), 0.014, alloy_dark, MAT_ALLOY);
    buf.add_box(Vec3::new(-2.34, 1.08, 1.954), Vec3::new(-1.42, 1.78, 1.962), [0.27, 0.31, 0.35], MAT_ALLOY);
    // Corner hex bolts on panel
    for &(bx, by) in &[(-2.35_f32, 1.79_f32), (-1.41, 1.79), (-2.35, 1.07), (-1.41, 1.07)] {
        buf.add_disc_neg_z(Vec3::new(bx, by, 1.952), 0.012, 6, chrome, MAT_ALLOY);
    }

    // Three circular analog instrument bezels, cream dials, tick marks & dark engraved label plates!
    for &gc in &GAUGE_CENTERS {
        buf.add_cylinder(
            Vec3::new(gc.x, gc.y, 1.960),
            Vec3::new(gc.x, gc.y, 1.944),
            0.112,
            0.106,
            24,
            [0.13, 0.15, 0.18],
            MAT_ALLOY,
            true,
        );
        buf.add_disc_neg_z(Vec3::new(gc.x, gc.y, 1.943), 0.095, 24, [0.95, 0.93, 0.87], MAT_MATTE);
        // Brass center pivot cap
        buf.add_disc_neg_z(Vec3::new(gc.x, gc.y, 1.936), 0.015, 12, brass, MAT_ALLOY);
        // Scale ticks around 240-deg sweep
        for t in 0..=10 {
            let frac = (t as f64) / 10.0;
            let ang = (210.0 - frac * 240.0) * PI / 180.0;
            let (ca, sa) = (dcos(ang) as f32, dsin(ang) as f32);
            let r_in = if t % 2 == 0 { 0.070 } else { 0.077 };
            let p_in = Vec3::new(gc.x + r_in * ca, gc.y + r_in * sa, 1.941);
            let p_out = Vec3::new(gc.x + 0.089 * ca, gc.y + 0.089 * sa, 1.941);
            let tick_col = if t >= 8 { [0.85, 0.22, 0.16] } else { [0.16, 0.18, 0.20] };
            buf.add_beam(p_in, p_out, 0.0028, tick_col, MAT_MATTE);
        }
        // Engraved nameplate below each gauge
        buf.add_box(Vec3::new(gc.x - 0.068, gc.y - 0.145, 1.948), Vec3::new(gc.x + 0.068, gc.y - 0.120, 1.955), [0.11, 0.12, 0.14], MAT_ALLOY);
    }

    // Physical Copper/Steel Conduits connecting Panel -> AC Unit -> Ceiling Fan -> Kitchen Kettle!
    let copper_pipe = [0.76, 0.49, 0.30];
    let conduit_steel = [0.54, 0.58, 0.63];
    buf.add_cylinder(Vec3::new(-1.68, 1.82, 1.97), Vec3::new(-1.68, 2.06, 1.92), 0.011, 0.011, 8, conduit_steel, MAT_ALLOY, false);
    buf.add_cylinder(Vec3::new(-2.24, 2.06, 1.90), Vec3::new(-2.24, 1.92, 2.08), 0.016, 0.016, 8, copper_pipe, MAT_ALLOY, false);
    buf.add_cylinder(Vec3::new(-1.95, 1.82, 1.97), Vec3::new(-1.95, 2.53, 1.97), 0.010, 0.010, 8, conduit_steel, MAT_ALLOY, false);
    buf.add_cylinder(Vec3::new(-1.95, 2.53, 1.97), Vec3::new(0.0, 2.53, 0.25), 0.010, 0.010, 8, conduit_steel, MAT_ALLOY, false);
    buf.add_cylinder(Vec3::new(-1.95, 2.53, 1.97), Vec3::new(1.62, 2.53, 1.97), 0.010, 0.010, 8, conduit_steel, MAT_ALLOY, false);
    buf.add_cylinder(Vec3::new(1.62, 2.53, 1.97), Vec3::new(1.62, 1.15, 1.99), 0.009, 0.009, 8, conduit_steel, MAT_ALLOY, false);

    // C. "the small table and the bonsai plant" (`x in [-2.46, -1.44]`, `z in [1.44, 1.96]`, `y = 0.0..0.54`)
    // Crafted Japanese Keyaki low table with 4 tapered legs, lower slatted stretcher shelf, and beveled top!
    buf.add_bevel_box(Vec3::new(-2.46, 0.49, 1.44), Vec3::new(-1.44, 0.54, 1.96), 0.010, walnut_warm, MAT_WOOD);
    for &(tx, tz) in &[(-2.40_f32, 1.49_f32), (-1.50, 1.49), (-2.40, 1.91), (-1.50, 1.91)] {
        buf.add_cylinder(Vec3::new(tx, 0.0, tz), Vec3::new(tx, 0.49, tz), 0.020, 0.026, 8, cedar_dark, MAT_WOOD, false);
    }
    // Lower slatted shelf + Tansu storage box
    buf.add_box(Vec3::new(-2.41, 0.16, 1.49), Vec3::new(-1.49, 0.185, 1.91), cedar_dark, MAT_WOOD);
    buf.add_bevel_box(Vec3::new(-2.34, 0.185, 1.54), Vec3::new(-1.96, 0.36, 1.86), 0.008, hinoki_light, MAT_WOOD);
    buf.add_box(Vec3::new(-2.20, 0.27, 1.53), Vec3::new(-2.10, 0.29, 1.542), brass, MAT_ALLOY);

    // Sculpted Multi-Tiered Japanese Black Pine Bonsai on the Small Table (`x = -1.78, z = 1.68`)!
    // Glazed dark-indigo stoneware bonsai pot with feet + green moss mound
    buf.add_bevel_box(Vec3::new(-1.98, 0.54, 1.54), Vec3::new(-1.58, 0.62, 1.82), 0.010, [0.16, 0.22, 0.28], MAT_CERAMIC);
    buf.add_lathe_y(
        Vec3::new(-1.78, 0.615, 1.68),
        &[(0.0, 0.16), (0.025, 0.12), (0.04, 0.02)],
        12,
        [0.24, 0.46, 0.20],
        MAT_FOLIAGE,
        true,
    );
    // Sinuous 4-segment gnarled pine trunk + branches
    let b0 = Vec3::new(-1.78, 0.63, 1.68);
    let b1 = Vec3::new(-1.84, 0.76, 1.65);
    let b2 = Vec3::new(-1.74, 0.89, 1.70);
    let b3 = Vec3::new(-1.80, 0.99, 1.67);
    let br_left = Vec3::new(-1.63, 0.83, 1.64);
    let br_right = Vec3::new(-1.92, 0.87, 1.72);
    buf.add_cylinder(b0, b1, 0.028, 0.021, 8, cedar_dark, MAT_WOOD, false);
    buf.add_cylinder(b1, b2, 0.021, 0.015, 8, cedar_dark, MAT_WOOD, false);
    buf.add_cylinder(b2, b3, 0.015, 0.009, 8, cedar_dark, MAT_WOOD, false);
    buf.add_cylinder(b1, br_left, 0.014, 0.008, 7, cedar_dark, MAT_WOOD, false);
    buf.add_cylinder(b2, br_right, 0.013, 0.008, 7, cedar_dark, MAT_WOOD, false);
    // Layered cloud-pruned pine needle pads
    for &(px, py, pz, pr, col) in &[
        (br_left.x, br_left.y - 0.01, br_left.z, 0.115_f32, [0.16, 0.42, 0.20]),
        (br_right.x, br_right.y - 0.01, br_right.z, 0.110, [0.14, 0.38, 0.18]),
        (b2.x + 0.04, b2.y + 0.01, b2.z - 0.04, 0.105, [0.18, 0.45, 0.22]),
        (b3.x, b3.y - 0.01, b3.z, 0.125, [0.20, 0.48, 0.23]),
    ] {
        buf.add_lathe_y(
            Vec3::new(px, py, pz),
            &[(0.0, pr * 0.45), (0.025, pr), (0.060, pr * 0.82), (0.085, 0.015)],
            12,
            col,
            MAT_FOLIAGE,
            true,
        );
    }

    // Kumiko Wooden Table Andon Lantern on left end of Small Bonsai Table (`x = -2.26, z = 1.70`)
    let (ax0, ax1, az0, az1) = (-2.36_f32, -2.14_f32, 1.59_f32, 1.81_f32);
    buf.add_box(Vec3::new(ax0 - 0.01, 0.54, az0 - 0.01), Vec3::new(ax1 + 0.01, 0.565, az1 + 0.01), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(ax0 - 0.015, 0.95, az0 - 0.015), Vec3::new(ax1 + 0.015, 0.975, az1 + 0.015), cedar_dark, MAT_WOOD);
    for &(px, pz) in &[(ax0, az0), (ax1, az0), (ax1, az1), (ax0, az1)] {
        buf.add_box(Vec3::new(px - 0.010, 0.56, pz - 0.010), Vec3::new(px + 0.010, 0.96, pz + 0.010), cedar_dark, MAT_WOOD);
    }
    // Celadon ceramic incense burner beside bonsai
    buf.add_lathe_y(
        Vec3::new(-2.05, 0.54, 1.56),
        &[(0.0, 0.032), (0.028, 0.046), (0.055, 0.030)],
        12,
        [0.36, 0.54, 0.50],
        MAT_CERAMIC,
        true,
    );

    // =========================================================================
    // 6. BOTTOM-LEFT (`x in [0.42, 2.54]`, `z in [-2.04, -0.46]`):
    //    COZY RECESSED SLEEPING BED ALCOVE (`TargetId::SleepingBunk`)
    // =========================================================================
    // Warm Walnut & Star Citizen Alloy Platform Frame + Recessed Toe-Kick
    buf.add_box(Vec3::new(0.46, 0.0, -2.02), Vec3::new(2.52, 0.08, -0.50), [0.11, 0.12, 0.14], MAT_ALLOY);
    buf.add_bevel_box(Vec3::new(0.42, 0.08, -2.04), Vec3::new(2.54, 0.36, -0.46), 0.018, walnut_warm, MAT_WOOD);
    // Upholstered acoustic headboard along `+X` wall (`x in [2.42, 2.54]`)
    buf.add_bevel_box(Vec3::new(2.40, 0.36, -2.00), Vec3::new(2.53, 1.06, -0.50), 0.022, [0.25, 0.28, 0.33], MAT_MATTE);
    // Plump beveled mattress (`x in [0.48, 2.40]`, `z in [-1.98, -0.52]`)
    buf.add_bevel_box(Vec3::new(0.48, 0.36, -1.98), Vec3::new(2.40, 0.53, -0.52), 0.038, [0.92, 0.91, 0.87], MAT_MATTE);
    // Crisp white folded turnover sheet + quilted deep-indigo Japanese duvet cover!
    buf.add_bevel_box(Vec3::new(0.47, 0.51, -1.99), Vec3::new(1.88, 0.575, -0.51), 0.026, [0.18, 0.29, 0.44], MAT_MATTE);
    buf.add_bevel_box(Vec3::new(1.74, 0.52, -1.985), Vec3::new(1.94, 0.585, -0.515), 0.018, [0.95, 0.95, 0.93], MAT_MATTE);
    // Two sculpted rounded pillows against headboard
    buf.add_bevel_box(Vec3::new(2.02, 0.53, -1.88), Vec3::new(2.36, 0.65, -1.30), 0.036, [0.95, 0.94, 0.90], MAT_MATTE);
    buf.add_bevel_box(Vec3::new(2.02, 0.53, -1.20), Vec3::new(2.36, 0.65, -0.62), 0.036, [0.86, 0.88, 0.92], MAT_MATTE);
    // Overhead Port Olisar Chamfered Storage Lockers above bed (`y in [1.80, 2.54]`) + Warm Reading Strip
    buf.add_bevel_box(Vec3::new(0.42, 1.80, -2.04), Vec3::new(2.54, 2.54, -1.40), 0.028, alloy_panel, MAT_ALLOY);
    for &lx in &[1.12_f32, 1.82] {
        buf.add_box(Vec3::new(lx, 1.83, -1.405), Vec3::new(lx + 0.015, 2.50, -1.395), alloy_dark, MAT_ALLOY);
    }
    buf.add_box(Vec3::new(0.56, 1.785, -1.48), Vec3::new(2.40, 1.80, -1.44), [1.0, 0.72, 0.36], MAT_EMISSIVE_WARM);

    // =========================================================================
    // 7. BOTTOM-CENTER (`x in [-0.38, +0.42]`, `z in [-2.04, -0.56]`):
    //    BUILT-IN SMALL SHELF DIVIDER (`image-3.png`: "small shelf")
    // =========================================================================
    // Full-height side stiles + 4 warm Hinoki shelves facing into the room (`+Z`)
    buf.add_box(Vec3::new(0.36, 0.0, -2.04), Vec3::new(0.42, 2.54, -0.56), walnut_warm, MAT_WOOD);
    buf.add_box(Vec3::new(-0.38, 0.0, -2.04), Vec3::new(-0.32, 2.54, -0.48), alloy_panel, MAT_ALLOY);
    // Back panel of shelf bay
    buf.add_box(Vec3::new(-0.32, 0.0, -1.24), Vec3::new(0.36, 2.20, -1.16), cedar_dark, MAT_WOOD);
    for &sy in &[0.08_f32, 0.56, 1.04, 1.52, 2.00] {
        buf.add_bevel_box(Vec3::new(-0.32, sy, -1.18), Vec3::new(0.36, sy + 0.035, -0.58), 0.006, hinoki_light, MAT_WOOD);
    }
    // Books, technical flight binders, ceramic jar & spare instrument box on the shelves!
    let book_cols = [
        (-0.26_f32, -0.21_f32, 0.80_f32, [0.76, 0.24, 0.18]),
        (-0.20, -0.14, 0.84, [0.18, 0.36, 0.56]),
        (-0.13, -0.08, 0.78, [0.82, 0.68, 0.28]),
        (-0.07, -0.01, 0.83, [0.22, 0.46, 0.36]),
    ];
    for &(bx0, bx1, bh, bcol) in &book_cols {
        buf.add_bevel_box(Vec3::new(bx0, 1.075, -0.96), Vec3::new(bx1, 1.075 + bh * 0.36, -0.68), 0.004, bcol, MAT_MATTE);
    }
    // Lathed ceramic vase on upper shelf + aluminum storage case on lower shelf
    buf.add_lathe_y(
        Vec3::new(0.16, 1.555, -0.86),
        &[(0.0, 0.045), (0.09, 0.072), (0.18, 0.035), (0.23, 0.042)],
        12,
        [0.84, 0.79, 0.70],
        MAT_CERAMIC,
        true,
    );
    buf.add_bevel_box(Vec3::new(-0.24, 0.595, -1.02), Vec3::new(0.22, 0.82, -0.68), 0.010, alloy_mid, MAT_ALLOY);

    // =========================================================================
    // 8. BOTTOM-RIGHT (`x in [-2.54, -0.38]`, `z in [-2.04, -0.28]`):
    //    ENCLOSED MODULAR UNIT-BATH / TOILET POD (`image-3.png`: "toilet")
    // =========================================================================
    // Raised waterproof composite wet-bath pan (`y = 0.0..0.06`) with light ceramic tile tone
    buf.add_box(Vec3::new(-2.54, 0.0, -2.04), Vec3::new(-0.38, 0.06, -0.28), [0.82, 0.85, 0.86], MAT_CERAMIC);
    // Front Unit-Bath Wall at `z = -0.28` with Doorway Opening `x in [-1.36, -0.60]`, `y in [0.06, 2.08]`!
    // Left jamb/wall (`x in [-0.60, -0.38]`)
    buf.add_box(Vec3::new(-0.60, 0.0, -0.34), Vec3::new(-0.38, 2.56, -0.24), alloy_panel, MAT_ALLOY);
    // Right wall section (`x in [-2.54, -1.36]`) with slim frosted vertical sidelight strip!
    buf.add_box(Vec3::new(-2.54, 0.0, -0.34), Vec3::new(-1.54, 2.56, -0.24), alloy_panel, MAT_ALLOY);
    buf.add_box(Vec3::new(-1.54, 0.0, -0.34), Vec3::new(-1.36, 0.35, -0.24), alloy_panel, MAT_ALLOY);
    buf.add_box(Vec3::new(-1.54, 2.08, -0.34), Vec3::new(-1.36, 2.56, -0.24), alloy_panel, MAT_ALLOY);
    // Frosted vertical privacy glass slit (`x in [-1.52, -1.39], y in [0.35, 2.08]`)
    buf.add_box(Vec3::new(-1.52, 0.35, -0.30), Vec3::new(-1.39, 2.08, -0.275), [0.76, 0.88, 0.90], MAT_SHOJI);
    // Header over bathroom doorway
    buf.add_box(Vec3::new(-1.36, 2.08, -0.34), Vec3::new(-0.60, 2.56, -0.24), alloy_panel, MAT_ALLOY);
    // Door frame trim + VACANT/OCCUPIED indicator badge
    buf.add_box(Vec3::new(-1.38, 0.06, -0.35), Vec3::new(-0.58, 2.10, -0.335), alloy_dark, MAT_ALLOY);
    buf.add_box(Vec3::new(-0.54, 1.32, -0.238), Vec3::new(-0.48, 1.38, -0.232), [0.22, 0.92, 0.58], MAT_CRT);

    // --- INTERIOR OF UNIT-BATH POD (Visible when sliding door opens!) ---
    // Warm waterproof interior wall cladding + backlit vanity mirror + ceiling LED dome
    buf.add_box(Vec3::new(-2.52, 0.06, -2.02), Vec3::new(-0.40, 2.48, -1.98), [0.88, 0.90, 0.90], MAT_CERAMIC);
    buf.add_box(Vec3::new(-2.52, 0.06, -2.02), Vec3::new(-2.48, 2.48, -0.34), [0.86, 0.88, 0.89], MAT_CERAMIC);
    buf.add_box(Vec3::new(-1.75, 2.44, -1.35), Vec3::new(-1.15, 2.47, -0.95), [1.0, 0.88, 0.68], MAT_EMISSIVE_WARM);

    // Sculpted Japanese Washlet Toilet (`x = -1.96, z = -1.62` facing `+Z` toward doorway)
    // Lathed porcelain pedestal & contoured bowl
    buf.add_lathe_y(
        Vec3::new(-1.96, 0.06, -1.52),
        &[
            (0.0, 0.14),
            (0.16, 0.15),
            (0.32, 0.21),
            (0.39, 0.22),
            (0.41, 0.19),
        ],
        16,
        ceramic_white,
        MAT_CERAMIC,
        true,
    );
    // Rear low-profile cistern tank + chrome flush button + Japanese washlet side control arm!
    buf.add_bevel_box(Vec3::new(-2.16, 0.42, -1.94), Vec3::new(-1.76, 0.82, -1.72), 0.022, ceramic_white, MAT_CERAMIC);
    buf.add_lathe_y(Vec3::new(-1.96, 0.82, -1.83), &[(0.0, 0.028), (0.010, 0.028)], 10, chrome, MAT_ALLOY, true);
    buf.add_bevel_box(Vec3::new(-1.75, 0.44, -1.66), Vec3::new(-1.65, 0.47, -1.44), 0.006, ceramic_white, MAT_CERAMIC);
    // Contoured toilet lid (slightly raised lip) + wall paper-holder roll
    buf.add_bevel_box(Vec3::new(-2.15, 0.465, -1.71), Vec3::new(-1.77, 0.495, -1.32), 0.015, ceramic_white, MAT_CERAMIC);
    buf.add_cylinder(Vec3::new(-2.48, 0.68, -1.32), Vec3::new(-2.38, 0.68, -1.32), 0.045, 0.045, 12, [0.96, 0.96, 0.95], MAT_MATTE, true);

    // Compact Japanese Wall-Hung Washbasin + Chrome Tap + Backlit Mirror (`x = -0.96, z = -1.76`)
    buf.add_bevel_box(Vec3::new(-1.24, 0.76, -1.96), Vec3::new(-0.68, 0.88, -1.58), 0.020, ceramic_white, MAT_CERAMIC);
    buf.add_cylinder(Vec3::new(-0.96, 0.88, -1.90), Vec3::new(-0.96, 1.00, -1.82), 0.014, 0.011, 10, chrome, MAT_ALLOY, true);
    // Backlit vanity mirror above basin
    buf.add_box(Vec3::new(-1.20, 1.12, -1.978), Vec3::new(-0.72, 1.78, -1.968), [1.0, 0.86, 0.62], MAT_EMISSIVE_WARM);
    buf.add_box(Vec3::new(-1.18, 1.14, -1.966), Vec3::new(-0.74, 1.76, -1.956), [0.68, 0.78, 0.84], MAT_ALLOY);

    // =========================================================================
    // 9. OVERHEAD CEILING FAN HUB (`(0.0, 2.38..2.68, 0.35)`)
    // =========================================================================
    buf.add_lathe_y(
        Vec3::new(0.0, 2.38, 0.35),
        &[(0.0, 0.125), (0.045, 0.135), (0.075, 0.026), (0.30, 0.026)],
        16,
        alloy_dark,
        MAT_ALLOY,
        true,
    );

    // =========================================================================
    // 10. "SHADOWS OF A SUMMER NOON" EXTERIOR COASTAL TOWN & UCHINOURA PAD-1
    // =========================================================================
    // Balcony concrete deck & steel safety railing right outside window (`z = 2.10 .. 2.90`)
    buf.add_box(Vec3::new(-2.70, -0.08, 2.08), Vec3::new(2.70, 0.02, 2.92), [0.62, 0.63, 0.60], MAT_MATTE);
    buf.add_cylinder(Vec3::new(-2.62, 1.04, 2.86), Vec3::new(2.62, 1.04, 2.86), 0.022, 0.022, 8, alloy_panel, MAT_ALLOY, true);
    buf.add_cylinder(Vec3::new(-2.62, 0.56, 2.86), Vec3::new(2.62, 0.56, 2.86), 0.010, 0.010, 6, alloy_panel, MAT_ALLOY, false);
    for i in -4..=4 {
        let bx = (i as f32) * 0.62;
        buf.add_cylinder(Vec3::new(bx, 0.02, 2.86), Vec3::new(bx, 1.04, 2.86), 0.015, 0.015, 6, alloy_panel, MAT_ALLOY, false);
    }
    // Outdoor AC Condenser Unit on Top-Right Balcony (`x in [-2.38, -1.58]`, connected directly to Top-Right AC!)
    buf.add_bevel_box(Vec3::new(-2.38, 0.02, 2.22), Vec3::new(-1.58, 0.64, 2.58), 0.018, [0.84, 0.85, 0.82], MAT_ALLOY);
    buf.add_disc_neg_z(Vec3::new(-1.92, 0.33, 2.215), 0.21, 18, alloy_dark, MAT_ALLOY);

    // Coastal Town Ground Slope (`z = 2.8..32m`), Asphalt Seawall Road (`z = 32..40m`), Concrete Seawall (`z = 40..42m`)!
    let coastal_grass = [0.22, 0.46, 0.19];
    buf.add_box(Vec3::new(-85.0, -3.4, 2.8), Vec3::new(85.0, -3.0, 32.0), coastal_grass, MAT_FOLIAGE);
    // Coastal road + crisp white center line + concrete seawall parapet
    buf.add_box(Vec3::new(-95.0, -3.05, 32.0), Vec3::new(95.0, -2.92, 40.0), [0.24, 0.26, 0.29], MAT_MATTE);
    buf.add_box(Vec3::new(-80.0, -2.91, 35.8), Vec3::new(80.0, -2.90, 36.2), [0.92, 0.92, 0.90], MAT_MATTE);
    buf.add_box(Vec3::new(-95.0, -2.95, 40.0), Vec3::new(95.0, -2.15, 41.4), [0.74, 0.75, 0.72], MAT_MATTE);

    // Deep Cobalt Sparkling Pacific Ocean (`z = 41.4m` out to `460m`, elevated to `y = -2.35m` so clearly visible from desk!)
    buf.push_quad(
        Vec3::new(-440.0, -2.35, 41.4),
        Vec3::new(440.0, -2.35, 41.4),
        Vec3::new(440.0, -2.35, 460.0),
        Vec3::new(-440.0, -2.35, 460.0),
        [0.04, 0.28, 0.62],
        MAT_OCEAN,
    );
    // White coastal surf foam ribbon along seawall base
    buf.push_quad(
        Vec3::new(-120.0, -2.32, 41.4),
        Vec3::new(120.0, -2.32, 41.4),
        Vec3::new(120.0, -2.32, 43.2),
        Vec3::new(-120.0, -2.32, 43.2),
        [0.88, 0.94, 0.98],
        MAT_MATTE,
    );

    // Neighboring Japanese Coastal Buildings (refined with eaves, windows, rust roofs & rooftop details!)
    // 1. Screen-Left Coastal House (`x in [5.4, 15.6]`, `z in [12.0, 21.0]`) with dark Kawara pitched roof & timber windows
    buf.add_box(Vec3::new(5.4, -3.0, 12.0), Vec3::new(15.6, 0.35, 21.0), [0.88, 0.85, 0.78], MAT_MATTE);
    // Dark window panes + cedar frames facing our apartment (`-Z` wall at `z = 12.0`)
    for &wx in &[7.2_f32, 11.4] {
        buf.add_box(Vec3::new(wx, -1.35, 11.92), Vec3::new(wx + 2.1, -0.20, 12.02), cedar_dark, MAT_WOOD);
        buf.add_box(Vec3::new(wx + 0.12, -1.25, 11.90), Vec3::new(wx + 1.98, -0.30, 11.96), [0.14, 0.22, 0.30], MAT_ALLOY);
    }
    // Overhanging Kawara pitched roof
    buf.push_quad(
        Vec3::new(4.8, 0.30, 11.2),
        Vec3::new(16.2, 0.30, 11.2),
        Vec3::new(16.2, 1.55, 16.5),
        Vec3::new(4.8, 1.55, 16.5),
        [0.24, 0.28, 0.33],
        MAT_CORRUGATED,
    );
    buf.push_quad(
        Vec3::new(4.8, 1.55, 16.5),
        Vec3::new(16.2, 1.55, 16.5),
        Vec3::new(16.2, 0.30, 21.8),
        Vec3::new(4.8, 0.30, 21.8),
        [0.20, 0.24, 0.29],
        MAT_CORRUGATED,
    );

    // 2. Screen-Right Weathered Corrugated Marine Workshop (`x in [-15.8, -5.6]`, `z in [11.0, 19.8]`)
    buf.add_box(Vec3::new(-15.8, -3.0, 11.0), Vec3::new(-5.6, 0.25, 19.8), [0.74, 0.77, 0.78], MAT_CORRUGATED);
    // Sliding steel bay door + rusty awning + clerical windows
    buf.add_box(Vec3::new(-10.8, -3.0, 10.92), Vec3::new(-7.2, -0.65, 11.02), [0.32, 0.40, 0.46], MAT_CORRUGATED);
    buf.push_quad(
        Vec3::new(-11.2, -0.55, 10.1),
        Vec3::new(-6.8, -0.55, 10.1),
        Vec3::new(-6.8, -0.30, 11.05),
        Vec3::new(-11.2, -0.30, 11.05),
        [0.58, 0.31, 0.22],
        MAT_CORRUGATED,
    );
    buf.add_box(Vec3::new(-14.6, -1.25, 10.92), Vec3::new(-12.0, -0.35, 11.02), [0.16, 0.24, 0.30], MAT_ALLOY);
    // Pitched oxidized blue-slate corrugated roof + rooftop TV Yagi antenna
    buf.push_quad(
        Vec3::new(-16.3, 0.22, 10.4),
        Vec3::new(-5.1, 0.22, 10.4),
        Vec3::new(-5.1, 1.38, 15.4),
        Vec3::new(-16.3, 1.38, 15.4),
        [0.38, 0.52, 0.62],
        MAT_CORRUGATED,
    );
    buf.push_quad(
        Vec3::new(-16.3, 1.38, 15.4),
        Vec3::new(-5.1, 1.38, 15.4),
        Vec3::new(-5.1, 0.22, 20.4),
        Vec3::new(-16.3, 0.22, 20.4),
        [0.34, 0.47, 0.57],
        MAT_CORRUGATED,
    );
    buf.add_cylinder(Vec3::new(-7.5, 1.15, 14.5), Vec3::new(-7.5, 2.65, 14.5), 0.025, 0.018, 6, alloy_panel, MAT_ALLOY, false);
    buf.add_cylinder(Vec3::new(-8.1, 2.45, 14.5), Vec3::new(-6.9, 2.45, 14.5), 0.015, 0.015, 6, alloy_panel, MAT_ALLOY, false);

    // Lush Japanese Coastal Black Pines & Camphor Trees framing the view!
    buf.add_coastal_tree(Vec3::new(3.9, -3.0, 9.2), 1.18);
    buf.add_coastal_tree(Vec3::new(-4.3, -3.0, 9.8), 1.08);
    buf.add_coastal_tree(Vec3::new(17.5, -3.0, 24.0), 1.65);
    buf.add_coastal_tree(Vec3::new(-18.5, -3.0, 25.0), 1.75);

    // Japanese Concrete Utility Poles + Ceramic Insulators + Transformer + Swaying Catenary Cables!
    let pole_a = Vec3::new(-6.2, -3.0, 7.8);
    let pole_b = Vec3::new(6.4, -3.0, 8.5);
    for &p in &[pole_a, pole_b] {
        buf.add_lathe_y(p, &[(0.0, 0.14), (6.4, 0.092)], 10, [0.50, 0.49, 0.46], MAT_MATTE, true);
        buf.add_box(
            Vec3::new(p.x - 0.68, p.y + 5.75, p.z - 0.05),
            Vec3::new(p.x + 0.68, p.y + 5.85, p.z + 0.05),
            alloy_dark,
            MAT_ALLOY,
        );
        for &ix in &[-0.48_f32, 0.0, 0.48] {
            buf.add_lathe_y(Vec3::new(p.x + ix, p.y + 5.85, p.z), &[(0.0, 0.035), (0.08, 0.025)], 8, ceramic_white, MAT_CERAMIC, true);
        }
        if p.x > 0.0 {
            buf.add_lathe_y(
                Vec3::new(p.x - 0.26, p.y + 4.70, p.z),
                &[(0.0, 0.19), (0.62, 0.19)],
                12,
                [0.48, 0.52, 0.55],
                MAT_ALLOY,
                true,
            );
        }
    }
    for &dz in &[-0.18_f32, 0.0, 0.18] {
        buf.add_catenary(
            Vec3::new(pole_a.x, pole_a.y + 5.91, pole_a.z + dz),
            Vec3::new(pole_b.x, pole_b.y + 5.91, pole_b.z + dz),
            0.36,
            14,
            0.010,
            [0.12, 0.13, 0.15],
        );
    }

    // Layered Uchinoura Forested Headland + Launch Pad-1 Complex (`z = 148m, x = -14m`)!
    let cape_near = [0.16, 0.40, 0.22];
    let cape_far = [0.18, 0.36, 0.32];
    // Distant secondary mountain ridge across the bay (normal +Y/-Z toward window!)
    buf.push_quad(
        Vec3::new(-180.0, 24.0, 290.0),
        Vec3::new(45.0, 18.0, 310.0),
        Vec3::new(140.0, -2.3, 280.0),
        Vec3::new(-220.0, -2.3, 240.0),
        cape_far,
        MAT_FOLIAGE,
    );
    // Main Uchinoura Launch Cape peninsula jutting into the Pacific (normal +Y/-Z!)
    buf.push_quad(
        Vec3::new(-98.0, 11.2, 154.0),
        Vec3::new(-4.0, 6.4, 158.0),
        Vec3::new(16.0, -2.3, 128.0),
        Vec3::new(-98.0, -2.3, 96.0),
        cape_near,
        MAT_FOLIAGE,
    );
    // Sea-cliff rock face below launch pad (normal +Y/-Z!)
    buf.push_quad(
        Vec3::new(-26.0, 5.4, 144.0),
        Vec3::new(-4.0, 5.4, 146.0),
        Vec3::new(6.0, -2.3, 138.0),
        Vec3::new(-28.0, -2.3, 134.0),
        [0.44, 0.42, 0.39],
        MAT_MATTE,
    );
    // Concrete Pad-1 Apron, Spherical White LOX Tank, Lightning Mast, Red/White Umbilical Tower & Rocket!
    buf.add_box(Vec3::new(-24.0, 4.8, 142.0), Vec3::new(-6.0, 5.8, 154.0), [0.76, 0.77, 0.74], MAT_MATTE);
    // White LOX sphere tank on cape
    buf.add_lathe_y(
        Vec3::new(-9.5, 5.8, 148.5),
        &[(0.0, 0.4), (0.8, 1.45), (1.9, 1.65), (3.0, 1.45), (3.6, 0.2)],
        12,
        [0.95, 0.96, 0.97],
        MAT_ALLOY,
        true,
    );
    // Red-and-white Umbilical Gantry Tower (`x = -16.8, z = 148.0`)
    for tier in 0..4 {
        let ty0 = 5.8 + (tier as f32) * 3.1;
        let tcol = if tier % 2 == 0 { [0.86, 0.22, 0.18] } else { [0.95, 0.95, 0.94] };
        buf.add_box(Vec3::new(-17.6, ty0, 147.3), Vec3::new(-16.3, ty0 + 3.1, 148.6), tcol, MAT_ALLOY);
    }
    // Umbilical swing arms reaching to rocket
    buf.add_beam(Vec3::new(-16.3, 11.5, 148.0), Vec3::new(-14.2, 11.5, 148.0), 0.18, [0.86, 0.22, 0.18], MAT_ALLOY);
    buf.add_beam(Vec3::new(-16.3, 15.2, 148.0), Vec3::new(-14.2, 15.2, 148.0), 0.16, [0.95, 0.95, 0.94], MAT_ALLOY);
    // Tall slender lightning protection mast
    buf.add_cylinder(Vec3::new(-21.5, 5.8, 148.0), Vec3::new(-21.5, 21.0, 148.0), 0.22, 0.06, 6, [0.88, 0.89, 0.90], MAT_ALLOY, false);
    // Sounding Rocket Airframe, Red Stripe & Nosecone on Static-Fire Stand (`x = -14.0, y = 5.8..16.8, z = 148.0`)
    buf.add_lathe_y(
        Vec3::new(-14.0, 5.8, 148.0),
        &[(0.0, 0.62), (0.9, 0.42), (8.2, 0.42), (11.2, 0.03)],
        12,
        [0.97, 0.97, 0.96],
        MAT_ALLOY,
        true,
    );
    buf.add_lathe_y(
        Vec3::new(-14.0, 11.8, 148.0),
        &[(0.0, 0.43), (1.1, 0.43)],
        12,
        [0.86, 0.20, 0.16],
        MAT_ALLOY,
        false,
    );
}

/// Rebuilds only the dynamic moving elements (Shoji screen, glass window, AC louver, fan blades,
/// bathroom sliding door, analog gauge needles, China-marker ticks, physical switches, lamps, and rocket plume).
pub fn build_dynamic_parts(buf: &mut MeshBuffer, p: &DynamicSceneParams) {
    buf.clear();

    let cedar_dark = [0.30, 0.18, 0.11];
    let washi_paper = [0.97, 0.94, 0.87];
    let alloy_panel = [0.46, 0.50, 0.55];

    // 1. Sliding Shoji Screen across centered window opening (`x in [-1.25, +1.25]`, `y in [0.77, 2.26]`, `z = 1.965`)
    // Slides from Screen-Left (`+X`, `x = +1.25`) across toward Screen-Right (`-X`, `x = -1.25`)!
    let span_max = 2.50_f32;
    let cover_w = 0.24 + p.shoji_closed.clamp(0.0, 1.0) * (span_max - 0.24);
    let sx1 = 1.25_f32;
    let sx0 = sx1 - cover_w;
    let sy0 = 0.77_f32;
    let sy1 = 2.26_f32;
    let sz = 1.965 + p.window_rattle_m * 0.4;

    buf.add_box(Vec3::new(sx0, sy0, sz - 0.005), Vec3::new(sx1, sy1, sz + 0.005), washi_paper, MAT_SHOJI);
    // Outer cedar stiles & rails
    buf.add_box(Vec3::new(sx0, sy0, sz - 0.014), Vec3::new(sx1, sy0 + 0.032, sz + 0.014), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(sx0, sy1 - 0.032, sz - 0.014), Vec3::new(sx1, sy1, sz + 0.014), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(sx0, sy0, sz - 0.016), Vec3::new(sx0 + 0.034, sy1, sz + 0.016), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(sx1 - 0.030, sy0, sz - 0.014), Vec3::new(sx1, sy1, sz + 0.014), cedar_dark, MAT_WOOD);
    // Horizontal & vertical Kumiko lattice bars
    for r in 1..=4 {
        let ry = sy0 + (sy1 - sy0) * (r as f32) / 5.0;
        buf.add_box(Vec3::new(sx0, ry - 0.006, sz - 0.010), Vec3::new(sx1, ry + 0.006, sz + 0.010), cedar_dark, MAT_WOOD);
    }
    let v_count = ((cover_w / 0.26) as usize).clamp(1, 9);
    for c in 1..v_count {
        let rx = sx0 + cover_w * (c as f32) / (v_count as f32);
        buf.add_box(Vec3::new(rx - 0.006, sy0, sz - 0.010), Vec3::new(rx + 0.006, sy1, sz + 0.010), cedar_dark, MAT_WOOD);
    }

    // 2. Sliding Glass Window Sash Frame (Screen-Right half `x in [-1.25 .. 0.0]`, slides left `+X` when opened!)
    let sash_shift = p.window_open.clamp(0.0, 1.0) * 1.05;
    let wx0 = -1.25 + sash_shift;
    let wx1 = 0.00 + sash_shift;
    let wz = 2.035 + p.window_rattle_m;
    let sash_col = [0.22, 0.26, 0.30];
    buf.add_box(Vec3::new(wx0, 0.77, wz - 0.012), Vec3::new(wx1, 0.805, wz + 0.012), sash_col, MAT_ALLOY);
    buf.add_box(Vec3::new(wx0, 2.225, wz - 0.012), Vec3::new(wx1, 2.26, wz + 0.012), sash_col, MAT_ALLOY);
    buf.add_box(Vec3::new(wx0, 0.77, wz - 0.012), Vec3::new(wx0 + 0.032, 2.26, wz + 0.012), sash_col, MAT_ALLOY);
    buf.add_box(Vec3::new(wx1 - 0.032, 0.77, wz - 0.012), Vec3::new(wx1, 2.26, wz + 0.012), sash_col, MAT_ALLOY);
    // Crescent sash lock handle
    buf.add_box(Vec3::new(wx0 + 0.032, 1.42, wz - 0.025), Vec3::new(wx0 + 0.048, 1.52, wz - 0.010), [0.78, 0.80, 0.82], MAT_ALLOY);

    // 3. AC Motorized Airflow Louver Vane (tilts open when AC is active!)
    let ac_running = p.ac_switch_on && p.main_breaker_on && !p.breaker_tripped;
    let vane_z = if ac_running { 1.652 } else { 1.678 };
    let vane_y = if ac_running { 2.045 } else { 2.065 };
    buf.add_box(Vec3::new(-2.35, vane_y, vane_z), Vec3::new(-1.45, vane_y + 0.018, 1.690), [0.86, 0.88, 0.87], MAT_ALLOY);

    // 4. Interactive Unit-Bath / Toilet Sliding Pocket Door (`z = -0.29`, doorway `x in [-1.36, -0.60]`)
    let door_shift = p.toilet_door_open.clamp(0.0, 1.0) * 0.70;
    let dx0 = -1.36 - door_shift;
    let dx1 = -0.60 - door_shift;
    buf.add_bevel_box(Vec3::new(dx0, 0.065, -0.31), Vec3::new(dx1, 2.075, -0.27), 0.010, alloy_panel, MAT_ALLOY);
    // Recessed vertical pull handle on toilet door
    buf.add_box(Vec3::new(dx1 - 0.09, 0.96, -0.268), Vec3::new(dx1 - 0.05, 1.18, -0.258), [0.18, 0.20, 0.23], MAT_ALLOY);

    // 5. Ceiling Fan 4 Sculpted Wooden Blades rotating around `(0.0, 2.405, 0.35)`
    for b in 0..4 {
        let ang = (p.fan_angle_rad as f64) + (b as f64) * (TAU * 0.25);
        let (ca, sa) = (dcos(ang) as f32, dsin(ang) as f32);
        let root = Vec3::new(0.11 * ca, 2.405, 0.35 + 0.11 * sa);
        let tip = Vec3::new(0.72 * ca, 2.405, 0.35 + 0.72 * sa);
        let side_r = Vec3::new(-0.042 * sa, 0.0, 0.042 * ca);
        let side_t = Vec3::new(-0.062 * sa, 0.0, 0.062 * ca);
        // Brass blade iron bracket
        buf.add_beam(Vec3::new(0.08 * ca, 2.405, 0.35 + 0.08 * sa), root, 0.012, [0.80, 0.64, 0.28], MAT_ALLOY);
        buf.push_quad(root.sub(side_r), tip.sub(side_t), tip.add(side_t), root.add(side_r), cedar_dark, MAT_WOOD);
        buf.push_quad(root.add(side_r), tip.add(side_t), tip.sub(side_t), root.sub(side_r), cedar_dark, MAT_WOOD);
    }

    // 6. Bonsai Table Washi Andon Core + Articulated Desk Lamp Bulb!
    let andon_col = if p.lamp_on { [1.0, 0.74, 0.36] } else { [0.80, 0.77, 0.70] };
    let andon_mat = if p.lamp_on { MAT_EMISSIVE_WARM } else { MAT_SHOJI };
    buf.add_box(Vec3::new(-2.345, 0.565, 1.605), Vec3::new(-2.155, 0.948, 1.795), andon_col, andon_mat);
    if p.lamp_on {
        buf.add_lathe_y(
            Vec3::new(-0.26, 1.075, 1.46),
            &[(0.0, 0.015), (0.025, 0.036), (0.045, 0.015)],
            10,
            [1.0, 0.85, 0.52],
            MAT_EMISSIVE_WARM,
            true,
        );
    }

    // 7. Three Analog Gauge Steel Needles + Yellow China-Marker Prediction Ticks!
    let ranges = [
        (15.0_f32, 40.0_f32, p.room_temp_c),
        (0.0, 1800.0, p.bus_power_w),
        (85.0, 105.0, p.bus_voltage_v),
    ];
    for (idx, &(lo, hi, val)) in ranges.iter().enumerate() {
        let gc = GAUGE_CENTERS[idx];
        let frac = ((val - lo) / (hi - lo)).clamp(0.0, 1.0) as f64;
        let ang = (210.0 - frac * 240.0) * PI / 180.0;
        let tip = Vec3::new(gc.x + 0.078 * (dcos(ang) as f32), gc.y + 0.078 * (dsin(ang) as f32), 1.939);
        let tail = Vec3::new(gc.x - 0.018 * (dcos(ang) as f32), gc.y - 0.018 * (dsin(ang) as f32), 1.939);
        buf.add_beam(tail, tip, 0.0038, [0.10, 0.12, 0.14], MAT_ALLOY);

        let mark_val = p.gauge_marks[idx];
        if mark_val >= lo - 0.01 {
            let mfrac = ((mark_val - lo) / (hi - lo)).clamp(0.0, 1.0) as f64;
            let mang = (210.0 - mfrac * 240.0) * PI / 180.0;
            let m_in = Vec3::new(gc.x + 0.058 * (dcos(mang) as f32), gc.y + 0.058 * (dsin(mang) as f32), 1.935);
            let m_out = Vec3::new(gc.x + 0.095 * (dcos(mang) as f32), gc.y + 0.095 * (dsin(mang) as f32), 1.935);
            buf.add_beam(m_in, m_out, 0.0062, [1.0, 0.86, 0.14], MAT_EMISSIVE_WARM);
        }
    }

    // 8. Five Physical Breaker / Rocker Toggle Switches along bottom of Top-Right Hab Panel (`y = 1.18`)
    let switch_states = [
        p.main_breaker_on && !p.breaker_tripped,
        p.ac_switch_on,
        p.fan_switch_on,
        p.lamp_on,
        p.kettle_switch_on,
    ];
    for i in 0..5 {
        let sx = PANEL_SWITCH_XS[i];
        let on = switch_states[i];
        buf.add_bevel_box(Vec3::new(sx - 0.034, 1.11, 1.944), Vec3::new(sx + 0.034, 1.25, 1.956), 0.004, [0.12, 0.14, 0.17], MAT_ALLOY);
        let dy = if on { 0.042 } else { -0.042 };
        let lever_col = if i == 0 && p.breaker_tripped {
            [0.96, 0.22, 0.16]
        } else if on {
            [0.84, 0.87, 0.90]
        } else {
            [0.45, 0.48, 0.52]
        };
        buf.add_cylinder(
            Vec3::new(sx, 1.18, 1.946),
            Vec3::new(sx, 1.18 + dy, 1.908),
            0.011,
            0.013,
            8,
            lever_col,
            MAT_ALLOY,
            true,
        );
        let led_col = if i == 0 && p.breaker_tripped {
            [1.0, 0.18, 0.12]
        } else if on {
            [0.24, 0.94, 0.54]
        } else {
            [0.18, 0.20, 0.22]
        };
        let led_mat = if on || (i == 0 && p.breaker_tripped) { MAT_CRT } else { MAT_MATTE };
        buf.add_box(Vec3::new(sx - 0.013, 1.27, 1.944), Vec3::new(sx + 0.013, 1.29, 1.953), led_col, led_mat);
    }

    // 9. Distant Coastal Pad-1 Static-Fire Rocket Plume & Condensation Cloud (`x = -14.0, y = 5.8, z = 148.0`)!
    if p.plume_intensity > 0.01 || p.plume_age_s < 18.0 {
        let pad_base = Vec3::new(-14.0, 5.8, 148.0);
        if p.plume_intensity > 0.02 {
            let core_h = 6.0 * p.plume_intensity;
            buf.add_lathe_y(
                Vec3::new(pad_base.x, pad_base.y - 0.4, pad_base.z),
                &[(0.0, 0.45), (core_h * 0.5, 1.55 * p.plume_intensity), (core_h, 0.15)],
                12,
                [1.0, 0.88, 0.52],
                MAT_PLUME,
                true,
            );
        }
        let drift_h = (p.plume_age_s * 3.5).clamp(1.0, 50.0);
        let fade = (1.0 - (p.plume_age_s / 18.0)).clamp(0.0, 1.0);
        if fade > 0.04 {
            for puff in 0..6 {
                let frac = (puff as f32) / 5.0;
                let py = pad_base.y + 1.2 + frac * drift_h;
                let px = pad_base.x + frac * (p.plume_age_s * 0.36);
                let rad = (1.5 + frac * 5.0) * (0.45 + 0.55 * fade);
                let c = 0.93 + 0.06 * fade;
                buf.add_lathe_y(
                    Vec3::new(px, py, pad_base.z),
                    &[(0.0, rad * 0.35), (rad * 0.45, rad), (rad * 1.05, rad * 0.86), (rad * 1.45, 0.08)],
                    10,
                    [c, c, c * 0.99],
                    MAT_PLUME,
                    true,
                );
            }
        }
    }
}

/// Builds 3D Eidos AR Visor Causal Sensitivity Flow Lines connecting the new room layout subsystems.
pub fn build_eidos_vectors(buf: &mut MeshBuffer, shoji_closed: f32, ac_active: bool, kettle_active: bool) {
    buf.clear();
    // 1. Solar Radiative Vector: Centered Window -> Desk/Room Air (amber-gold)
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
    // 2. Thermal Heat-Pump Extraction Vector: Room Air -> Top-Right AC Evaporator -> Balcony Condenser (cyan -> coral)
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
    // 3. Electrical Bus Flow: Top-Right Breaker Panel -> AC Unit & Top-Left Kitchen Kettle (emerald / amber)
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
        assert!(static_tris > 500 && static_tris < 20_000, "static tris: {}", static_tris);

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
                toilet_door_open: 0.0,
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
    }
}
