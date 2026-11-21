//! Modular Japanese Coastal Town Streetscape & Distant Mountain Launch Complex (`town.rs`).
//!
//! Built from reusable parametric specs (`JapaneseHouseSpec`, `UtilityPoleSpec`) so every building,
//! Kawara roof, timber frame, koushi lattice window, utility pole, curb, and street prop can be
//! instantiated anywhere in the town without duplicating geometry code.

use crate::{
    props, DynamicSceneParams, MeshBuffer, Pose, MAT_ALLOY, MAT_CERAMIC, MAT_CORRUGATED,
    MAT_FOLIAGE, MAT_KAWARA, MAT_MATTE, MAT_PLUME, MAT_WOOD,
};
use odderon_dmath::Vec3;

/// Parameterized specification for a modular 2-story traditional-modern Japanese house (`Machiya / Showa`).
#[derive(Clone, Copy, Debug)]
pub struct JapaneseHouseSpec {
    pub width: f32,
    pub depth: f32,
    pub wall_col: [f32; 3],
    pub wood_col: [f32; 3],
    pub tile_col: [f32; 3],
    /// `true`: wide 2nd-floor wooden `koushi` vertical lattice grille (`image-8` right house).
    /// `false`: recessed 2nd-floor glass casement window with timber sun-hood (`image-8` left house).
    pub koushi_upper_window: bool,
    /// Adds a warm cedar-plank 3rd-level attic monitor (`image-8` & `image-10` upper right).
    pub cedar_attic_riser: bool,
    /// Mounts a modular outdoor Split-AC condenser on the 1st-floor tiled eave (`image-8, 9, 10`).
    pub eave_ac_unit: bool,
    /// Adds rooftop Yagi TV aerial + mini satellite dish (`image-8, 11`).
    pub rooftop_antenna: bool,
    /// Adds weathered corrugated metal side cladding (`image-10, 11`).
    pub corrugated_side_wall: bool,
}

/// Builds a complete 2-story Japanese townhouse in local space:
/// `y = 0` is sidewalk ground level, `-Z` (`z = 0`) is the street-facing front facade!
pub fn build_japanese_house(buf: &mut MeshBuffer, pose: Pose, s: &JapaneseHouseSpec) {
    buf.with_pose(pose, |b| {
        let hw = s.width * 0.5;
        let d = s.depth;
        let h1 = 2.85_f32; // 1st story ceiling (`y = 0 .. 2.85`)
        let h2 = 5.65_f32; // 2nd story eave (`y = 2.85 .. 5.65`, right across from our 2nd-floor Hab!)
        let dark_glass = [0.13, 0.19, 0.25];
        let gutter_col = [0.35, 0.32, 0.30];

        // 1. Main 2-story plaster/stucco core (`z in [0.0, d]`)
        b.add_box(Vec3::new(-hw, 0.0, 0.0), Vec3::new(hw, h2, d), s.wall_col, MAT_MATTE);

        // Optional weathered corrugated iron side cladding (`image-10, 11`)
        if s.corrugated_side_wall {
            b.add_box(
                Vec3::new(-hw - 0.03, 0.0, 0.15),
                Vec3::new(-hw + 0.02, h1 + 0.4, d - 0.15),
                [0.54, 0.44, 0.35],
                MAT_CORRUGATED,
            );
        }

        // 2. Exposed Japanese Post-and-Beam Timber Framing (`hashira` columns & `nuki` tie-beams) on street facade!
        for &px in &[-hw + 0.08, 0.0, hw - 0.08] {
            b.add_box(Vec3::new(px - 0.07, 0.0, -0.045), Vec3::new(px + 0.07, h2, 0.02), s.wood_col, MAT_WOOD);
        }
        for &py in &[h1 - 0.08, h1 + 0.32, h2 - 0.12] {
            b.add_box(Vec3::new(-hw, py, -0.04), Vec3::new(hw, py + 0.11, 0.02), s.wood_col, MAT_WOOD);
        }

        // 3. First-Floor Streetfront: Sliding Wooden Lattice Doors (`hikido`) + Shop/Genkan Window!
        // Left bay: wooden lattice sliding entry doors
        let door_x0 = -hw + 0.32;
        let door_x1 = -0.18_f32;
        b.add_u_frame_xy(door_x0, door_x1, 0.02, 2.18, -0.06, 0.02, 0.07, s.wood_col, MAT_WOOD);
        b.add_box(Vec3::new(door_x0, 0.02, -0.02), Vec3::new(door_x1, 0.72, 0.01), s.wood_col, MAT_WOOD);
        b.add_box(Vec3::new(door_x0, 0.72, -0.015), Vec3::new(door_x1, 2.18, 0.01), [0.84, 0.79, 0.68], MAT_MATTE);
        let slats_1f = 7;
        for i in 0..=slats_1f {
            let sx = door_x0 + (door_x1 - door_x0) * (i as f32) / (slats_1f as f32);
            b.add_box(Vec3::new(sx - 0.015, 0.02, -0.035), Vec3::new(sx + 0.015, 2.18, -0.01), s.wood_col, MAT_WOOD);
        }

        // Right bay: recessed dark glass shopfront / ground window with low timber balustrade (`image-8, 9`)
        let win1_x0 = 0.22_f32;
        let win1_x1 = hw - 0.34;
        b.add_u_frame_xy(win1_x0, win1_x1, 0.48, 2.15, -0.06, 0.02, 0.065, s.wood_col, MAT_WOOD);
        b.add_box(Vec3::new(win1_x0, 0.48, -0.015), Vec3::new(win1_x1, 2.15, 0.01), [0.24, 0.36, 0.46], MAT_ALLOY);

        // 4. First-Floor Overhanging Kawara Tiled Eave (`hisashi`, projecting `-1.05m` over sidewalk!)
        b.add_kawara_eave_slope(
            -hw - 0.22,
            hw + 0.22,
            h1 - 0.32,
            -1.05,
            h1 + 0.28,
            0.05,
            s.tile_col,
            s.wood_col,
        );
        // Vertical PVC rainwater downspout (`amadoi`) down left timber post (`image-8, 9, 10`)!
        b.add_cylinder(
            Vec3::new(-hw + 0.14, 0.0, -0.10),
            Vec3::new(-hw + 0.14, h2 - 0.15, -0.10),
            0.032,
            0.032,
            7,
            gutter_col,
            MAT_ALLOY,
            false,
        );
        b.add_cylinder(
            Vec3::new(-hw + 0.14, h1 - 0.42, -1.02),
            Vec3::new(-hw + 0.14, h1 - 0.56, -0.10),
            0.030,
            0.030,
            7,
            gutter_col,
            MAT_ALLOY,
            false,
        );

        // Optional Outdoor Split-AC Condenser perched on 1st-floor tiled eave (`image-8, 9, 10`!)
        if s.eave_ac_unit {
            props::outdoor_ac_condenser(b, Pose::at(-0.35, h1 + 0.02, -0.34));
        }

        // 5. Second-Story Windows (RIGHT AT EYE LEVEL WITH APARTMENT #204!)
        if s.koushi_upper_window {
            // Wide traditional vertical cedar `koushi` lattice grille (`image-8, 9, 10` right house!)
            let kx0 = -hw + 0.48;
            let kx1 = hw - 0.48;
            let (ky0, ky1) = (h1 + 0.62, h2 - 0.38);
            b.add_box(Vec3::new(kx0, ky0, -0.02), Vec3::new(kx1, ky1, 0.01), dark_glass, MAT_ALLOY);
            b.add_rounded_box(Vec3::new(kx0 - 0.08, ky0 - 0.09, -0.14), Vec3::new(kx1 + 0.08, ky0, 0.02), 0.015, s.wood_col, MAT_WOOD);
            b.add_box(Vec3::new(kx0 - 0.08, ky1, -0.12), Vec3::new(kx1 + 0.08, ky1 + 0.07, 0.02), s.wood_col, MAT_WOOD);
            let k_slats = ((kx1 - kx0) / 0.17) as usize;
            for i in 0..=k_slats {
                let sx = kx0 + (kx1 - kx0) * (i as f32) / (k_slats.max(1) as f32);
                b.add_box(Vec3::new(sx - 0.018, ky0, -0.11), Vec3::new(sx + 0.018, ky1, -0.03), s.wood_col, MAT_WOOD);
            }
        } else {
            // Twin 2nd-story timber-framed casement windows with overhanging wooden sun-hoods (`image-8, 12`!)
            for &(wx0, wx1) in &[(-hw + 0.42, -0.28_f32), (0.28, hw - 0.42)] {
                let (wy0, wy1) = (h1 + 0.68, h2 - 0.48);
                b.add_u_frame_xy(wx0, wx1, wy0, wy1, -0.07, 0.02, 0.06, s.wood_col, MAT_WOOD);
                b.add_box(Vec3::new(wx0, wy0, -0.02), Vec3::new(wx1, wy1, 0.01), [0.18, 0.28, 0.38], MAT_ALLOY);
                // Center mullion + projecting timber sun-hood
                let wm = 0.5 * (wx0 + wx1);
                b.add_box(Vec3::new(wm - 0.02, wy0, -0.04), Vec3::new(wm + 0.02, wy1, 0.01), s.wood_col, MAT_WOOD);
                b.add_box(Vec3::new(wx0 - 0.10, wy1 + 0.04, -0.32), Vec3::new(wx1 + 0.10, wy1 + 0.11, 0.02), s.wood_col, MAT_WOOD);
            }
        }

        // 6. Main Second-Story Overhanging Kawara Tiled Roof + Ridge Beam (`mune`)!
        let ridge_y = h2 + 1.48;
        let ridge_z = d * 0.48;
        b.add_kawara_eave_slope(
            -hw - 0.42,
            hw + 0.42,
            h2 - 0.12,
            -0.88,
            ridge_y,
            ridge_z,
            s.tile_col,
            s.wood_col,
        );
        // Rear roof slope + triangular side gable walls + heavy clay `mune` ridge cap!
        b.push_quad(
            Vec3::new(-hw - 0.42, h2 - 0.12, d + 0.65),
            Vec3::new(hw + 0.42, h2 - 0.12, d + 0.65),
            Vec3::new(hw + 0.42, ridge_y, ridge_z),
            Vec3::new(-hw - 0.42, ridge_y, ridge_z),
            s.tile_col,
            MAT_KAWARA,
        );
        b.add_box(Vec3::new(-hw + 0.05, h2, 0.15), Vec3::new(hw - 0.05, ridge_y - 0.15, d - 0.15), s.wall_col, MAT_MATTE);
        b.add_cylinder(
            Vec3::new(-hw - 0.46, ridge_y + 0.05, ridge_z),
            Vec3::new(hw + 0.46, ridge_y + 0.05, ridge_z),
            0.11,
            0.11,
            8,
            [0.20, 0.22, 0.25],
            MAT_KAWARA,
            true,
        );

        // Optional warm cedar-plank attic monitor (`image-8` & `image-10` top-right wooden 3rd tier!)
        if s.cedar_attic_riser {
            let cedar_red = [0.54, 0.27, 0.15];
            b.add_box(Vec3::new(-hw + 0.45, h2 + 0.55, 0.85), Vec3::new(hw - 0.25, ridge_y + 1.05, d - 0.65), cedar_red, MAT_WOOD);
            b.add_kawara_eave_slope(
                -hw + 0.20,
                hw - 0.05,
                ridge_y + 0.95,
                0.45,
                ridge_y + 1.48,
                2.25,
                s.tile_col,
                s.wood_col,
            );
        }

        // Optional Rooftop Yagi TV Aerial + Satellite Dish (`image-8, 11`!)
        if s.rooftop_antenna {
            let ax = hw * 0.35;
            b.add_cylinder(Vec3::new(ax, ridge_y, ridge_z), Vec3::new(ax, ridge_y + 1.75, ridge_z), 0.022, 0.015, 6, [0.72, 0.74, 0.76], MAT_ALLOY, false);
            b.add_cylinder(Vec3::new(ax, ridge_y + 1.55, ridge_z - 0.55), Vec3::new(ax, ridge_y + 1.55, ridge_z + 0.55), 0.014, 0.014, 6, [0.80, 0.42, 0.22], MAT_ALLOY, false);
            for &dz in &[-0.42_f32, -0.18, 0.08, 0.34] {
                b.add_cylinder(Vec3::new(ax - 0.32, ridge_y + 1.55, ridge_z + dz), Vec3::new(ax + 0.32, ridge_y + 1.55, ridge_z + dz), 0.009, 0.009, 5, [0.76, 0.78, 0.80], MAT_ALLOY, false);
            }
            // Small parabolic dish
            b.add_disc_neg_z(Vec3::new(ax + 0.85, ridge_y + 0.55, ridge_z - 0.35), 0.34, 14, [0.28, 0.30, 0.33], MAT_ALLOY);
        }
    });
}

/// Reusable Japanese Concrete Utility Pole (`image-11` & `image-12`!).
/// Features tapered concrete shaft, galvanized collar bands, cylindrical pole transformers,
/// teal vertical mast, orange top cap, and crossarm!
pub fn build_japanese_utility_pole(buf: &mut MeshBuffer, pose: Pose, has_transformers: bool) {
    buf.with_pose(pose, |b| {
        let concrete = [0.74, 0.72, 0.68];
        let steel_band = [0.42, 0.45, 0.48];
        let teal_mast = [0.16, 0.50, 0.58];
        let orange_cap = [0.84, 0.38, 0.16];

        // Tapered concrete pole (`height = 7.15 m`, rises well above our 2nd-floor window!)
        b.add_lathe_y(Vec3::ZERO, &[(0.0, 0.165), (7.10, 0.105)], 14, concrete, MAT_MATTE, true);

        // Galvanized steel collar bands along shaft (`image-11, 12`)
        for &by in &[1.65_f32, 3.10, 4.35, 5.55, 6.55] {
            let r = 0.165 - (0.060 * by / 7.10) + 0.006;
            b.add_lathe_y(Vec3::new(0.0, by, 0.0), &[(0.0, r), (0.045, r)], 14, steel_band, MAT_ALLOY, false);
        }

        // Yellow/black municipal pole number plate at street height + notice band
        b.add_box(Vec3::new(-0.14, 1.80, -0.155), Vec3::new(0.14, 2.15, -0.135), [0.88, 0.84, 0.68], MAT_MATTE);

        // Twin cylindrical grey/red-striped pole transformers at 2nd-floor eye height (`image-11, 12`!)
        if has_transformers {
            for &(tx, tz, r) in &[(-0.26_f32, -0.10_f32, 0.165_f32), (0.24, 0.08, 0.145)] {
                b.add_lathe_y(Vec3::new(tx, 4.55, tz), &[(0.0, r), (0.58, r)], 14, [0.62, 0.63, 0.61], MAT_ALLOY, true);
                b.add_lathe_y(Vec3::new(tx, 4.96, tz), &[(0.0, r + 0.004), (0.06, r + 0.004)], 14, [0.78, 0.24, 0.18], MAT_ALLOY, false);
                b.add_cylinder(Vec3::new(0.0, 4.82, 0.0), Vec3::new(tx, 4.82, tz), 0.025, 0.025, 6, steel_band, MAT_ALLOY, false);
            }
        }

        // Teal side riser mast + Orange top cap block + Insulator crossarm (`image-11, 12`!)
        b.add_cylinder(Vec3::new(-0.13, 5.45, 0.0), Vec3::new(-0.13, 7.25, 0.0), 0.032, 0.030, 8, teal_mast, MAT_ALLOY, true);
        b.add_rounded_box(Vec3::new(-0.18, 7.05, -0.18), Vec3::new(0.18, 7.30, 0.18), 0.022, orange_cap, MAT_ALLOY);
        // Horizontal cross-arm jutting toward street with ceramic pin insulators
        b.add_cylinder(Vec3::new(0.0, 6.88, -0.68), Vec3::new(0.0, 6.88, 0.68), 0.026, 0.026, 8, teal_mast, MAT_ALLOY, true);
        for &iz in &[-0.55_f32, -0.18, 0.18, 0.55] {
            b.add_lathe_y(Vec3::new(0.0, 6.90, iz), &[(0.0, 0.032), (0.075, 0.022)], 8, [0.94, 0.95, 0.95], MAT_CERAMIC, true);
        }
    });
}

/// Delicate Japanese Street Sapling / Bamboo-Maple Accent Tree (smooth ellipsoid foliage, never blocky!).
pub fn build_street_sapling(buf: &mut MeshBuffer, pose: Pose, blossom_tint: bool) {
    buf.with_pose(pose, |b| {
        let bark = [0.32, 0.23, 0.16];
        let leaf_a = if blossom_tint { [0.88, 0.54, 0.62] } else { [0.24, 0.52, 0.22] };
        let leaf_b = if blossom_tint { [0.94, 0.66, 0.72] } else { [0.30, 0.58, 0.25] };

        // Curb tree-pit surround
        b.add_rounded_box(Vec3::new(-0.42, 0.0, -0.42), Vec3::new(0.42, 0.08, 0.42), 0.015, [0.46, 0.34, 0.24], MAT_WOOD);
        b.add_cylinder(Vec3::ZERO, Vec3::new(0.08, 2.4, -0.05), 0.065, 0.035, 8, bark, MAT_WOOD, false);
        b.add_cylinder(Vec3::new(0.05, 1.8, -0.03), Vec3::new(0.48, 2.85, -0.18), 0.032, 0.016, 7, bark, MAT_WOOD, false);
        b.add_cylinder(Vec3::new(0.06, 2.1, -0.04), Vec3::new(-0.38, 3.15, 0.15), 0.028, 0.015, 7, bark, MAT_WOOD, false);

        for &(cx, cy, cz, rx, ry, rz, col) in &[
            (0.08_f32, 3.15_f32, -0.05_f32, 0.55_f32, 0.42_f32, 0.48_f32, leaf_b),
            (0.48, 2.95, -0.18, 0.44, 0.34, 0.40, leaf_a),
            (-0.36, 3.22, 0.14, 0.46, 0.36, 0.42, leaf_b),
            (0.16, 3.65, 0.04, 0.38, 0.32, 0.36, leaf_a),
        ] {
            b.add_ellipsoid(Vec3::new(cx, cy, cz), Vec3::new(rx, ry, rz), 12, 8, col, MAT_FOLIAGE);
        }
    });
}

/// Assembles the entire sunlit Japanese neighborhood street + distant mountain Launch Pad-1!
/// Note: Room #204 is on the 2nd floor (`y = 0.0`), so street ground level is `y = -3.20 m`.
pub fn build_uchinoura_street_static(buf: &mut MeshBuffer) {
    let ground_y = -3.20_f32;

    // 1. Warm Sunlit Neighborhood Ground, Sidewalks, Concrete Curbs & Narrow Asphalt Street!
    // Base town ground plane extending to mountain foothills (NO river/water strip!)
    buf.add_box(Vec3::new(-160.0, ground_y - 0.35, 2.2), Vec3::new(160.0, ground_y, 140.0), [0.30, 0.44, 0.24], MAT_FOLIAGE);

    // Near sidewalk under our apartment (`z in [2.5, 4.8]`) + Far sidewalk in front of 2-story houses (`z in [9.4, 12.2]`)
    let sidewalk_warm = [0.80, 0.77, 0.71];
    let curb_stone = [0.72, 0.68, 0.62];
    buf.add_box(Vec3::new(-42.0, ground_y, 2.5), Vec3::new(42.0, ground_y + 0.14, 4.8), sidewalk_warm, MAT_MATTE);
    buf.add_box(Vec3::new(-42.0, ground_y, 9.4), Vec3::new(42.0, ground_y + 0.14, 12.4), sidewalk_warm, MAT_MATTE);
    // Segmented concrete curbstones (`image-8, 9, 10`)
    buf.add_rounded_box(Vec3::new(-42.0, ground_y, 9.22), Vec3::new(42.0, ground_y + 0.16, 9.44), 0.02, curb_stone, MAT_MATTE);
    buf.add_rounded_box(Vec3::new(-42.0, ground_y, 4.78), Vec3::new(42.0, ground_y + 0.16, 5.00), 0.02, curb_stone, MAT_MATTE);

    // Sunlit Japanese Residential Asphalt Road (`z in [4.95, 9.25]`) + Crisp White Side Shoulder Lines (`image-8, 11, 12`)!
    buf.add_box(Vec3::new(-45.0, ground_y - 0.02, 4.95), Vec3::new(45.0, ground_y + 0.04, 9.25), [0.22, 0.23, 0.25], MAT_MATTE);
    buf.add_box(Vec3::new(-42.0, ground_y + 0.052, 5.45), Vec3::new(42.0, ground_y + 0.062, 5.55), [0.88, 0.87, 0.84], MAT_MATTE);
    buf.add_box(Vec3::new(-42.0, ground_y + 0.052, 8.68), Vec3::new(42.0, ground_y + 0.062, 8.78), [0.88, 0.87, 0.84], MAT_MATTE);

    // 2. Modular 2-Story Japanese Townhouses Right Across the Street (`z = 11.45 m`)!
    //    - House Center-Right (`x = -2.25`, directly visible in front of our window!): 2-story Koushi lattice + eave AC unit + cedar attic!
    //    - House Screen-Left  (`x = +6.95`, `image-8` left house): 2-story casement sun-hoods + rooftop Yagi TV antenna & dish!
    //    - Between `x = +1.55` and `x = +3.35`: Scenic Japanese `roji` alley / garden gap revealing distant green mountains & far-off Pad-1!
    //    - House Far-Right    (`x = -10.8`): 2-story Showa townhouse continuing down the street!
    let cedar_frame = [0.36, 0.20, 0.12];
    let kawara_charcoal = [0.22, 0.23, 0.25];
    let kawara_warm_grey = [0.28, 0.27, 0.26];

    // House B: Center-Right directly across from our window (`x in [-6.05, +1.55]`)
    build_japanese_house(
        buf,
        Pose::at(-2.25, ground_y + 0.14, 11.45),
        &JapaneseHouseSpec {
            width: 7.6,
            depth: 6.8,
            wall_col: [0.89, 0.84, 0.76],
            wood_col: cedar_frame,
            tile_col: kawara_charcoal,
            koushi_upper_window: true,
            cedar_attic_riser: true,
            eave_ac_unit: true,
            rooftop_antenna: false,
            corrugated_side_wall: true,
        },
    );

    // House A: Screen-Left across the narrow `roji` alley (`x in [+3.15, +10.55]`)
    build_japanese_house(
        buf,
        Pose::at(6.85, ground_y + 0.14, 11.65),
        &JapaneseHouseSpec {
            width: 7.4,
            depth: 6.6,
            wall_col: [0.86, 0.80, 0.70],
            wood_col: [0.33, 0.18, 0.11],
            tile_col: kawara_warm_grey,
            koushi_upper_window: false,
            cedar_attic_riser: false,
            eave_ac_unit: false,
            rooftop_antenna: true,
            corrugated_side_wall: false,
        },
    );

    // House C: Continuing down street on Screen-Right (`x in [-14.2, -7.2]`)
    build_japanese_house(
        buf,
        Pose::at(-10.7, ground_y + 0.14, 11.75),
        &JapaneseHouseSpec {
            width: 7.0,
            depth: 6.5,
            wall_col: [0.84, 0.81, 0.74],
            wood_col: cedar_frame,
            tile_col: kawara_charcoal,
            koushi_upper_window: false,
            cedar_attic_riser: false,
            eave_ac_unit: true,
            rooftop_antenna: true,
            corrugated_side_wall: true,
        },
    );

    // Small setback backyard shed/annex in the alley depth (`image-10`!)
    buf.add_box(Vec3::new(1.35, ground_y + 0.14, 15.8), Vec3::new(3.45, ground_y + 3.25, 19.5), [0.83, 0.78, 0.68], MAT_MATTE);
    buf.add_kawara_eave_slope(1.15, 3.65, ground_y + 3.15, 15.1, ground_y + 3.85, 17.5, kawara_warm_grey, cedar_frame);

    // 3. Lived-In Japanese Street Sidewalk Props (`image-8, 9, 10, 11, 12`!):
    //    Parked blue bicycle, wooden A-frame cafe chalkboard sign, cluster of terracotta planters, & street saplings!
    props::parked_bicycle(buf, Pose::at(0.65, ground_y + 0.14, 10.35));
    props::terracotta_planter(buf, Pose::at(-0.55, ground_y + 0.14, 10.25));
    props::terracotta_planter(buf, Pose::at(-3.95, ground_y + 0.14, 10.30).with_scale(0.85));
    props::terracotta_planter(buf, Pose::at(4.35, ground_y + 0.14, 10.45).with_scale(1.05));

    // Wooden A-Frame sidewalk menu board (`image-8, 9, 10`)
    buf.add_beam(Vec3::new(-1.35, ground_y + 0.14, 10.05), Vec3::new(-1.35, ground_y + 0.92, 10.25), 0.025, [0.58, 0.36, 0.18], MAT_WOOD);
    buf.add_beam(Vec3::new(-1.35, ground_y + 0.14, 10.45), Vec3::new(-1.35, ground_y + 0.92, 10.25), 0.025, [0.58, 0.36, 0.18], MAT_WOOD);
    buf.add_box(Vec3::new(-1.60, ground_y + 0.30, 10.12), Vec3::new(-1.10, ground_y + 0.86, 10.20), [0.14, 0.16, 0.17], MAT_MATTE);

    // Delicate street saplings down the sidewalk (`image-12`, tucked neatly so they frame rather than block!)
    build_street_sapling(buf, Pose::at(2.35, ground_y + 0.14, 9.85), true);
    build_street_sapling(buf, Pose::at(-6.45, ground_y + 0.14, 9.85), false);

    // 4. FOREGROUND UTILITY POLE RIGHT OUTSIDE THE LEFT OF OUR WINDOW (`x = +0.92, z = 5.25` — `image-11, 12`!)
    //    Plus secondary poles down the street so multi-tier black catenary cables sweep right across the window!
    let pole_left_fg = Vec3::new(0.92, ground_y + 0.14, 5.25);
    let pole_right = Vec3::new(-7.60, ground_y + 0.14, 5.65);
    let pole_far_left = Vec3::new(9.80, ground_y + 0.14, 5.35);

    build_japanese_utility_pole(buf, Pose::at(pole_left_fg.x, pole_left_fg.y, pole_left_fg.z), true);
    build_japanese_utility_pole(buf, Pose::at(pole_right.x, pole_right.y, pole_right.z), false);
    build_japanese_utility_pole(buf, Pose::at(pole_far_left.x, pole_far_left.y, pole_far_left.z), false);

    let wire_blk = [0.11, 0.12, 0.14];
    // Upper HV 3-wire catenary bundle sweeping from our left-window pole across the window to the right!
    for &dz in &[-0.42_f32, 0.0, 0.42] {
        buf.add_catenary(
            Vec3::new(pole_left_fg.x, pole_left_fg.y + 6.94, pole_left_fg.z + dz),
            Vec3::new(pole_right.x, pole_right.y + 6.94, pole_right.z + dz),
            0.34,
            14,
            0.010,
            wire_blk,
        );
        buf.add_catenary(
            Vec3::new(pole_far_left.x, pole_far_left.y + 6.94, pole_far_left.z + dz),
            Vec3::new(pole_left_fg.x, pole_left_fg.y + 6.94, pole_left_fg.z + dz),
            0.34,
            12,
            0.010,
            wire_blk,
        );
    }
    // Lower telecom/distribution bundle (`y + 5.55`, classic layered Japanese cable silhouette from `image-11, 12`!)
    buf.add_catenary(
        Vec3::new(pole_left_fg.x, pole_left_fg.y + 5.58, pole_left_fg.z - 0.12),
        Vec3::new(pole_right.x, pole_right.y + 5.58, pole_right.z - 0.12),
        0.46,
        14,
        0.013,
        wire_blk,
    );
    // Service drop wires crossing the street from our foreground pole to the 2-story houses (`image-11`!)
    buf.add_catenary(
        Vec3::new(pole_left_fg.x, pole_left_fg.y + 5.60, pole_left_fg.z + 0.10),
        Vec3::new(-1.85, ground_y + 5.20, 11.40),
        0.28,
        10,
        0.008,
        wire_blk,
    );
    buf.add_catenary(
        Vec3::new(pole_left_fg.x, pole_left_fg.y + 5.60, pole_left_fg.z + 0.15),
        Vec3::new(4.85, ground_y + 5.25, 11.60),
        0.28,
        10,
        0.008,
        wire_blk,
    );

    // 5. LAYERED JAPANESE MOUNTAIN RIDGES RISING BEHIND THE 2-STORY ROOFS (`image-12`!)
    //    + GENUINELY FAR-DISTANT LAUNCH PAD-1 (`z = 360 m`, visible through the rooftop valley!)
    // Mid-distance forested hill (`z = 125 m`)
    buf.push_quad(
        Vec3::new(-140.0, 15.5, 138.0),
        Vec3::new(-8.0, 9.5, 132.0),
        Vec3::new(12.0, ground_y, 98.0),
        Vec3::new(-140.0, ground_y, 98.0),
        [0.19, 0.40, 0.22],
        MAT_FOLIAGE,
    );
    buf.push_quad(
        Vec3::new(16.0, 11.0, 142.0),
        Vec3::new(150.0, 18.5, 152.0),
        Vec3::new(150.0, ground_y, 102.0),
        Vec3::new(2.0, ground_y, 102.0),
        [0.21, 0.42, 0.24],
        MAT_FOLIAGE,
    );

    // Dramatic peaked Japanese mountain range in the far distance (`z = 360..420 m`, like `image-12`!)
    buf.push_quad(
        Vec3::new(-220.0, 42.0, 415.0),
        Vec3::new(220.0, 36.0, 425.0),
        Vec3::new(240.0, ground_y, 310.0),
        Vec3::new(-240.0, ground_y, 300.0),
        [0.24, 0.41, 0.34],
        MAT_FOLIAGE,
    );

    // Distant Uchinoura Range Pad-1 Saddle (`x = +2.2, y = 14.5, z = 360.0` — framed cleanly in the gap between House A & House B roofs!)
    // At `z = 360 m`, a 15 m tower subtends only ~2.4 degrees — genuinely miles away outside town!
    let pad = Vec3::new(2.2, 14.5, 360.0);
    buf.add_box(
        Vec3::new(pad.x - 9.0, pad.y - 1.2, pad.z - 6.0),
        Vec3::new(pad.x + 9.0, pad.y, pad.z + 6.0),
        [0.74, 0.75, 0.73],
        MAT_MATTE,
    );
    // Tiny distant red/white gantry + slender white rocket needle + lightning mast
    buf.add_box(
        Vec3::new(pad.x - 1.8, pad.y, pad.z - 0.6),
        Vec3::new(pad.x - 0.9, pad.y + 7.5, pad.z + 0.6),
        [0.86, 0.24, 0.20],
        MAT_ALLOY,
    );
    buf.add_box(
        Vec3::new(pad.x - 1.7, pad.y + 7.5, pad.z - 0.5),
        Vec3::new(pad.x - 1.0, pad.y + 13.8, pad.z + 0.5),
        [0.95, 0.95, 0.94],
        MAT_ALLOY,
    );
    buf.add_lathe_y(
        Vec3::new(pad.x + 0.6, pad.y, pad.z),
        &[(0.0, 0.45), (9.5, 0.36), (12.8, 0.04)],
        10,
        [0.98, 0.98, 0.97],
        MAT_ALLOY,
        true,
    );
    buf.add_cylinder(
        Vec3::new(pad.x - 4.8, pad.y, pad.z),
        Vec3::new(pad.x - 4.8, pad.y + 16.5, pad.z),
        0.16,
        0.05,
        6,
        [0.86, 0.88, 0.90],
        MAT_ALLOY,
        false,
    );
}

/// Builds the far-distant static-fire flash & slender rising exhaust column at `z = 360 m`!
pub fn build_distant_pad_plume(buf: &mut MeshBuffer, p: &DynamicSceneParams) {
    if p.plume_intensity <= 0.01 && p.plume_age_s >= 18.0 {
        return;
    }
    let pad = Vec3::new(2.2, 14.5, 360.0);
    if p.plume_intensity > 0.02 {
        let core_h = 6.5 * p.plume_intensity;
        buf.add_ellipsoid(
            Vec3::new(pad.x + 0.6, pad.y + core_h * 0.4, pad.z),
            Vec3::new(1.5 * p.plume_intensity, core_h * 0.55, 1.5 * p.plume_intensity),
            10,
            6,
            [1.0, 0.88, 0.50],
            MAT_PLUME,
        );
    }
    let drift_h = (p.plume_age_s * 3.8).clamp(1.2, 58.0);
    let fade = (1.0 - (p.plume_age_s / 18.0)).clamp(0.0, 1.0);
    if fade > 0.04 {
        for puff in 0..6 {
            let frac = (puff as f32) / 5.0;
            let py = pad.y + 1.5 + frac * drift_h;
            let px = pad.x + 0.6 + frac * (p.plume_age_s * 0.42);
            let rad = (1.4 + frac * 5.2) * (0.45 + 0.55 * fade);
            let c = 0.94 + 0.05 * fade;
            buf.add_ellipsoid(
                Vec3::new(px, py, pad.z),
                Vec3::new(rad, rad * 0.78, rad),
                10,
                7,
                [c, c, c * 0.98],
                MAT_PLUME,
            );
        }
    }
}
