//! Modular Local-Space 3D Props Library (`Pose`-placed, reusable across rooms, streets & stations!).
//! Every prop is authored around local origin `(0, 0, 0)` (`y = 0` resting surface, `-Z` facing viewer)
//! using smooth-normal rounded boxes, ellipsoids, cylinders, and lathes so nothing looks blocky.

use crate::{
    MeshBuffer, Pose, MAT_ALLOY, MAT_CERAMIC, MAT_CRT, MAT_EMISSIVE_WARM, MAT_FOLIAGE, MAT_MATTE,
    MAT_WOOD,
};
use odderon_dmath::{dcos, dsin, Vec3, TAU};

/// Sculpted Retro-Futuristic CRT Vector Telemetry Oscilloscope (smooth rounded housing + rear tube + rotary dials).
pub fn crt_oscilloscope(buf: &mut MeshBuffer, pose: Pose) {
    buf.with_pose(pose, |b| {
        let alloy_body = [0.76, 0.77, 0.74]; // Warm vintage Tektronix/Japanese lab cream-grey!
        let alloy_dark = [0.17, 0.20, 0.23];
        let chrome = [0.84, 0.86, 0.89];

        // Rubber isolation feet
        for &(fx, fz) in &[(-0.20_f32, -0.13_f32), (0.20, -0.13), (-0.20, 0.13), (0.20, 0.13)] {
            b.add_cylinder(Vec3::new(fx, 0.0, fz), Vec3::new(fx, 0.018, fz), 0.022, 0.018, 8, alloy_dark, MAT_MATTE, true);
        }
        // Smooth filleted main instrument body + tapered rear CRT neck housing
        b.add_rounded_box(Vec3::new(-0.25, 0.016, -0.17), Vec3::new(0.25, 0.35, 0.17), 0.032, alloy_body, MAT_ALLOY);
        b.add_rounded_box(Vec3::new(-0.16, 0.05, 0.14), Vec3::new(0.16, 0.30, 0.25), 0.040, [0.56, 0.58, 0.56], MAT_ALLOY);
        // Dark rounded CRT glare bezel + phosphor vector screen (`-Z` front)
        b.add_rounded_box(Vec3::new(-0.08, 0.055, -0.192), Vec3::new(0.22, 0.31, -0.155), 0.020, alloy_dark, MAT_ALLOY);
        b.add_box(Vec3::new(-0.055, 0.078, -0.194), Vec3::new(0.195, 0.288, -0.186), [0.14, 0.84, 0.76], MAT_CRT);

        // Left-side rotary BNC control strip (`x in [-0.22, -0.10]`)
        b.add_rounded_box(Vec3::new(-0.225, 0.055, -0.180), Vec3::new(-0.10, 0.31, -0.162), 0.010, [0.22, 0.25, 0.28], MAT_ALLOY);
        for &ky in &[0.255_f32, 0.180, 0.105] {
            b.add_cylinder(Vec3::new(-0.162, ky, -0.175), Vec3::new(-0.162, ky, -0.205), 0.022, 0.018, 14, alloy_dark, MAT_ALLOY, true);
            b.add_cylinder(Vec3::new(-0.162, ky, -0.205), Vec3::new(-0.162, ky, -0.212), 0.009, 0.008, 10, chrome, MAT_ALLOY, true);
        }
        // Carrying bail handle arching smoothly across top
        b.add_cylinder(Vec3::new(-0.16, 0.345, -0.02), Vec3::new(-0.16, 0.385, -0.02), 0.009, 0.009, 8, chrome, MAT_ALLOY, true);
        b.add_cylinder(Vec3::new(0.16, 0.345, -0.02), Vec3::new(0.16, 0.385, -0.02), 0.009, 0.009, 8, chrome, MAT_ALLOY, true);
        b.add_cylinder(Vec3::new(-0.16, 0.385, -0.02), Vec3::new(0.16, 0.385, -0.02), 0.011, 0.011, 10, alloy_dark, MAT_MATTE, true);
    });
}

/// Sculpted Coastal Range VHF Transceiver (smooth gunmetal housing, VFO flywheel, coiled mic & whip antenna).
pub fn vhf_transceiver(buf: &mut MeshBuffer, pose: Pose) {
    buf.with_pose(pose, |b| {
        let body = [0.24, 0.28, 0.31];
        let face = [0.12, 0.14, 0.16];
        let chrome = [0.84, 0.86, 0.89];
        let brass = [0.84, 0.67, 0.30];

        b.add_rounded_box(Vec3::new(-0.24, 0.0, -0.15), Vec3::new(0.24, 0.185, 0.16), 0.024, body, MAT_ALLOY);
        b.add_rounded_box(Vec3::new(-0.215, 0.022, -0.162), Vec3::new(0.215, 0.162, -0.142), 0.012, face, MAT_ALLOY);
        // Backlit frequency window
        b.add_box(Vec3::new(-0.185, 0.102, -0.165), Vec3::new(0.03, 0.144, -0.158), [0.18, 0.82, 0.74], MAT_CRT);
        // Large machined VFO tuning flywheel + volume & squelch knobs
        b.add_cylinder(Vec3::new(0.10, 0.092, -0.158), Vec3::new(0.10, 0.092, -0.192), 0.034, 0.030, 16, [0.46, 0.50, 0.54], MAT_ALLOY, true);
        b.add_cylinder(Vec3::new(0.175, 0.122, -0.158), Vec3::new(0.175, 0.122, -0.180), 0.014, 0.012, 12, chrome, MAT_ALLOY, true);
        // Guarded Red `PAD-1 STATIC FIRE` pushbutton
        b.add_rounded_box(Vec3::new(0.148, 0.034, -0.172), Vec3::new(0.202, 0.084, -0.156), 0.006, brass, MAT_ALLOY);
        b.add_cylinder(Vec3::new(0.175, 0.059, -0.162), Vec3::new(0.175, 0.059, -0.184), 0.017, 0.015, 14, [0.96, 0.20, 0.14], MAT_EMISSIVE_WARM, true);
        // Speaker grille slots
        for s in 0..4 {
            let sy = 0.036 + (s as f32) * 0.014;
            b.add_box(Vec3::new(-0.185, sy, -0.165), Vec3::new(-0.03, sy + 0.006, -0.158), [0.34, 0.38, 0.42], MAT_ALLOY);
        }
        // BNC spring base + stainless whip antenna
        b.add_lathe_y(Vec3::new(-0.16, 0.184, 0.10), &[(0.0, 0.020), (0.045, 0.015), (0.08, 0.007)], 12, chrome, MAT_ALLOY, true);
        b.add_cylinder(Vec3::new(-0.16, 0.26, 0.10), Vec3::new(-0.19, 0.86, 0.13), 0.0045, 0.002, 8, chrome, MAT_ALLOY, true);
        // Sculpted rounded PTT Hand-Microphone on side cradle
        b.add_rounded_box(Vec3::new(0.238, 0.045, -0.05), Vec3::new(0.285, 0.14, 0.04), 0.018, [0.15, 0.17, 0.19], MAT_MATTE);
    });
}

/// Articulated Mid-Century Japanese Architect Desk Lamp (lathed base, twin tubular arms, conical bell shade).
pub fn architect_lamp(buf: &mut MeshBuffer, pose: Pose) {
    buf.with_pose(pose, |b| {
        let base_col = [0.18, 0.20, 0.22];
        let chrome = [0.84, 0.86, 0.89];
        let brass = [0.84, 0.67, 0.30];
        let sage_enamel = [0.22, 0.44, 0.38];

        b.add_lathe_y(
            Vec3::ZERO,
            &[(0.0, 0.092), (0.016, 0.088), (0.032, 0.028), (0.058, 0.018)],
            18,
            base_col,
            MAT_ALLOY,
            true,
        );
        let elbow = Vec3::new(-0.04, 0.31, 0.04);
        let wrist = Vec3::new(0.06, 0.47, -0.15);
        let shade_open = Vec3::new(0.12, 0.34, -0.25);

        b.add_cylinder(Vec3::new(0.01, 0.05, 0.0), elbow, 0.0065, 0.0065, 8, chrome, MAT_ALLOY, false);
        b.add_cylinder(Vec3::new(-0.01, 0.05, 0.01), elbow.add(Vec3::new(-0.02, 0.0, 0.01)), 0.0065, 0.0065, 8, chrome, MAT_ALLOY, false);
        b.add_cylinder(elbow, wrist, 0.0075, 0.0065, 8, chrome, MAT_ALLOY, true);
        b.add_ellipsoid(elbow, Vec3::new(0.018, 0.018, 0.018), 10, 6, brass, MAT_ALLOY);
        // Dome cap + flared conical bell shade
        b.add_ellipsoid(wrist, Vec3::new(0.026, 0.024, 0.026), 12, 6, brass, MAT_ALLOY);
        b.add_cylinder(wrist, shade_open, 0.024, 0.088, 18, sage_enamel, MAT_ALLOY, false);
    });
}

/// Sculpted Ergonomic Swivel Chair (smooth contoured waterfall seat, curved backrest, 5-star base & casters).
pub fn ergonomic_chair(buf: &mut MeshBuffer, pose: Pose) {
    buf.with_pose(pose, |b| {
        let alloy_dark = [0.16, 0.18, 0.21];
        let alloy_mid = [0.36, 0.40, 0.45];
        let upholstery = [0.20, 0.24, 0.29];

        b.add_cylinder(Vec3::new(0.0, 0.08, 0.0), Vec3::new(0.0, 0.44, 0.0), 0.028, 0.022, 12, alloy_dark, MAT_ALLOY, true);
        for leg in 0..5 {
            let ang = (leg as f64) * TAU / 5.0;
            let (ca, sa) = (dcos(ang) as f32, dsin(ang) as f32);
            let tip = Vec3::new(0.28 * ca, 0.052, 0.28 * sa);
            b.add_cylinder(Vec3::new(0.0, 0.095, 0.0), tip, 0.018, 0.012, 8, alloy_mid, MAT_ALLOY, true);
            b.add_ellipsoid(Vec3::new(tip.x, 0.026, tip.z), Vec3::new(0.024, 0.026, 0.024), 8, 5, [0.10, 0.11, 0.12], MAT_ALLOY);
        }
        // Smooth rounded seat cushion
        b.add_rounded_box(Vec3::new(-0.24, 0.43, -0.22), Vec3::new(0.24, 0.515, 0.22), 0.038, upholstery, MAT_MATTE);
        // Spine tube + smooth rounded ergonomic backrest (`-Z` behind sitter)
        b.add_cylinder(Vec3::new(0.0, 0.45, -0.17), Vec3::new(0.0, 0.84, -0.24), 0.020, 0.015, 10, alloy_dark, MAT_ALLOY, true);
        b.add_rounded_box(Vec3::new(-0.22, 0.56, -0.27), Vec3::new(0.22, 0.94, -0.19), 0.036, upholstery, MAT_MATTE);
        // Smooth rounded armrests
        for &sx in &[-0.255_f32, 0.255] {
            b.add_cylinder(Vec3::new(sx, 0.46, -0.03), Vec3::new(sx, 0.65, -0.03), 0.012, 0.011, 8, alloy_dark, MAT_ALLOY, false);
            b.add_rounded_box(Vec3::new(sx - 0.032, 0.645, -0.15), Vec3::new(sx + 0.032, 0.682, 0.11), 0.015, alloy_dark, MAT_MATTE);
        }
    });
}

/// Stainless Balmud-style Gooseneck Electric Kettle + Cutting Board + Two Yunomi Tea Cups.
pub fn kettle_set(buf: &mut MeshBuffer, pose: Pose) {
    buf.with_pose(pose, |b| {
        let chrome = [0.85, 0.87, 0.90];
        let matte_blk = [0.14, 0.15, 0.17];
        b.add_lathe_y(Vec3::ZERO, &[(0.0, 0.104), (0.022, 0.098), (0.026, 0.088)], 18, matte_blk, MAT_ALLOY, true);
        b.add_lathe_y(
            Vec3::new(0.0, 0.025, 0.0),
            &[
                (0.0, 0.092),
                (0.03, 0.095),
                (0.13, 0.080),
                (0.19, 0.070),
                (0.205, 0.073),
                (0.218, 0.064),
                (0.230, 0.018),
            ],
            20,
            chrome,
            MAT_ALLOY,
            true,
        );
        b.add_ellipsoid(Vec3::new(0.0, 0.265, 0.0), Vec3::new(0.018, 0.014, 0.018), 10, 6, matte_blk, MAT_MATTE);
        // Smooth curved handle + slender gooseneck pouring spout
        b.add_cylinder(Vec3::new(0.072, 0.20, 0.0), Vec3::new(0.135, 0.19, 0.0), 0.009, 0.009, 8, matte_blk, MAT_MATTE, true);
        b.add_cylinder(Vec3::new(0.135, 0.19, 0.0), Vec3::new(0.145, 0.06, 0.0), 0.010, 0.009, 8, matte_blk, MAT_MATTE, true);
        b.add_cylinder(Vec3::new(0.080, 0.06, 0.0), Vec3::new(0.145, 0.06, 0.0), 0.009, 0.009, 8, matte_blk, MAT_MATTE, true);
        b.add_cylinder(Vec3::new(-0.078, 0.09, 0.0), Vec3::new(-0.135, 0.18, 0.0), 0.013, 0.008, 10, chrome, MAT_ALLOY, false);
        b.add_cylinder(Vec3::new(-0.135, 0.18, 0.0), Vec3::new(-0.158, 0.225, 0.0), 0.008, 0.006, 10, chrome, MAT_ALLOY, false);
    });
}

/// Sculpted Japanese Black Pine Bonsai in a Smooth Glazed Stoneware Pot (smooth ellipsoid cloud pads!).
pub fn bonsai_planter(buf: &mut MeshBuffer, pose: Pose) {
    buf.with_pose(pose, |b| {
        let cedar_bark = [0.32, 0.20, 0.12];
        // Glazed indigo-slate oval/rounded bonsai pot + soft green moss cushion
        b.add_rounded_box(Vec3::new(-0.19, 0.0, -0.14), Vec3::new(0.19, 0.082, 0.14), 0.026, [0.16, 0.22, 0.29], MAT_CERAMIC);
        b.add_ellipsoid(Vec3::new(0.0, 0.076, 0.0), Vec3::new(0.155, 0.028, 0.110), 14, 6, [0.25, 0.48, 0.20], MAT_FOLIAGE);

        let b0 = Vec3::new(0.0, 0.08, 0.0);
        let b1 = Vec3::new(-0.06, 0.22, -0.02);
        let b2 = Vec3::new(0.04, 0.35, 0.02);
        let b3 = Vec3::new(-0.02, 0.45, -0.01);
        let br_l = Vec3::new(0.15, 0.28, -0.03);
        let br_r = Vec3::new(-0.14, 0.33, 0.04);

        b.add_cylinder(b0, b1, 0.028, 0.021, 10, cedar_bark, MAT_WOOD, false);
        b.add_cylinder(b1, b2, 0.021, 0.015, 10, cedar_bark, MAT_WOOD, false);
        b.add_cylinder(b2, b3, 0.015, 0.009, 9, cedar_bark, MAT_WOOD, false);
        b.add_cylinder(b1, br_l, 0.014, 0.008, 8, cedar_bark, MAT_WOOD, false);
        b.add_cylinder(b2, br_r, 0.013, 0.008, 8, cedar_bark, MAT_WOOD, false);

        // Smooth spherical-normal cloud pads — completely eliminates low-poly hexagon tree artifacts!
        let pads = [
            (br_l, Vec3::new(0.115, 0.042, 0.095), [0.17, 0.43, 0.20]),
            (br_r, Vec3::new(0.110, 0.040, 0.092), [0.15, 0.39, 0.18]),
            (b2.add(Vec3::new(0.04, 0.02, -0.03)), Vec3::new(0.102, 0.038, 0.088), [0.19, 0.46, 0.22]),
            (b3.add(Vec3::new(0.0, 0.02, 0.0)), Vec3::new(0.125, 0.046, 0.105), [0.21, 0.50, 0.24]),
        ];
        for &(center, radii, col) in &pads {
            b.add_ellipsoid(center, radii, 14, 8, col, MAT_FOLIAGE);
        }
    });
}

/// Sculpted Smooth-Porcelain Japanese Washlet Toilet (`Pose`-placed!).
pub fn washlet_toilet(buf: &mut MeshBuffer, pose: Pose) {
    buf.with_pose(pose, |b| {
        let porcelain = [0.95, 0.96, 0.95];
        let chrome = [0.84, 0.86, 0.89];
        // Smooth lathed porcelain bowl & base pedestal
        b.add_lathe_y(
            Vec3::new(0.0, 0.0, 0.08),
            &[(0.0, 0.135), (0.16, 0.145), (0.31, 0.205), (0.39, 0.215), (0.41, 0.19)],
            20,
            porcelain,
            MAT_CERAMIC,
            true,
        );
        // Smooth rounded cistern tank + flush button + washlet control arm + rounded seat lid!
        b.add_rounded_box(Vec3::new(-0.195, 0.36, -0.28), Vec3::new(0.195, 0.77, -0.06), 0.036, porcelain, MAT_CERAMIC);
        b.add_cylinder(Vec3::new(0.0, 0.77, -0.17), Vec3::new(0.0, 0.785, -0.17), 0.028, 0.026, 12, chrome, MAT_ALLOY, true);
        b.add_rounded_box(Vec3::new(-0.19, 0.405, -0.06), Vec3::new(0.19, 0.445, 0.29), 0.018, porcelain, MAT_CERAMIC);
        b.add_rounded_box(Vec3::new(0.19, 0.39, -0.04), Vec3::new(0.28, 0.43, 0.18), 0.012, porcelain, MAT_CERAMIC);
        // Tiny green/cyan washlet status LEDs
        b.add_box(Vec3::new(0.22, 0.431, 0.08), Vec3::new(0.25, 0.435, 0.11), [0.20, 0.88, 0.65], MAT_CRT);
    });
}

/// Reusable Japanese Outdoor Split-AC Condenser Unit (used on BOTH our balcony AND the 2-story house across the street — ref `image-8`!).
pub fn outdoor_ac_condenser(buf: &mut MeshBuffer, pose: Pose) {
    buf.with_pose(pose, |b| {
        let ivory: [f32; 3] = [0.85, 0.84, 0.79];
        let dark = [0.18, 0.20, 0.22];
        b.add_rounded_box(Vec3::new(-0.40, 0.0, -0.18), Vec3::new(0.40, 0.58, 0.18), 0.026, ivory, MAT_ALLOY);
        // Circular recessed fan shroud on `-Z` front face + center fan hub (exact look of `image-8, 9, 10`!)
        b.add_disc_neg_z(Vec3::new(-0.10, 0.29, -0.182), 0.20, 18, dark, MAT_ALLOY);
        b.add_disc_neg_z(Vec3::new(-0.10, 0.29, -0.186), 0.055, 12, [0.55, 0.56, 0.54], MAT_ALLOY);
        // Louver ring circles / nameplate badge on right
        b.add_box(Vec3::new(0.18, 0.42, -0.183), Vec3::new(0.32, 0.47, -0.178), [0.46, 0.48, 0.50], MAT_ALLOY);
        // Mounting feet
        b.add_box(Vec3::new(-0.34, -0.04, -0.16), Vec3::new(-0.26, 0.01, 0.16), [0.35, 0.36, 0.38], MAT_ALLOY);
        b.add_box(Vec3::new(0.26, -0.04, -0.16), Vec3::new(0.34, 0.01, 0.16), [0.35, 0.36, 0.38], MAT_ALLOY);
    });
}

/// Terracotta Street Planter Pot with Sunlit Green Foliage (`image-8, 9, 10, 11` street curb detail!).
pub fn terracotta_planter(buf: &mut MeshBuffer, pose: Pose) {
    buf.with_pose(pose, |b| {
        let clay = [0.74, 0.36, 0.19];
        b.add_lathe_y(
            Vec3::ZERO,
            &[(0.0, 0.14), (0.28, 0.20), (0.31, 0.22), (0.35, 0.21)],
            12,
            clay,
            MAT_CERAMIC,
            true,
        );
        b.add_ellipsoid(Vec3::new(0.0, 0.44, 0.0), Vec3::new(0.22, 0.16, 0.20), 10, 6, [0.22, 0.50, 0.20], MAT_FOLIAGE);
        b.add_ellipsoid(Vec3::new(0.07, 0.56, -0.04), Vec3::new(0.14, 0.14, 0.14), 9, 6, [0.28, 0.56, 0.22], MAT_FOLIAGE);
    });
}

/// Iconic Blue Japanese Street Bicycle (`Mamachari` parked outside house in `image-8, 9, 10, 11`!).
pub fn parked_bicycle(buf: &mut MeshBuffer, pose: Pose) {
    buf.with_pose(pose, |b| {
        let tire = [0.12, 0.13, 0.14];
        let rim = [0.78, 0.80, 0.82];
        let frame_blue = [0.24, 0.48, 0.62];
        let saddle_tan = [0.58, 0.34, 0.18];

        // Front (`x = -0.52`) & Rear (`x = +0.52`) 12-segment wheels (`r = 0.33`)
        for &wx in &[-0.52_f32, 0.52] {
            for i in 0..12 {
                let a0 = (i as f64) * TAU / 12.0;
                let a1 = ((i + 1) as f64) * TAU / 12.0;
                let p0 = Vec3::new(wx + 0.33 * (dcos(a0) as f32), 0.33 + 0.33 * (dsin(a0) as f32), 0.0);
                let p1 = Vec3::new(wx + 0.33 * (dcos(a1) as f32), 0.33 + 0.33 * (dsin(a1) as f32), 0.0);
                b.add_cylinder(p0, p1, 0.022, 0.022, 5, tire, MAT_MATTE, false);
            }
            b.add_cylinder(Vec3::new(wx, 0.33, -0.02), Vec3::new(wx, 0.33, 0.02), 0.03, 0.03, 6, rim, MAT_ALLOY, true);
        }
        // Step-through steel tube frame + fork + handlebars + tan saddle!
        let crank = Vec3::new(0.05, 0.28, 0.0);
        let seat_top = Vec3::new(0.24, 0.78, 0.0);
        let head_top = Vec3::new(-0.38, 0.82, 0.0);
        b.add_cylinder(Vec3::new(-0.52, 0.33, 0.0), head_top, 0.016, 0.016, 6, frame_blue, MAT_ALLOY, false);
        b.add_cylinder(head_top.sub(Vec3::new(0.0, 0.14, 0.0)), crank, 0.017, 0.017, 6, frame_blue, MAT_ALLOY, false);
        b.add_cylinder(crank, seat_top, 0.016, 0.016, 6, frame_blue, MAT_ALLOY, false);
        b.add_cylinder(crank, Vec3::new(0.52, 0.33, 0.0), 0.014, 0.014, 6, frame_blue, MAT_ALLOY, false);
        b.add_cylinder(seat_top.sub(Vec3::new(0.0, 0.10, 0.0)), Vec3::new(0.52, 0.33, 0.0), 0.013, 0.013, 6, frame_blue, MAT_ALLOY, false);
        //Swept handlebars + leather saddle
        b.add_cylinder(Vec3::new(-0.36, 0.94, -0.22), Vec3::new(-0.36, 0.94, 0.22), 0.013, 0.013, 6, rim, MAT_ALLOY, true);
        b.add_cylinder(head_top, Vec3::new(-0.36, 0.94, 0.0), 0.014, 0.014, 6, rim, MAT_ALLOY, false);
        b.add_ellipsoid(Vec3::new(0.25, 0.81, 0.0), Vec3::new(0.11, 0.035, 0.075), 8, 5, saddle_tan, MAT_MATTE);
    });
}
