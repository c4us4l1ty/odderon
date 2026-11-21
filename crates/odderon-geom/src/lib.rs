//! Zero-Asset Parametric Geometry Engine for Odderon.
//! Every wall rib, tatami mat, shoji lattice, kettle, bonsai, analog gauge, utility pole,
//! catenary cable, corrugated workshop roof, and coastal rocket pad is generated from pure math.

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
            verts: Vec::with_capacity(16384),
            indices: Vec::with_capacity(24576),
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

    /// Oriented beam / tube segment of square cross-section between `a` and `b`.
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

/// Center positions of the 3 wall-mounted analog gauges on the Hab Control Panel (`z = 1.915`).
pub const GAUGE_CENTERS: [Vec3; 3] = [
    Vec3::new(1.52, 1.54, 1.912), // Gauge 0: Room Air Temp (15 C .. 40 C)
    Vec3::new(1.78, 1.54, 1.912), // Gauge 1: Hab Bus Power (0 W .. 1800 W)
    Vec3::new(2.04, 1.54, 1.912), // Gauge 2: Hab Bus Voltage (85 V .. 105 V)
];

/// Builds the entire static Apartment + Exterior Coastal Launch Town in ONE single draw-call mesh!
pub fn build_static_world(buf: &mut MeshBuffer) {
    buf.clear();

    // =========================================================================
    // 1. APARTMENT SHELL (4.8m wide x 4.0m deep x 2.62m high)
    //    Fusing Star Citizen Hab modular chamfered ribs + Japantown warm cedar!
    // =========================================================================
    let plaster_warm = [0.84, 0.81, 0.75];
    let alloy_panel = [0.46, 0.50, 0.54];
    let alloy_dark = [0.22, 0.25, 0.28];
    let cedar_dark = [0.34, 0.21, 0.13];
    let hinoki_warm = [0.68, 0.49, 0.31];
    let floor_slate = [0.27, 0.28, 0.30];

    // Main sub-floor & ceiling slab
    buf.add_box(Vec3::new(-2.45, -0.12, -2.05), Vec3::new(2.45, 0.0, 2.05), floor_slate, MAT_ALLOY);
    buf.add_box(Vec3::new(-2.45, 2.62, -2.05), Vec3::new(2.45, 2.76, 2.05), [0.32, 0.35, 0.38], MAT_ALLOY);

    // Back wall (-Z), Left wall (-X), Right wall (+X)
    buf.add_box(Vec3::new(-2.45, 0.0, -2.08), Vec3::new(2.45, 2.62, -1.98), plaster_warm, MAT_MATTE);
    buf.add_box(Vec3::new(-2.48, 0.0, -2.05), Vec3::new(-2.38, 2.62, 2.05), plaster_warm, MAT_MATTE);
    buf.add_box(Vec3::new(2.38, 0.0, -2.05), Vec3::new(2.48, 2.62, 2.05), plaster_warm, MAT_MATTE);

    // Front Coastal Window Wall (+Z = +1.98): Panoramic opening x in [-1.55, +1.30], y in [0.76, 2.26]
    // Below sill
    buf.add_box(Vec3::new(-2.42, 0.0, 1.95), Vec3::new(2.42, 0.76, 2.06), plaster_warm, MAT_MATTE);
    // Above header
    buf.add_box(Vec3::new(-2.42, 2.26, 1.95), Vec3::new(2.42, 2.62, 2.06), plaster_warm, MAT_MATTE);
    // Left pier (-2.42 .. -1.55)
    buf.add_box(Vec3::new(-2.42, 0.76, 1.95), Vec3::new(-1.55, 2.26, 2.06), plaster_warm, MAT_MATTE);
    // Right pier (+1.30 .. +2.42) — holds the Hab Control Panel & AC Unit!
    buf.add_box(Vec3::new(1.30, 0.76, 1.95), Vec3::new(2.42, 2.26, 2.06), plaster_warm, MAT_MATTE);

    // Deep wooden Japanese window sill + frame mullions
    buf.add_box(Vec3::new(-1.60, 0.72, 1.88), Vec3::new(1.35, 0.77, 2.08), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(-1.60, 2.24, 1.90), Vec3::new(1.35, 2.30, 2.08), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(-1.60, 0.76, 1.92), Vec3::new(-1.53, 2.26, 2.08), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(1.28, 0.76, 1.92), Vec3::new(1.35, 2.26, 2.08), cedar_dark, MAT_WOOD);
    // Center fixed window mullion post
    buf.add_box(Vec3::new(-0.14, 0.76, 1.97), Vec3::new(-0.09, 2.25, 2.04), cedar_dark, MAT_WOOD);

    // Star Citizen Port Olisar / Area18 modular structural wall ribs & ceiling beams
    for &z_rib in &[-1.15_f32, 0.35, 1.82] {
        // Left & right vertical alloy ribs
        buf.add_box(Vec3::new(-2.39, 0.0, z_rib - 0.05), Vec3::new(-2.31, 2.62, z_rib + 0.05), alloy_panel, MAT_ALLOY);
        buf.add_box(Vec3::new(2.31, 0.0, z_rib - 0.05), Vec3::new(2.39, 2.62, z_rib + 0.05), alloy_panel, MAT_ALLOY);
        // Overhead ceiling cross-truss
        buf.add_box(Vec3::new(-2.35, 2.52, z_rib - 0.05), Vec3::new(2.35, 2.62, z_rib + 0.05), alloy_dark, MAT_ALLOY);
        // Chamfered corner gussets (classic Star Citizen silhouette!)
        buf.add_beam(
            Vec3::new(-2.34, 2.22, z_rib),
            Vec3::new(-2.02, 2.55, z_rib),
            0.035,
            alloy_panel,
            MAT_ALLOY,
        );
        buf.add_beam(
            Vec3::new(2.34, 2.22, z_rib),
            Vec3::new(2.02, 2.55, z_rib),
            0.035,
            alloy_panel,
            MAT_ALLOY,
        );
    }

    // =========================================================================
    // 2. RAISED JAPANESE TATAMI LIVING ZONE + STAR CITIZEN SLEEPING POD ALCOVE
    // =========================================================================
    // Raised cedar step platform (x: -2.36 .. 0.38, z: -1.96 .. 1.72, y: 0.0 .. 0.11)
    buf.add_box(Vec3::new(-2.36, 0.0, -1.96), Vec3::new(0.38, 0.10, 1.72), cedar_dark, MAT_WOOD);
    // 4 Tatami mats with woven indigo-charcoal cloth borders (heri)
    let tatami_straw = [0.76, 0.72, 0.47];
    let tatami_border = [0.14, 0.18, 0.22];
    let mats = [
        (-2.32, -0.42, -1.00, 0.62),
        (-0.97, -0.42, 0.34, 0.62),
        (-2.32, 0.65, -1.00, 1.68),
        (-0.97, 0.65, 0.34, 1.68),
    ];
    for &(mx0, mz0, mx1, mz1) in &mats {
        buf.add_box(Vec3::new(mx0, 0.10, mz0), Vec3::new(mx1, 0.122, mz1), tatami_border, MAT_MATTE);
        buf.add_box(
            Vec3::new(mx0 + 0.035, 0.102, mz0 + 0.035),
            Vec3::new(mx1 - 0.035, 0.126, mz1 - 0.035),
            tatami_straw,
            MAT_TATAMI,
        );
    }

    // Star Citizen Recessed Sleeping Bunk Pod (back-left corner: x in [-2.36, -0.52], z in [-1.96, -0.48])
    buf.add_box(Vec3::new(-2.36, 0.10, -1.96), Vec3::new(-0.52, 0.44, -0.48), alloy_dark, MAT_ALLOY);
    // Bunk privacy partition bulkhead
    buf.add_box(Vec3::new(-0.58, 0.10, -1.96), Vec3::new(-0.48, 2.52, -0.48), alloy_panel, MAT_ALLOY);
    // Overhead EZ-Hab storage lockers above bunk
    buf.add_box(Vec3::new(-2.36, 1.78, -1.96), Vec3::new(-0.52, 2.52, -1.32), alloy_panel, MAT_ALLOY);
    // Warm amber recessed LED strip under bunk overhead locker
    buf.add_box(Vec3::new(-2.25, 1.765, -1.38), Vec3::new(-0.64, 1.78, -1.34), [1.0, 0.72, 0.34], MAT_EMISSIVE_WARM);
    // Futon / hab mattress, crisp white sheet, indigo blanket & pillow
    buf.add_box(Vec3::new(-2.30, 0.44, -1.90), Vec3::new(-0.60, 0.56, -0.54), [0.88, 0.87, 0.83], MAT_MATTE);
    buf.add_box(Vec3::new(-1.55, 0.562, -1.88), Vec3::new(-0.62, 0.59, -0.56), [0.20, 0.30, 0.42], MAT_MATTE);
    buf.add_box(Vec3::new(-2.22, 0.56, -1.62), Vec3::new(-1.86, 0.65, -0.86), [0.93, 0.91, 0.86], MAT_MATTE);

    // Heavy Star Citizen Chamfered Entry Door on Left Wall (-X, z in [0.55, 1.45])
    buf.add_box(Vec3::new(-2.38, 0.12, 0.52), Vec3::new(-2.30, 2.18, 1.48), alloy_dark, MAT_ALLOY);
    buf.add_box(Vec3::new(-2.33, 0.16, 0.60), Vec3::new(-2.28, 2.10, 1.40), [0.38, 0.42, 0.46], MAT_ALLOY);
    // Green door lock status strip
    buf.add_box(Vec3::new(-2.278, 1.12, 1.28), Vec3::new(-2.27, 1.26, 1.33), [0.25, 0.92, 0.58], MAT_CRT);

    // =========================================================================
    // 3. AREA18 KITCHENETTE + PARAMETRIC STAINLESS ELECTRIC KETTLE (Back-Right)
    // =========================================================================
    buf.add_box(Vec3::new(0.55, 0.0, -1.96), Vec3::new(2.34, 0.86, -1.18), alloy_dark, MAT_ALLOY);
    buf.add_box(Vec3::new(0.52, 0.86, -1.96), Vec3::new(2.35, 0.91, -1.14), [0.74, 0.76, 0.78], MAT_ALLOY);
    // Overhead galley cabinet + cyan/amber accent bar
    buf.add_box(Vec3::new(0.58, 1.68, -1.96), Vec3::new(2.34, 2.42, -1.48), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(0.64, 1.66, -1.54), Vec3::new(2.26, 1.68, -1.50), [0.98, 0.76, 0.42], MAT_EMISSIVE_WARM);

    // Parametric Stainless Electric Kettle on Counter (x = 1.12, y = 0.91, z = -1.48)
    buf.add_lathe_y(
        Vec3::new(1.12, 0.91, -1.48),
        &[
            (0.0, 0.095),
            (0.025, 0.095),
            (0.03, 0.088),
            (0.14, 0.078),
            (0.21, 0.064),
            (0.23, 0.068),
        ],
        14,
        [0.78, 0.80, 0.83],
        MAT_ALLOY,
        true,
    );
    // Kettle handle & spout
    buf.add_box(Vec3::new(1.20, 0.96, -1.50), Vec3::new(1.24, 1.11, -1.46), [0.16, 0.16, 0.18], MAT_MATTE);
    buf.add_beam(Vec3::new(1.05, 1.07, -1.48), Vec3::new(1.00, 1.14, -1.48), 0.014, [0.80, 0.82, 0.85], MAT_ALLOY);

    // =========================================================================
    // 4. CYBERPUNK JAPANTOWN DETAILS: TANSU CHEST, BONSAI, INCENSE & SHOJI ANDON
    // =========================================================================
    // Low Japanese Tansu Chest along +X wall (x in [1.74, 2.34], z in [-0.82, 0.42])
    buf.add_box(Vec3::new(1.74, 0.0, -0.82), Vec3::new(2.34, 0.58, 0.42), cedar_dark, MAT_WOOD);
    // Brass drawer pulls
    for &dz in &[-0.55_f32, -0.20, 0.15] {
        buf.add_box(Vec3::new(1.725, 0.36, dz - 0.05), Vec3::new(1.74, 0.40, dz + 0.05), [0.85, 0.68, 0.28], MAT_ALLOY);
        buf.add_box(Vec3::new(1.725, 0.16, dz - 0.05), Vec3::new(1.74, 0.20, dz + 0.05), [0.85, 0.68, 0.28], MAT_ALLOY);
    }

    // Parametric Bonsai Planter + Twisted Trunk + Pine Cushions on Tansu Chest (x = 2.02, z = -0.18)
    buf.add_box(Vec3::new(1.88, 0.58, -0.34), Vec3::new(2.16, 0.66, -0.02), [0.24, 0.22, 0.20], MAT_MATTE);
    buf.add_beam(Vec3::new(2.02, 0.65, -0.18), Vec3::new(1.96, 0.80, -0.14), 0.022, cedar_dark, MAT_WOOD);
    buf.add_beam(Vec3::new(1.96, 0.80, -0.14), Vec3::new(2.05, 0.93, -0.20), 0.017, cedar_dark, MAT_WOOD);
    buf.add_beam(Vec3::new(1.98, 0.82, -0.15), Vec3::new(1.87, 0.89, -0.08), 0.013, cedar_dark, MAT_WOOD);
    // Bonsai pine foliage pads (lathed oblate cushions)
    let pine_green = [0.19, 0.42, 0.21];
    for &(bx, by, bz, br) in &[
        (2.05_f32, 0.92, -0.20, 0.13),
        (1.86, 0.88, -0.08, 0.10),
        (1.98, 0.99, -0.14, 0.11),
    ] {
        buf.add_lathe_y(
            Vec3::new(bx, by, bz),
            &[(0.0, br * 0.45), (0.03, br), (0.07, br * 0.82), (0.10, 0.01)],
            10,
            pine_green,
            MAT_MATTE,
            true,
        );
    }

    // Ceramic Incense Holder on Tansu (x = 1.96, z = 0.22)
    buf.add_lathe_y(
        Vec3::new(1.96, 0.58, 0.22),
        &[(0.0, 0.04), (0.04, 0.055), (0.07, 0.035)],
        10,
        [0.32, 0.46, 0.44],
        MAT_ALLOY,
        true,
    );

    // Tall Japanese Shoji Floor Lantern (Andon) at (x = 2.02, z = 0.78) — straight out of Japantown ref!
    let ax0 = 1.88_f32;
    let ax1 = 2.16_f32;
    let az0 = 0.64_f32;
    let az1 = 0.92_f32;
    for &(px, pz) in &[(ax0, az0), (ax1, az0), (ax1, az1), (ax0, az1)] {
        buf.add_box(Vec3::new(px - 0.015, 0.0, pz - 0.015), Vec3::new(px + 0.015, 1.08, pz + 0.015), cedar_dark, MAT_WOOD);
    }
    buf.add_box(Vec3::new(ax0 - 0.02, 1.05, az0 - 0.02), Vec3::new(ax1 + 0.02, 1.09, az1 + 0.02), cedar_dark, MAT_WOOD);

    // =========================================================================
    // 5. SUNLIT COASTAL WINDOW ENGINEERING DESK + CRT + VHF PAD RADIO + CLIPBOARD
    // =========================================================================
    // Wide Hinoki wood desk right beneath the coastal window (x in [-1.55, 1.24], z in [1.22, 1.93], y = 0.71..0.75)
    buf.add_box(Vec3::new(-1.55, 0.70, 1.22), Vec3::new(1.24, 0.745, 1.93), hinoki_warm, MAT_WOOD);
    buf.add_box(Vec3::new(-1.52, 0.12, 1.26), Vec3::new(-1.44, 0.70, 1.88), alloy_dark, MAT_ALLOY);
    buf.add_box(Vec3::new(1.14, 0.0, 1.26), Vec3::new(1.22, 0.70, 1.88), alloy_dark, MAT_ALLOY);

    // Swivel task chair tucked slightly beside desk (x = -0.15, z = 0.82)
    buf.add_lathe_y(
        Vec3::new(-0.15, 0.12, 0.82),
        &[(0.0, 0.22), (0.03, 0.04), (0.32, 0.035), (0.34, 0.21), (0.39, 0.21)],
        12,
        alloy_dark,
        MAT_ALLOY,
        true,
    );
    buf.add_box(Vec3::new(-0.36, 0.50, 0.58), Vec3::new(0.06, 0.92, 0.65), [0.20, 0.22, 0.25], MAT_MATTE);

    // CRT Vector Telemetry Scope on left of desk (x in [-1.32, -0.78], z in [1.44, 1.82])
    buf.add_box(Vec3::new(-1.32, 0.745, 1.44), Vec3::new(-0.78, 1.12, 1.82), [0.30, 0.33, 0.35], MAT_ALLOY);
    // Glowing phosphor screen facing player (-Z)
    buf.add_box(Vec3::new(-1.27, 0.79, 1.432), Vec3::new(-0.83, 1.07, 1.44), [0.16, 0.82, 0.76], MAT_CRT);

    // VHF Coastal Pad-1 Telemetry Transceiver Box (x in [0.52, 0.98], z in [1.46, 1.78])
    buf.add_box(Vec3::new(0.52, 0.745, 1.46), Vec3::new(0.98, 0.92, 1.78), [0.22, 0.26, 0.28], MAT_ALLOY);
    // Whip antenna rising in front of window
    buf.add_beam(Vec3::new(0.92, 0.92, 1.72), Vec3::new(0.96, 1.48, 1.76), 0.006, [0.75, 0.78, 0.82], MAT_ALLOY);
    // Red VHF Static-Fire Test Button cap on front face
    buf.add_box(Vec3::new(0.84, 0.80, 1.442), Vec3::new(0.92, 0.86, 1.46), [0.92, 0.22, 0.16], MAT_EMISSIVE_WARM);

    // Pad Technician Clipboard resting on the sunlit desk center (x in [-0.36, 0.02], z in [1.34, 1.68])
    buf.add_box(Vec3::new(-0.36, 0.746, 1.34), Vec3::new(0.02, 0.758, 1.68), [0.54, 0.36, 0.20], MAT_WOOD);
    buf.add_box(Vec3::new(-0.33, 0.759, 1.37), Vec3::new(-0.01, 0.764, 1.64), [0.95, 0.94, 0.89], MAT_MATTE);
    buf.add_box(Vec3::new(-0.24, 0.764, 1.61), Vec3::new(-0.10, 0.778, 1.66), [0.76, 0.78, 0.82], MAT_ALLOY);

    // Desk Task Lamp (x = 0.28, z = 1.66)
    buf.add_lathe_y(
        Vec3::new(0.28, 0.745, 1.66),
        &[(0.0, 0.08), (0.025, 0.075), (0.03, 0.015)],
        12,
        alloy_dark,
        MAT_ALLOY,
        true,
    );
    buf.add_beam(Vec3::new(0.28, 0.77, 1.66), Vec3::new(0.24, 1.12, 1.56), 0.010, alloy_panel, MAT_ALLOY);
    buf.add_beam(Vec3::new(0.24, 1.12, 1.56), Vec3::new(0.14, 1.06, 1.44), 0.018, [0.22, 0.34, 0.30], MAT_ALLOY);

    // =========================================================================
    // 6. REAL EXPOSED SPACECRAFT-STYLE UTILITY PIPES + WINDOW AC + GAUGE PANEL
    // =========================================================================
    // Indoor Split/Window AC Evaporator Unit mounted high right of window (x in [1.38, 2.26], y in [2.04, 2.38], z in [1.66, 1.96])
    buf.add_box(Vec3::new(1.38, 2.04, 1.66), Vec3::new(2.26, 2.38, 1.96), [0.86, 0.87, 0.84], MAT_ALLOY);
    // Louver discharge slot
    buf.add_box(Vec3::new(1.42, 2.05, 1.648), Vec3::new(2.22, 2.11, 1.66), alloy_dark, MAT_ALLOY);

    // Hab Breaker & Analog Telemetry Panel right beside the window (x in [1.38, 2.22], y in [0.96, 1.74], z in [1.915, 1.96])
    // You can stand by the window/desk, operate breakers/switches, mark gauges, AND see sunbeams + rocket pad simultaneously!
    buf.add_box(Vec3::new(1.38, 0.96, 1.918), Vec3::new(2.22, 1.74, 1.955), [0.20, 0.23, 0.26], MAT_ALLOY);
    buf.add_box(Vec3::new(1.40, 0.98, 1.914), Vec3::new(2.20, 1.72, 1.920), [0.28, 0.31, 0.35], MAT_ALLOY);

    // Three circular analog instrument bezels & cream dial faces on the panel
    for &gc in &GAUGE_CENTERS {
        // Metallic bezel ring
        buf.add_disc_neg_z(Vec3::new(gc.x, gc.y, 1.913), 0.105, 20, [0.14, 0.16, 0.18], MAT_ALLOY);
        // Cream instrument face
        buf.add_disc_neg_z(Vec3::new(gc.x, gc.y, 1.911), 0.092, 20, [0.94, 0.92, 0.85], MAT_MATTE);
        // Tick marks around 240-degree sweep (from +210 deg down to -30 deg)
        for t in 0..=8 {
            let frac = (t as f64) / 8.0;
            let ang = (210.0 - frac * 240.0) * PI / 180.0;
            let (ca, sa) = (dcos(ang) as f32, dsin(ang) as f32);
            let p_in = Vec3::new(gc.x + 0.072 * ca, gc.y + 0.072 * sa, 1.909);
            let p_out = Vec3::new(gc.x + 0.088 * ca, gc.y + 0.088 * sa, 1.909);
            let tick_col = if t >= 7 { [0.82, 0.22, 0.16] } else { [0.18, 0.20, 0.22] };
            buf.add_beam(p_in, p_out, 0.003, tick_col, MAT_MATTE);
        }
    }

    // Physical Copper/Alloy Conduits connecting Panel -> AC Unit -> Ceiling Fan -> Kitchenette Kettle!
    let copper_pipe = [0.74, 0.48, 0.30];
    let conduit_steel = [0.52, 0.56, 0.60];
    // Panel up to AC unit
    buf.add_beam(Vec3::new(1.56, 1.74, 1.93), Vec3::new(1.56, 2.04, 1.90), 0.012, conduit_steel, MAT_ALLOY);
    buf.add_beam(Vec3::new(2.12, 2.04, 1.86), Vec3::new(2.12, 1.85, 2.06), 0.016, copper_pipe, MAT_ALLOY);
    // Panel up to ceiling raceway -> Ceiling Fan hub (0.0, 2.58, 0.20) & Kettle wall
    buf.add_beam(Vec3::new(1.88, 1.74, 1.93), Vec3::new(1.88, 2.50, 1.93), 0.011, conduit_steel, MAT_ALLOY);
    buf.add_beam(Vec3::new(1.88, 2.50, 1.93), Vec3::new(0.0, 2.50, 0.20), 0.011, conduit_steel, MAT_ALLOY);
    buf.add_beam(Vec3::new(1.88, 2.50, 1.93), Vec3::new(1.88, 2.50, -1.48), 0.011, conduit_steel, MAT_ALLOY);
    buf.add_beam(Vec3::new(1.88, 2.50, -1.48), Vec3::new(1.12, 1.15, -1.94), 0.010, conduit_steel, MAT_ALLOY);

    // Ceiling Fan downrod & motor housing at center (0.0, y=2.34..2.62, z=0.20)
    buf.add_lathe_y(
        Vec3::new(0.0, 2.34, 0.20),
        &[(0.0, 0.13), (0.05, 0.14), (0.08, 0.025), (0.28, 0.025)],
        14,
        alloy_dark,
        MAT_ALLOY,
        true,
    );

    // =========================================================================
    // 7. "SHADOWS OF A SUMMER NOON" EXTERIOR LAUNCH TOWN & UCHINOURA PAD-1
    // =========================================================================
    // Balcony concrete deck & steel safety railing right outside window (z = 2.06 .. 2.85)
    buf.add_box(Vec3::new(-2.60, -0.08, 2.05), Vec3::new(2.60, 0.02, 2.85), [0.58, 0.59, 0.57], MAT_MATTE);
    buf.add_box(Vec3::new(-2.55, 1.02, 2.80), Vec3::new(2.55, 1.06, 2.85), alloy_panel, MAT_ALLOY);
    for i in -4..=4 {
        let bx = (i as f32) * 0.60;
        buf.add_box(Vec3::new(bx - 0.018, 0.02, 2.81), Vec3::new(bx + 0.018, 1.02, 2.84), alloy_panel, MAT_ALLOY);
    }
    // Outdoor boxy AC Condenser Unit on right balcony (just like "Shadows of a Summer Noon" ref!)
    buf.add_box(Vec3::new(1.52, 0.02, 2.18), Vec3::new(2.26, 0.62, 2.54), [0.82, 0.83, 0.80], MAT_ALLOY);
    buf.add_disc_neg_z(Vec3::new(1.80, 0.32, 2.175), 0.20, 16, [0.22, 0.24, 0.26], MAT_ALLOY);

    // Lush green coastal hillsides & Uchinoura peninsula overlooking the Pacific Ocean!
    let grass_lush = [0.24, 0.54, 0.21];
    let hill_far = [0.20, 0.48, 0.26];
    // Foreground grassy yard below apartment (y = -3.4m second-floor drop!)
    buf.add_box(Vec3::new(-65.0, -3.6, 2.5), Vec3::new(65.0, -3.2, 46.0), grass_lush, MAT_MATTE);

    // Neighboring Japanese coastal workshops with corrugated iron siding & roofs ("Shadows of a Summer Noon"!)
    // Left workshop shed
    buf.add_box(Vec3::new(-14.5, -3.2, 10.5), Vec3::new(-4.8, 0.35, 19.5), [0.76, 0.78, 0.77], MAT_CORRUGATED);
    // Pitched corrugated galvanized blue-rust roof
    buf.push_quad(
        Vec3::new(-15.0, 0.32, 10.0),
        Vec3::new(-4.4, 0.32, 10.0),
        Vec3::new(-4.4, 1.45, 15.0),
        Vec3::new(-15.0, 1.45, 15.0),
        [0.42, 0.54, 0.62],
        MAT_CORRUGATED,
    );
    buf.push_quad(
        Vec3::new(-15.0, 1.45, 15.0),
        Vec3::new(-4.4, 1.45, 15.0),
        Vec3::new(-4.4, 0.32, 20.0),
        Vec3::new(-15.0, 0.32, 20.0),
        [0.38, 0.50, 0.58],
        MAT_CORRUGATED,
    );

    // Right coastal house roof & timber eaves
    buf.add_box(Vec3::new(6.8, -3.2, 13.0), Vec3::new(16.5, 0.1, 22.0), [0.84, 0.81, 0.74], MAT_MATTE);
    buf.push_quad(
        Vec3::new(6.3, 0.08, 12.5),
        Vec3::new(17.0, 0.08, 12.5),
        Vec3::new(17.0, 1.25, 17.5),
        Vec3::new(6.3, 1.25, 17.5),
        [0.58, 0.33, 0.24],
        MAT_CORRUGATED,
    );

    // Japanese Concrete/Timber Utility Poles + Parametric Catenary Power Lines outside window!
    let pole_a = Vec3::new(-5.8, -3.2, 7.6);
    let pole_b = Vec3::new(6.4, -3.2, 8.4);
    for &p in &[pole_a, pole_b] {
        buf.add_lathe_y(
            p,
            &[(0.0, 0.14), (6.4, 0.095)],
            8,
            [0.46, 0.44, 0.40],
            MAT_MATTE,
            true,
        );
        // Cross-arm + ceramic insulators
        buf.add_box(
            Vec3::new(p.x - 0.65, p.y + 5.75, p.z - 0.06),
            Vec3::new(p.x + 0.65, p.y + 5.86, p.z + 0.06),
             alloy_dark,
            MAT_ALLOY,
        );
        // Pole transformer drum on left pole
        if p.x < 0.0 {
            buf.add_lathe_y(
                Vec3::new(p.x + 0.26, p.y + 4.75, p.z),
                &[(0.0, 0.18), (0.58, 0.18)],
                10,
                [0.48, 0.52, 0.54],
                MAT_ALLOY,
                true,
            );
        }
    }
    // Three catenary power cables sweeping across the sunny coastal view!
    for &dx in &[-0.45_f32, 0.0, 0.45] {
        buf.add_catenary(
            Vec3::new(pole_a.x + dx, pole_a.y + 5.84, pole_a.z),
            Vec3::new(pole_b.x + dx, pole_b.y + 5.84, pole_b.z),
            0.38,
            12,
            0.011,
            [0.14, 0.15, 0.17],
        );
    }

    // Sparkling Pacific Ocean plane stretching from z = 46m out to the horizon (z = 420m)
    buf.push_quad(
        Vec3::new(-380.0, -4.2, 46.0),
        Vec3::new(380.0, -4.2, 46.0),
        Vec3::new(380.0, -4.2, 420.0),
        Vec3::new(-380.0, -4.2, 420.0),
        [0.08, 0.38, 0.64],
        MAT_OCEAN,
    );

    // Uchinoura Coastal Rocket Cape & Static-Fire Pad-1 (optically foreshortened perspective headland at z = 148m, x = -18m)
    // Represents the 2,820 m coastal cape test stand overlooking the Pacific!
    buf.push_quad(
        Vec3::new(-95.0, -4.1, 88.0),
        Vec3::new(14.0, -4.1, 118.0),
        Vec3::new(-8.0, 6.2, 162.0),
        Vec3::new(-95.0, 9.5, 155.0),
        hill_far,
        MAT_MATTE,
    );
    // Concrete Launch Pad Apron & Flame Deflector Trench on Cape Ridge (-18.0, 5.2, 148.0)
    buf.add_box(Vec3::new(-26.0, 4.4, 142.0), Vec3::new(-10.0, 5.4, 154.0), [0.72, 0.73, 0.70], MAT_MATTE);
    // Red-and-white Umbilical Gantry Tower + White Sounding Rocket on Stand
    buf.add_box(Vec3::new(-20.4, 5.4, 147.2), Vec3::new(-19.1, 12.6, 148.6), [0.84, 0.24, 0.20], MAT_ALLOY);
    buf.add_box(Vec3::new(-20.3, 12.6, 147.3), Vec3::new(-19.2, 17.8, 148.5), [0.94, 0.94, 0.92], MAT_ALLOY);
    // Sounding rocket airframe & nosecone on static-fire mount (-17.2, 5.4..15.2, 148.0)
    buf.add_lathe_y(
        Vec3::new(-17.2, 5.4, 148.0),
        &[(0.0, 0.55), (0.8, 0.38), (7.6, 0.38), (10.2, 0.02)],
        10,
        [0.96, 0.96, 0.95],
        MAT_ALLOY,
        true,
    );
}

/// Rebuilds only the dynamic moving elements (Shoji screen, glass window, fan blades,
/// analog gauge needles, China-marker ticks, physical switches, lamp core, and distant rocket plume).
pub fn build_dynamic_parts(buf: &mut MeshBuffer, p: &DynamicSceneParams) {
    buf.clear();

    let cedar_dark = [0.34, 0.21, 0.13];
    let washi_paper = [0.96, 0.93, 0.86];

    // 1. Sliding Shoji Screen across window opening (x in [-1.53, +1.28], y in [0.77, 2.24], z = 1.935)
    // Slides from parked-left (-1.53 .. -0.12 when shoji_closed=0 -> Wait: let's have 2 panels that slide!)
    // At shoji_closed = 0.0, panel sits compactly on the far left (-1.53 .. -1.25); at 1.0 it covers [-1.53 .. +1.28]!
    let span_max = 2.81_f32;
    let cover_w = 0.24 + p.shoji_closed.clamp(0.0, 1.0) * (span_max - 0.24);
    let sx0 = -1.53_f32;
    let sx1 = sx0 + cover_w;
    let sy0 = 0.77_f32;
    let sy1 = 2.24_f32;
    let sz = 1.935 + p.window_rattle_m * 0.4;

    // Translucent Washi paper sheet (`MAT_SHOJI` glows warm gold with sunlight!)
    buf.add_box(Vec3::new(sx0, sy0, sz - 0.006), Vec3::new(sx1, sy1, sz + 0.006), washi_paper, MAT_SHOJI);
    // Outer cedar frame + Kumiko lattice ribs
    buf.add_box(Vec3::new(sx0, sy0, sz - 0.014), Vec3::new(sx1, sy0 + 0.03, sz + 0.014), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(sx0, sy1 - 0.03, sz - 0.014), Vec3::new(sx1, sy1, sz + 0.014), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(sx1 - 0.032, sy0, sz - 0.016), Vec3::new(sx1, sy1, sz + 0.016), cedar_dark, MAT_WOOD);
    // Horizontal Kumiko bars
    for r in 1..=4 {
        let ry = sy0 + (sy1 - sy0) * (r as f32) / 5.0;
        buf.add_box(Vec3::new(sx0, ry - 0.007, sz - 0.011), Vec3::new(sx1, ry + 0.007, sz + 0.011), cedar_dark, MAT_WOOD);
    }
    // Vertical Kumiko bars proportional to width
    let v_count = ((cover_w / 0.28) as usize).clamp(1, 10);
    for c in 1..v_count {
        let rx = sx0 + cover_w * (c as f32) / (v_count as f32);
        buf.add_box(Vec3::new(rx - 0.007, sy0, sz - 0.011), Vec3::new(rx + 0.007, sy1, sz + 0.011), cedar_dark, MAT_WOOD);
    }

    // 2. Sliding Glass Window Sash Frame (right half x in [-0.10 .. 1.28], slides left when window_open > 0)
    let sash_shift = p.window_open.clamp(0.0, 1.0) * 1.15;
    let wx0 = -0.10 - sash_shift;
    let wx1 = 1.28 - sash_shift;
    let wz = 2.01 + p.window_rattle_m;
    let sash_col = [0.24, 0.28, 0.30];
    buf.add_box(Vec3::new(wx0, 0.77, wz - 0.012), Vec3::new(wx1, 0.805, wz + 0.012), sash_col, MAT_ALLOY);
    buf.add_box(Vec3::new(wx0, 2.205, wz - 0.012), Vec3::new(wx1, 2.24, wz + 0.012), sash_col, MAT_ALLOY);
    buf.add_box(Vec3::new(wx0, 0.77, wz - 0.012), Vec3::new(wx0 + 0.032, 2.24, wz + 0.012), sash_col, MAT_ALLOY);
    buf.add_box(Vec3::new(wx1 - 0.032, 0.77, wz - 0.012), Vec3::new(wx1, 2.24, wz + 0.012), sash_col, MAT_ALLOY);

    // 3. Ceiling Fan 4 Wooden Blades rotating at `fan_angle_rad` around (0.0, 2.36, 0.20)
    for b in 0..4 {
        let ang = (p.fan_angle_rad as f64) + (b as f64) * (TAU * 0.25);
        let (ca, sa) = (dcos(ang) as f32, dsin(ang) as f32);
        let root = Vec3::new(0.11 * ca, 2.365, 0.20 + 0.11 * sa);
        let tip = Vec3::new(0.68 * ca, 2.365, 0.20 + 0.68 * sa);
        let side = Vec3::new(-0.055 * sa, 0.0, 0.055 * ca);
        buf.push_quad(
            root.sub(side),
            tip.sub(side),
            tip.add(side),
            root.add(side),
            cedar_dark,
            MAT_WOOD,
        );
        buf.push_quad(
            root.add(side),
            tip.add(side),
            tip.sub(side),
            root.sub(side),
            cedar_dark,
            MAT_WOOD,
        );
    }

    // 4. Shoji Floor Andon Lantern Washi Core + Desk Task Bulb (glows warm amber when lamp_on!)
    let andon_col = if p.lamp_on {
        [1.0, 0.74, 0.36]
    } else {
        [0.72, 0.69, 0.63]
    };
    let andon_mat = if p.lamp_on { MAT_EMISSIVE_WARM } else { MAT_SHOJI };
    buf.add_box(Vec3::new(1.90, 0.06, 0.66), Vec3::new(2.14, 1.04, 0.90), andon_col, andon_mat);
    if p.lamp_on {
        buf.add_box(Vec3::new(0.11, 1.03, 1.41), Vec3::new(0.17, 1.07, 1.47), [1.0, 0.82, 0.48], MAT_EMISSIVE_WARM);
    }

    // 5. Three Analog Gauge Steel Needles + Yellow China-Marker Prediction Ticks!
    // Ranges: Gauge 0 = Temp [15..40 C], Gauge 1 = Bus Power [0..1800 W], Gauge 2 = Bus Voltage [85..105 V]
    let ranges = [(15.0_f32, 40.0_f32, p.room_temp_c), (0.0, 1800.0, p.bus_power_w), (85.0, 105.0, p.bus_voltage_v)];
    for (idx, &(lo, hi, val)) in ranges.iter().enumerate() {
        let gc = GAUGE_CENTERS[idx];
        let frac = ((val - lo) / (hi - lo)).clamp(0.0, 1.0) as f64;
        let ang = (210.0 - frac * 240.0) * PI / 180.0;
        let tip = Vec3::new(
            gc.x + 0.076 * (dcos(ang) as f32),
            gc.y + 0.076 * (dsin(ang) as f32),
            1.906,
        );
        let tail = Vec3::new(
            gc.x - 0.018 * (dcos(ang) as f32),
            gc.y - 0.018 * (dsin(ang) as f32),
            1.906,
        );
        buf.add_beam(tail, tip, 0.0042, [0.12, 0.14, 0.16], MAT_ALLOY);

        // China-marker yellow grease-pencil tick on glass (Bible §5.1b & §6.1!)
        let mark_val = p.gauge_marks[idx];
        if mark_val >= lo - 0.01 {
            let mfrac = ((mark_val - lo) / (hi - lo)).clamp(0.0, 1.0) as f64;
            let mang = (210.0 - mfrac * 240.0) * PI / 180.0;
            let m_in = Vec3::new(
                gc.x + 0.056 * (dcos(mang) as f32),
                gc.y + 0.056 * (dsin(mang) as f32),
                1.903,
            );
            let m_out = Vec3::new(
                gc.x + 0.094 * (dcos(mang) as f32),
                gc.y + 0.094 * (dsin(mang) as f32),
                1.903,
            );
            buf.add_beam(m_in, m_out, 0.0065, [1.0, 0.86, 0.14], MAT_EMISSIVE_WARM);
        }
    }

    // 6. Five Physical Breaker / Rocker Toggle Switches along bottom row of Hab Panel (y = 1.16, z = 1.91)
    let switch_xs = [1.48_f32, 1.63, 1.78, 1.93, 2.08];
    let switch_states = [
        p.main_breaker_on && !p.breaker_tripped,
        p.ac_switch_on,
        p.fan_switch_on,
        p.lamp_on,
        p.kettle_switch_on,
    ];
    for i in 0..5 {
        let sx = switch_xs[i];
        let on = switch_states[i];
        // Switch housing
        buf.add_box(Vec3::new(sx - 0.032, 1.10, 1.906), Vec3::new(sx + 0.032, 1.23, 1.916), [0.12, 0.14, 0.16], MAT_ALLOY);
        // Physical toggle lever angled UP when ON, DOWN when OFF/TRIPPED
        let dy = if on { 0.042 } else { -0.042 };
        let lever_col = if i == 0 && p.breaker_tripped {
            [0.95, 0.22, 0.16]
        } else if on {
            [0.82, 0.85, 0.88]
        } else {
            [0.45, 0.47, 0.50]
        };
        buf.add_beam(
            Vec3::new(sx, 1.165, 1.908),
            Vec3::new(sx, 1.165 + dy, 1.872),
            0.011,
            lever_col,
            MAT_ALLOY,
        );
        // Status pilot LED above each breaker
        let led_col = if i == 0 && p.breaker_tripped {
            [1.0, 0.18, 0.12]
        } else if on {
            [0.24, 0.94, 0.52]
        } else {
            [0.18, 0.20, 0.22]
        };
        let led_mat = if on || (i == 0 && p.breaker_tripped) { MAT_CRT } else { MAT_MATTE };
        buf.add_box(Vec3::new(sx - 0.012, 1.25, 1.906), Vec3::new(sx + 0.012, 1.27, 1.914), led_col, led_mat);
    }

    // 7. Distant Coastal Pad-1 Static-Fire Rocket Plume & Condensation Cloud!
    if p.plume_intensity > 0.01 || p.plume_age_s < 18.0 {
        let pad_base = Vec3::new(-17.2, 5.4, 148.0);
        // Bright incandescent Mach-diamond core while firing
        if p.plume_intensity > 0.02 {
            let core_h = 5.5 * p.plume_intensity;
            buf.add_lathe_y(
                Vec3::new(pad_base.x, pad_base.y - 0.4, pad_base.z),
                &[(0.0, 0.4), (core_h * 0.5, 1.45 * p.plume_intensity), (core_h, 0.15)],
                10,
                [1.0, 0.88, 0.52],
                MAT_PLUME,
                true,
            );
        }
        // Rising white steam / exhaust condensation column drifting up into the summer sky!
        let drift_h = (p.plume_age_s * 3.4).clamp(1.0, 48.0);
        let fade = (1.0 - (p.plume_age_s / 18.0)).clamp(0.0, 1.0);
        if fade > 0.04 {
            for puff in 0..6 {
                let frac = (puff as f32) / 5.0;
                let py = pad_base.y + 1.2 + frac * drift_h;
                let px = pad_base.x + frac * (p.plume_age_s * 0.38); // light sea-breeze drift
                let rad = (1.4 + frac * 4.8) * (0.45 + 0.55 * fade);
                let c = 0.92 + 0.07 * fade;
                buf.add_lathe_y(
                    Vec3::new(px, py, pad_base.z),
                    &[
                        (0.0, rad * 0.35),
                        (rad * 0.45, rad),
                        (rad * 1.05, rad * 0.86),
                        (rad * 1.45, 0.08),
                    ],
                    10,
                    [c, c, c * 0.99],
                    MAT_PLUME,
                    true,
                );
            }
        }
    }
}

/// Builds 3D Eidos AR Visor Causal Sensitivity Flow Lines connecting physical subsystems.
pub fn build_eidos_vectors(buf: &mut MeshBuffer, shoji_closed: f32, ac_active: bool, kettle_active: bool) {
    buf.clear();
    // 1. Solar Radiative Vector: Window -> Desk/Room Air (amber-gold)
    let sun_beam_strength = 1.0 - 0.82 * shoji_closed;
    if sun_beam_strength > 0.05 {
        buf.add_beam(
            Vec3::new(-0.2, 1.62, 1.90),
            Vec3::new(-0.2, 0.88, 0.85),
            0.008 + 0.010 * sun_beam_strength,
            [1.0, 0.78, 0.24],
            MAT_EIDOS,
        );
    }
    // 2. Thermal Heat-Pump Extraction Vector: Room Air -> Window AC Evaporator -> Outdoor Condenser (cyan)
    if ac_active {
        buf.add_beam(
            Vec3::new(0.15, 1.15, 0.85),
            Vec3::new(1.76, 2.08, 1.72),
            0.010,
            [0.22, 0.88, 0.98],
            MAT_EIDOS,
        );
        buf.add_beam(
            Vec3::new(1.82, 2.18, 1.92),
            Vec3::new(1.82, 0.45, 2.34),
            0.010,
            [0.98, 0.45, 0.24],
            MAT_EIDOS,
        );
    }
    // 3. Electrical Bus Flow: Breaker Panel -> AC Unit & Kettle (emerald green)
    buf.add_beam(
        Vec3::new(1.63, 1.24, 1.89),
        Vec3::new(1.63, 2.04, 1.78),
        0.007,
        [0.28, 0.96, 0.62],
        MAT_EIDOS,
    );
    if kettle_active {
        buf.add_beam(
            Vec3::new(2.08, 1.24, 1.89),
            Vec3::new(1.12, 1.05, -1.48),
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
        assert!(static_tris > 200 && static_tris < 20_000, "static tris: {}", static_tris);

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
