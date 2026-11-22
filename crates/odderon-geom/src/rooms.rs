//! Modular *interior* kit.
//!
//! Same doctrine as `modules` (which does the exteriors): every part is authored
//! once, in a local frame, from plain boxes and quads, and then composed with a
//! `Pose`. A room is a shell plus a list of parts, not a hand-written pile of
//! boxes - that is what makes it possible to change one part and have every
//! room that uses it improve.
//!
//! Conventions, matching the rest of the geometry crate:
//! * `y` is **up** and `y = 0` is the floor plane of the room being built.
//! * Parts that hang on a wall are authored in the **x-y plane* with `+z`
//!   pointing *into* the room, so a wall at `x = 2.54` facing `-x` is placed
//!   with `Pose::at_yaw(x, y, z, PI/2)`, and `z` runs along the wall.
//! * Floor parts are authored flat in `x-z` at `y = 0`.
//!
//! Colours come from [`IntPal`], so a whole room can be re-toned in one place.

use crate::{
    modules::{plate_y, plate_z},
    MeshBuffer, Pose, MAT_ALLOY, MAT_EMISSIVE_WARM, MAT_FOLIAGE, MAT_MATTE,
    MAT_SHOJI, MAT_TATAMI, MAT_WOOD,
};
use odderon_dmath::Vec3;

/// Interior palette: one tint per material story, so a room re-tones as a unit.
#[derive(Clone, Copy, Debug)]
pub struct IntPal {
    /// Lime plaster (`shikkui`) wall.
    pub plaster: [f32; 3],
    /// Cedar/pine joinery: posts, rails, frames, furniture carcasses.
    pub wood: [f32; 3],
    /// Darker stained cedar: cornices, baseboards, beam sides.
    pub wood_dark: [f32; 3],
    /// Light new timber: desk tops, shelf boards, ceiling boards.
    pub wood_light: [f32; 3],
    /// Washi paper.
    pub paper: [f32; 3],
    /// Woven rush for tatami faces.
    pub rush: [f32; 3],
    /// Tatami cloth border (`heri`).
    pub heri: [f32; 3],
    /// Cloth: cushions, curtains, noren.
    pub cloth: [f32; 3],
    /// Accent cloth (indigo, persimmon) - the one saturated colour allowed.
    pub accent: [f32; 3],
    /// Glazed ceramic: pots, cups, tiles.
    pub ceramic: [f32; 3],
    /// Metal: lamp shades, tap, handles, tracks.
    pub metal: [f32; 3],
}

impl IntPal {
    /// The Hab #204 story: warm shikkui, cedar joinery, aged rush, indigo cloth.
    pub const HAB: Self = Self {
        plaster: [0.660, 0.618, 0.540],
        wood: [0.330, 0.205, 0.118],
        wood_dark: [0.150, 0.088, 0.050],
        wood_light: [0.520, 0.372, 0.212],
        paper: [0.880, 0.845, 0.760],
        rush: [0.235, 0.228, 0.118],
        heri: [0.098, 0.125, 0.150],
        cloth: [0.360, 0.330, 0.292],
        accent: [0.128, 0.196, 0.300],
        ceramic: [0.700, 0.690, 0.650],
        metal: [0.560, 0.575, 0.590],
    };

    fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
        [
            a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t,
        ]
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn slab(b: &mut MeshBuffer, x0: f32, x1: f32, y0: f32, y1: f32, z0: f32, z1: f32, col: [f32; 3], mat: f32) {
    b.add_box(Vec3::new(x0, y0, z0), Vec3::new(x1, y1, z1), col, mat);
}

/// Deterministic texture jitter so repeated boards/scales never share a tone.
#[inline]
fn wav(i: usize, seed: u32) -> f32 {
    let h = (i as u32).wrapping_mul(0x9E37_79B1) ^ seed.wrapping_mul(0x85EB_CA6B);
    let h = h ^ (h >> 15);
    ((h & 0xFFFF) as f32) / 65535.0
}

// ---------------------------------------------------------------------------
// Floor
// ---------------------------------------------------------------------------

/// A tatami floor: a raised timber deck, then mats laid in the traditional
/// staggered pattern, each with its dark cloth border and a seam of shadow
/// between mats. `x0..x1` / `z0..z1` is the matted area in room space.
///
/// Mats are 1.82 x 0.91 m, which is what fixes the pattern to the room: the
/// grid is fitted and then the last row/column is trimmed, like a real job.
pub fn tatami_floor(b: &mut MeshBuffer, x0: f32, x1: f32, z0: f32, z1: f32, pal: &IntPal, seed: u32) {
    const W: f32 = 1.82;
    const D: f32 = 0.91;
    // Deck under the mats, with a visible recess line at its edge.
    slab(b, x0 - 0.02, x1 + 0.02, -0.005, 0.018, z0 - 0.02, z1 + 0.02, pal.wood_dark, MAT_WOOD);
    slab(b, x0, x1, 0.018, 0.024, z0, z1, [0.075, 0.070, 0.062], MAT_MATTE);

    // Fit the mats to the room: mats run along the longer side of the area, so a
    // 3.3 x 2.3 m floor takes six mats in 2 x 3 rather than four 2.3 m planks.
    let long_x = (x1 - x0) >= (z1 - z0);
    let (cols, rows) = if long_x {
        (
            ((x1 - x0) / W).round().max(1.0) as usize,
            ((z1 - z0) / D).round().max(1.0) as usize,
        )
    } else {
        (
            ((x1 - x0) / D).round().max(1.0) as usize,
            ((z1 - z0) / W).round().max(1.0) as usize,
        )
    };
    let cw = (x1 - x0) / cols as f32;
    let rh = (z1 - z0) / rows as f32;

    for r in 0..rows {
        for c in 0..cols {
            // Every other row is offset by half a mat: the classic staggered
            // laying, and the reason a tatami floor never looks like tiles.
            let mx0 = x0 + cw * c as f32;
            let mz0 = z0 + rh * r as f32;
            let mx1 = mx0 + cw;
            let mz1 = mz0 + rh;
            let t = wav(r * 7 + c, seed);
            let face = IntPal::mix3(pal.rush, [0.320, 0.310, 0.170], t * 0.6);
            // Woven rush has a grain across the mat: a handful of slightly
            // darker bands is enough to keep the face from reading as painted
            // board, and it costs a few hundred triangles for the whole room.
            let weave_n = 7usize;
            for wi in 0..weave_n {
                let wt = (wi as f32 + 0.5) / weave_n as f32;
                let shade = 0.86 + 0.10 * wav(wi + r * 3 + c, seed);
                let c = [face[0] * shade, face[1] * shade, face[2] * shade];
                if (mx1 - mx0) > (mz1 - mz0) {
                    let y = mz0 + (mz1 - mz0) * wt;
                    plate_y(b, mx0 + 0.05, mx1 - 0.05, y - 0.011, y + 0.011, 0.0505, true, c, MAT_TATAMI);
                } else {
                    let x = mx0 + (mx1 - mx0) * wt;
                    plate_y(b, x - 0.011, x + 0.011, mz0 + 0.05, mz1 - 0.05, 0.0505, true, c, MAT_TATAMI);
                }
            }
            slab(b, mx0 + 0.006, mx1 - 0.006, 0.024, 0.049, mz0 + 0.006, mz1 - 0.006, face, MAT_TATAMI);
            // Cloth border on the two long sides (`heri`), which is what stops
            // a tatami looking like a straw biscuit.
            let long_along_x = (mx1 - mx0) > (mz1 - mz0);
            if long_along_x {
                slab(b, mx0 + 0.006, mx1 - 0.006, 0.024, 0.050, mz0 + 0.006, mz0 + 0.040, pal.heri, MAT_MATTE);
                slab(b, mx0 + 0.006, mx1 - 0.006, 0.024, 0.050, mz1 - 0.040, mz1 - 0.006, pal.heri, MAT_MATTE);
            } else {
                slab(b, mx0 + 0.006, mx0 + 0.040, 0.024, 0.050, mz0 + 0.006, mz1 - 0.006, pal.heri, MAT_MATTE);
                slab(b, mx1 - 0.040, mx1 - 0.006, 0.024, 0.050, mz0 + 0.006, mz1 - 0.006, pal.heri, MAT_MATTE);
            }
        }
    }
}

/// Boards: a plank floor or a plank ceiling. Tone varies per board, and the
/// boards are butted with a 4 mm shadow gap rather than drawn as one slab.
pub fn plank_deck(
    b: &mut MeshBuffer,
    x0: f32,
    x1: f32,
    z0: f32,
    z1: f32,
    y: f32,
    thickness: f32,
    run_along_x: bool,
    pal: &IntPal,
    seed: u32,
) {
    let span = if run_along_x { x1 - x0 } else { z1 - z0 };
    let n = ((span / 0.145).round().max(3.0)) as usize;
    let step = span / n as f32;
    for i in 0..n {
        let t = 0.88 + 0.26 * wav(i, seed);
        let col = IntPal::mix3(pal.wood_light, pal.wood, wav(i + 31, seed) * 0.55);
        let col = [col[0] * t, col[1] * t, col[2] * t];
        // One board is a board, not a beam: a face up and a face down cost four
        // triangles and still close the deck from either side, where a box cost
        // twelve and spent ten of them on 2 cm edges nobody can see.
        if run_along_x {
            let bx0 = x0 + step * i as f32;
            plate_y(b, bx0 + 0.002, bx0 + step - 0.002, z0, z1, y + thickness, true, col, MAT_WOOD);
            plate_y(b, bx0 + 0.002, bx0 + step - 0.002, z0, z1, y, false, col, MAT_WOOD);
        } else {
            let bz0 = z0 + step * i as f32;
            plate_y(b, x0, x1, bz0 + 0.002, bz0 + step - 0.002, y + thickness, true, col, MAT_WOOD);
            plate_y(b, x0, x1, bz0 + 0.002, bz0 + step - 0.002, y, false, col, MAT_WOOD);
        }
    }
}

// ---------------------------------------------------------------------------
// Screens
// ---------------------------------------------------------------------------

/// One shoji panel: a cedar frame with a `kumiko` grid and washi behind it.
/// Authored standing up in the x-y plane, `+z` = into the room, centred on x = 0
/// and based at y = 0. Place with a `Pose` (yaw) on the wall it belongs to.
pub fn shoji_panel(b: &mut MeshBuffer, w: f32, h: f32, pal: &IntPal, seed: u32) {
    let hw = w * 0.5;
    let ft = 0.032; // frame thickness
    let fd = 0.030; // frame depth
    // Paper sits behind the grid, glowing very slightly. It is the one surface
    // on a shoji that genuinely has two sides, so it keeps its volume.
    slab(b, -hw + ft, hw - ft, ft, h - ft, -0.004, 0.001, pal.paper, MAT_SHOJI);
    // Outer frame. Every member of a shoji is a flat board seen face-on, so it
    // is a plate at the face plane: four boxes were 48 triangles of end grain
    // and inner faces that no one can see through the paper behind them.
    plate_z(b, -hw, hw, 0.0, ft, fd, pal.wood, MAT_WOOD);
    plate_z(b, -hw, hw, h - ft, h, fd, pal.wood, MAT_WOOD);
    plate_z(b, -hw, -hw + ft, ft, h - ft, fd, pal.wood, MAT_WOOD);
    plate_z(b, hw - ft, hw, ft, h - ft, fd, pal.wood, MAT_WOOD);
    // Kumiko: 3 vertical, 4 horizontal bars, at slightly uneven spacing so the
    // paper panes are not all the same size.
    for i in 1..4 {
        let x = -hw + ft + (w - 2.0 * ft) * (i as f32) / 4.0 + (wav(i, seed) - 0.5) * 0.012;
        plate_z(b, x - 0.008, x + 0.008, ft, h - ft, 0.018, pal.wood, MAT_WOOD);
    }
    for i in 1..5 {
        let y = ft + (h - 2.0 * ft) * (i as f32) / 5.0 + (wav(i + 9, seed) - 0.5) * 0.012;
        plate_z(b, -hw + ft, hw - ft, y - 0.008, y + 0.008, 0.018, pal.wood, MAT_WOOD);
    }
    // Kick rail along the bottom and a pull recess on the leading edge.
    plate_z(b, -hw + ft, hw - ft, 0.10, 0.145, 0.026, pal.wood_dark, MAT_WOOD);
    plate_z(b, hw - ft - 0.055, hw - ft, h * 0.45, h * 0.55, 0.028, pal.wood_dark, MAT_WOOD);
}

/// A run of sliding shoji in a timber track: `panels` leaves, one of them slid
/// back over its neighbour, which is what makes a screen read as *open* rather
/// than as a wall with lines drawn on it. Authored in x-y, `+z` into the room.
pub fn shoji_run(b: &mut MeshBuffer, w: f32, h: f32, panels: usize, pal: &IntPal, seed: u32) {
    let panels = panels.max(2);
    let pw = w / panels as f32;
    // Head and floor tracks.
    slab(b, -w * 0.5, w * 0.5, h, h + 0.05, -0.03, 0.05, pal.wood_dark, MAT_WOOD);
    slab(b, -w * 0.5, w * 0.5, 0.0, 0.028, -0.02, 0.06, pal.wood_dark, MAT_WOOD);
    for i in 0..panels {
        let cx = -w * 0.5 + pw * (i as f32 + 0.5);
        // Alternate leaves between the near and far track.
        let z = if i % 2 == 0 { 0.012 } else { 0.044 };
        // The second leaf is open: it slides behind the first.
        let (px, pz) = if i == 1 && panels > 2 {
            (cx - pw * 0.55, z + 0.004)
        } else {
            (cx, z)
        };
        b.with_pose(Pose::at(px, 0.02, pz), |b| {
            shoji_panel(b, pw - 0.012, h - 0.06, pal, seed.wrapping_add(i as u32 * 31))
        });
    }
}

/// An `oshiire`: a closet with two sliding panelled doors, a raised sill and a
/// dark reveal behind them. Fills a wall without pretending the room is bigger
/// than it is. Authored in x-y, `+z` into the room.
pub fn oshiire(b: &mut MeshBuffer, w: f32, h: f32, pal: &IntPal, seed: u32) {
    let hw = w * 0.5;
    // Dark interior box so the doors open onto shadow, not onto the wall.
    slab(b, -hw + 0.05, hw - 0.05, 0.06, h - 0.05, 0.0, 0.02, [0.045, 0.040, 0.036], MAT_MATTE);
    for i in 0..2 {
        let x0 = -hw + 0.04 + (w - 0.08) * (i as f32) * 0.5;
        let x1 = x0 + (w - 0.08) * 0.5 - 0.008;
        let z = 0.03 + 0.012 * i as f32;
        // Frame.
        let d = 0.024;
        slab(b, x0, x1, 0.06, h - 0.06, z, z + d, pal.wood_light, MAT_WOOD);
        slab(b, x0 + 0.05, x1 - 0.05, 0.14, h - 0.14, z + d, z + d + 0.004, IntPal::mix3(pal.paper, pal.wood_light, 0.55), MAT_MATTE);
        // Recessed pull.
        slab(
            b,
            if i == 0 { x1 - 0.09 } else { x0 + 0.02 },
            if i == 0 { x1 - 0.02 } else { x0 + 0.09 },
            h * 0.48,
            h * 0.56,
            z + d,
            z + d + 0.006,
            pal.wood_dark,
            MAT_WOOD,
        );
        let _ = seed;
    }
    // Head casing.
    slab(b, -hw, hw, h - 0.06, h, 0.0, 0.07, pal.wood, MAT_WOOD);
}

// ---------------------------------------------------------------------------
// Furniture
// ---------------------------------------------------------------------------

/// Low table: a plank top on four splayed legs, with a shadow gap between top
/// and apron so it reads as furniture rather than a box.
pub fn low_table(b: &mut MeshBuffer, cx: f32, cz: f32, w: f32, d: f32, h: f32, pal: &IntPal) {
    let hw = w * 0.5;
    let hd = d * 0.5;
    slab(b, cx - hw, cx + hw, h - 0.042, h, cz - hd, cz + hd, pal.wood_light, MAT_WOOD);
    // Top edge chamfer: a proud lip on the two long sides.
    slab(b, cx - hw, cx + hw, h - 0.052, h - 0.040, cz - hd - 0.006, cz - hd + 0.010, pal.wood, MAT_WOOD);
    slab(b, cx - hw, cx + hw, h - 0.052, h - 0.040, cz + hd - 0.010, cz + hd + 0.006, pal.wood, MAT_WOOD);
    for sx in [-1.0_f32, 1.0] {
        for sz in [-1.0_f32, 1.0] {
            let x = cx + sx * (hw - 0.075);
            let z = cz + sz * (hd - 0.075);
            b.add_beam(
                Vec3::new(x, 0.0, z),
                Vec3::new(x + sx * 0.012, h - 0.042, z + sz * 0.012),
                0.024,
                pal.wood,
                MAT_WOOD,
            );
        }
    }
    // Stretcher between the legs, low.
    slab(b, cx - hw + 0.075, cx + hw - 0.075, 0.10, 0.135, cz - 0.02, cz + 0.02, pal.wood, MAT_WOOD);
}

/// A floor cushion (`zabuton`): a squashed, tufted pad. Two of these and a low
/// table is a whole living room.
pub fn cushion(b: &mut MeshBuffer, cx: f32, cz: f32, w: f32, d: f32, col: [f32; 3]) {
    b.add_rounded_box(
        Vec3::new(cx - w * 0.5, 0.0, cz - d * 0.5),
        Vec3::new(cx + w * 0.5, 0.085, cz + d * 0.5),
        0.035,
        col,
        MAT_MATTE,
    );
    // Tuft in the middle.
    b.add_box(
        Vec3::new(cx - 0.02, 0.082, cz - 0.02),
        Vec3::new(cx + 0.02, 0.092, cz + 0.02),
        [col[0] * 0.7, col[1] * 0.7, col[2] * 0.7],
        MAT_MATTE,
    );
}

/// Paper pendant lamp: cord, metal cap, washi drum. Warm because it is on.
pub fn pendant_lamp(b: &mut MeshBuffer, cx: f32, ceiling_y: f32, drop: f32, cz: f32, r: f32, pal: &IntPal) {
    let y = ceiling_y - drop;
    b.add_beam(
        Vec3::new(cx, ceiling_y, cz),
        Vec3::new(cx, y + 0.16, cz),
        0.008,
        [0.11, 0.10, 0.09],
        MAT_ALLOY,
    );
    slab(b, cx - 0.05, cx + 0.05, y + 0.14, y + 0.18, cz - 0.05, cz + 0.05, pal.metal, MAT_ALLOY);
    b.add_cylinder(
        Vec3::new(cx, y - 0.16, cz),
        Vec3::new(cx, y + 0.14, cz),
        r,
        r * 0.86,
        12,
        pal.paper,
        MAT_SHOJI,
        false,
    );
    // Two rings, so the drum is not a naked cylinder.
    for ry in [y - 0.10, y + 0.02] {
        b.add_cylinder(
            Vec3::new(cx, ry, cz),
            Vec3::new(cx, ry + 0.016, cz),
            r * 0.99,
            r * 0.99,
            12,
            IntPal::mix3(pal.wood_light, pal.metal, 0.35),
            MAT_ALLOY,
            false,
        );
    }
    // The bulb, visible through the washi from below.
    b.add_cylinder(
        Vec3::new(cx, y - 0.14, cz),
        Vec3::new(cx, y - 0.06, cz),
        0.035,
        0.045,
        8,
        [1.0, 0.86, 0.58],
        MAT_EMISSIVE_WARM,
        true,
    );
}

/// Curtains: a rail plus two gathered panels. The folds are modelled, not
/// painted, so they catch the window light and move the room from "rendered
/// box" to "somebody lives here".
pub fn curtain_pair(
    b: &mut MeshBuffer,
    w: f32,
    y_top: f32,
    y_bottom: f32,
    z: f32,
    panel_w: f32,
    pal: &IntPal,
    col: [f32; 3],
) {
    let hw = w * 0.5;
    // Rail and brackets.
    b.add_beam(
        Vec3::new(-hw, y_top, z),
        Vec3::new(hw, y_top, z),
        0.014,
        pal.metal,
        MAT_ALLOY,
    );
    for sx in [-1.0_f32, 1.0] {
        b.add_beam(
            Vec3::new(sx * (hw - 0.06), y_top, z),
            Vec3::new(sx * (hw - 0.06), y_top, z - 0.09),
            0.010,
            pal.metal,
            MAT_ALLOY,
        );
    }
    for sx in [-1.0_f32, 1.0] {
        let x_out = sx * hw;
        let x_in = sx * (hw - panel_w);
        let folds = 5;
        let fw = panel_w / folds as f32;
        for f in 0..folds {
            let x0 = x_in.min(x_out) + fw * f as f32;
            let x1 = x0 + fw;
            // Alternate the panel in and out of plane: a gathered panel has a
            // real silhouette, and the folds give the shader something to shade.
            let zf = z - 0.028 - 0.024 * if f % 2 == 0 { 1.0 } else { 0.35 };
            let tone = 0.86 + 0.20 * wav(f + (sx > 0.0) as usize * 5, 3);
            let c = [col[0] * tone, col[1] * tone, col[2] * tone];
            slab(b, x0, x1 - 0.004, y_bottom, y_top - 0.02, zf, zf + 0.018, c, MAT_MATTE);
        }
        // Header tape and hem.
        slab(b, x_in.min(x_out), x_in.max(x_out), y_top - 0.055, y_top - 0.02, z - 0.075, z - 0.02, pal.cloth, MAT_MATTE);
        slab(b, x_in.min(x_out), x_in.max(x_out), y_bottom, y_bottom + 0.035, z - 0.075, z - 0.02, pal.cloth, MAT_MATTE);
    }
}

/// Potted plant: a glazed pot, soil, then a fan of stems and leaf plates. Sits
/// on the floor at `y = 0` (or anywhere, if composed with a pose).
pub fn plant(b: &mut MeshBuffer, cx: f32, cz: f32, scale: f32, pal: &IntPal, seed: u32) {
    let r = 0.13 * scale;
    let h = 0.26 * scale;
    b.add_cylinder(
        Vec3::new(cx, 0.0, cz),
        Vec3::new(cx, h, cz),
        r * 0.82,
        r,
        10,
        pal.ceramic,
        MAT_MATTE,
        true,
    );
    b.add_cylinder(
        Vec3::new(cx, h - 0.02, cz),
        Vec3::new(cx, h + 0.005, cz),
        r * 0.92,
        r * 0.92,
        10,
        IntPal::mix3(pal.ceramic, [0.24, 0.22, 0.19], 0.5),
        MAT_MATTE,
        false,
    );
    b.add_cylinder(
        Vec3::new(cx, h - 0.03, cz),
        Vec3::new(cx, h - 0.005, cz),
        r * 0.84,
        r * 0.84,
        8,
        [0.088, 0.070, 0.050],
        MAT_MATTE,
        false,
    );
    for i in 0..7 {
        let a = (wav(i, seed) - 0.5) * 2.4;
        let lean = 0.10 + 0.16 * wav(i + 3, seed);
        let len = (0.34 + 0.26 * wav(i + 7, seed)) * scale;
        let top = Vec3::new(cx + a * 0.10, h + len, cz + lean * 0.4);
        b.add_beam(Vec3::new(cx, h, cz), top, 0.007 * scale, [0.12, 0.16, 0.08], MAT_FOLIAGE);
        // Leaf plate: a flattened ellipsoid at the tip of each stem.
        b.add_ellipsoid(
            Vec3::new(top.x + a * 0.05, top.y, top.z + lean * 0.2),
            Vec3::new(0.13 * scale, 0.055 * scale, 0.10 * scale),
            7,
            4,
            IntPal::mix3([0.088, 0.145, 0.055], [0.150, 0.215, 0.075], wav(i + 11, seed)),
            MAT_FOLIAGE,
        );
    }
}

/// Open shelving with the clutter that makes it read as used: books of varying
/// height and depth, a couple of boxes, a bowl.
pub fn shelf_run(
    b: &mut MeshBuffer,
    w: f32,
    h: f32,
    depth: f32,
    shelves: usize,
    pal: &IntPal,
    seed: u32,
) {
    let hw = w * 0.5;
    // Carcass: two sides, a back, and the shelf boards.
    slab(b, -hw, -hw + 0.028, 0.0, h, 0.0, depth, pal.wood, MAT_WOOD);
    slab(b, hw - 0.028, hw, 0.0, h, 0.0, depth, pal.wood, MAT_WOOD);
    slab(b, -hw, hw, 0.0, h, 0.0, 0.012, pal.wood_dark, MAT_WOOD);
    let n = shelves.max(2);
    for i in 0..=n {
        let y = h * (i as f32) / n as f32;
        slab(b, -hw + 0.028, hw - 0.028, y - 0.018, y, 0.012, depth, pal.wood_light, MAT_WOOD);
    }
    // Contents: books on the top shelf, boxes and a bowl lower down.
    let mut k = 0usize;
    for i in 1..n {
        let y = h * (i as f32) / n as f32;
        let mut x = -hw + 0.05;
        while x < hw - 0.12 {
            let bw = 0.035 + 0.045 * wav(k, seed);
            let bh = 0.16 + 0.14 * wav(k + 17, seed);
            let bd = 0.12 + 0.08 * wav(k + 5, seed);
            let col = IntPal::mix3(pal.cloth, pal.accent, wav(k + 29, seed));
            // A book in a packed row shows its spine: one face, two triangles.
            let top = y + bh.min(h / n as f32 - 0.04);
            plate_z(b, x, x + bw, y, top, 0.03 + bd, col, MAT_MATTE);
            plate_y(b, x, x + bw, 0.03 + bd - 0.02, 0.03 + bd, top, true, col, MAT_MATTE);
            x += bw + 0.008;
            k += 1;
        }
    }
    if n >= 2 {
        // A bowl on the middle board.
        let y = h / n as f32;
        b.add_cylinder(
            Vec3::new(hw * 0.35, y, depth * 0.5),
            Vec3::new(hw * 0.35, y + 0.055, depth * 0.5),
            0.055,
            0.075,
            9,
            pal.ceramic,
            MAT_MATTE,
            true,
        );
    }
}

/// A wall scroll: hanging rod, washi field, and a dark ink wash. One of these
/// on a blank plaster wall does more than any amount of extra trim.
pub fn wall_scroll(b: &mut MeshBuffer, w: f32, h: f32, pal: &IntPal, seed: u32) {
    let hw = w * 0.5;
    // Mount: brocade border, then the paper field inside it.
    slab(b, -hw, hw, 0.0, h, 0.0, 0.010, IntPal::mix3(pal.cloth, pal.wood_dark, 0.4), MAT_MATTE);
    slab(b, -hw + 0.055, hw - 0.055, 0.055, h - 0.055, 0.010, 0.013, pal.paper, MAT_SHOJI);
    // Ink: a few strokes, deterministic per seed.
    for i in 0..3 {
        let y0 = h * (0.28 + 0.18 * i as f32) + (wav(i, seed) - 0.5) * 0.04;
        let wdt = (hw - 0.09) * (0.5 + 0.5 * wav(i + 4, seed));
        slab(b, -wdt * 0.5, wdt * 0.5, y0, y0 + 0.012, 0.013, 0.015, [0.10, 0.098, 0.094], MAT_MATTE);
    }
    // Rods top and bottom.
    for y in [0.0, h - 0.03] {
        b.add_beam(
            Vec3::new(-hw - 0.03, y + 0.015, 0.02),
            Vec3::new(hw + 0.03, y + 0.015, 0.02),
            0.011,
            pal.wood_dark,
            MAT_WOOD,
        );
    }
}

// ---------------------------------------------------------------------------
// Composer
// ---------------------------------------------------------------------------

/// What a room needs to be authored from parts: the inner wall rectangle, the
/// ceiling height, and which parts are switched on.
#[derive(Clone, Copy, Debug)]
pub struct RoomSpec {
    /// Inner half-width along x (the room is `-hw..hw`).
    pub hw: f32,
    /// Inner extent along z (`-hz..hz`).
    pub hz: f32,
    pub ceiling: f32,
    pub pal: IntPal,
    pub seed: u32,
    /// Lay tatami over the living area centred a little off the entry.
    pub tatami: bool,
    /// Shoji run on the `+x` wall, with the width given.
    pub shoji_w: f32,
    /// An oshiire on the `-z` wall at `x = oshiire_x` with the given width.
    pub oshiire: Option<(f32, f32)>,
    pub scroll: bool,
    pub plant: bool,
    pub pendant: bool,
}

/// Compose an interior from the kit: floor, wall screens, ceiling, and the
/// furniture that reads from the doorway. This is the interior counterpart of
/// [`crate::modules::building`] - one spec, many possible rooms.
pub fn room(b: &mut MeshBuffer, s: &RoomSpec) {
    let pal = &s.pal;
    // --- ceiling --------------------------------------------------------
    // Coffered: plank deck, then exposed beams in both directions, then a
    // cornice. A flat slab ceiling is the single biggest tell of a box room.
    plank_deck(b, -s.hw, s.hw, -s.hz, s.hz, s.ceiling - 0.02, 0.02, true, pal, s.seed);
    let beam_n = ((s.hz * 2.0) / 1.05).round().max(2.0) as usize;
    for i in 0..=beam_n {
        let z = -s.hz + (s.hz * 2.0) * (i as f32) / beam_n as f32;
        slab(b, -s.hw, s.hw, s.ceiling - 0.115, s.ceiling - 0.02, z - 0.035, z + 0.035, pal.wood_dark, MAT_WOOD);
    }
    for i in 0..2 {
        let x = -s.hw * 0.5 + s.hw * i as f32;
        slab(b, x - 0.026, x + 0.026, s.ceiling - 0.135, s.ceiling - 0.10, -s.hz, s.hz, pal.wood, MAT_WOOD);
    }
    // Cornice round the perimeter, closing the gap between beams and wall.
    for &(x0, x1, z0, z1) in &[
        (-s.hw, s.hw, -s.hz, -s.hz + 0.075),
        (-s.hw, s.hw, s.hz - 0.075, s.hz),
        (-s.hw, -s.hw + 0.075, -s.hz, s.hz),
        (s.hw - 0.075, s.hw, -s.hz, s.hz),
    ] {
        slab(b, x0, x1, s.ceiling - 0.115, s.ceiling, z0, z1, pal.wood_dark, MAT_WOOD);
    }

    // --- floor ----------------------------------------------------------
    if s.tatami {
        let x0 = -s.hw + 0.16;
        let x1 = s.hw - 0.28;
        tatami_floor(b, x0, x1, -s.hz + 0.45, s.hz - 0.12, pal, s.seed);
        // Timber margin between the mats and the walls (`tatami-yose`).
        plank_deck(b, x0, x1, s.hz - 0.12, s.hz - 0.02, 0.0, 0.026, true, pal, s.seed ^ 0x51);
        plank_deck(b, x0, x1, -s.hz + 0.02, -s.hz + 0.45, 0.0, 0.026, true, pal, s.seed ^ 0x91);
    }

    // --- screens --------------------------------------------------------
    if s.shoji_w > 0.2 {
        // On the +x wall, facing into the room (-x), running along z.
        let h = s.ceiling - 0.22;
        b.with_pose(Pose::at_yaw(s.hw - 0.03, 0.02, 0.0, -core::f32::consts::FRAC_PI_2), |b| {
            shoji_run(b, s.shoji_w, h, 4, pal, s.seed ^ 0x77)
        });
    }
    if let Some((cx, w)) = s.oshiire {
        let h = s.ceiling - 0.16;
        b.with_pose(Pose::at(cx, 0.02, -s.hz + 0.02), |b| {
            oshiire(b, w, h, pal, s.seed ^ 0x33)
        });
    }
    if s.scroll {
        b.with_pose(Pose::at(-s.hw + 0.035, s.ceiling - 1.62, s.hz * 0.35), |b| {
            b.with_pose(Pose::at_yaw(0.0, 0.0, 0.0, core::f32::consts::FRAC_PI_2), |b| {
                wall_scroll(b, 0.62, 1.28, pal, s.seed ^ 0x11)
            })
        });
    }
    if s.plant {
        plant(b, s.hw - 0.42, s.hz - 0.55, 1.15, pal, s.seed ^ 0xAB);
    }
    if s.pendant {
        pendant_lamp(b, 0.10, s.ceiling - 0.135, 0.62, -0.10, 0.20, pal);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FLOATS_PER_VERTEX;

    /// The interior kit must stay cheap: a whole room is furniture and trim, not
    /// a city block. This also catches a part that accidentally emits a dense
    /// lathe or a per-slat loop with a silly count.
    #[test]
    fn a_room_stays_within_budget() {
        let mut b = MeshBuffer::new();
        room(
            &mut b,
            &RoomSpec {
                hw: 2.6,
                hz: 2.1,
                ceiling: 2.7,
                pal: IntPal::HAB,
                seed: 5,
                tatami: true,
                shoji_w: 1.8,
                oshiire: Some((1.2, 1.6)),
                scroll: true,
                plant: true,
                pendant: true,
            },
        );
        let tris = b.indices.len() / 3;
        assert!(tris > 400, "room is too sparse: {tris} tris");
        assert!(tris < 9000, "room is too heavy: {tris} tris");
        assert_eq!(b.verts.len() % FLOATS_PER_VERTEX, 0);
    }
}
