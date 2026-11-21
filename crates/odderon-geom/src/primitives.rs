//! Reusable Parametric Geometric Primitives (`MeshBuffer` methods).
//! Includes smooth-normal rounded boxes (`add_rounded_box`) so 3D props, cushions, electronics,
//! and ceramics have soft, organic curved lighting instead of harsh square cube facets!

use crate::{MeshBuffer, MAT_ALLOY, MAT_KAWARA, MAT_WOOD};
use odderon_dmath::{dcos, dexp, dsin, Vec3, TAU};

impl MeshBuffer {
    /// Crisp architectural flat-shaded axis-aligned box (walls, beams, timber posts, shelves).
    pub fn add_box(&mut self, min: Vec3, max: Vec3, col: [f32; 3], mat: f32) {
        let (x0, y0, z0) = (min.x, min.y, min.z);
        let (x1, y1, z1) = (max.x, max.y, max.z);
        self.push_quad(Vec3::new(x0, y0, z1), Vec3::new(x1, y0, z1), Vec3::new(x1, y1, z1), Vec3::new(x0, y1, z1), col, mat);
        self.push_quad(Vec3::new(x1, y0, z0), Vec3::new(x0, y0, z0), Vec3::new(x0, y1, z0), Vec3::new(x1, y1, z0), col, mat);
        self.push_quad(Vec3::new(x1, y0, z1), Vec3::new(x1, y0, z0), Vec3::new(x1, y1, z0), Vec3::new(x1, y1, z1), col, mat);
        self.push_quad(Vec3::new(x0, y0, z0), Vec3::new(x0, y0, z1), Vec3::new(x0, y1, z1), Vec3::new(x0, y1, z0), col, mat);
        self.push_quad(Vec3::new(x0, y1, z1), Vec3::new(x1, y1, z1), Vec3::new(x1, y1, z0), Vec3::new(x0, y1, z0), col, mat);
        self.push_quad(Vec3::new(x0, y0, z0), Vec3::new(x1, y0, z0), Vec3::new(x1, y0, z1), Vec3::new(x0, y0, z1), col, mat);
    }

    /// Smooth-normal 3D rounded/filleted box (`4x4x4` corner grid = 96 triangles, smooth Phong normals!).
    /// Eliminates the "blocky / square-ish" look on cushions, mattresses, CRT/VHF enclosures, AC units & ceramics!
    pub fn add_rounded_box(&mut self, min: Vec3, max: Vec3, radius: f32, col: [f32; 3], mat: f32) {
        let r = radius
            .min((max.x - min.x) * 0.46)
            .min((max.y - min.y) * 0.46)
            .min((max.z - min.z) * 0.46);
        if r <= 1e-4 {
            self.add_box(min, max, col, mat);
            return;
        }
        // Inner core AABB; every surface vertex projects out from `core` by `r * normal`!
        let c0 = Vec3::new(min.x + r, min.y + r, min.z + r);
        let c1 = Vec3::new(max.x - r, max.y - r, max.z - r);

        // Watertight 2-strip smooth arc fillet per edge:
        // idx 1..2 = flat inner face (n_tangent = 0), idx 0 & 3 = 45-deg arc midpointshared watertightly with adjacent face!
        let offs = [-1.0_f32, 0.0, 0.0, 1.0];

        let mut emit_grid = |pts: [[(Vec3, Vec3); 4]; 4]| {
            let mut ids = [[0u32; 4]; 4];
            for u in 0..4 {
                for v in 0..4 {
                    let (p, n) = pts[u][v];
                    ids[u][v] = self.push_vert(p, n, col, mat);
                }
            }
            for u in 0..3 {
                for v in 0..3 {
                    let a = ids[u][v];
                    let b = ids[u + 1][v];
                    let c = ids[u + 1][v + 1];
                    let d = ids[u][v + 1];
                    self.indices.extend_from_slice(&[a, b, c, a, c, d]);
                }
            }
        };

        let map_axis = |idx: usize, lo: f32, hi: f32| -> (f32, f32) {
            let o = offs[idx];
            if idx <= 1 {
                (lo, o)
            } else {
                (hi, o)
            }
        };

        let make_pt = |cx: f32, cy: f32, cz: f32, nx: f32, ny: f32, nz: f32| -> (Vec3, Vec3) {
            let n = Vec3::new(nx, ny, nz).normalized();
            (Vec3::new(cx, cy, cz).add(n.scale(r)), n)
        };

        // +Z and -Z faces
        let mut f_pz = [[(Vec3::ZERO, Vec3::ZERO); 4]; 4];
        let mut f_nz = [[(Vec3::ZERO, Vec3::ZERO); 4]; 4];
        for u in 0..4 {
            for v in 0..4 {
                let (cx, nx) = map_axis(u, c0.x, c1.x);
                let (cy, ny) = map_axis(v, c0.y, c1.y);
                f_pz[u][v] = make_pt(cx, cy, c1.z, nx, ny, 1.0);
                let (cx2, nx2) = map_axis(3 - u, c0.x, c1.x);
                f_nz[u][v] = make_pt(cx2, cy, c0.z, nx2, ny, -1.0);
            }
        }
        emit_grid(f_pz);
        emit_grid(f_nz);

        // +X and -X faces
        let mut f_px = [[(Vec3::ZERO, Vec3::ZERO); 4]; 4];
        let mut f_nx = [[(Vec3::ZERO, Vec3::ZERO); 4]; 4];
        for u in 0..4 {
            for v in 0..4 {
                let (cz, nz) = map_axis(3 - u, c0.z, c1.z);
                let (cy, ny) = map_axis(v, c0.y, c1.y);
                f_px[u][v] = make_pt(c1.x, cy, cz, 1.0, ny, nz);
                let (cz2, nz2) = map_axis(u, c0.z, c1.z);
                f_nx[u][v] = make_pt(c0.x, cy, cz2, -1.0, ny, nz2);
            }
        }
        emit_grid(f_px);
        emit_grid(f_nx);

        // +Y face (always smooth-filleted) and -Y underside (single quad when resting near floor!)
        let mut f_py = [[(Vec3::ZERO, Vec3::ZERO); 4]; 4];
        for u in 0..4 {
            for v in 0..4 {
                let (cx, nx) = map_axis(u, c0.x, c1.x);
                let (cz, nz) = map_axis(3 - v, c0.z, c1.z);
                f_py[u][v] = make_pt(cx, c1.y, cz, nx, 1.0, nz);
            }
        }
        emit_grid(f_py);
        if min.y > 0.60 {
            let mut f_ny = [[(Vec3::ZERO, Vec3::ZERO); 4]; 4];
            for u in 0..4 {
                for v in 0..4 {
                    let (cx, nx) = map_axis(u, c0.x, c1.x);
                    let (cz2, nz2) = map_axis(v, c0.z, c1.z);
                    f_ny[u][v] = make_pt(cx, c0.y, cz2, nx, -1.0, nz2);
                }
            }
            emit_grid(f_ny);
        } else {
            self.push_quad(
                Vec3::new(c0.x, min.y, c0.z),
                Vec3::new(c1.x, min.y, c0.z),
                Vec3::new(c1.x, min.y, c1.z),
                Vec3::new(c0.x, min.y, c1.z),
                col,
                mat,
            );
        }
    }

    /// 3-piece hollow U-shaped doorway / window casing along X-Y plane (left jamb, right jamb, top lintel).
    /// Guarantees an open pass-through aperture — never blocks a doorway with a solid face!
    pub fn add_u_frame_xy(
        &mut self,
        open_x0: f32,
        open_x1: f32,
        open_y0: f32,
        open_y1: f32,
        z0: f32,
        z1: f32,
        trim_w: f32,
        col: [f32; 3],
        mat: f32,
    ) {
        // Left & right jambs stop cleanly at `open_y1`; full-width lintel caps `[open_y1, open_y1 + trim_w]`!
        self.add_box(Vec3::new(open_x0 - trim_w, open_y0, z0), Vec3::new(open_x0, open_y1, z1), col, mat);
        self.add_box(Vec3::new(open_x1, open_y0, z0), Vec3::new(open_x1 + trim_w, open_y1, z1), col, mat);
        self.add_box(Vec3::new(open_x0 - trim_w, open_y1, z0), Vec3::new(open_x1 + trim_w, open_y1 + trim_w, z1), col, mat);
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

    /// Smooth round cylinder or tapered cone between arbitrary 3D endpoints `a` and `b`.
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

                // Smooth upward-blended normal option when profile closes at top (organic foliage/domes!)
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

    /// Smooth organic ellipsoid puff (perfect smooth spherical normals so foliage/pillows never look faceted!).
    pub fn add_ellipsoid(&mut self, center: Vec3, radii: Vec3, u_segs: usize, v_segs: usize, col: [f32; 3], mat: f32) {
        let mut grid = Vec::with_capacity((u_segs + 1) * (v_segs + 1));
        for v in 0..=v_segs {
            let phi = -0.5 * core::f64::consts::PI + (v as f64) * core::f64::consts::PI / (v_segs as f64);
            let (cp, sp) = (dcos(phi) as f32, dsin(phi) as f32);
            for u in 0..=u_segs {
                let th = (u as f64) * TAU / (u_segs as f64);
                let (ct, st) = (dcos(th) as f32, dsin(th) as f32);
                let unit = Vec3::new(cp * ct, sp, cp * st);
                let p = Vec3::new(
                    center.x + unit.x * radii.x,
                    center.y + unit.y * radii.y,
                    center.z + unit.z * radii.z,
                );
                let n = Vec3::new(
                    unit.x / radii.x.max(1e-3),
                    unit.y / radii.y.max(1e-3),
                    unit.z / radii.z.max(1e-3),
                )
                .normalized();
                grid.push(self.push_vert(p, n, col, mat));
            }
        }
        let stride = u_segs + 1;
        for v in 0..v_segs {
            for u in 0..u_segs {
                let i0 = grid[v * stride + u];
                let i1 = grid[v * stride + u + 1];
                let i2 = grid[(v + 1) * stride + u + 1];
                let i3 = grid[(v + 1) * stride + u];
                // Outward CCW winding: `(+Y) x (+Z) = +X` at `(phi=0, th=0)`!
                self.indices.extend_from_slice(&[i0, i3, i2, i0, i2, i1]);
            }
        }
    }

    /// Circular disc facing `-Z`.
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

    /// Parametric catenary cable between `p0` and `p1`.
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
            self.add_cylinder(prev, cur, radius, radius, 5, col, MAT_ALLOY, false);
            prev = cur;
        }
    }

    /// Japanese clay Kawara tiled roof slope (`MAT_KAWARA`) sloping down toward `-Z` (toward street/viewer),
    /// complete with dark timber fascia board, rain gutter (`amadoi`), and round eave tile end-caps (`marugawara`)!
    pub fn add_kawara_eave_slope(
        &mut self,
        x0: f32,
        x1: f32,
        y_eave: f32,
        z_eave: f32,
        y_ridge: f32,
        z_ridge: f32,
        tile_col: [f32; 3],
        wood_col: [f32; 3],
    ) {
        // Upper Kawara tile deck (normal +Y/-Z!)
        self.push_quad(
            Vec3::new(x0, y_ridge, z_ridge),
            Vec3::new(x1, y_ridge, z_ridge),
            Vec3::new(x1, y_eave, z_eave),
            Vec3::new(x0, y_eave, z_eave),
            tile_col,
            MAT_KAWARA,
        );
        // Dark roasted-cedar eave underside soffit (`nokiten`, normal -Y/+Z so closed under CULL_FACE!)
        self.push_quad(
            Vec3::new(x0, y_eave - 0.05, z_eave + 0.12),
            Vec3::new(x1, y_eave - 0.05, z_eave + 0.12),
            Vec3::new(x1, y_ridge - 0.08, z_ridge),
            Vec3::new(x0, y_ridge - 0.08, z_ridge),
            wood_col,
            MAT_WOOD,
        );
        // Timber eave soffit & fascia beam under front lip
        self.add_box(Vec3::new(x0, y_eave - 0.11, z_eave), Vec3::new(x1, y_eave, z_eave + 0.14), wood_col, MAT_WOOD);
        // Sculpted round Kawara end-cap cylinders (`marugawara`) along eave front — iconic silhouette from ref images!
        let span = x1 - x0;
        let count = ((span / 0.42) as usize).clamp(4, 20);
        for i in 0..=count {
            let tx = x0 + span * (i as f32) / (count as f32);
            self.add_cylinder(
                Vec3::new(tx, y_eave + 0.035, z_eave + 0.14),
                Vec3::new(tx, y_eave + 0.015, z_eave - 0.02),
                0.048,
                0.052,
                5,
                tile_col,
                MAT_KAWARA,
                false,
            );
            self.add_disc_neg_z(Vec3::new(tx, y_eave + 0.015, z_eave - 0.02), 0.052, 6, tile_col, MAT_KAWARA);
        }
        // Horizontal half-round PVC/copper rain gutter right below the Kawara eave!
        self.add_cylinder(
            Vec3::new(x0 - 0.04, y_eave - 0.08, z_eave - 0.04),
            Vec3::new(x1 + 0.04, y_eave - 0.08, z_eave - 0.04),
            0.038,
            0.038,
            6,
            [0.36, 0.33, 0.31],
            MAT_ALLOY,
            false,
        );
    }
}
