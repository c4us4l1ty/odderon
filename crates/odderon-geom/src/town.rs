//! Modular "Summer Noon" Japanese Machiya Streetscape & Distant Launch Pad Vista (`town.rs`).
//!
//! Styled directly after photorealistic Japanese Summer Noon architecture (`image-15.png`):
//! - Deep roasted `kominka` cedar timber framing, aged warm sand-plaster, dark charcoal Kawara roofs,
//!   projecting 2nd-floor wooden `te-suri` balconies, sliding `amado` shutters, hanging `chouchin` lanterns,
//!   `noren` shop curtains, weathered corrugated steel annex, and foreground spun-concrete utility pole.
//! - Composed with a deliberate right-of-center scenic gap (`x in [-3.55, -0.35]`) framing a small,
//!   atmospheric sneak view of the far-away Rocket Launch Pad (`z = 380 m`) flanked by a second
//!   2-story Machiya townhouse on the right edge of the window!

use crate::{
    props, DynamicSceneParams, MeshBuffer, Pose, EXTERIOR_PAD_POS, MAT_ALLOY, MAT_CERAMIC,
    MAT_CORRUGATED, MAT_EMISSIVE_WARM, MAT_FOLIAGE, MAT_KAWARA, MAT_MATTE, MAT_PLUME, MAT_SHOJI,
    MAT_WOOD,
};
use odderon_dmath::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct MachiyaHouseSpec {
    pub width: f32,
    pub depth: f32,
    pub plaster_col: [f32; 3],
    pub timber_col: [f32; 3],
    pub shutter_wood_col: [f32; 3],
    pub tile_col: [f32; 3],
    pub has_wide_balcony: bool,
    pub has_eave_ac: bool,
    pub has_izakaya_lanterns: bool,
    pub has_rooftop_aerial: bool,
}

/// Builds a photorealistic 2-story "Summer Noon" Japanese Machiya townhouse (`image-15.png`!).
/// Engineered with strict non-coplanar mortise-and-tenon step offsets so zero Z-fighting ever occurs.
pub fn build_machiya_house(buf: &mut MeshBuffer, pose: Pose, s: &MachiyaHouseSpec) {
    buf.with_pose(pose, |b| {
        let hw = s.width * 0.5;
        let d = s.depth;
        let h1 = 2.85_f32;
        let h2 = 5.62_f32;
        let iron_dark = [0.09, 0.09, 0.10];
        let warm_glass = [0.16, 0.14, 0.12];
        let warm_shoji = [0.48, 0.40, 0.30];

        // 1. Main 2-story aged plaster & dark timber core (`z in [0.0, d]`)
        b.add_box(Vec3::new(-hw, 0.005, 0.0), Vec3::new(hw, h2, d), s.plaster_col, MAT_MATTE);
        // Dark vertical cedar plank cladding across 1st-floor facade (`z in [-0.018, -0.002]`, `image-15.png`!)
        b.add_box(Vec3::new(-hw + 0.08, 0.01, -0.018), Vec3::new(hw - 0.08, h1 - 0.06, -0.002), s.shutter_wood_col, MAT_WOOD);

        // 2. Stepped Japanese Post-and-Beam Joinery (`hashira` posts proud at `z = -0.075..-0.020`, `nuki` beams at `-0.048..-0.004`!)
        for &px in &[-hw + 0.09, -hw * 0.34, hw * 0.34, hw - 0.09] {
            b.add_box(Vec3::new(px - 0.075, 0.006, -0.075), Vec3::new(px + 0.075, h2 - 0.02, -0.020), s.timber_col, MAT_WOOD);
        }
        for &py in &[h1 - 0.08, h1 + 0.34, h2 - 0.14] {
            b.add_box(Vec3::new(-hw + 0.02, py, -0.048), Vec3::new(hw - 0.02, py + 0.11, -0.004), s.timber_col, MAT_WOOD);
        }

        // 3. First-Floor Machiya Entrance (`sliding dark lattice hikido + crisp white noren + hanging chouchin lanterns!`)
        let door_x0 = -hw * 0.45;
        let door_x1 = hw * 0.45;
        b.add_box(Vec3::new(door_x0, 0.02, -0.032), Vec3::new(door_x1, 2.16, -0.019), warm_glass, MAT_ALLOY);
        let slats_1f = 10;
        for i in 0..=slats_1f {
            let sx = door_x0 + (door_x1 - door_x0) * (i as f32) / (slats_1f as f32);
            b.add_box(Vec3::new(sx - 0.016, 0.02, -0.052), Vec3::new(sx + 0.016, 2.16, -0.033), s.timber_col, MAT_WOOD);
        }

        if s.has_izakaya_lanterns {
            // Crisp white cloth `noren` curtain (`image-15.png` center doorway!)
            for panel in 0..3 {
                let nx0 = -0.58 + (panel as f32) * 0.40;
                b.add_box(Vec3::new(nx0, 1.38, -0.22), Vec3::new(nx0 + 0.36, 2.14, -0.19), [0.76, 0.75, 0.72], MAT_MATTE);
            }
            // Framed menu board on left cedar post (`image-15.png`!)
            b.add_box(Vec3::new(-hw + 0.34, 0.78, -0.095), Vec3::new(-hw + 1.08, 1.92, -0.055), [0.42, 0.16, 0.09], MAT_WOOD);
            b.add_box(Vec3::new(-hw + 0.40, 0.84, -0.104), Vec3::new(-hw + 1.02, 1.86, -0.096), [0.78, 0.73, 0.62], MAT_MATTE);

            // Row of 4 hanging white `chouchin` paper lanterns under the dark 1st-floor eave (`image-15.png`!)
            for i in 0..4 {
                let lx = -hw * 0.56 + (i as f32) * (hw * 0.37);
                b.add_cylinder(Vec3::new(lx, 2.46, -0.76), Vec3::new(lx, 2.34, -0.76), 0.008, 0.008, 6, iron_dark, MAT_ALLOY, false);
                b.add_lathe_y(
                    Vec3::new(lx, 1.98, -0.76),
                    &[(0.0, 0.055), (0.06, 0.115), (0.28, 0.118), (0.35, 0.058)],
                    12,
                    [0.88, 0.85, 0.78],
                    MAT_SHOJI,
                    true,
                );
                b.add_cylinder(Vec3::new(lx, 2.33, -0.76), Vec3::new(lx, 2.36, -0.76), 0.062, 0.060, 10, iron_dark, MAT_ALLOY, true);
                b.add_cylinder(Vec3::new(lx, 1.95, -0.76), Vec3::new(lx, 1.98, -0.76), 0.060, 0.062, 10, iron_dark, MAT_ALLOY, true);
            }
        }

        // 4. First-Floor Dark Kawara Tiled Eave (`hisashi`) + Dark Iron Downspout (`amadoi`)!
        b.add_kawara_eave_slope(
            -hw - 0.18,
            hw + 0.18,
            h1 - 0.34,
            -0.98,
            h1 + 0.26,
            -0.02,
            s.tile_col,
            s.timber_col,
        );
        b.add_cylinder(
            Vec3::new(-hw + 0.18, 0.02, -0.14),
            Vec3::new(-hw + 0.18, h2 - 0.16, -0.14),
            0.034,
            0.034,
            7,
            iron_dark,
            MAT_ALLOY,
            false,
        );

        // Weathered Outdoor Split-AC Condenser mounted on the 1st-floor eave (`image-15.png`!)
        if s.has_eave_ac {
            props::outdoor_ac_condenser(b, Pose::at(-hw + 1.18, h1 + 0.02, -0.32));
        }

        // 5. Second-Story Dark Roasted-Wood Sliding `Amado` Shutters + Projecting `Te-suri` Wooden Balcony (`image-15.png`!)
        let win_x0 = -hw * 0.56;
        let win_x1 = hw * 0.68;
        let (wy0, wy1) = (h1 + 0.48, h2 - 0.28);
        // Recessed warm-amber shoji/glass window bay + louvered wooden `tobukuro` shutter box on left!
        b.add_box(Vec3::new(win_x0, wy0, -0.028), Vec3::new(win_x0 + 0.72, wy1, -0.005), s.shutter_wood_col, MAT_WOOD);
        b.add_box(Vec3::new(win_x0 + 0.73, wy0, -0.022), Vec3::new(win_x1, wy1, -0.004), warm_shoji, MAT_WOOD);
        for m in 0..=4 {
            let mx = (win_x0 + 0.73) + (win_x1 - (win_x0 + 0.73)) * (m as f32) / 4.0;
            b.add_box(Vec3::new(mx - 0.022, wy0, -0.052), Vec3::new(mx + 0.022, wy1, -0.023), s.timber_col, MAT_WOOD);
        }
        b.add_box(Vec3::new(win_x0 + 0.73, (wy0 + wy1) * 0.5 - 0.018, -0.048), Vec3::new(win_x1, (wy0 + wy1) * 0.5 + 0.018, -0.024), s.timber_col, MAT_WOOD);

        // Projecting dark wooden 2nd-story balcony handrail (`te-suri`, exact centerpiece of `image-15.png`!)
        let bal_x0 = if s.has_wide_balcony { win_x0 - 0.06 } else { -0.85 };
        let bal_x1 = if s.has_wide_balcony { win_x1 + 0.06 } else { 0.85 };
        let bal_z = -0.38_f32;
        b.add_box(Vec3::new(bal_x0, wy0 - 0.08, bal_z), Vec3::new(bal_x1, wy0 + 0.02, -0.03), s.timber_col, MAT_WOOD);
        b.add_box(Vec3::new(bal_x0 - 0.04, wy0 + 0.54, bal_z - 0.03), Vec3::new(bal_x1 + 0.04, wy0 + 0.62, bal_z + 0.05), s.timber_col, MAT_WOOD);
        b.add_box(Vec3::new(bal_x0, wy0 + 0.54, bal_z), Vec3::new(bal_x0 + 0.06, wy0 + 0.61, -0.03), s.timber_col, MAT_WOOD);
        b.add_box(Vec3::new(bal_x1 - 0.06, wy0 + 0.54, bal_z), Vec3::new(bal_x1, wy0 + 0.61, -0.03), s.timber_col, MAT_WOOD);
        let slat_n = (((bal_x1 - bal_x0) / 0.22) as usize).clamp(4, 24);
        for k in 0..=slat_n {
            let bx = bal_x0 + (bal_x1 - bal_x0) * (k as f32) / (slat_n as f32);
            b.add_box(Vec3::new(bx - 0.022, wy0 + 0.022, bal_z - 0.01), Vec3::new(bx + 0.022, wy0 + 0.538, bal_z + 0.03), s.shutter_wood_col, MAT_WOOD);
        }

        // 6. Main Second-Story Overhanging Dark Kawara Tiled Roof (`non-coplanar gable inset!`)
        let ridge_y = h2 + 1.42;
        let ridge_z = d * 0.48;
        b.add_kawara_eave_slope(
            -hw - 0.38,
            hw + 0.38,
            h2 - 0.12,
            -0.86,
            ridge_y,
            ridge_z,
            s.tile_col,
            s.timber_col,
        );
        b.push_quad(
            Vec3::new(-hw - 0.38, h2 - 0.12, d + 0.62),
            Vec3::new(hw + 0.38, h2 - 0.12, d + 0.62),
            Vec3::new(hw + 0.38, ridge_y, ridge_z),
            Vec3::new(-hw - 0.38, ridge_y, ridge_z),
            s.tile_col,
            MAT_KAWARA,
        );
        b.add_box(Vec3::new(-hw + 0.08, h2 + 0.01, 0.22), Vec3::new(hw - 0.08, ridge_y - 0.22, d - 0.22), s.plaster_col, MAT_MATTE);
        b.add_cylinder(
            Vec3::new(-hw - 0.42, ridge_y + 0.05, ridge_z),
            Vec3::new(hw + 0.42, ridge_y + 0.05, ridge_z),
            0.10,
            0.10,
            8,
            [0.10, 0.11, 0.13],
            MAT_KAWARA,
            true,
        );

        if s.has_rooftop_aerial {
            let ax = hw * 0.42;
            b.add_cylinder(Vec3::new(ax, ridge_y + 0.05, ridge_z), Vec3::new(ax, ridge_y + 1.65, ridge_z), 0.018, 0.012, 6, [0.46, 0.48, 0.50], MAT_ALLOY, false);
            b.add_cylinder(Vec3::new(ax, ridge_y + 1.48, ridge_z - 0.48), Vec3::new(ax, ridge_y + 1.48, ridge_z + 0.48), 0.012, 0.012, 6, [0.46, 0.48, 0.50], MAT_ALLOY, false);
            for &dz in &[-0.36_f32, -0.12, 0.12, 0.36] {
                b.add_cylinder(Vec3::new(ax - 0.28, ridge_y + 1.48, ridge_z + dz), Vec3::new(ax + 0.28, ridge_y + 1.48, ridge_z + dz), 0.008, 0.008, 5, [0.50, 0.52, 0.54], MAT_ALLOY, false);
            }
        }
    });
}

/// Realistic Weathered Spun-Concrete Japanese Utility Pole (`電柱`: grey concrete, lineman step-bolts,
/// municipal ID plaque, streetlamp arm, twin transformers & yellow/black guy-wire guard `支線ガード`).
pub fn build_japanese_utility_pole(buf: &mut MeshBuffer, pose: Pose, has_transformers: bool) {
    buf.with_pose(pose, |b| {
        let concrete = [0.40, 0.40, 0.38];
        let steel_band = [0.16, 0.18, 0.20];

        b.add_lathe_y(Vec3::ZERO, &[(0.0, 0.158), (7.10, 0.100)], 12, concrete, MAT_MATTE, true);

        for &by in &[1.65_f32, 3.10, 4.35, 5.55, 6.55] {
            let r = 0.158 - (0.058 * by / 7.10) + 0.006;
            b.add_lathe_y(Vec3::new(0.0, by, 0.0), &[(0.0, r), (0.042, r)], 12, steel_band, MAT_ALLOY, false);
        }
        // Vertical Japanese municipal ID plate (`東幹 14`) + alternating lineman step-bolts (`足場ボルト`)
        b.add_box(Vec3::new(-0.042, 1.72, -0.156), Vec3::new(0.042, 2.38, -0.140), [0.76, 0.75, 0.69], MAT_MATTE);
        for step in 0..6 {
            let sy = 2.65 + (step as f32) * 0.62;
            let side = if step % 2 == 0 { -1.0_f32 } else { 1.0_f32 };
            b.add_box(Vec3::new(side * 0.10, sy, -0.012), Vec3::new(side * 0.21, sy + 0.022, 0.012), steel_band, MAT_ALLOY);
        }

        // Curved galvanized residential streetlamp arm (`防犯灯`)
        b.add_beam(Vec3::new(0.0, 4.18, -0.12), Vec3::new(0.0, 4.46, -0.78), 0.022, [0.34, 0.37, 0.40], MAT_ALLOY);
        b.add_box(Vec3::new(-0.075, 4.42, -0.94), Vec3::new(0.075, 4.49, -0.62), [0.78, 0.80, 0.82], MAT_ALLOY);

        if has_transformers {
            for &(tx, tz, r) in &[(-0.25_f32, -0.09_f32, 0.155_f32), (0.23, 0.07, 0.138)] {
                b.add_lathe_y(Vec3::new(tx, 4.58, tz), &[(0.0, r), (0.56, r)], 12, [0.38, 0.40, 0.41], MAT_ALLOY, true);
                b.add_cylinder(Vec3::new(0.0, 4.84, 0.0), Vec3::new(tx, 4.84, tz), 0.022, 0.022, 6, steel_band, MAT_ALLOY, false);
            }
            // Diagonal yellow-and-black Japanese guy-wire guard (`支線ガード`) anchored at curb!
            b.add_beam(Vec3::new(0.08, 4.55, -0.04), Vec3::new(0.86, 1.72, -0.26), 0.010, [0.10, 0.11, 0.12], MAT_ALLOY);
            for seg in 0..5 {
                let f0 = (seg as f32) / 5.0;
                let f1 = ((seg + 1) as f32) / 5.0;
                let p0 = Vec3::new(0.86 + 0.48 * f0, 1.72 * (1.0 - f0), -0.26);
                let p1 = Vec3::new(0.86 + 0.48 * f1, 1.72 * (1.0 - f1), -0.26);
                let gcol = if seg % 2 == 0 { [0.84, 0.68, 0.09] } else { [0.10, 0.11, 0.12] };
                b.add_beam(p0, p1, 0.030, gcol, MAT_ALLOY);
            }
        }

        // Galvanized steel crossarm & white porcelain insulators
        b.add_cylinder(Vec3::new(0.0, 6.88, -0.62), Vec3::new(0.0, 6.88, 0.62), 0.026, 0.026, 8, steel_band, MAT_ALLOY, true);
        for &iz in &[-0.50_f32, -0.16, 0.16, 0.50] {
            b.add_lathe_y(Vec3::new(0.0, 6.90, iz), &[(0.0, 0.028), (0.068, 0.018)], 8, [0.80, 0.82, 0.82], MAT_CERAMIC, true);
        }
    });
}

/// Assembles the "Summer Noon" Japanese Machiya street + Right-Side Sneak View of Far-Away Launch Pad-1!
pub fn build_uchinoura_street_static(buf: &mut MeshBuffer) {
    let ground_y = -3.20_f32;

    // 1. Deep Sunlit Summer Meadow/Town Ground, Sidewalks, Curbs & Dark Weathered Asphalt Street!
    buf.add_box(Vec3::new(-220.0, ground_y - 0.35, -95.0), Vec3::new(220.0, ground_y, 160.0), [0.12, 0.22, 0.08], MAT_FOLIAGE);

    let sidewalk_stone = [0.40, 0.38, 0.34];
    let curb_stone = [0.32, 0.31, 0.28];
    buf.add_box(Vec3::new(-42.0, ground_y + 0.005, -2.5), Vec3::new(42.0, ground_y + 0.14, 4.78), sidewalk_stone, MAT_MATTE);
    buf.add_box(Vec3::new(-42.0, ground_y + 0.005, 9.45), Vec3::new(42.0, ground_y + 0.14, 12.6), sidewalk_stone, MAT_MATTE);
    buf.add_box(Vec3::new(-42.0, ground_y + 0.006, 9.24), Vec3::new(42.0, ground_y + 0.16, 9.44), curb_stone, MAT_MATTE);
    buf.add_box(Vec3::new(-42.0, ground_y + 0.006, 4.80), Vec3::new(42.0, ground_y + 0.16, 5.00), curb_stone, MAT_MATTE);
    // Lush summer grass verge beds along the roadside curb seams (leaving bus-stop boarding zone clear!)
    buf.add_box(Vec3::new(-18.0, ground_y + 0.141, 4.56), Vec3::new(-8.10, ground_y + 0.168, 4.79), [0.14, 0.30, 0.08], MAT_FOLIAGE);
    buf.add_box(Vec3::new(-3.60, ground_y + 0.141, 4.58), Vec3::new(16.0, ground_y + 0.168, 4.79), [0.14, 0.30, 0.08], MAT_FOLIAGE);
    buf.add_box(Vec3::new(-18.0, ground_y + 0.044, 8.94), Vec3::new(18.0, ground_y + 0.072, 9.23), [0.13, 0.28, 0.08], MAT_FOLIAGE);

    // CHECKPOINT 1B NEAR-SIDEWALK PROPS: Weathered Vending Machine + Kominka `バス停` Range Shuttle Stop!
    props::japanese_vending_machine(buf, Pose::at(-1.10, ground_y + 0.142, 2.72));
    props::bus_stop_shelter(buf, Pose::at(-6.80, ground_y + 0.142, 4.30));

    // Sunlit 3D Summer Grass Verges & Golden-Blossom Roadside Shrubs (`image-18.png` foreground & stone base!)
    props::summer_grass_strip(buf, Pose::at(-10.40, ground_y + 0.165, 4.68), 4.8, 20, 101);
    props::summer_grass_strip(buf, Pose::at(-1.85, ground_y + 0.165, 4.68), 3.2, 16, 203);
    props::summer_grass_strip(buf, Pose::at(2.85, ground_y + 0.165, 4.68), 4.8, 22, 307);
    props::summer_grass_strip(buf, Pose::at(8.20, ground_y + 0.165, 4.68), 5.2, 20, 409);
    props::summer_grass_strip(buf, Pose::at(-4.50, ground_y + 0.070, 9.08), 5.4, 18, 701);
    props::summer_grass_strip(buf, Pose::at(2.60, ground_y + 0.070, 9.08), 5.4, 18, 809);
    props::summer_grass_strip(buf, Pose::at(5.60, ground_y + 0.142, 2.52), 4.4, 16, 511);
    props::summer_grass_strip(buf, Pose::at(-5.95, ground_y + 0.142, 2.38), 2.8, 12, 613);
    props::flowering_roadside_shrub(buf, Pose::at(3.55, ground_y + 0.142, 2.68));
    props::flowering_roadside_shrub(buf, Pose::at(-5.68, ground_y + 0.142, 2.52).with_scale(0.82));

    // Dark weathered Japanese residential asphalt (`z in [5.01, 9.23]`) + muted shoulder lines
    buf.add_box(Vec3::new(-45.0, ground_y + 0.004, 5.01), Vec3::new(45.0, ground_y + 0.042, 9.23), [0.14, 0.15, 0.17], MAT_MATTE);
    buf.add_box(Vec3::new(-42.0, ground_y + 0.048, 5.45), Vec3::new(42.0, ground_y + 0.056, 5.54), [0.64, 0.63, 0.59], MAT_MATTE);
    buf.add_box(Vec3::new(-42.0, ground_y + 0.048, 8.68), Vec3::new(42.0, ground_y + 0.056, 8.77), [0.64, 0.63, 0.59], MAT_MATTE);

    // 2A. `-Z` STREETSCAPE NEIGHBORS FLANKING `UCHINOURA MAISON` (`image-18.png` Shoulder-to-Shoulder Architecture!):
    let kominka_timber = [0.12, 0.065, 0.032];
    let kominka_shutter = [0.20, 0.105, 0.052];
    let kawara_dark = [0.10, 0.11, 0.13];

    // Right of `Uchinoura Maison` (`x in [2.82, 8.45], z in [-2.5, 2.38]`): 2-Story Weathered Corrugated Steel Annex
    // (`image-18.png` left reference: ribbed blue-grey steel, Oya-stone plinth, cobalt casement window, rooftop wooden rail!)
    buf.add_box(Vec3::new(2.80, ground_y + 0.142, -2.52), Vec3::new(8.48, ground_y + 0.68, 2.42), [0.34, 0.33, 0.30], MAT_MATTE);
    buf.add_box(Vec3::new(2.84, ground_y + 0.68, -2.48), Vec3::new(8.44, 2.86, 2.38), [0.24, 0.30, 0.36], MAT_CORRUGATED);
    // Cobalt-blue painted industrial window frame (`image-18.png` middle-left!) + dark recessed glass
    buf.add_u_frame_xy(4.35, 5.75, -0.05, 1.05, 2.36, 2.44, 0.075, [0.14, 0.31, 0.52], MAT_ALLOY);
    buf.add_box(Vec3::new(4.35, -0.12, 2.36), Vec3::new(5.75, -0.05, 2.45), [0.14, 0.31, 0.52], MAT_ALLOY);
    buf.add_box(Vec3::new(4.36, -0.04, 2.381), Vec3::new(5.74, 1.04, 2.405), [0.08, 0.11, 0.14], MAT_ALLOY);
    // Rooftop wooden cantilevered drying-deck railing (`image-18.png` top-left!)
    buf.add_box(Vec3::new(2.80, 2.86, -2.52), Vec3::new(8.48, 2.96, 2.46), kominka_timber, MAT_WOOD);
    buf.add_box(Vec3::new(2.86, 3.58, 2.38), Vec3::new(8.42, 3.65, 2.45), kominka_shutter, MAT_WOOD);
    for p in 0..=6 {
        let px = 2.92 + (p as f32) * 0.90;
        buf.add_box(Vec3::new(px - 0.03, 2.96, 2.39), Vec3::new(px + 0.03, 3.64, 2.44), kominka_timber, MAT_WOOD);
    }

    // Far-Right `-Z` Neighbor (`x in [8.60, 15.60]`): Traditional 2-Story Plaster & Timber `Kura` Storehouse!
    buf.add_box(Vec3::new(8.60, ground_y + 0.142, -2.60), Vec3::new(15.60, 2.62, 2.32), [0.54, 0.49, 0.40], MAT_MATTE);
    buf.add_box(Vec3::new(8.58, ground_y + 0.142, 2.321), Vec3::new(15.62, -0.35, 2.355), kominka_shutter, MAT_WOOD);
    buf.push_quad(
        Vec3::new(8.42, 2.54, 2.86),
        Vec3::new(15.78, 2.54, 2.86),
        Vec3::new(15.78, 3.52, -0.10),
        Vec3::new(8.42, 3.52, -0.10),
        kawara_dark,
        MAT_KAWARA,
    );

    // Left of `Uchinoura Maison` & Behind the `バス停` Bus Stop (`x in [-13.20, -5.80], z = 2.22`, rotated PI to face `+Z`!):
    // Sister 2-Story Kominka Machiya (`House #3`, eliminating the left `-Z` green void!)
    build_machiya_house(
        buf,
        Pose::at_yaw(-9.50, ground_y + 0.142, 2.22, core::f32::consts::PI),
        &MachiyaHouseSpec {
            width: 7.40,
            depth: 6.4,
            plaster_col: [0.50, 0.43, 0.33],
            timber_col: kominka_timber,
            shutter_wood_col: kominka_shutter,
            tile_col: kawara_dark,
            has_wide_balcony: true,
            has_eave_ac: true,
            has_izakaya_lanterns: false,
            has_rooftop_aerial: true,
        },
    );

    // Far-Left `-Z` Oya-Stone Compound Wall (`hei`, `x in [-22.0, -13.25]`) + Lush Summer Canopy Tree (`image-18.png` foliage!)
    buf.add_box(Vec3::new(-22.0, ground_y + 0.142, 1.95), Vec3::new(-13.25, ground_y + 2.05, 2.32), [0.36, 0.35, 0.32], MAT_MATTE);
    buf.add_box(Vec3::new(-22.05, ground_y + 2.05, 1.88), Vec3::new(-13.20, ground_y + 2.18, 2.40), kawara_dark, MAT_KAWARA);
    // Tall roadside Zelkova (`Keyaki`) summer shade tree (`x = -14.4, z = 1.35`)
    buf.add_cylinder(Vec3::new(-14.4, ground_y + 0.14, 1.35), Vec3::new(-14.2, 2.40, 1.45), 0.24, 0.15, 8, [0.18, 0.12, 0.07], MAT_WOOD, false);
    buf.add_ellipsoid(Vec3::new(-14.1, 3.35, 1.65), Vec3::new(2.35, 1.65, 2.10), 10, 7, [0.12, 0.34, 0.10], MAT_FOLIAGE);
    buf.add_ellipsoid(Vec3::new(-12.8, 2.75, 2.25), Vec3::new(1.55, 1.20, 1.45), 9, 6, [0.16, 0.40, 0.11], MAT_FOLIAGE);

    // Second summer Zelkova tree on far-right (`x = 16.4, z = 1.60`) framing the `-Z` streetscape
    buf.add_cylinder(Vec3::new(16.4, ground_y + 0.14, 1.60), Vec3::new(16.2, 2.55, 1.70), 0.24, 0.15, 8, [0.18, 0.12, 0.07], MAT_WOOD, false);
    buf.add_ellipsoid(Vec3::new(16.0, 3.50, 1.85), Vec3::new(2.40, 1.70, 2.10), 10, 7, [0.13, 0.36, 0.10], MAT_FOLIAGE);

    // 2B. `+Z` WINDOW COMPOSITION (`image-15.png` aesthetic + Right-Side Launch Pad Sneak View!):

    // Corrugated galvanized steel neighbor annex on the far left (`image-15.png` left edge!)
    buf.add_box(Vec3::new(8.05, ground_y + 0.142, 10.85), Vec3::new(13.8, ground_y + 6.35, 17.2), [0.26, 0.30, 0.35], MAT_CORRUGATED);
    buf.add_u_frame_xy(9.10, 10.35, ground_y + 3.65, ground_y + 4.45, 10.78, 10.86, 0.05, [0.18, 0.20, 0.22], MAT_ALLOY);
    buf.add_box(Vec3::new(9.11, ground_y + 3.66, 10.81), Vec3::new(10.34, ground_y + 4.44, 10.84), [0.10, 0.14, 0.18], MAT_ALLOY);

    // House #1: Main 2-Story Kominka Machiya (`x in [-0.30, +7.80]`, left & center of window!)
    build_machiya_house(
        buf,
        Pose::at(3.75, ground_y + 0.142, 11.45),
        &MachiyaHouseSpec {
            width: 8.10,
            depth: 6.8,
            plaster_col: [0.56, 0.50, 0.40],
            timber_col: kominka_timber,
            shutter_wood_col: kominka_shutter,
            tile_col: kawara_dark,
            has_wide_balcony: true,
            has_eave_ac: true,
            has_izakaya_lanterns: true,
            has_rooftop_aerial: true,
        },
    );

    // House #2: Second 2-Story Machiya Townhouse framing the right edge of the window (`x in [-10.95, -3.55]`)!
    build_machiya_house(
        buf,
        Pose::at(-7.25, ground_y + 0.142, 11.75),
        &MachiyaHouseSpec {
            width: 7.40,
            depth: 6.6,
            plaster_col: [0.52, 0.47, 0.38],
            timber_col: kominka_timber,
            shutter_wood_col: [0.25, 0.14, 0.072],
            tile_col: [0.14, 0.15, 0.17],
            has_wide_balcony: false,
            has_eave_ac: false,
            has_izakaya_lanterns: false,
            has_rooftop_aerial: false,
        },
    );

    // 3. Ground-Level Japanese Street Details (`image-15.png`: stone plinth, street sign, low shrubs, bicycle — keeping gap sky-clear!)
    // Weathered Oya-stone block wall at ground level inside the alley gap (`y = ground_y + 0.14 .. +1.05` so vista above is 100% open!)
    buf.add_rounded_box(Vec3::new(-3.45, ground_y + 0.142, 11.55), Vec3::new(-1.85, ground_y + 1.02, 12.15), 0.03, [0.40, 0.39, 0.36], MAT_MATTE);
    // Small Japanese round red-and-white street sign post (`image-15.png` bottom right!)
    buf.add_cylinder(Vec3::new(-3.25, ground_y + 0.142, 10.15), Vec3::new(-3.25, ground_y + 2.05, 10.15), 0.022, 0.022, 8, [0.58, 0.60, 0.62], MAT_ALLOY, false);
    buf.add_disc_neg_z(Vec3::new(-3.25, ground_y + 1.92, 10.12), 0.18, 14, [0.76, 0.16, 0.14], MAT_ALLOY);
    buf.add_disc_neg_z(Vec3::new(-3.25, ground_y + 1.92, 10.112), 0.12, 12, [0.86, 0.85, 0.82], MAT_MATTE);

    props::parked_bicycle(buf, Pose::at(5.85, ground_y + 0.142, 10.35));
    props::terracotta_planter(buf, Pose::at(-0.05, ground_y + 0.142, 10.30));
    props::terracotta_planter(buf, Pose::at(-3.85, ground_y + 0.142, 10.55).with_scale(0.90));
    props::outdoor_ac_condenser(buf, Pose::at(-4.55, ground_y + 0.15, 11.35));

    // 4. FOREGROUND CONCRETE UTILITY POLE ON THE LEFT OUTSIDE WINDOW (`x = +1.08, z = 5.35`) + CATENARY CABLES!
    let pole_left_fg = Vec3::new(1.08, ground_y + 0.142, 5.35);
    let pole_right = Vec3::new(-8.20, ground_y + 0.142, 5.75);
    let pole_far_left = Vec3::new(10.20, ground_y + 0.142, 5.45);

    build_japanese_utility_pole(buf, Pose::at(pole_left_fg.x, pole_left_fg.y, pole_left_fg.z), true);
    build_japanese_utility_pole(buf, Pose::at(pole_right.x, pole_right.y, pole_right.z), false);
    build_japanese_utility_pole(buf, Pose::at(pole_far_left.x, pole_far_left.y, pole_far_left.z), false);

    let wire_blk = [0.07, 0.08, 0.09];
    for &dz in &[-0.38_f32, 0.0, 0.38] {
        buf.add_catenary(
            Vec3::new(pole_left_fg.x, pole_left_fg.y + 6.94, pole_left_fg.z + dz),
            Vec3::new(pole_right.x, pole_right.y + 6.94, pole_right.z + dz),
            0.34,
            14,
            0.009,
            wire_blk,
        );
        buf.add_catenary(
            Vec3::new(pole_far_left.x, pole_far_left.y + 6.94, pole_far_left.z + dz),
            Vec3::new(pole_left_fg.x, pole_left_fg.y + 6.94, pole_left_fg.z + dz),
            0.34,
            12,
            0.009,
            wire_blk,
        );
    }
    buf.add_catenary(
        Vec3::new(pole_left_fg.x, pole_left_fg.y + 5.58, pole_left_fg.z - 0.10),
        Vec3::new(pole_right.x, pole_right.y + 5.58, pole_right.z - 0.10),
        0.46,
        14,
        0.011,
        wire_blk,
    );
    buf.add_catenary(
        Vec3::new(pole_left_fg.x, pole_left_fg.y + 5.60, pole_left_fg.z + 0.10),
        Vec3::new(1.85, ground_y + 5.20, 11.40),
        0.26,
        10,
        0.008,
        wire_blk,
    );
    // Service drop cable (`引込線`) from foreground pole across to `Uchinoura Maison` eave (`image-18.png` overhead wires!)
    buf.add_catenary(
        Vec3::new(pole_left_fg.x, pole_left_fg.y + 5.56, pole_left_fg.z - 0.14),
        Vec3::new(-2.45, 2.52, 2.38),
        0.22,
        10,
        0.008,
        wire_blk,
    );

    // 5. LAYERED DEEP-FOREST UCHINOURA MOUNTAINS (`+Z` Launch Vista AND `-Z` Coastal Hills — Zero Barren Horizon!):
    // `-Z` Forested Uchinoura Hills rising behind `Uchinoura Maison` & neighboring roofs (`outward +Z/+Y CCW normals!`)
    buf.push_quad(
        Vec3::new(-200.0, ground_y, -68.0),
        Vec3::new(20.0, ground_y, -68.0),
        Vec3::new(20.0, 21.0, -125.0),
        Vec3::new(-200.0, 26.0, -135.0),
        [0.09, 0.22, 0.10],
        MAT_FOLIAGE,
    );
    buf.push_quad(
        Vec3::new(-15.0, ground_y, -74.0),
        Vec3::new(200.0, ground_y, -74.0),
        Vec3::new(200.0, 24.5, -132.0),
        Vec3::new(-15.0, 17.5, -118.0),
        [0.10, 0.24, 0.11],
        MAT_FOLIAGE,
    );
    // Mid-distance lush cedar-forested hills (`z = 135..160 m`, dipping into a saddle around `x = -26` so the distant Pad ridge rises clearly!)
    buf.push_quad(
        Vec3::new(-180.0, 16.0, 155.0),
        Vec3::new(-38.0, 6.5, 148.0),
        Vec3::new(-12.0, ground_y, 108.0),
        Vec3::new(-180.0, ground_y, 108.0),
        [0.09, 0.21, 0.10],
        MAT_FOLIAGE,
    );
    buf.push_quad(
        Vec3::new(-18.0, 6.2, 152.0),
        Vec3::new(180.0, 19.5, 165.0),
        Vec3::new(180.0, ground_y, 112.0),
        Vec3::new(-36.0, ground_y, 112.0),
        [0.10, 0.23, 0.11],
        MAT_FOLIAGE,
    );

    // Distant Uchinoura Coastal Mountain Ridge (`z = 385..430 m`) supporting Launch Pad-1 at `(-58.0, 17.8, 380.0)`!
    let pad = EXTERIOR_PAD_POS;
    buf.push_quad(
        Vec3::new(-260.0, 29.0, 425.0),
        Vec3::new(pad.x - 12.0, pad.y - 0.5, 392.0),
        Vec3::new(pad.x + 25.0, ground_y, 320.0),
        Vec3::new(-260.0, ground_y, 320.0),
        [0.12, 0.24, 0.20],
        MAT_FOLIAGE,
    );
    buf.push_quad(
        Vec3::new(pad.x - 16.0, pad.y - 0.4, 394.0),
        Vec3::new(240.0, 38.0, 435.0),
        Vec3::new(260.0, ground_y, 325.0),
        Vec3::new(pad.x - 32.0, ground_y, 325.0),
        [0.13, 0.25, 0.21],
        MAT_FOLIAGE,
    );

    // Far-Away Rocket Launch Pad-1 (`EXTERIOR_PAD_POS = (-58.0, 17.8, 380.0)` — framed in the right-side sneak-view gap!):
    // Concrete flame-trench apron cut into the distant ridge + red/white service gantry + glinting white orbital vehicle + lightning towers!
    buf.add_box(
        Vec3::new(pad.x - 14.0, pad.y - 3.5, pad.z - 8.0),
        Vec3::new(pad.x + 14.0, pad.y, pad.z + 8.0),
        [0.52, 0.53, 0.51],
        MAT_MATTE,
    );
    // Red-and-white JAXA-style umbilical gantry tower (`height = 24 m` at `380 m` distance -> crisp ~3.6 deg silhouette!)
    for tier in 0..4 {
        let ty0 = pad.y + (tier as f32) * 6.0;
        let col = if tier % 2 == 0 { [0.78, 0.18, 0.14] } else { [0.88, 0.88, 0.86] };
        buf.add_box(
            Vec3::new(pad.x - 3.6, ty0, pad.z - 1.1),
            Vec3::new(pad.x - 1.4, ty0 + 6.0, pad.z + 1.1),
            col,
            MAT_ALLOY,
        );
    }
    // Umbilical swing arms connecting gantry to rocket
    for &ay in &[8.0_f32, 15.0, 20.5] {
        buf.add_box(
            Vec3::new(pad.x - 1.4, pad.y + ay, pad.z - 0.35),
            Vec3::new(pad.x + 1.2, pad.y + ay + 0.55, pad.z + 0.35),
            [0.76, 0.77, 0.78],
            MAT_ALLOY,
        );
    }
    // Slender sunlit white 3-stage launcher on the pad (`height = 22.5 m`) + orange fairing band + beacon!
    buf.add_lathe_y(
        Vec3::new(pad.x + 1.6, pad.y, pad.z),
        &[(0.0, 0.85), (14.5, 0.78), (18.5, 0.68), (22.8, 0.06)],
        12,
        [0.95, 0.95, 0.93],
        MAT_ALLOY,
        true,
    );
    buf.add_lathe_y(
        Vec3::new(pad.x + 1.6, pad.y + 14.5, pad.z),
        &[(0.0, 0.80), (1.4, 0.80)],
        12,
        [0.86, 0.32, 0.12],
        MAT_ALLOY,
        false,
    );
    buf.add_box(
        Vec3::new(pad.x - 3.2, pad.y + 24.0, pad.z - 0.45),
        Vec3::new(pad.x - 1.8, pad.y + 25.0, pad.z + 0.45),
        [1.0, 0.36, 0.16],
        MAT_EMISSIVE_WARM,
    );
    // Twin lightning protection masts flanking the distant pad
    for &lx in &[-9.5_f32, 9.5] {
        buf.add_cylinder(
            Vec3::new(pad.x + lx, pad.y, pad.z),
            Vec3::new(pad.x + lx, pad.y + 27.0, pad.z),
            0.24,
            0.08,
            6,
            [0.72, 0.74, 0.76],
            MAT_ALLOY,
            false,
        );
    }
}

/// Builds dynamic town actors: scheduled Range Shuttle Microbus + far-distant static-fire exhaust plume!
pub fn build_distant_pad_plume(buf: &mut MeshBuffer, p: &DynamicSceneParams) {
    if p.shuttle_x.abs() <= 46.0 {
        props::uchinoura_shuttle_bus(buf, Pose::at(p.shuttle_x, -3.20 + 0.045, 6.35), p.shuttle_door_open);
    }

    if p.plume_intensity <= 0.01 && p.plume_age_s >= 18.0 {
        return;
    }
    let pad = EXTERIOR_PAD_POS;
    if p.plume_intensity > 0.02 {
        let core_h = 9.5 * p.plume_intensity;
        buf.add_ellipsoid(
            Vec3::new(pad.x + 1.6, pad.y + core_h * 0.38, pad.z - 1.2),
            Vec3::new(2.6 * p.plume_intensity, core_h * 0.55, 2.6 * p.plume_intensity),
            10,
            6,
            [1.0, 0.86, 0.44],
            MAT_PLUME,
        );
    }
    let drift_h = (p.plume_age_s * 4.5).clamp(1.8, 68.0);
    let fade = (1.0 - (p.plume_age_s / 18.0)).clamp(0.0, 1.0);
    if fade > 0.04 {
        for puff in 0..6 {
            let frac = (puff as f32) / 5.0;
            let py = pad.y + 2.0 + frac * drift_h;
            let px = pad.x + 1.6 + frac * (p.plume_age_s * 0.48);
            let rad = (2.2 + frac * 6.8) * (0.45 + 0.55 * fade);
            let c = 0.90 + 0.08 * fade;
            buf.add_ellipsoid(
                Vec3::new(px, py, pad.z),
                Vec3::new(rad, rad * 0.78, rad),
                10,
                7,
                [c, c * 0.98, c * 0.95],
                MAT_PLUME,
            );
        }
    }
}
