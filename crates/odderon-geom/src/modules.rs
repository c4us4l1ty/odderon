//! Modular building kit (`modules.rs`).
//!
//! Why this file exists: a town built out of "one box per building" reads as a
//! cartoon no matter how good the grade is, because every facade is a single
//! flat plane with a window painted on it. The reference frames are the
//! opposite: a facade is a *stack of small manufactured parts* - a stone plinth,
//! a board skin, a proud timber frame, an aluminium sash set back into a reveal,
//! a sill that casts its own shadow, a drip line under it, a meter box, a
//! downpipe, a satin-painted shutter.
//!
//! So the kit is deliberately made of parts, and every part is parametric:
//!
//! * [`wall`] - plaster / weathered board / corrugated / tile skins
//! * [`window_unit`] - sash, lattice, casement, shopfront, shoji
//! * [`door_unit`] - shop glass, plank, roller shutter, noren
//! * [`eave`], [`roof_kawara`], [`roof_corrugated`], [`balcony`]
//! * [`ac_unit`], [`downpipe`], [`meter_box`], [`sign_plate`], [`lantern`],
//!   [`vent`], [`ailerons`] ... small kit parts
//! * [`building`] - composes the above from a [`BuildingSpec`] plus a seed
//!
//! Coordinate convention: every part is authored in *facade space* - `x` is
//! centred on the bay, `y` runs up from the building's base, `z = 0` is the
//! wall plane and `+Z` points out of the wall. Callers wrap the whole thing in
//! `buf.with_pose(pose, ..)`, so a building can be dropped anywhere at any yaw.
//!
//! Variation is the point, so nothing here is hand-placed: [`Rng`] (a seeded
//! xorshift) decides bay widths, window styles, which side the door is on, how
//! tall the plinth is, whether the sash is open, where the downpipe hangs. Two
//! buildings with different seeds share every part but none of their look.

use crate::{
    MeshBuffer, MAT_ALLOY, MAT_CERAMIC, MAT_CORRUGATED, MAT_EMISSIVE_WARM, MAT_FOLIAGE, MAT_KAWARA,
    MAT_MATTE, MAT_SHOJI, MAT_WOOD,
};
use odderon_dmath::Vec3;

// ===========================================================================
// Deterministic variation
// ===========================================================================

/// Seeded xorshift32. Deterministic so a rebuild is byte-identical, and cheap
/// enough to call per-bay without showing up in the build time.
pub struct Rng(u32);

impl Rng {
    pub const fn new(seed: u32) -> Self {
        Self(seed | 1)
    }
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// `[0, 1)`
    pub fn f(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / 16_777_216.0
    }
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
    pub fn pick(&mut self, n: usize) -> usize {
        (self.next_u32() as usize) % n.max(1)
    }
    pub fn chance(&mut self, p: f32) -> bool {
        self.f() < p
    }
}

// ===========================================================================
// Palettes
// ===========================================================================

/// One building's material story. Everything the kit emits takes its colours
/// from here, so a palette swap re-skins a whole facade consistently.
#[derive(Clone, Copy, Debug)]
pub struct Pal {
    pub plaster: [f32; 3],
    pub plaster_dark: [f32; 3],
    pub board: [f32; 3],
    pub board_dark: [f32; 3],
    pub timber: [f32; 3],
    pub timber_light: [f32; 3],
    pub steel: [f32; 3],
    pub trim: [f32; 3],
    pub stone: [f32; 3],
    pub tile: [f32; 3],
    pub glass: [f32; 3],
    pub accent: [f32; 3],
}

impl Pal {
    /// Aged lime plaster over roasted cedar - the machiya default.
    pub const MACHIYA: Self = Self {
        plaster: [0.442, 0.418, 0.382],
        plaster_dark: [0.330, 0.310, 0.284],
        board: [0.150, 0.112, 0.088],
        board_dark: [0.098, 0.070, 0.054],
        timber: [0.135, 0.082, 0.050],
        timber_light: [0.260, 0.170, 0.104],
        steel: [0.250, 0.290, 0.330],
        trim: [0.560, 0.545, 0.505],
        stone: [0.360, 0.350, 0.320],
        tile: [0.150, 0.160, 0.186],
        glass: [0.075, 0.090, 0.105],
        accent: [0.520, 0.170, 0.130],
    };
    /// Whitewashed kura storehouse: brighter plaster, charcoal tile, dark cedar.
    pub const KURA: Self = Self {
        plaster: [0.520, 0.500, 0.470],
        plaster_dark: [0.380, 0.365, 0.345],
        board: [0.185, 0.135, 0.098],
        board_dark: [0.115, 0.082, 0.060],
        timber: [0.120, 0.075, 0.048],
        timber_light: [0.240, 0.160, 0.100],
        steel: [0.230, 0.240, 0.250],
        trim: [0.470, 0.455, 0.425],
        stone: [0.330, 0.320, 0.300],
        tile: [0.138, 0.146, 0.166],
        glass: [0.070, 0.082, 0.095],
        accent: [0.240, 0.290, 0.360],
    };
    /// Galvanised steel shed / workshop annex: cold, ribbed, rusting.
    pub const INDUSTRIAL: Self = Self {
        plaster: [0.380, 0.375, 0.360],
        plaster_dark: [0.280, 0.275, 0.265],
        board: [0.210, 0.170, 0.130],
        board_dark: [0.130, 0.100, 0.075],
        timber: [0.150, 0.110, 0.078],
        timber_light: [0.280, 0.210, 0.150],
        steel: [0.240, 0.265, 0.285],
        trim: [0.330, 0.345, 0.360],
        stone: [0.340, 0.330, 0.310],
        tile: [0.156, 0.168, 0.188],
        glass: [0.085, 0.100, 0.115],
        accent: [0.145, 0.310, 0.520],
    };
    /// Small shop with a painted timber front and an awning.
    pub const SHOP: Self = Self {
        plaster: [0.470, 0.448, 0.412],
        plaster_dark: [0.340, 0.322, 0.298],
        board: [0.230, 0.128, 0.082],
        board_dark: [0.145, 0.082, 0.055],
        timber: [0.165, 0.095, 0.058],
        timber_light: [0.320, 0.205, 0.125],
        steel: [0.260, 0.285, 0.300],
        trim: [0.545, 0.520, 0.470],
        stone: [0.355, 0.345, 0.320],
        tile: [0.152, 0.160, 0.180],
        glass: [0.085, 0.100, 0.108],
        accent: [0.230, 0.240, 0.520],
    };

    /// Blend toward another palette - lets one spec interpolate between stories
    /// (a shop that grew out of a house is half SHOP, half MACHIYA).
    pub fn mix(self, o: Pal, t: f32) -> Pal {
        let m = |a: [f32; 3], b: [f32; 3]| {
            [
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
            ]
        };
        Pal {
            plaster: m(self.plaster, o.plaster),
            plaster_dark: m(self.plaster_dark, o.plaster_dark),
            board: m(self.board, o.board),
            board_dark: m(self.board_dark, o.board_dark),
            timber: m(self.timber, o.timber),
            timber_light: m(self.timber_light, o.timber_light),
            steel: m(self.steel, o.steel),
            trim: m(self.trim, o.trim),
            stone: m(self.stone, o.stone),
            tile: m(self.tile, o.tile),
            glass: m(self.glass, o.glass),
            accent: m(self.accent, o.accent),
        }
    }
}

// ===========================================================================
// Kit enums
// ===========================================================================

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Skin {
    /// Lime plaster, mottled, with a proud timber frame.
    Plaster,
    /// Vertical cedar boards, gaps visible, darker toward the ground.
    Board,
    /// Vertical cedar boards over a plaster upper storey.
    BoardOverPlaster,
    /// Ribbed galvanised steel.
    Corrugated,
    /// Rough Oya-stone block.
    Stone,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WindowStyle {
    /// Aluminium sliding sash, two lights, set back in a reveal. The most common.
    Sash,
    /// Dark timber lattice (koshi) over shoji paper - machiya ground floor.
    Lattice,
    /// Steel casement, single pane, painted frame.
    Casement,
    /// Wide low shopfront glass with a transom and slim mullions.
    Shopfront,
    /// Full shoji paper screen.
    Shoji,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DoorStyle {
    /// Aluminium-framed sliding glass, the shop kind.
    ShopGlass,
    /// Vertical plank door, iron strap.
    Plank,
    /// Corrugated roller shutter, half up.
    RollerShutter,
    /// Recessed entrance with a noren curtain over it.
    Noren,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RoofKind {
    Kawara,
    Corrugated,
    /// Flat parapet, seen on the workshops.
    Parapet,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GroundFloor {
    /// Shop: wide glazing, awning, sign board.
    Shop,
    /// House: lattice windows, plank door, step.
    House,
    /// Workshop: roller shutter, meter boxes, a steel door.
    Workshop,
    /// Open carport / drying deck under the first floor.
    OpenBay,
}

// ===========================================================================
// Small helpers
// ===========================================================================

/// How far a roof eave lip projects past the facade. Real machiya eaves are
/// deep, but the projection has to stay under a metre: from a street camera
/// anything more turns the whole upper frame into a black ceiling.
pub const EAVE_LIP: f32 = 0.62;

#[inline]
fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

#[inline]
fn scale3(a: [f32; 3], k: f32) -> [f32; 3] {
    [a[0] * k, a[1] * k, a[2] * k]
}

/// A rectangle of wall between two heights, with a proud capping rail on top.
/// Returns nothing; purely geometry, but it is the atom the facades are made of.
fn slab(b: &mut MeshBuffer, x0: f32, x1: f32, y0: f32, y1: f32, z0: f32, z1: f32, col: [f32; 3], mat: f32) {
    b.add_box(Vec3::new(x0, y0, z0), Vec3::new(x1, y1, z1), col, mat);
}

// ===========================================================================
// 1. Walls
// ===========================================================================

/// A hole punched through a wall skin, in facade coordinates: `x` across the
/// facade (x = 0 is the bay centre), `y` above the building base. Holes must be
/// passed sorted by `x0` and must not overlap.
#[derive(Clone, Copy, Debug)]
pub struct Hole {
    pub x0: f32,
    pub x1: f32,
    pub y0: f32,
    pub y1: f32,
}

impl Hole {
    pub fn new(x0: f32, x1: f32, y0: f32, y1: f32) -> Self {
        Self {
            x0: x0.min(x1),
            x1: x0.max(x1),
            y0: y0.min(y1),
            y1: y0.max(y1),
        }
    }
}

/// Everything the skin needs to stay consistent across the panels a facade is
/// cut into: one board pitch, one tone sequence, one plinth height.
struct SkinCtx<'a> {
    pal: &'a Pal,
    skin: Skin,
    y1: f32,
    plinth_top: f32,
    step: f32,
    boards: usize,
    dark_mix: f32,
    split: f32,
    z_skin: f32,
    hw: f32,
}

/// The base of a building: a stone plinth with a proud course line and a
/// chamfered cap, then a wall skin above it. No openings.
///
/// `w` is the full facade width; the skin is centred on x = 0.
pub fn wall(b: &mut MeshBuffer, w: f32, y0: f32, y1: f32, skin: Skin, pal: &Pal, rng: &mut Rng) {
    wall_open(b, w, y0, y1, skin, pal, rng, &[]);
}

/// A wall skin with openings punched out of it. The skin is emitted as
/// rectangular panels around each hole, so a window is a genuine void with a
/// reveal behind it rather than a dark rectangle painted onto a flat wall -
/// which is the single biggest read-quality difference on a facade.
pub fn wall_open(
    b: &mut MeshBuffer,
    w: f32,
    y0: f32,
    y1: f32,
    skin: Skin,
    pal: &Pal,
    rng: &mut Rng,
    holes: &[Hole],
) {
    let hw = w * 0.5;
    let plinth_h = rng.range(0.30, 0.58);
    // The board pitch is chosen once per storey, so cutting the skin into
    // panels does not change the rhythm of the boards.
    let pitch = rng.range(0.165, 0.255);
    let boards = ((w / pitch).round() as usize).max(6);
    // Boards are a `koshi` skirt around the base, not a storey-high cladding:
    // run them any higher and every window on the floor is framed by wood,
    // which is what makes the machiya read as a barn instead of a house.
    let split = if skin == Skin::BoardOverPlaster {
        (y0 + plinth_h + 0.045 + rng.range(0.42, 0.72)).min(y0 + (y1 - y0) * 0.42)
    } else {
        y1
    };
    let ctx = SkinCtx {
        pal,
        skin,
        y1,
        plinth_top: y0 + plinth_h + 0.045,
        step: w / boards as f32,
        boards,
        dark_mix: rng.range(0.0, 1.0),
        split,
        z_skin: if skin == Skin::Corrugated { -0.075 } else { -0.085 },
        hw,
    };

    // Panels: full-height columns between holes, plus the strips above and
    // below each hole.
    let mut cursor = -hw;
    for h in holes {
        if h.x0 > cursor + 0.004 {
            skin_panel(b, &ctx, cursor, h.x0, y0, y1, None);
        }
        let a = h.x0.max(cursor);
        if h.y0 > y0 + 0.01 {
            skin_panel(b, &ctx, a, h.x1, y0, h.y0, Some(h));
        }
        if h.y1 < y1 - 0.01 {
            skin_panel(b, &ctx, a, h.x1, h.y1, y1, Some(h));
        }
        cursor = h.x1;
    }
    if cursor < hw - 0.004 {
        skin_panel(b, &ctx, cursor, hw, y0, y1, None);
    }
}

/// One rectangular piece of wall skin, with its plinth share.
fn skin_panel(
    b: &mut MeshBuffer,
    c: &SkinCtx,
    x0: f32,
    x1: f32,
    ya: f32,
    yb: f32,
    hole: Option<&Hole>,
) {
    if x1 - x0 < 0.012 || yb - ya < 0.012 {
        return;
    }
    let pal = c.pal;
    let edge_left = x0 <= -c.hw + 0.002;
    let edge_right = x1 >= c.hw - 0.002;

    // --- plinth ---------------------------------------------------------
    let p0 = ya;
    let p1 = yb.min(c.plinth_top);
    if p1 > p0 + 0.008 {
        slab(b, x0 - if edge_left { 0.045 } else { 0.0 }, x1 + if edge_right { 0.045 } else { 0.0 }, p0, p1, -0.16, 0.055, pal.stone, MAT_MATTE);
        // The proud cap course sits right at the top of the plinth; only emit
        // the part of it this panel actually owns.
        if yb >= c.plinth_top - 0.002 {
            slab(
                b,
                x0 - if edge_left { 0.06 } else { 0.0 },
                x1 + if edge_right { 0.06 } else { 0.0 },
                c.plinth_top - 0.045,
                c.plinth_top,
                -0.17,
                0.072,
                scale3(pal.stone, 1.08),
                MAT_MATTE,
            );
        }
    }

    // --- skin -----------------------------------------------------------
    let s0 = ya.max(c.plinth_top);
    if yb <= s0 + 0.008 {
        return;
    }
    let (s0, s1) = (s0, yb);
    let top_edge = yb >= c.y1 - 0.002;
    match c.skin {
        Skin::Plaster => {
            slab(b, x0, x1, s0, s1, c.z_skin, 0.0, pal.plaster, MAT_MATTE);
            // Shallow pilaster strips, aligned to a global 2.4 m grid so they
            // continue across panels.
            let mut sx = (x0 / 2.4).ceil() * 2.4;
            while sx < x1 {
                if sx > x0 + 0.1 && sx < x1 - 0.1 {
                    slab(b, sx - 0.035, sx + 0.035, s0, s1, -0.02, 0.012, pal.plaster_dark, MAT_MATTE);
                }
                sx += 2.4;
            }
        }
        Skin::Board | Skin::BoardOverPlaster => {
            // Boards, clipped to this panel but indexed on the global grid so
            // the tone sequence runs unbroken across the facade.
            let board_top = c.split.min(s1);
            for k in 0..c.boards {
                let bx0 = -c.hw + c.step * k as f32;
                let bx1 = bx0 + c.step;
                let a = bx0.max(x0);
                let z = bx1.min(x1);
                if z <= a + 0.004 {
                    continue;
                }
                let tone = 0.86 + 0.28 * (((k * 37 + 11) % 13) as f32 / 13.0) - 0.10 * c.dark_mix;
                let col = lerp3(scale3(pal.board, tone), pal.board_dark, ((k * 7 + 3) % 11) as f32 / 22.0);
                slab(b, a + 0.004, z - 0.004, s0, board_top, c.z_skin, -0.004, col, MAT_WOOD);
            }
            if c.split < s1 - 0.05 {
                slab(b, x0, x1, c.split, s1, -0.09, -0.005, pal.plaster, MAT_MATTE);
                slab(b, x0, x1, c.split - 0.10, c.split + 0.02, -0.12, 0.035, pal.timber, MAT_WOOD);
            }
        }
        Skin::Corrugated => {
            slab(b, x0, x1, s0, s1, c.z_skin, 0.0, pal.steel, MAT_CORRUGATED);
            if edge_left {
                slab(b, -c.hw - 0.02, -c.hw + 0.05, s0, s1, -0.10, 0.03, scale3(pal.steel, 0.92), MAT_ALLOY);
            }
            if edge_right {
                slab(b, c.hw - 0.05, c.hw + 0.02, s0, s1, -0.10, 0.03, scale3(pal.steel, 0.92), MAT_ALLOY);
            }
            if top_edge {
                slab(b, x0, x1, s1 - 0.06, s1, -0.10, 0.045, scale3(pal.steel, 0.88), MAT_ALLOY);
            }
        }
        Skin::Stone => {
            slab(b, x0, x1, s0, s1, c.z_skin, 0.0, pal.stone, MAT_MATTE);
            let rows = (((c.y1 - c.plinth_top) / 0.34).round() as usize).max(2);
            for r in 1..rows {
                let y = c.plinth_top + (c.y1 - c.plinth_top) * (r as f32) / (rows as f32);
                if y > s0 + 0.02 && y < s1 - 0.02 {
                    slab(b, x0, x1, y - 0.012, y + 0.012, -0.02, 0.008, scale3(pal.stone, 0.82), MAT_MATTE);
                }
            }
        }
    }
    let _ = hole;
}

/// A horizontal structural band (nageshi / floor separation) across the facade.
pub fn band(b: &mut MeshBuffer, w: f32, y: f32, pal: &Pal) {
    let hw = w * 0.5;
    slab(b, -hw, hw, y - 0.055, y + 0.075, -0.115, 0.050, pal.timber, MAT_WOOD);
    // Small shadow shadow-gap under the beam.
    slab(b, -hw, hw, y - 0.075, y - 0.055, -0.10, 0.030, scale3(pal.timber, 0.6), MAT_WOOD);
}

// ===========================================================================
// 2. Windows
// ===========================================================================

/// A window, authored so the glass sits *inside* a reveal: jambs, sill and
/// head are all proud of the glass, which is what gives the window a shadow
/// and stops it reading as a decal.
pub fn window_unit(
    b: &mut MeshBuffer,
    cx: f32,
    w: f32,
    y0: f32,
    y1: f32,
    style: WindowStyle,
    pal: &Pal,
    rng: &mut Rng,
) {
    let x0 = cx - w * 0.5;
    let x1 = cx + w * 0.5;
    let frame = match style {
        WindowStyle::Sash => lerp3(pal.trim, [0.60, 0.61, 0.60], 0.5),
        WindowStyle::Lattice => pal.timber,
        WindowStyle::Casement => pal.accent,
        WindowStyle::Shopfront => pal.timber_light,
        WindowStyle::Shoji => pal.timber_light,
    };
    let mat = match style {
        WindowStyle::Sash | WindowStyle::Casement => MAT_ALLOY,
        _ => MAT_WOOD,
    };

    // Architrave: a slim casing proud of the wall face all the way round the
    // opening. It is the one element that tells the eye "this is a hole with a
    // frame in it", and it catches the raking sun as a bright outline.
    let case_col = match style {
        WindowStyle::Lattice | WindowStyle::Shoji => lerp3(pal.trim, pal.plaster, 0.35),
        WindowStyle::Shopfront => pal.timber_light,
        _ => lerp3(pal.trim, [0.66, 0.65, 0.62], 0.4),
    };
    let cw = 0.065;
    let cz0 = -0.03;
    let cz1 = 0.050;
    slab(b, x0 - cw, x1 + cw, y1, y1 + cw, cz0, cz1, case_col, MAT_WOOD);
    slab(b, x0 - cw, x0, y1, y1 + cw, cz0, cz1, case_col, MAT_WOOD);
    slab(b, x1, x1 + cw, y1, y1 + cw, cz0, cz1, case_col, MAT_WOOD);
    slab(b, x0 - cw, x0, y0, y1, cz0, cz1, case_col, MAT_WOOD);
    slab(b, x1, x1 + cw, y0, y1, cz0, cz1, case_col, MAT_WOOD);

    // Reveal: a recessed dark pocket so the glass has depth behind the skin.
    slab(b, x0, x1, y0, y1, -0.19, -0.16, scale3(pal.glass, 0.55), MAT_MATTE);
    // Glass itself, set back.
    let glass_mat = match style {
        WindowStyle::Shoji => MAT_SHOJI,
        _ => MAT_ALLOY,
    };
    let glass_col = match style {
        WindowStyle::Shoji => [0.46, 0.40, 0.30],
        // Glass in shade is not black: it holds the sky. Tinting it toward the
        // horizon colour is what stops windows reading as holes cut in cardboard.
        _ => lerp3(pal.glass, [0.175, 0.215, 0.255], 0.42),
    };
    slab(b, x0 + 0.02, x1 - 0.02, y0 + 0.02, y1 - 0.02, -0.165, -0.150, glass_col, glass_mat);

    match style {
        WindowStyle::Sash => {
            // Two sliding lights: one pane is returned slightly proud of the
            // other, which is the detail that stops a sash reading as a grid.
            let mid = cx + rng.range(-0.06, 0.06);
            let fw = 0.042;
            slab(b, x0, x1, y0, y0 + fw, -0.175, -0.108, frame, mat);
            slab(b, x0, x1, y1 - fw, y1, -0.175, -0.108, frame, mat);
            slab(b, x0, x0 + fw, y0, y1, -0.175, -0.108, frame, mat);
            slab(b, x1 - fw, x1, y0, y1, -0.175, -0.108, frame, mat);
            slab(b, mid - 0.020, mid + 0.020, y0, y1, -0.178, -0.112, frame, mat);
            slab(b, mid - 0.020, mid + 0.020, y0, y1, -0.196, -0.176, scale3(frame, 0.9), mat);
            // Sill: proud, with a drip shadow under it.
            slab(b, x0 - 0.06, x1 + 0.06, y0 - 0.05, y0, -0.055, 0.085, lerp3(frame, pal.stone, 0.45), MAT_MATTE);
            // Drip line: a thin rust/dirt streak below the sill is added in the
            // shader (see `sillStreak`), here we only give it a lip to run off.
            slab(b, x0 - 0.03, x1 + 0.03, y1, y1 + 0.055, -0.10, 0.020, scale3(frame, 0.75), mat);
            // Fly screen on one side, half the time.
            if rng.chance(0.4) {
                let sx0 = if rng.chance(0.5) { x0 + 0.05 } else { mid + 0.02 };
                let sx1 = if sx0 < mid { mid - 0.02 } else { x1 - 0.05 };
                slab(b, sx0, sx1, y0 + 0.06, y1 - 0.06, -0.140, -0.134, [0.10, 0.11, 0.12], MAT_ALLOY);
            }
        }
        WindowStyle::Lattice => {
            // Koshi lattice: vertical slats over shoji. Slat pitch varies,
            // slats are proud by different amounts (hand-made).
            let pitch = rng.range(0.070, 0.094);
            let n = ((w / pitch).round() as usize).max(4);
            let step = w / n as f32;
            for i in 0..=n {
                let x = x0 + step * i as f32;
                let proud = if i % 3 == 0 { 0.042 } else { 0.020 };
                slab(b, x - 0.010, x + 0.010, y0, y1, -0.02, proud, pal.timber, MAT_WOOD);
            }
            for &fy in &[y0 + 0.06, y1 - 0.075] {
                slab(b, x0, x1, fy, fy + 0.045, 0.02, 0.055, pal.timber, MAT_WOOD);
            }
            slab(b, x0 - 0.03, x1 + 0.03, y0 - 0.045, y0, -0.05, 0.06, pal.timber, MAT_WOOD);
        }
        WindowStyle::Casement => {
            let fw = 0.05;
            slab(b, x0, x1, y0, y0 + fw, -0.175, -0.100, frame, mat);
            slab(b, x0, x1, y1 - fw, y1, -0.175, -0.100, frame, mat);
            slab(b, x0, x0 + fw, y0, y1, -0.175, -0.100, frame, mat);
            slab(b, x1 - fw, x1, y0, y1, -0.175, -0.100, frame, mat);
            // Opened pane: a swung light at an angle, which is the whole point
            // of a casement and a strong cue at eye height.
            if rng.chance(0.55) {
                let open = rng.range(0.35, 0.9);
                let hinge = if rng.chance(0.5) { x0 + fw } else { x1 - fw };
                let dir = if hinge == x0 + fw { 1.0 } else { -1.0 };
                let px = hinge + dir * (w * 0.45) * open;
                let pz = 0.30 * open;
                b.push_quad(
                    Vec3::new(hinge, y0 + fw, -0.13),
                    Vec3::new(px, y0 + fw, -0.13 + pz),
                    Vec3::new(px, y1 - fw, -0.13 + pz),
                    Vec3::new(hinge, y1 - fw, -0.13),
                    pal.glass,
                    MAT_ALLOY,
                );
                b.push_quad(
                    Vec3::new(hinge, y0 + fw, -0.14),
                    Vec3::new(px, y0 + fw, -0.145 + pz),
                    Vec3::new(px, y1 - fw, -0.145 + pz),
                    Vec3::new(hinge, y1 - fw, -0.14),
                    scale3(frame, 0.8),
                    MAT_ALLOY,
                );
            }
            slab(b, x0 - 0.05, x1 + 0.05, y0 - 0.04, y0, -0.045, 0.07, lerp3(frame, pal.stone, 0.4), MAT_MATTE);
        }
        WindowStyle::Shopfront => {
            // Big single light, transom above, slim mullions, a kick panel at
            // the floor. This is the one the reference coffee shop uses.
            slab(b, x0, x1, y0, y0 + 0.18, -0.06, 0.02, scale3(pal.timber, 0.85), MAT_WOOD);
            let transom = y1 - (y1 - y0) * 0.28;
            slab(b, x0, x1, transom - 0.022, transom + 0.022, -0.15, -0.06, pal.timber, MAT_WOOD);
            slab(b, x0, x1, y1 - 0.045, y1, -0.16, -0.05, pal.timber, MAT_WOOD);
            slab(b, x0, x0 + 0.045, y0, y1, -0.16, -0.05, pal.timber, MAT_WOOD);
            slab(b, x1 - 0.045, x1, y0, y1, -0.16, -0.05, pal.timber, MAT_WOOD);
            let mull = ((w / 0.85).round() as usize).max(1);
            for i in 1..mull {
                let x = x0 + w * (i as f32) / (mull as f32);
                slab(b, x - 0.012, x + 0.012, y0 + 0.18, transom, -0.155, -0.075, pal.timber, MAT_WOOD);
            }
            // Warm interior glow behind the glass: a shop is a lit box.
            slab(b, x0 + 0.05, x1 - 0.05, y0 + 0.2, transom - 0.03, -0.24, -0.20, [0.30, 0.22, 0.13], MAT_MATTE);
        }
        WindowStyle::Shoji => {
            let cols = ((w / 0.46).round() as usize).max(1);
            let rows = (((y1 - y0) / 0.34).round() as usize).max(1);
            let cw = w / cols as f32;
            let rh = (y1 - y0) / rows as f32;
            for i in 0..=cols {
                let x = x0 + cw * i as f32;
                slab(b, x - 0.012, x + 0.012, y0, y1, -0.145, -0.125, pal.timber_light, MAT_WOOD);
            }
            for j in 0..=rows {
                let y = y0 + rh * j as f32;
                slab(b, x0, x1, y - 0.010, y + 0.010, -0.145, -0.125, pal.timber_light, MAT_WOOD);
            }
        }
    }
}

// ===========================================================================
// 3. Doors
// ===========================================================================

pub fn door_unit(
    b: &mut MeshBuffer,
    cx: f32,
    w: f32,
    y0: f32,
    y1: f32,
    style: DoorStyle,
    pal: &Pal,
    rng: &mut Rng,
) {
    let x0 = cx - w * 0.5;
    let x1 = cx + w * 0.5;
    // Recessed threshold + reveal.
    slab(b, x0, x1, y0 - 0.10, y0, -0.02, 0.10, scale3(pal.stone, 0.9), MAT_MATTE);
    slab(b, x0, x1, y0, y1, -0.24, -0.20, scale3(pal.board_dark, 0.8), MAT_MATTE);

    match style {
        DoorStyle::ShopGlass => {
            let fw = 0.055;
            slab(b, x0, x1, y0, y0 + fw, -0.21, -0.14, pal.trim, MAT_ALLOY);
            slab(b, x0, x1, y1 - fw, y1, -0.21, -0.14, pal.trim, MAT_ALLOY);
            slab(b, x0, x0 + fw, y0, y1, -0.21, -0.14, pal.trim, MAT_ALLOY);
            slab(b, x1 - fw, x1, y0, y1, -0.21, -0.14, pal.trim, MAT_ALLOY);
            let mid = cx + rng.range(-0.03, 0.03);
            slab(b, mid - 0.022, mid + 0.022, y0, y1, -0.212, -0.135, pal.trim, MAT_ALLOY);
            slab(b, x0 + fw, x1 - fw, y0 + fw, y1 - fw, -0.235, -0.225, pal.glass, MAT_ALLOY);
            // Kick plate, hand-height push bar.
            slab(b, x0 + fw, x1 - fw, y0 + fw, y0 + 0.26, -0.215, -0.13, scale3(pal.trim, 0.8), MAT_ALLOY);
            slab(b, x0 + 0.10, x1 - 0.10, y0 + 1.02, y0 + 1.06, -0.30, -0.24, [0.55, 0.55, 0.56], MAT_ALLOY);
        }
        DoorStyle::Plank => {
            let n = ((w / 0.17).round() as usize).max(3);
            let step = w / n as f32;
            for i in 0..n {
                let x = x0 + step * i as f32;
                let tone = 0.85 + 0.3 * (((i * 29 + 5) % 9) as f32 / 9.0);
                slab(b, x + 0.006, x + step - 0.006, y0 + 0.02, y1 - 0.02, -0.20, -0.14, scale3(pal.board, tone), MAT_WOOD);
            }
            // Iron straps and a handle.
            for &sy in &[y0 + 0.35, y1 - 0.55] {
                slab(b, x0, x1, sy, sy + 0.05, -0.13, -0.115, [0.10, 0.10, 0.11], MAT_ALLOY);
            }
            slab(b, x0 + w - 0.16, x0 + w - 0.10, y0 + 0.95, y0 + 1.02, -0.135, -0.075, [0.35, 0.30, 0.22], MAT_ALLOY);
            // Frame.
            slab(b, x0 - 0.06, x0, y0, y1 + 0.06, -0.20, -0.06, pal.timber, MAT_WOOD);
            slab(b, x1, x1 + 0.06, y0, y1 + 0.06, -0.20, -0.06, pal.timber, MAT_WOOD);
            slab(b, x0 - 0.06, x1 + 0.06, y1, y1 + 0.06, -0.20, -0.06, pal.timber, MAT_WOOD);
        }
        DoorStyle::RollerShutter => {
            let open = rng.range(0.15, 0.45);
            let top = y1 - (y1 - y0) * open;
            // Guide rails.
            slab(b, x0 - 0.03, x0 + 0.03, y0, y1, -0.16, -0.06, scale3(pal.steel, 0.9), MAT_ALLOY);
            slab(b, x1 - 0.03, x1 + 0.03, y0, y1, -0.16, -0.06, scale3(pal.steel, 0.9), MAT_ALLOY);
            // Slats.
            let slat = 0.085;
            let mut y = top;
            while y < y1 - slat {
                slab(b, x0, x1, y, y + slat * 0.82, -0.14, -0.075, lerp3(pal.steel, pal.trim, 0.35), MAT_CORRUGATED);
                y += slat;
            }
            // Roll housing + a dark interior below the shutter.
            slab(b, x0 - 0.06, x1 + 0.06, y1 - 0.02, y1 + 0.16, -0.20, 0.02, scale3(pal.steel, 0.8), MAT_ALLOY);
            slab(b, x0, x1, y0 + 0.02, top, -0.23, -0.20, [0.03, 0.03, 0.035], MAT_MATTE);
        }
        DoorStyle::Noren => {
            // Timber door behind a split noren curtain, the curtain blown a
            // little by the breeze so it is not a flat card.
            slab(b, x0, x1, y0, y1, -0.22, -0.19, pal.board_dark, MAT_WOOD);
            let n = 3usize;
            let step = w / n as f32;
            let cy1 = y1 - 0.16;
            for i in 0..n {
                let x = x0 + step * i as f32;
                let sway = rng.range(-0.02, 0.02) + (i as f32 - 1.0) * 0.012;
                b.push_quad(
                    Vec3::new(x + 0.01, y1 - 0.14, -0.06),
                    Vec3::new(x + step - 0.01, y1 - 0.14, -0.06),
                    Vec3::new(x + step - 0.02, cy1 - 0.55, -0.075 + sway),
                    Vec3::new(x + 0.02, cy1 - 0.55, -0.075 + sway),
                    lerp3(pal.accent, pal.trim, 0.5),
                    MAT_MATTE,
                );
            }
            // Curtain rail.
            b.add_cylinder(
                Vec3::new(x0 - 0.05, y1 - 0.14, -0.06),
                Vec3::new(x1 + 0.05, y1 - 0.14, -0.06),
                0.016,
                0.016,
                5,
                pal.timber,
                MAT_WOOD,
                false,
            );
        }
    }
}

// ===========================================================================
// 4. Roofs, eaves, balconies
// ===========================================================================

/// A hisashi / eave: slab, rafters underneath, gutter on the lip, and an
/// optional downpipe at one end. Every machiya in the reference has one.
pub fn eave(b: &mut MeshBuffer, w: f32, y: f32, depth: f32, pal: &Pal, rng: &mut Rng, rafters: bool) {
    let hw = w * 0.5;
    let pitch = rng.range(0.10, 0.20);
    // Slightly sloped top surface (tile or steel), 40 mm thick at the lip.
    b.push_quad(
        Vec3::new(-hw, y + pitch, -0.02),
        Vec3::new(hw, y + pitch, -0.02),
        Vec3::new(hw, y, depth),
        Vec3::new(-hw, y, depth),
        // Sun-bleached tile: a raw dark tile colour turns every intermediate
        // eave into a black gash across the facade.
        lerp3(pal.tile, [0.235, 0.240, 0.255], 0.40),
        MAT_KAWARA,
    );
    // Underside, visible from the street and always in shade. It is timber, but
    // it is *lit* timber: a near-black soffit reads as a hole in the building.
    b.push_quad(
        Vec3::new(-hw, y - 0.055, -0.02),
        Vec3::new(hw, y - 0.055, -0.02),
        Vec3::new(hw, y - 0.045, depth - 0.02),
        Vec3::new(-hw, y - 0.045, depth - 0.02),
        lerp3(pal.timber, [0.46, 0.375, 0.285], 0.58),
        MAT_WOOD,
    );
    // Fascia + gutter lip.
    slab(b, -hw, hw, y - 0.09, y + 0.02, depth - 0.06, depth + 0.02, pal.timber, MAT_WOOD);
    b.add_cylinder(
        Vec3::new(-hw, y - 0.06, depth - 0.02),
        Vec3::new(hw, y - 0.06, depth - 0.02),
        0.035,
        0.035,
        5,
        scale3(pal.steel, 1.15),
        MAT_ALLOY,
        false,
    );
    if rafters {
        // Exposed rafter tails every 40-55 cm: the little repeating shadow
        // pattern under an eave is half of why they read as Japanese.
        let pitch_x = rng.range(0.40, 0.55);
        let n = ((w / pitch_x).round() as usize).max(2);
        for i in 0..=n {
            let x = -hw + w * (i as f32) / (n as f32);
            b.add_beam(
                Vec3::new(x, y - 0.07, 0.0),
                Vec3::new(x, y - 0.045, depth - 0.04),
                0.022,
                scale3(lerp3(pal.timber, [0.42, 0.34, 0.26], 0.45), 1.0),
                MAT_WOOD,
            );
        }
    }
}

pub fn downpipe(b: &mut MeshBuffer, x: f32, y0: f32, y1: f32, pal: &Pal) {
    b.add_cylinder(
        Vec3::new(x, y0, -0.02),
        Vec3::new(x, y1, -0.02),
        0.042,
        0.042,
        6,
        scale3(pal.steel, 1.1),
        MAT_ALLOY,
        true,
    );
    // Bracket collars.
    let mut y = y0 + 0.6;
    while y < y1 - 0.2 {
        b.add_cylinder(
            Vec3::new(x, y, -0.02),
            Vec3::new(x, y + 0.045, -0.02),
            0.052,
            0.052,
            6,
            scale3(pal.steel, 0.8),
            MAT_ALLOY,
            false,
        );
        y += 1.6;
    }
    // Shoe at the bottom, kicking out over the drain.
    b.add_beam(
        Vec3::new(x, y0 + 0.10, -0.02),
        Vec3::new(x + 0.10, y0 - 0.02, 0.10),
        0.042,
        scale3(pal.steel, 1.1),
        MAT_ALLOY,
    );
}

/// Kawara roof: a proper two-pitch gable. The front slope carries the eave
/// (soffit, fascia, gutter, `marugawara` end caps) and the back slope closes the
/// volume all the way to the rear wall, which is what keeps the flat top of the
/// body box from showing as a beige terrace from any elevated camera.
pub fn roof_kawara(b: &mut MeshBuffer, w: f32, y_eave: f32, depth: f32, rise: f32, pal: &Pal) {
    let hw = w * 0.5 + 0.22;
    // The soffit is cedar lit by bounce off the street, not raw timber: an
    // honest dark brown turns into a black slab from below.
    let soffit = lerp3(pal.timber, [0.46, 0.375, 0.285], 0.55);
    let z_eave = EAVE_LIP;
    let z_ridge = -(depth * 0.5);
    let z_back = -(depth + 0.18);
    b.add_kawara_eave_slope(-hw, hw, y_eave, z_eave, y_eave + rise, z_ridge, pal.tile, soffit);
    // Back slope: same tiles, falling away from the ridge to the rear eave.
    b.push_quad(
        Vec3::new(hw, y_eave + rise, z_ridge),
        Vec3::new(hw, y_eave, z_back),
        Vec3::new(-hw, y_eave, z_back),
        Vec3::new(-hw, y_eave + rise, z_ridge),
        pal.tile,
        MAT_KAWARA,
    );
    // Rear fascia so the back slope has an edge rather than a paper-thin lip.
    slab(b, -hw, hw, y_eave - 0.10, y_eave + 0.01, z_back - 0.10, z_back, pal.timber, MAT_WOOD);
    // Ridge: stacked half-round tiles.
    slab(
        b,
        -hw - 0.05,
        hw + 0.05,
        y_eave + rise - 0.02,
        y_eave + rise + 0.13,
        z_ridge - 0.12,
        z_ridge + 0.12,
        scale3(pal.tile, 1.18),
        MAT_KAWARA,
    );
    // Gable barge boards. They sit *under* the tile plane and just inside the
    // eave line: stood proud of it they read as antennae sticking into the sky
    // from every low camera in the street.
    for sx in [-1.0_f32, 1.0] {
        let x = sx * (hw - 0.035);
        b.add_beam(
            Vec3::new(x, y_eave - 0.16, z_eave - 0.10),
            Vec3::new(x, y_eave + rise - 0.16, z_ridge),
            0.030,
            pal.timber,
            MAT_WOOD,
        );
        b.add_beam(
            Vec3::new(x, y_eave + rise - 0.16, z_ridge),
            Vec3::new(x, y_eave - 0.16, z_back + 0.10),
            0.030,
            pal.timber,
            MAT_WOOD,
        );
    }
}

pub fn roof_corrugated(b: &mut MeshBuffer, w: f32, y_eave: f32, run: f32, pal: &Pal) {
    let hw = w * 0.5 + 0.20;
    // A shallow single-pitch lean-to roof: low at the street, rising back over
    // the body. `run` is measured back from the facade.
    let z_eave = EAVE_LIP + 0.10;
    let z_ridge = -(run + 0.2);
    b.push_quad(
        Vec3::new(-hw, y_eave + 0.34, z_ridge),
        Vec3::new(hw, y_eave + 0.34, z_ridge),
        Vec3::new(hw, y_eave, z_eave),
        Vec3::new(-hw, y_eave, z_eave),
        pal.steel,
        MAT_CORRUGATED,
    );
    slab(b, -hw, hw, y_eave - 0.10, y_eave + 0.01, z_eave, z_eave + 0.10, scale3(pal.steel, 0.9), MAT_ALLOY);
    // Underside, so the lean-to is never a one-sided plane when seen from below.
    b.push_quad(
        Vec3::new(-hw, y_eave - 0.10, z_ridge),
        Vec3::new(hw, y_eave - 0.10, z_ridge),
        Vec3::new(hw, y_eave - 0.10, z_eave),
        Vec3::new(-hw, y_eave - 0.10, z_eave),
        lerp3(pal.timber, pal.plaster, 0.40),
        MAT_WOOD,
    );
}

/// Cantilevered timber balcony with a vertical-baluster railing (`te-suri`).
pub fn balcony(b: &mut MeshBuffer, w: f32, y: f32, depth: f32, pal: &Pal, rng: &mut Rng) {
    let hw = w * 0.5;
    // Deck slab.
    slab(b, -hw, hw, y - 0.10, y, -0.02, depth, pal.timber_light, MAT_WOOD);
    // Joists under the deck.
    let n = ((w / 0.55).round() as usize).max(2);
    for i in 0..=n {
        let x = -hw + w * (i as f32) / (n as f32);
        slab(b, x - 0.03, x + 0.03, y - 0.16, y - 0.10, -0.02, depth - 0.04, scale3(pal.timber, 0.85), MAT_WOOD);
    }
    // Railing: handrail, bottom rail, balusters at ~11 cm pitch, and a mid rail
    // every other bay (the reference railings are dense and dark).
    let rail_h = rng.range(0.86, 0.98);
    let ry = y + rail_h;
    slab(b, -hw, hw, ry - 0.045, ry + 0.02, depth - 0.09, depth - 0.02, pal.timber, MAT_WOOD);
    slab(b, -hw, hw, y + 0.08, y + 0.12, depth - 0.08, depth - 0.03, pal.timber, MAT_WOOD);
    // Ends.
    for sx in [-1.0_f32, 1.0] {
        let x = sx * (hw - 0.04);
        slab(b, x - 0.035, x + 0.035, y, ry, depth - 0.10, depth - 0.01, pal.timber, MAT_WOOD);
    }
    let pitch = rng.range(0.105, 0.135);
    let nb = ((w / pitch).round() as usize).max(4);
    for i in 1..nb {
        let x = -hw + w * (i as f32) / (nb as f32);
        let top = if i % 2 == 0 { ry - 0.045 } else { ry - 0.20 };
        slab(b, x - 0.012, x + 0.012, y + 0.10, top, depth - 0.075, depth - 0.045, pal.timber, MAT_WOOD);
    }
    // Steel brackets back to the wall.
    for sx in [-1.0_f32, 1.0] {
        let x = sx * (hw - 0.25);
        b.add_beam(
            Vec3::new(x, y - 0.60, 0.0),
            Vec3::new(x, y - 0.12, depth - 0.14),
            0.035,
            [0.12, 0.13, 0.14],
            MAT_ALLOY,
        );
    }
}

// ===========================================================================
// 5. Small kit parts
// ===========================================================================

/// Split-type AC condenser, bracket-mounted on the wall.
pub fn ac_unit(b: &mut MeshBuffer, cx: f32, y: f32, rng: &mut Rng) {
    let w = 0.78;
    let h = 0.54;
    let d = 0.26;
    b.add_box(Vec3::new(cx - w * 0.5, y, -0.02), Vec3::new(cx + w * 0.5, y + h, d), [0.56, 0.565, 0.55], MAT_ALLOY);
    // Fan grille: concentric rings + hub, faces -Z (the street).
    b.add_disc_neg_z(Vec3::new(cx, y + h * 0.5, d + 0.005), 0.20, 14, [0.30, 0.31, 0.31], MAT_ALLOY);
    b.add_disc_neg_z(Vec3::new(cx, y + h * 0.5, d + 0.012), 0.045, 8, [0.16, 0.16, 0.17], MAT_ALLOY);
    // Feet + wall bracket + drain hose.
    for sx in [-1.0_f32, 1.0] {
        b.add_box(
            Vec3::new(cx + sx * (w * 0.5 - 0.09), y - 0.06, 0.02),
            Vec3::new(cx + sx * (w * 0.5 - 0.05), y, d - 0.03),
            [0.30, 0.30, 0.30],
            MAT_ALLOY,
        );
    }
    let hx = cx + if rng.chance(0.5) { w * 0.5 - 0.05 } else { -(w * 0.5 - 0.05) };
    b.add_cylinder(
        Vec3::new(hx, y + 0.05, d - 0.04),
        Vec3::new(hx + 0.05, y - 0.75, d - 0.10),
        0.012,
        0.012,
        4,
        [0.42, 0.42, 0.40],
        MAT_ALLOY,
        false,
    );
}

/// Electricity meter box + a couple of grey conduits. Every older Japanese
/// facade has this cluster of plastic boxes and pipes.
pub fn meter_box(b: &mut MeshBuffer, cx: f32, y: f32, pal: &Pal, rng: &mut Rng) {
    let w = 0.36;
    let h = 0.48;
    b.add_box(Vec3::new(cx - w * 0.5, y, -0.02), Vec3::new(cx + w * 0.5, y + h, 0.11), lerp3(pal.trim, [0.62, 0.63, 0.60], 0.6), MAT_ALLOY);
    b.add_box(Vec3::new(cx - w * 0.5 + 0.02, y + h * 0.55, 0.11), Vec3::new(cx + w * 0.5 - 0.02, y + h - 0.03, 0.118), [0.22, 0.25, 0.22], MAT_ALLOY);
    let pipes = 1 + rng.pick(2);
    for i in 0..pipes {
        let px = cx + 0.24 + 0.05 * i as f32;
        b.add_cylinder(
            Vec3::new(px, y - 0.10, 0.03),
            Vec3::new(px, y + h + 0.55, 0.03),
            0.017,
            0.017,
            5,
            [0.40, 0.40, 0.38],
            MAT_ALLOY,
            false,
        );
    }
}

/// A small painted plate: shop sign, address plate, notice board.
pub fn sign_plate(b: &mut MeshBuffer, cx: f32, y: f32, w: f32, h: f32, col: [f32; 3], pal: &Pal) {
    b.add_box(
        Vec3::new(cx - w * 0.5, y, 0.0),
        Vec3::new(cx + w * 0.5, y + h, 0.035),
        col,
        MAT_MATTE,
    );
    b.add_box(
        Vec3::new(cx - w * 0.5 - 0.02, y - 0.02, 0.0),
        Vec3::new(cx + w * 0.5 + 0.02, y + h + 0.02, 0.02),
        pal.timber,
        MAT_WOOD,
    );
    // A second, inner face so the plate reads as printed rather than painted.
    b.add_box(
        Vec3::new(cx - w * 0.5 + 0.05, y + 0.05, 0.035),
        Vec3::new(cx + w * 0.5 - 0.05, y + h - 0.05, 0.04),
        lerp3(col, [0.86, 0.84, 0.78], 0.55),
        MAT_MATTE,
    );
}

/// Paper lantern under an eave (emissive material, so it glows at any hour).
pub fn lantern(b: &mut MeshBuffer, cx: f32, y: f32) {
    b.add_cylinder(
        Vec3::new(cx, y, 0.02),
        Vec3::new(cx, y - 0.03, 0.02),
        0.008,
        0.008,
        4,
        [0.10, 0.09, 0.08],
        MAT_ALLOY,
        false,
    );
    b.add_lathe_y(
        Vec3::new(cx, y - 0.30, 0.02),
        &[(0.0, 0.02), (0.09, 0.06), (0.12, 0.14), (0.09, 0.23), (0.0, 0.28)],
        10,
        [0.90, 0.82, 0.62],
        MAT_EMISSIVE_WARM,
        true,
    );
}

/// A wall vent / louvre grille.
pub fn vent(b: &mut MeshBuffer, cx: f32, y: f32, w: f32, h: f32, pal: &Pal) {
    b.add_box(Vec3::new(cx - w * 0.5, y, -0.02), Vec3::new(cx + w * 0.5, y + h, 0.035), scale3(pal.steel, 0.75), MAT_ALLOY);
    let n = 6usize;
    for i in 0..n {
        let yy = y + h * (i as f32) / n as f32 + 0.012;
        b.add_box(
            Vec3::new(cx - w * 0.45, yy, 0.035),
            Vec3::new(cx + w * 0.45, yy + 0.014, 0.055),
            [0.16, 0.17, 0.17],
            MAT_ALLOY,
        );
    }
}

/// A striped shop awning, sagging between brackets.
pub fn awning(b: &mut MeshBuffer, w: f32, y: f32, depth: f32, a: [f32; 3], b_col: [f32; 3]) {
    let hw = w * 0.5;
    let bays = ((w / 0.42).round() as usize).max(3);
    let step = w / bays as f32;
    for i in 0..bays {
        let x0 = -hw + step * i as f32;
        let col = if i % 2 == 0 { a } else { b_col };
        b.push_quad(
            Vec3::new(x0, y + 0.16, 0.0),
            Vec3::new(x0 + step, y + 0.16, 0.0),
            Vec3::new(x0 + step, y - 0.14, depth),
            Vec3::new(x0, y - 0.14, depth),
            col,
            MAT_MATTE,
        );
        // Valance hanging off the front edge.
        b.push_quad(
            Vec3::new(x0, y - 0.14, depth),
            Vec3::new(x0 + step, y - 0.14, depth),
            Vec3::new(x0 + step, y - 0.30, depth),
            Vec3::new(x0, y - 0.30, depth),
            col,
            MAT_MATTE,
        );
    }
    // Steel frame arms.
    b.add_beam(Vec3::new(-hw, y + 0.16, 0.0), Vec3::new(-hw, y - 0.12, depth), 0.022, [0.20, 0.20, 0.21], MAT_ALLOY);
    b.add_beam(Vec3::new(hw, y + 0.16, 0.0), Vec3::new(hw, y - 0.12, depth), 0.022, [0.20, 0.20, 0.21], MAT_ALLOY);
}

// ===========================================================================
// 6. Building composer
// ===========================================================================

#[derive(Clone, Copy, Debug)]
pub struct BuildingSpec {
    pub width: f32,
    pub depth: f32,
    pub floors: usize,
    pub floor_h: f32,
    pub seed: u32,
    pub pal: Pal,
    pub skin: Skin,
    pub upper_skin: Option<Skin>,
    pub ground: GroundFloor,
    pub roof: RoofKind,
    pub balcony: bool,
    /// 0..1, higher = more wear, smaller openings, more clutter.
    pub wear: f32,
}

impl Default for BuildingSpec {
    fn default() -> Self {
        Self {
            width: 7.4,
            depth: 6.6,
            floors: 2,
            floor_h: 2.72,
            seed: 1,
            pal: Pal::MACHIYA,
            skin: Skin::BoardOverPlaster,
            upper_skin: None,
            ground: GroundFloor::House,
            roof: RoofKind::Kawara,
            balcony: true,
            wear: 0.5,
        }
    }
}

/// Builds a whole facade from kit parts.
///
/// Layout strategy: the facade is divided into `bays` of unequal width (a
/// rhythm sampled from the seed), each bay gets a window or a door, the
/// remaining wall is filled by the skin, and the storey heights get eaves,
/// bands and (sometimes) a balcony. Nothing is symmetric unless the seed says
/// so, which is exactly how a real street of shops accreted.
pub fn building(b: &mut MeshBuffer, s: &BuildingSpec) {
    let mut rng = Rng::new(s.seed);
    let hw = s.width * 0.5;
    let total_h = s.floor_h * s.floors as f32;
    let gh = s.floor_h * 0.86;

    // --- body -------------------------------------------------------------
    // A thin mass so the building has a side and a top, not just a facade.
    // The front face has to sit *behind* the deepest window element (the reveal
    // pocket at z = -0.19). At -0.06 it showed through every opening and the
    // windows read as beige panels instead of dark interiors.
    b.add_box(
        Vec3::new(-hw, 0.0, -s.depth),
        Vec3::new(hw, total_h - 0.02, -0.24),
        s.pal.plaster_dark,
        MAT_MATTE,
    );

    // Where the front door lands: the ground floor units and the signage both
    // need it, and it must be stable across the whole build, so decide once.
    let door_w = if s.width > 6.4 { 1.55 } else { 1.25 };
    let door_left = rng.chance(0.55);
    let door_cx = if door_left {
        -hw + 0.55 + door_w * 0.5
    } else {
        hw - 0.55 - door_w * 0.5
    };

    // --- per storey: cut the skin, then fill the holes ---------------------
    let mut holes: [Hole; 6] = [Hole::new(0.0, 0.0, 0.0, 0.0); 6];
    for f in 0..s.floors {
        let y0 = f as f32 * s.floor_h;
        let y1 = y0 + s.floor_h;
        let skin = if f == 0 { s.skin } else { s.upper_skin.unwrap_or(s.skin) };
        let mut n_holes = 0usize;

        // ---- ground floor layout
        if f == 0 {
            match s.ground {
                GroundFloor::Shop => {
                    let shop_w = s.width * 0.56;
                    let shop_cx = if door_left { hw - 0.35 - shop_w * 0.5 } else { -hw + 0.35 + shop_w * 0.5 };
                    holes[n_holes] = Hole::new(shop_cx - shop_w * 0.5, shop_cx + shop_w * 0.5, 0.58, gh - 0.26);
                    n_holes += 1;
                    holes[n_holes] = Hole::new(door_cx - door_w * 0.5, door_cx + door_w * 0.5, 0.10, gh - 0.34);
                    n_holes += 1;
                }
                GroundFloor::House => {
                    let win_w = (s.width * 0.40).clamp(1.15, 2.5);
                    let win_cx = if door_left {
                        door_cx + door_w * 0.5 + 0.40 + win_w * 0.5
                    } else {
                        door_cx - door_w * 0.5 - 0.40 - win_w * 0.5
                    };
                    let win_cx = win_cx.max(-hw + win_w * 0.5 + 0.12).min(hw - win_w * 0.5 - 0.12);
                    holes[n_holes] = Hole::new(win_cx - win_w * 0.5, win_cx + win_w * 0.5, 0.70, gh - 0.40);
                    n_holes += 1;
                    holes[n_holes] = Hole::new(door_cx - door_w * 0.5, door_cx + door_w * 0.5, 0.10, gh - 0.14);
                    n_holes += 1;
                }
                GroundFloor::Workshop => {
                    let sh_w = s.width * 0.42;
                    let sh_cx = -hw * 0.35;
                    holes[n_holes] = Hole::new(sh_cx - sh_w * 0.5, sh_cx + sh_w * 0.5, 0.05, gh - 0.20);
                    n_holes += 1;
                    let win_w = s.width * 0.28;
                    let win_cx = hw * 0.46;
                    holes[n_holes] = Hole::new(win_cx - win_w * 0.5, win_cx + win_w * 0.5, 1.20, gh - 0.55);
                    n_holes += 1;
                }
                GroundFloor::OpenBay => {
                    holes[n_holes] = Hole::new(-hw + 0.40, hw - 0.40, 0.02, gh - 0.28);
                    n_holes += 1;
                }
            }
        } else {
            // ---- upper floor bays, of unequal width ------------------------
            let win_y0 = y0 + 0.96;
            let win_y1 = y1 - 0.52;
            let bays = if s.width > 7.0 { 3 } else { 2 };
            let mut weights = [1.0_f32; 3];
            let mut sum = 0.0;
            for i in 0..bays {
                weights[i] = rng.range(0.75, 1.35);
                sum += weights[i];
            }
            let mut x = -hw;
            for i in 0..bays {
                let bw = weights[i] / sum * s.width;
                let cx = x + bw * 0.5;
                let blank = rng.chance(0.08 + 0.14 * s.wear) && i != bays - 1;
                if !blank {
                    // Narrower than the bay by a good margin: a window that eats
                    // its whole bay turns three windows into one dark band.
                    let ww = (bw * 0.58).clamp(0.85, 1.50);
                    holes[n_holes] = Hole::new(cx - ww * 0.5, cx + ww * 0.5, win_y0, win_y1);
                    n_holes += 1;
                }
                x += bw;
            }
        }

        // ---- the skin itself
        wall_open(b, s.width, y0, y1 - 0.06, skin, &s.pal, &mut rng, &holes[..n_holes]);
        if f > 0 {
            band(b, s.width, y0 + 0.02, &s.pal);
        }

        // ---- fill the holes with real units
        if f == 0 {
            match s.ground {
                GroundFloor::Shop => {
                    let shop_w = s.width * 0.56;
                    let shop_cx = if door_left { hw - 0.35 - shop_w * 0.5 } else { -hw + 0.35 + shop_w * 0.5 };
                    window_unit(b, shop_cx, shop_w, 0.58, gh - 0.26, WindowStyle::Shopfront, &s.pal, &mut rng);
                    door_unit(b, door_cx, door_w * 0.94, 0.10, gh - 0.34, DoorStyle::ShopGlass, &s.pal, &mut rng);
                    awning(b, s.width - 0.5, gh + 0.06, 0.72, s.pal.accent, lerp3(s.pal.accent, [0.92, 0.90, 0.86], 0.75));
                    sign_plate(
                        b,
                        door_cx * 0.2,
                        gh + 0.40,
                        rng.range(1.0, 48.0_f32.min(s.width * 0.5)),
                        0.34,
                        lerp3(s.pal.accent, [0.95, 0.93, 0.88], 0.30),
                        &s.pal,
                    );
                }
                GroundFloor::House => {
                    let win_w = (s.width * 0.40).clamp(1.15, 2.5);
                    let win_cx = if door_left {
                        door_cx + door_w * 0.5 + 0.40 + win_w * 0.5
                    } else {
                        door_cx - door_w * 0.5 - 0.40 - win_w * 0.5
                    };
                    let win_cx = win_cx.max(-hw + win_w * 0.5 + 0.12).min(hw - win_w * 0.5 - 0.12);
                    window_unit(b, win_cx, win_w, 0.70, gh - 0.40, WindowStyle::Lattice, &s.pal, &mut rng);
                    door_unit(b, door_cx, door_w, 0.10, gh - 0.14, DoorStyle::Plank, &s.pal, &mut rng);
                    slab(b, door_cx - door_w * 0.6, door_cx + door_w * 0.6, 0.0, 0.13, -0.05, 0.46, scale3(s.pal.stone, 1.05), MAT_MATTE);
                    // A noren curtain in the accent colour: the cheapest way to
                    // put colour on a street that is otherwise all cedar.
                    if rng.chance(0.45) {
                        let ny = gh - 0.62;
                        slab(b, door_cx - door_w * 0.44, door_cx + door_w * 0.44, ny, ny + 0.52, 0.13, 0.17, s.pal.accent, MAT_MATTE);
                    }
                }
                GroundFloor::Workshop => {
                    let sh_w = s.width * 0.42;
                    let sh_cx = -hw * 0.35;
                    door_unit(b, sh_cx, sh_w, 0.05, gh - 0.20, DoorStyle::RollerShutter, &s.pal, &mut rng);
                    let win_w = s.width * 0.28;
                    window_unit(b, hw * 0.46, win_w, 1.20, gh - 0.55, WindowStyle::Casement, &s.pal, &mut rng);
                    meter_box(b, -hw + 0.32, 1.15, &s.pal, &mut rng);
                    vent(b, hw - 0.45, 2.20, 0.34, 0.24, &s.pal);
                }
                GroundFloor::OpenBay => {
                    // Dark interior void, a lintel, and the pier edges the skin
                    // left standing either side.
                    slab(b, -hw + 0.40, hw - 0.40, 0.02, gh - 0.28, -1.9, -1.82, [0.040, 0.040, 0.045], MAT_MATTE);
                    slab(b, -hw + 0.30, hw - 0.30, gh - 0.34, gh - 0.28, -0.12, 0.07, scale3(s.pal.timber, 0.85), MAT_WOOD);
                }
            }
        } else {
            let win_y0 = y0 + 0.96;
            let win_y1 = y1 - 0.52;
            let style = match rng.pick(3) {
                0 => WindowStyle::Sash,
                1 => WindowStyle::Shoji,
                _ => WindowStyle::Lattice,
            };
            for h in &holes[..n_holes] {
                window_unit(b, (h.x0 + h.x1) * 0.5, h.x1 - h.x0, win_y0, win_y1, style, &s.pal, &mut rng);
            }
            // Vertical timber post between bays: the frame that holds a machiya
            // together visually.
            for h in &holes[..n_holes] {
                slab(b, h.x1 + 0.02, h.x1 + 0.13, y0, y1 - 0.04, -0.12, 0.03, s.pal.timber, MAT_WOOD);
            }

            // Eave over this storey, over the ground floor and every floor above.
            eave(b, s.width + 0.10, y0 - 0.06, rng.range(0.46, 0.66), &s.pal, &mut rng, true);

            if f == 1 && s.balcony {
                balcony(b, (s.width * rng.range(0.62, 0.86)).min(s.width - 0.5), y0 + 0.03, rng.range(0.74, 0.95), &s.pal, &mut rng);
            }
        }
    }

    // --- roof -------------------------------------------------------------
    let top = total_h - 0.02;
    match s.roof {
        RoofKind::Kawara => {
            // ~24 degrees: a machiya roof is shallow, but at 13 the tile plane
            // was invisible edge-on and every house read as flat-roofed.
            let rise = (s.depth * 0.5) * rng.range(0.40, 0.50);
            roof_kawara(b, s.width, top + 0.02, s.depth, rise, &s.pal)
        }
        RoofKind::Corrugated => roof_corrugated(b, s.width, top + 0.02, s.depth, &s.pal),
        RoofKind::Parapet => {
            let pd = s.depth;
            // Bitumen deck: a bare plaster slab up there reads as an unfinished
            // box from every elevated camera, and flat roofs are where the
            // street's clutter (tanks, vents, rails) actually lives.
            slab(b, -hw + 0.02, hw - 0.02, top, top + 0.03, -pd + 0.02, -0.26, [0.052, 0.052, 0.056], MAT_MATTE);
            // Parapet: wall, then a proud concrete coping with a drip edge.
            slab(b, -hw - 0.06, hw + 0.06, top, top + 0.46, -0.18, 0.12, s.pal.plaster, MAT_MATTE);
            slab(b, -hw - 0.06, hw + 0.06, top, top + 0.40, -pd - 0.06, -pd + 0.06, s.pal.plaster, MAT_MATTE);
            slab(b, -hw + 0.06, hw - 0.06, top + 0.40, top + 0.46, -pd - 0.06, 0.12, scale3(s.pal.stone, 0.95), MAT_MATTE);
            for sx in [-1.0_f32, 1.0] {
                slab(b, sx * (hw + 0.06) - 0.06, sx * (hw + 0.06) + 0.06, top, top + 0.44, -pd + 0.06, 0.10, s.pal.plaster, MAT_MATTE);
            }
            slab(b, -hw - 0.10, hw + 0.10, top + 0.46, top + 0.52, -0.22, 0.16, scale3(s.pal.stone, 1.1), MAT_MATTE);
            // Water tank on legs + a vent stack: reads instantly as a real roof.
            let tx = rng.range(-hw + 1.2, hw - 1.2);
            let tz = -pd * 0.45;
            for lx in [-0.28_f32, 0.28] {
                for lz in [-0.28_f32, 0.28] {
                    b.add_beam(
                        Vec3::new(tx + lx, top + 0.03, tz + lz),
                        Vec3::new(tx + lx, top + 0.36, tz + lz),
                        0.022,
                        scale3(s.pal.steel, 0.8),
                        MAT_ALLOY,
                    );
                }
            }
            b.add_cylinder(
                Vec3::new(tx, top + 0.36, tz),
                Vec3::new(tx, top + 1.02, tz),
                0.42,
                0.42,
                10,
                [0.42, 0.44, 0.43],
                MAT_ALLOY,
                true,
            );
            b.add_cylinder(
                Vec3::new(hw * 0.5, top + 0.03, -pd * 0.7),
                Vec3::new(hw * 0.5, top + 0.55, -pd * 0.7),
                0.07,
                0.07,
                6,
                scale3(s.pal.steel, 0.9),
                MAT_ALLOY,
                true,
            );
            // Guard rail along the street edge: silhouette against the sky.
            for i in 0..=6 {
                let x = -hw + 0.35 + (hw * 2.0 - 0.7) * (i as f32) / 6.0;
                b.add_beam(
                    Vec3::new(x, top + 0.46, -0.10),
                    Vec3::new(x, top + 1.02, -0.10),
                    0.018,
                    [0.20, 0.21, 0.22],
                    MAT_ALLOY,
                );
            }
            b.add_beam(
                Vec3::new(-hw + 0.35, top + 1.02, -0.10),
                Vec3::new(hw - 0.35, top + 1.02, -0.10),
                0.022,
                [0.20, 0.21, 0.22],
                MAT_ALLOY,
            );
        }
    }

    // --- fixtures ---------------------------------------------------------
    let pipe_x = if rng.chance(0.5) { -hw + 0.10 } else { hw - 0.10 };
    downpipe(b, pipe_x, 0.15, top - 0.1, &s.pal);

    let ac_count = if s.floors > 1 && rng.chance(0.55) { 1 } else { 0 };
    for i in 0..ac_count {
        let f = 1 + (i % (s.floors - 1).max(1));
        let y = f as f32 * s.floor_h + rng.range(1.15, 1.55);
        let cx = rng.range(-hw + 0.9, hw - 0.9);
        ac_unit(b, cx, y, &mut rng);
    }

    if s.floors > 1 && rng.chance(0.34) {
        let n = 2 + rng.pick(3);
        for i in 0..n {
            let cx = -hw + s.width * ((i as f32) + 1.0) / ((n + 1) as f32);
            lantern(b, cx, s.floor_h - 0.22);
        }
    }

    sign_plate(b, door_cx + door_w * 0.7, 1.55, 0.30, 0.42, s.pal.trim, &s.pal);

    if rng.chance(0.6) {
        let cx = if door_left { door_cx - door_w * 0.8 } else { door_cx + door_w * 0.8 };
        let n = 1 + rng.pick(3);
        for i in 0..n {
            let h = 0.34 + 0.05 * i as f32;
            b.add_box(
                Vec3::new(cx - 0.26, 0.0, 0.10 + 0.30 * i as f32),
                Vec3::new(cx + 0.26, h, 0.62 + 0.30 * i as f32),
                lerp3(s.pal.timber_light, s.pal.trim, rng.range(0.0, 0.4)),
                MAT_WOOD,
            );
        }
    }
}

/// A shop unit that can be dropped in front of a machiya: sign, awning, glass
/// front, A-frame board and planters. This is the "hachimi's coffee" frame.
pub fn shop_unit(b: &mut MeshBuffer, w: f32, seed: u32, pal: &Pal) {
    let mut rng = Rng::new(seed);
    let hw = w * 0.5;
    window_unit(b, 0.0, w - 0.6, 0.70, 2.30, WindowStyle::Shopfront, pal, &mut rng);
    awning(b, w - 0.2, 2.52, 0.85, pal.accent, lerp3(pal.accent, [0.90, 0.89, 0.84], 0.8));
    sign_plate(b, 0.0, 2.62, (w * 0.55).min(1.9), 0.40, lerp3(pal.accent, [0.94, 0.92, 0.86], 0.35), pal);
    // A-frame pavement sign.
    let ax = hw - 0.45;
    b.push_quad(Vec3::new(ax - 0.26, 0.0, 0.62), Vec3::new(ax + 0.26, 0.0, 0.62), Vec3::new(ax + 0.26, 0.90, 0.46), Vec3::new(ax - 0.26, 0.90, 0.46), pal.accent, MAT_MATTE);
    b.push_quad(Vec3::new(ax + 0.26, 0.0, 0.62), Vec3::new(ax + 0.26, 0.0, 0.30), Vec3::new(ax + 0.26, 0.90, 0.30), Vec3::new(ax + 0.26, 0.90, 0.46), scale3(pal.trim, 0.9), MAT_WOOD);
    // Planter box with a shrub.
    let px = -hw + 0.40;
    b.add_box(Vec3::new(px - 0.22, 0.0, 0.30), Vec3::new(px + 0.22, 0.34, 0.70), [0.36, 0.28, 0.22], MAT_CERAMIC);
    b.add_ellipsoid(Vec3::new(px, 0.55, 0.50), Vec3::new(0.26, 0.22, 0.24), 8, 6, [0.10, 0.26, 0.08], MAT_FOLIAGE);
}

#[cfg(test)]
mod kit_tests {
    use super::*;
    use crate::FLOATS_PER_VERTEX;

    fn vert(b: &MeshBuffer, i: u32) -> (f32, f32, f32, f32) {
        let o = i as usize * FLOATS_PER_VERTEX;
        (b.verts[o], b.verts[o + 1], b.verts[o + 2], b.verts[o + 9])
    }

    /// Horizontal (plan-projected) area per material. Topology agnostic: it
    /// walks the index buffer, so it cannot be fooled by a mesh that mixes
    /// quads, triangles and lathes.
    fn plan_area(b: &MeshBuffer, mat: f32) -> f32 {
        let mut a = 0.0;
        for t in 0..b.indices.len() / 3 {
            let (x0, _, z0, m) = vert(b, b.indices[t * 3]);
            let (x1, _, z1, _) = vert(b, b.indices[t * 3 + 1]);
            let (x2, _, z2, _) = vert(b, b.indices[t * 3 + 2]);
            if m == mat {
                a += 0.5 * ((x1 - x0) * (z2 - z0) - (x2 - x0) * (z1 - z0)).abs();
            }
        }
        a
    }

    /// Elevation (x-y) area: what a wall contributes when you look at it.
    fn facade_area(b: &MeshBuffer, mat: f32) -> f32 {
        let mut a = 0.0;
        for t in 0..b.indices.len() / 3 {
            let (x0, y0, _, m) = vert(b, b.indices[t * 3]);
            let (x1, y1, _, _) = vert(b, b.indices[t * 3 + 1]);
            let (x2, y2, _, _) = vert(b, b.indices[t * 3 + 2]);
            if m == mat {
                a += 0.5 * ((x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0)).abs();
            }
        }
        a
    }

    fn spec(roof: RoofKind) -> BuildingSpec {
        BuildingSpec {
            width: 8.0,
            depth: 6.8,
            floors: 2,
            floor_h: 2.8,
            seed: 0x5EED_0505,
            pal: Pal::MACHIYA,
            skin: Skin::BoardOverPlaster,
            upper_skin: None,
            ground: GroundFloor::House,
            roof,
            balcony: true,
            wear: 0.5,
        }
    }

    #[test]
    fn building_has_a_body() {
        let mut b = MeshBuffer::new();
        building(&mut b, &spec(RoofKind::Kawara));
        let tris = b.indices.len() / 3;
        assert!(tris > 400, "a two-storey building should be detailed, got {tris} tris");
        assert!(tris < 9000, "kit building is too heavy: {tris} tris");
    }

    /// The roof has to close the volume: if it does not cover the footprint, the
    /// flat top of the body box shows through from any elevated camera (which
    /// is exactly what a player sees from the 2F window).
    #[test]
    fn roof_covers_the_footprint() {
        let s = spec(RoofKind::Kawara);
        let mut b = MeshBuffer::new();
        building(&mut b, &s);
        let tiles = plan_area(&b, MAT_KAWARA);
        let footprint = s.width * s.depth;
        assert!(
            tiles > footprint * 0.95,
            "kawara plan area {tiles:.1} does not cover footprint {footprint:.1}"
        );
    }

    /// Windows are holes, not decals: opening one must remove wall area.
    #[test]
    fn a_window_removes_wall() {
        let mut rng = Rng::new(7);
        let mut solid = MeshBuffer::new();
        wall(&mut solid, 8.0, 0.0, 2.7, Skin::Plaster, &Pal::MACHIYA, &mut rng);
        let mut holed = MeshBuffer::new();
        let mut rng = Rng::new(7);
        wall_open(
            &mut holed,
            8.0,
            0.0,
            2.7,
            Skin::Plaster,
            &Pal::MACHIYA,
            &mut rng,
            &[Hole::new(-0.9, 0.9, 0.9, 2.3)],
        );
        let a = facade_area(&solid, MAT_MATTE);
        let b2 = facade_area(&holed, MAT_MATTE);
        assert!(b2 < a - 1.0, "the window hole did not cut the wall ({a:.1} vs {b2:.1})");
    }
}
