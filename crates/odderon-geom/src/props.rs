//! Modular Local-Space 3D Props Library (`Pose`-placed, reusable across rooms, streets & stations!).
//! Every prop is authored around local origin `(0, 0, 0)` (`y = 0` resting surface, `-Z` facing viewer)
//! using smooth-normal rounded boxes, ellipsoids, cylinders, and lathes so nothing looks blocky.

use crate::{
    MeshBuffer, Pose, MAT_ALLOY, MAT_CERAMIC, MAT_CORRUGATED, MAT_CRT, MAT_EMISSIVE_WARM,
    MAT_FOLIAGE, MAT_MATTE, MAT_WOOD,
};
use odderon_dmath::{dcos, dsin, Vec3, TAU};

/// Sculpted Retro-Futuristic CRT Vector Telemetry Oscilloscope (smooth rounded housing + rear tube + rotary dials).
pub fn crt_oscilloscope(buf: &mut MeshBuffer, pose: Pose) {
    buf.with_pose(pose, |b| {
        let alloy_body = [0.58, 0.57, 0.53]; // Warm aged Tektronix/Japanese lab satin grey
        let alloy_dark = [0.13, 0.15, 0.17];
        let chrome = [0.76, 0.78, 0.81];

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
        let base_col = [0.14, 0.16, 0.18];
        let chrome = [0.76, 0.78, 0.80];
        let brass = [0.72, 0.54, 0.22];
        let sage_enamel = [0.16, 0.28, 0.23];

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

/// Sculpted Japanese Black Pine Bonsai in a Smooth Glazed Stoneware Pot (realistic dark pine needle pads).
pub fn bonsai_planter(buf: &mut MeshBuffer, pose: Pose) {
    buf.with_pose(pose, |b| {
        let cedar_bark = [0.20, 0.12, 0.07];
        // Glazed Tokoname stoneware pot + rich dark moss cushion
        b.add_rounded_box(Vec3::new(-0.19, 0.002, -0.14), Vec3::new(0.19, 0.082, 0.14), 0.026, [0.14, 0.17, 0.21], MAT_CERAMIC);
        b.add_ellipsoid(Vec3::new(0.0, 0.078, 0.0), Vec3::new(0.155, 0.026, 0.110), 14, 6, [0.14, 0.26, 0.10], MAT_FOLIAGE);

        let b0 = Vec3::new(0.0, 0.08, 0.0);
        let b1 = Vec3::new(-0.06, 0.22, -0.02);
        let b2 = Vec3::new(0.04, 0.35, 0.02);
        let b3 = Vec3::new(-0.02, 0.44, -0.01);
        let br_l = Vec3::new(0.15, 0.28, -0.03);
        let br_r = Vec3::new(-0.14, 0.33, 0.04);

        b.add_cylinder(b0, b1, 0.028, 0.021, 10, cedar_bark, MAT_WOOD, false);
        b.add_cylinder(b1, b2, 0.021, 0.015, 10, cedar_bark, MAT_WOOD, false);
        b.add_cylinder(b2, b3, 0.015, 0.009, 9, cedar_bark, MAT_WOOD, false);
        b.add_cylinder(b1, br_l, 0.014, 0.008, 8, cedar_bark, MAT_WOOD, false);
        b.add_cylinder(b2, br_r, 0.013, 0.008, 8, cedar_bark, MAT_WOOD, false);

        let pads = [
            (br_l, Vec3::new(0.120, 0.034, 0.095), [0.09, 0.21, 0.09]),
            (br_r, Vec3::new(0.115, 0.032, 0.092), [0.08, 0.19, 0.08]),
            (b2.add(Vec3::new(0.04, 0.02, -0.03)), Vec3::new(0.105, 0.030, 0.088), [0.10, 0.23, 0.10]),
            (b3.add(Vec3::new(0.0, 0.02, 0.0)), Vec3::new(0.130, 0.038, 0.105), [0.12, 0.26, 0.11]),
        ];
        for &(center, radii, col) in &pads {
            b.add_ellipsoid(center, radii, 14, 8, col, MAT_FOLIAGE);
        }
    });
}

/// Sculpted Japanese TOTO-Style Washlet Toilet with Hand-Wash Cistern Spout (`手洗い付きウォシュレット`) & Supply Plumbing!
pub fn washlet_toilet(buf: &mut MeshBuffer, pose: Pose) {
    buf.with_pose(pose, |b| {
        let porcelain = [0.91, 0.92, 0.90];
        let porcelain_inner = [0.80, 0.84, 0.86];
        let water_tint = [0.58, 0.76, 0.82];
        let chrome = [0.78, 0.80, 0.83];
        let dark_valve = [0.24, 0.26, 0.29];

        // 1. Contoured porcelain pedestal foot + sculpted elongated bowl + rear trapway skirt
        b.add_rounded_box(Vec3::new(-0.145, 0.001, -0.24), Vec3::new(0.145, 0.042, 0.24), 0.018, porcelain, MAT_CERAMIC);
        b.add_rounded_box(Vec3::new(-0.135, 0.042, -0.26), Vec3::new(0.135, 0.36, -0.02), 0.028, porcelain, MAT_CERAMIC);
        b.add_lathe_y(
            Vec3::new(0.0, 0.040, 0.09),
            &[(0.0, 0.125), (0.14, 0.140), (0.28, 0.205), (0.355, 0.218), (0.382, 0.206)],
            20,
            porcelain,
            MAT_CERAMIC,
            true,
        );
        // Inner glazed water basin & clean water seal (`strictly stacked Y offsets!`)
        b.add_ellipsoid(Vec3::new(0.0, 0.376, 0.10), Vec3::new(0.145, 0.014, 0.165), 14, 6, porcelain_inner, MAT_CERAMIC);
        b.add_ellipsoid(Vec3::new(0.0, 0.384, 0.09), Vec3::new(0.105, 0.008, 0.120), 12, 5, water_tint, MAT_ALLOY);

        // 2. Heated contoured Washlet seat ring + rear nozzle/hinge housing + closed/slightly-sculpted soft lid
        b.add_rounded_box(Vec3::new(-0.185, 0.388, -0.10), Vec3::new(0.185, 0.432, 0.02), 0.016, porcelain, MAT_CERAMIC);
        // Seat side rails & front bridge forming a realistic open oval seat ring!
        b.add_rounded_box(Vec3::new(-0.192, 0.392, 0.01), Vec3::new(-0.122, 0.422, 0.28), 0.014, porcelain, MAT_CERAMIC);
        b.add_rounded_box(Vec3::new(0.122, 0.392, 0.01), Vec3::new(0.192, 0.422, 0.28), 0.014, porcelain, MAT_CERAMIC);
        b.add_rounded_box(Vec3::new(-0.175, 0.392, 0.22), Vec3::new(0.175, 0.422, 0.305), 0.014, porcelain, MAT_CERAMIC);
        // Upright open contoured lid resting against tank (`classic inviting Japanese hotel/apartment presentation!`)
        b.add_rounded_box(Vec3::new(-0.188, 0.432, -0.085), Vec3::new(0.188, 0.815, -0.052), 0.016, porcelain, MAT_CERAMIC);

        // 3. Rear Cistern Tank + Iconic Japanese Top Hand-Wash Basin & Curved Chrome Spout (`手洗い管`)!
        b.add_rounded_box(Vec3::new(-0.198, 0.365, -0.30), Vec3::new(0.198, 0.775, -0.095), 0.032, porcelain, MAT_CERAMIC);
        b.add_ellipsoid(Vec3::new(0.0, 0.776, -0.195), Vec3::new(0.135, 0.012, 0.068), 12, 5, porcelain_inner, MAT_CERAMIC);
        b.add_cylinder(Vec3::new(0.0, 0.776, -0.235), Vec3::new(0.0, 0.855, -0.225), 0.010, 0.009, 10, chrome, MAT_ALLOY, false);
        b.add_cylinder(Vec3::new(0.0, 0.855, -0.225), Vec3::new(0.0, 0.840, -0.175), 0.009, 0.008, 10, chrome, MAT_ALLOY, true);
        // Side chrome flush handle (`大 / 小`)
        b.add_cylinder(Vec3::new(0.198, 0.69, -0.19), Vec3::new(0.238, 0.69, -0.19), 0.012, 0.010, 8, chrome, MAT_ALLOY, true);
        b.add_cylinder(Vec3::new(0.232, 0.69, -0.19), Vec3::new(0.242, 0.675, -0.13), 0.007, 0.006, 8, chrome, MAT_ALLOY, true);

        // 4. Side Washlet Electronic Module + Braided Stainless Water Supply Line & Shutoff Valve
        b.add_rounded_box(Vec3::new(-0.255, 0.388, -0.06), Vec3::new(-0.186, 0.428, 0.14), 0.010, porcelain, MAT_CERAMIC);
        b.add_box(Vec3::new(-0.242, 0.429, 0.04), Vec3::new(-0.215, 0.434, 0.08), [0.22, 0.88, 0.56], MAT_EMISSIVE_WARM);
        b.add_cylinder(Vec3::new(-0.22, 0.18, -0.34), Vec3::new(-0.22, 0.18, -0.26), 0.014, 0.014, 8, dark_valve, MAT_ALLOY, true);
        b.add_cylinder(Vec3::new(-0.22, 0.18, -0.27), Vec3::new(-0.16, 0.37, -0.21), 0.007, 0.007, 8, chrome, MAT_ALLOY, false);
    });
}

/// Japanese Unit-Bath Chrome Shower Slide-Rail, Thermostatic Mixer, Hinoki Bath Stool & Amenity Bottles!
pub fn unit_bath_shower_set(buf: &mut MeshBuffer, pose: Pose) {
    buf.with_pose(pose, |b| {
        let chrome = [0.78, 0.80, 0.83];
        let hinoki = [0.56, 0.40, 0.23];
        let hose_col = [0.62, 0.64, 0.67];

        // Vertical chrome shower slide bar (`y = 0.88 .. 1.86`) + wall standoffs
        b.add_cylinder(Vec3::new(0.0, 0.88, 0.06), Vec3::new(0.0, 1.86, 0.06), 0.011, 0.011, 10, chrome, MAT_ALLOY, true);
        for &sy in &[0.92_f32, 1.82] {
            b.add_cylinder(Vec3::new(0.0, sy, 0.0), Vec3::new(0.0, sy, 0.06), 0.013, 0.013, 8, chrome, MAT_ALLOY, false);
        }
        // Handheld angled shower wand & spray head (`y = 1.66`)
        b.add_cylinder(Vec3::new(0.0, 1.60, 0.06), Vec3::new(0.0, 1.73, 0.14), 0.012, 0.016, 10, chrome, MAT_ALLOY, false);
        b.add_ellipsoid(Vec3::new(0.0, 1.75, 0.16), Vec3::new(0.044, 0.018, 0.044), 12, 6, chrome, MAT_ALLOY);

        // Horizontal thermostatic mixer bar (`y = 0.74`) with hot (red) & cold (blue) indicator rings!
        b.add_cylinder(Vec3::new(-0.13, 0.74, 0.07), Vec3::new(0.13, 0.74, 0.07), 0.018, 0.018, 12, chrome, MAT_ALLOY, true);
        b.add_cylinder(Vec3::new(-0.125, 0.74, 0.07), Vec3::new(-0.115, 0.74, 0.07), 0.019, 0.019, 10, [0.84, 0.20, 0.16], MAT_MATTE, false);
        b.add_cylinder(Vec3::new(0.115, 0.74, 0.07), Vec3::new(0.125, 0.74, 0.07), 0.019, 0.019, 10, [0.16, 0.46, 0.86], MAT_MATTE, false);
        b.add_cylinder(Vec3::new(0.0, 0.74, 0.07), Vec3::new(0.0, 0.67, 0.13), 0.010, 0.008, 8, chrome, MAT_ALLOY, true);

        // Flexible drooping shower hose from mixer to shower wand
        b.add_cylinder(Vec3::new(0.03, 0.73, 0.07), Vec3::new(0.06, 0.56, 0.09), 0.007, 0.007, 6, hose_col, MAT_ALLOY, false);
        b.add_cylinder(Vec3::new(0.06, 0.56, 0.09), Vec3::new(0.02, 1.60, 0.06), 0.007, 0.007, 6, hose_col, MAT_ALLOY, false);

        // Corner amenity shelf + amber shampoo & sage body-soap pump bottles
        b.add_rounded_box(Vec3::new(0.16, 0.96, 0.005), Vec3::new(0.38, 0.985, 0.14), 0.006, [0.82, 0.84, 0.83], MAT_CERAMIC);
        for &(bx, bcol) in &[(0.22_f32, [0.62, 0.34, 0.12]), (0.31, [0.18, 0.38, 0.28])] {
            b.add_lathe_y(Vec3::new(bx, 0.986, 0.07), &[(0.0, 0.028), (0.11, 0.030), (0.14, 0.012)], 10, bcol, MAT_CERAMIC, true);
            b.add_cylinder(Vec3::new(bx, 1.126, 0.07), Vec3::new(bx, 1.155, 0.09), 0.005, 0.004, 6, [0.88, 0.88, 0.86], MAT_MATTE, true);
        }

        // Low Japanese Hinoki wooden bath stool (`furo-isu`) + washbasin (`senmenki`) on floor!
        b.add_rounded_box(Vec3::new(-0.14, 0.18, 0.20), Vec3::new(0.14, 0.215, 0.40), 0.008, hinoki, MAT_WOOD);
        b.add_rounded_box(Vec3::new(-0.13, 0.002, 0.21), Vec3::new(-0.10, 0.182, 0.39), 0.005, hinoki, MAT_WOOD);
        b.add_rounded_box(Vec3::new(0.10, 0.002, 0.21), Vec3::new(0.13, 0.182, 0.39), 0.005, hinoki, MAT_WOOD);
        b.add_lathe_y(Vec3::new(0.0, 0.216, 0.30), &[(0.0, 0.075), (0.075, 0.105)], 14, hinoki, MAT_WOOD, true);
    });
}

/// Reusable Japanese Outdoor Split-AC Condenser Unit (weathered ivory metal + recessed dark louver grille — ref `image-15`!).
pub fn outdoor_ac_condenser(buf: &mut MeshBuffer, pose: Pose) {
    buf.with_pose(pose, |b| {
        let ivory: [f32; 3] = [0.70, 0.68, 0.63];
        let dark = [0.10, 0.11, 0.13];
        b.add_rounded_box(Vec3::new(-0.42, 0.01, -0.18), Vec3::new(0.42, 0.60, 0.18), 0.024, ivory, MAT_ALLOY);
        // Circular recessed fan shroud on `-Z` front face (`8 mm` proud so zero Z-fighting!)
        b.add_disc_neg_z(Vec3::new(-0.10, 0.30, -0.188), 0.21, 18, dark, MAT_ALLOY);
        b.add_disc_neg_z(Vec3::new(-0.10, 0.30, -0.194), 0.055, 12, [0.36, 0.37, 0.36], MAT_ALLOY);
        b.add_box(Vec3::new(0.18, 0.44, -0.189), Vec3::new(0.33, 0.49, -0.182), [0.32, 0.34, 0.36], MAT_ALLOY);
        b.add_box(Vec3::new(-0.35, -0.04, -0.16), Vec3::new(-0.27, 0.015, 0.16), [0.22, 0.23, 0.25], MAT_ALLOY);
        b.add_box(Vec3::new(0.27, -0.04, -0.16), Vec3::new(0.35, 0.015, 0.16), [0.22, 0.23, 0.25], MAT_ALLOY);
    });
}

/// Weathered Terracotta Street Planter Pot with Deep Olive-Green Shrubbery (`image-15` street level!).
pub fn terracotta_planter(buf: &mut MeshBuffer, pose: Pose) {
    buf.with_pose(pose, |b| {
        let clay = [0.48, 0.23, 0.12];
        b.add_lathe_y(
            Vec3::ZERO,
            &[(0.0, 0.14), (0.28, 0.20), (0.31, 0.22), (0.35, 0.21)],
            12,
            clay,
            MAT_CERAMIC,
            true,
        );
        b.add_ellipsoid(Vec3::new(-0.04, 0.42, 0.02), Vec3::new(0.20, 0.13, 0.18), 10, 6, [0.11, 0.25, 0.09], MAT_FOLIAGE);
        b.add_ellipsoid(Vec3::new(0.08, 0.48, -0.04), Vec3::new(0.15, 0.12, 0.14), 9, 6, [0.16, 0.31, 0.11], MAT_FOLIAGE);
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

/// Iconic Japanese Roadside Beverage Vending Machine (`自動販売機`, faces `+Z` toward street!).
pub fn japanese_vending_machine(buf: &mut MeshBuffer, pose: Pose) {
    buf.with_pose(pose, |b| {
        let cabinet_white = [0.84, 0.84, 0.81];
        let accent_red = [0.72, 0.16, 0.14];
        let dark_slot = [0.10, 0.12, 0.14];

        b.add_rounded_box(Vec3::new(-0.50, 0.01, -0.32), Vec3::new(0.50, 1.84, 0.32), 0.028, cabinet_white, MAT_ALLOY);
        b.add_box(Vec3::new(-0.502, 0.01, -0.322), Vec3::new(0.502, 0.24, 0.322), accent_red, MAT_ALLOY);
        // Illuminated drink sample showcase (`+Z` face, proud `8 mm`)
        b.add_box(Vec3::new(-0.42, 0.86, 0.322), Vec3::new(0.24, 1.70, 0.334), [0.96, 0.94, 0.86], MAT_EMISSIVE_WARM);
        // Two shelves of miniature canned coffee / green tea / citrus sodas!
        let can_cols = [
            [0.42, 0.22, 0.10],
            [0.16, 0.46, 0.22],
            [0.14, 0.36, 0.68],
            [0.82, 0.48, 0.12],
            [0.76, 0.16, 0.16],
        ];
        for (idx, &col) in can_cols.iter().enumerate() {
            let cx = -0.34 + (idx as f32) * 0.125;
            b.add_cylinder(Vec3::new(cx, 1.34, 0.336), Vec3::new(cx, 1.46, 0.336), 0.024, 0.024, 8, col, MAT_ALLOY, true);
            b.add_cylinder(Vec3::new(cx, 0.98, 0.336), Vec3::new(cx, 1.10, 0.336), 0.024, 0.024, 8, col, MAT_ALLOY, true);
        }
        // IC-card / coin slot panel & dark bottom retrieval flap
        b.add_box(Vec3::new(0.28, 0.90, 0.322), Vec3::new(0.43, 1.48, 0.336), dark_slot, MAT_ALLOY);
        b.add_box(Vec3::new(0.31, 1.32, 0.337), Vec3::new(0.40, 1.39, 0.344), [0.24, 0.88, 0.62], MAT_EMISSIVE_WARM);
        b.add_rounded_box(Vec3::new(-0.36, 0.32, 0.322), Vec3::new(0.22, 0.56, 0.340), 0.010, dark_slot, MAT_ALLOY);
    });
}

/// Rural Japanese Coastal Bus Stop Bench & Round `バス停` Timetable Sign (`Uchinoura Space Center Line`!).
pub fn bus_stop_shelter(buf: &mut MeshBuffer, pose: Pose) {
    buf.with_pose(pose, |b| {
        let cedar_dark = [0.16, 0.09, 0.05];
        let hinoki = [0.48, 0.33, 0.18];
        let steel = [0.46, 0.49, 0.52];

        // Weathered wooden waiting bench (`x in [-0.85, +0.45]`)
        b.add_rounded_box(Vec3::new(-0.85, 0.42, -0.22), Vec3::new(0.45, 0.47, 0.22), 0.010, hinoki, MAT_WOOD);
        b.add_rounded_box(Vec3::new(-0.85, 0.56, -0.22), Vec3::new(0.45, 0.84, -0.17), 0.010, hinoki, MAT_WOOD);
        for &lx in &[-0.74_f32, 0.34] {
            b.add_box(Vec3::new(lx - 0.03, 0.002, -0.19), Vec3::new(lx + 0.03, 0.42, 0.19), cedar_dark, MAT_WOOD);
            b.add_box(Vec3::new(lx - 0.025, 0.42, -0.21), Vec3::new(lx + 0.025, 0.84, -0.16), cedar_dark, MAT_WOOD);
        }

        // Small angled corrugated-tin rain canopy over bench
        b.add_cylinder(Vec3::new(-0.80, 0.002, -0.24), Vec3::new(-0.80, 2.18, -0.24), 0.028, 0.028, 8, cedar_dark, MAT_WOOD, false);
        b.add_cylinder(Vec3::new(0.40, 0.002, -0.24), Vec3::new(0.40, 2.18, -0.24), 0.028, 0.028, 8, cedar_dark, MAT_WOOD, false);
        b.add_box(Vec3::new(-0.96, 2.14, -0.36), Vec3::new(0.56, 2.22, 0.46), [0.24, 0.29, 0.34], MAT_CORRUGATED);

        // Iconic Japanese `バス停` (Bus Stop) pole (`x = +0.85`): heavy cast-iron base, timetable plaque & round blue/white top disc!
        b.add_lathe_y(Vec3::new(0.85, 0.002, 0.12), &[(0.0, 0.17), (0.08, 0.15), (0.12, 0.035)], 12, [0.22, 0.24, 0.26], MAT_ALLOY, true);
        b.add_cylinder(Vec3::new(0.85, 0.12, 0.12), Vec3::new(0.85, 1.98, 0.12), 0.020, 0.020, 8, steel, MAT_ALLOY, false);
        // Timetable schedule frame (`時刻表`)
        b.add_rounded_box(Vec3::new(0.69, 1.10, 0.095), Vec3::new(1.01, 1.56, 0.145), 0.008, [0.88, 0.87, 0.83], MAT_MATTE);
        b.add_box(Vec3::new(0.71, 1.46, 0.146), Vec3::new(0.99, 1.53, 0.152), [0.12, 0.36, 0.68], MAT_MATTE);
        // Double-sided round Japanese Bus Stop disc (`y = 2.16`)
        b.add_cylinder(Vec3::new(0.85, 2.16, 0.095), Vec3::new(0.85, 2.16, 0.145), 0.19, 0.19, 16, [0.88, 0.89, 0.88], MAT_MATTE, true);
        b.add_disc_neg_z(Vec3::new(0.85, 2.16, 0.090), 0.15, 14, [0.12, 0.38, 0.72], MAT_ALLOY);
    });
}

/// Parametric Uchinoura Space Center Range Shuttle Microbus (`Mitsubishi Rosa` style, travels `-X` along street!).
pub fn uchinoura_shuttle_bus(buf: &mut MeshBuffer, pose: Pose, door_open: f32) {
    buf.with_pose(pose, |b| {
        let body_ivory = [0.86, 0.85, 0.81];
        let stripe_teal = [0.10, 0.36, 0.48];
        let dark_glass = [0.10, 0.14, 0.18];
        let bumper_dark = [0.14, 0.15, 0.17];
        let tire_blk = [0.09, 0.10, 0.11];
        let hub_alloy = [0.72, 0.74, 0.77];

        // Chassis & sculpted rounded microbus coachwork (`length = 6.2m` along X, `width = 2.06m` along Z)
        b.add_rounded_box(Vec3::new(-3.10, 0.36, -1.02), Vec3::new(3.10, 1.24, 1.02), 0.06, body_ivory, MAT_ALLOY);
        b.add_box(Vec3::new(-3.11, 0.78, -1.03), Vec3::new(3.11, 1.04, 1.03), stripe_teal, MAT_ALLOY);
        b.add_rounded_box(Vec3::new(-3.04, 1.22, -0.99), Vec3::new(3.04, 2.46, 0.99), 0.08, body_ivory, MAT_ALLOY);
        // Rooftop AC pod
        b.add_rounded_box(Vec3::new(-0.95, 2.46, -0.68), Vec3::new(1.15, 2.66, 0.68), 0.04, [0.78, 0.79, 0.77], MAT_ALLOY);

        // Panoramic tinted passenger windows (`-Z` curb side & `+Z` street side) + front `-X` windshield!
        b.add_box(Vec3::new(-1.35, 1.32, -1.002), Vec3::new(2.82, 2.16, -0.97), dark_glass, MAT_ALLOY);
        b.add_box(Vec3::new(-2.82, 1.32, 0.97), Vec3::new(2.82, 2.16, 1.002), dark_glass, MAT_ALLOY);
        b.add_box(Vec3::new(-3.055, 1.28, -0.90), Vec3::new(-3.01, 2.20, 0.90), dark_glass, MAT_ALLOY);
        // Illuminated LED destination header (`UCHINOURA PAD-1 / 内之浦射場`)
        b.add_box(Vec3::new(-3.06, 2.24, -0.58), Vec3::new(-3.01, 2.38, 0.58), [1.0, 0.68, 0.22], MAT_EMISSIVE_WARM);

        // Curb-side (`-Z`) animated bi-fold boarding doorway (`x in [-2.38, -1.52]`)!
        b.add_box(Vec3::new(-2.38, 0.40, -1.025), Vec3::new(-1.52, 2.18, -0.82), [0.12, 0.13, 0.15], MAT_ALLOY);
        let slide = door_open.clamp(0.0, 1.0) * 0.72;
        b.add_rounded_box(
            Vec3::new(-2.36 + slide, 0.42, -1.035),
            Vec3::new(-1.54, 2.16, -0.995),
            0.010,
            [0.22, 0.28, 0.32],
            MAT_ALLOY,
        );

        // Front & rear bumpers + warm headlights (`-X`) & red taillights (`+X`)
        b.add_rounded_box(Vec3::new(-3.16, 0.32, -1.04), Vec3::new(-2.98, 0.54, 1.04), 0.03, bumper_dark, MAT_ALLOY);
        b.add_rounded_box(Vec3::new(2.98, 0.32, -1.04), Vec3::new(3.16, 0.54, 1.04), 0.03, bumper_dark, MAT_ALLOY);
        for &lz in &[-0.76_f32, 0.76] {
            b.add_box(Vec3::new(-3.14, 0.60, lz - 0.14), Vec3::new(-3.09, 0.74, lz + 0.14), [1.0, 0.94, 0.76], MAT_EMISSIVE_WARM);
            b.add_box(Vec3::new(3.09, 0.60, lz - 0.14), Vec3::new(3.14, 0.76, lz + 0.14), [0.92, 0.16, 0.12], MAT_EMISSIVE_WARM);
        }

        // Four rubber wheels + alloy hubs (`r = 0.38`)
        for &wx in &[-1.92_f32, 1.78] {
            for &wz in &[-0.94_f32, 0.94] {
                let z_out = if wz < 0.0 { wz - 0.12 } else { wz + 0.12 };
                b.add_cylinder(Vec3::new(wx, 0.38, wz), Vec3::new(wx, 0.38, z_out), 0.38, 0.38, 14, tire_blk, MAT_MATTE, true);
                let z_hub = if wz < 0.0 { z_out - 0.008 } else { z_out + 0.008 };
                b.add_cylinder(Vec3::new(wx, 0.38, z_out), Vec3::new(wx, 0.38, z_hub), 0.21, 0.21, 12, hub_alloy, MAT_ALLOY, true);
            }
        }
    });
}
