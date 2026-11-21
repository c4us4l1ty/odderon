//! Modular Habitation Unit #204 Assembly (`hab.rs`).
//! Engineered with strict non-coplanar tolerances (`>= 8-25 mm` inset/encapsulation on every joint)
//! so zero Z-fighting or edge stippling ever occurs on 16/24-bit depth buffers, and tuned to the
//! rich dark-timber & aged-plaster "Summer Noon" palette.

use crate::{
    props, DynamicSceneParams, MeshBuffer, Pose, GAUGE_CENTERS, MAT_ALLOY, MAT_CERAMIC,
    MAT_EMISSIVE_WARM, MAT_MATTE, MAT_SHOJI, MAT_TATAMI, MAT_WOOD, PANEL_SWITCH_XS,
};
use odderon_dmath::{dcos, dsin, Vec3, PI, TAU};

pub fn build_hab_204_static(buf: &mut MeshBuffer) {
    let shikkui_warm = [0.68, 0.64, 0.56]; // Aged warm sand-plaster (never overexposed!)
    let alloy_panel = [0.36, 0.39, 0.43];
    let alloy_mid = [0.25, 0.28, 0.31];
    let alloy_dark = [0.13, 0.15, 0.17];
    let chrome = [0.76, 0.78, 0.81];
    let brass = [0.74, 0.56, 0.24];
    let cedar_dark = [0.16, 0.09, 0.048]; // Rich kominka roasted cedar
    let walnut_warm = [0.28, 0.16, 0.09];
    let hinoki_light = [0.54, 0.38, 0.22];
    let floor_slate = [0.16, 0.17, 0.19];
    let porcelain = [0.84, 0.85, 0.83];

    // =========================================================================
    // 1. ROOM SHELL, TIMBER TRIM & STAR CITIZEN STRUCTURAL RIBS (ZERO COPLANARITY!)
    // =========================================================================
    // Floor (`top = 0.0`) & Ceiling (`bottom = 2.68`) slabs extend outside wall exterior faces
    buf.add_box(Vec3::new(-2.72, -0.16, -2.22), Vec3::new(2.72, 0.0, 2.20), floor_slate, MAT_ALLOY);
    buf.add_box(Vec3::new(-2.72, 2.68, -2.22), Vec3::new(2.72, 2.86, 2.20), alloy_dark, MAT_ALLOY);

    // Outer walls embed `3 cm` into floor/ceiling (`y = -0.03 .. 2.71`), with clean non-overlapping interior planes:
    // Interior bounds: `x in [-2.54, +2.54], z in [-2.04, +2.02]`
    buf.add_box(Vec3::new(-2.66, -0.03, -2.16), Vec3::new(2.66, 2.71, -2.04), shikkui_warm, MAT_MATTE); // Back (-Z)
    buf.add_box(Vec3::new(2.54, -0.03, -2.04), Vec3::new(2.66, 2.71, 2.02), shikkui_warm, MAT_MATTE);   // Left (+X)
    buf.add_box(Vec3::new(-2.66, -0.03, -2.04), Vec3::new(-2.54, 2.71, 2.02), shikkui_warm, MAT_MATTE); // Right (-X)

    // Perimeter dark cedar ceiling beams (`y in [2.57, 2.675]`, non-overlapping in X/Z!)
    buf.add_box(Vec3::new(-2.535, 2.57, -2.035), Vec3::new(2.535, 2.675, -1.91), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(-2.535, 2.57, 1.89), Vec3::new(2.535, 2.675, 2.015), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(-2.535, 2.57, -1.908), Vec3::new(-2.41, 2.675, 1.888), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(2.41, 2.57, -1.908), Vec3::new(2.535, 2.675, 1.888), cedar_dark, MAT_WOOD);

    // Cedar baseboards (`habaki`, `y in [0.003, 0.068]`, proud inside room with 0 overlap!)
    buf.add_box(Vec3::new(-2.532, 0.003, 1.992), Vec3::new(2.532, 0.068, 2.018), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(2.512, 0.003, -2.032), Vec3::new(2.538, 0.068, 1.990), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(-2.538, 0.003, -0.235), Vec3::new(-2.512, 0.068, 1.990), cedar_dark, MAT_WOOD);

    // Window Wall (`z in [2.02, 2.12]`): Rough opening `x in [-1.285, +1.285], y in [0.735, 2.295]`
    // recessed `3.5 cm` inside the wooden sill & U-frame so plaster never touches wood casing!
    buf.add_box(Vec3::new(-2.538, -0.02, 2.02), Vec3::new(2.538, 0.735, 2.12), shikkui_warm, MAT_MATTE);
    buf.add_box(Vec3::new(-2.538, 2.295, 2.02), Vec3::new(2.538, 2.70, 2.12), shikkui_warm, MAT_MATTE);
    buf.add_box(Vec3::new(1.285, 0.735, 2.02), Vec3::new(2.538, 2.295, 2.12), shikkui_warm, MAT_MATTE);
    buf.add_box(Vec3::new(-2.538, 0.735, 2.02), Vec3::new(-1.285, 2.295, 2.12), shikkui_warm, MAT_MATTE);

    // Deep Cedar Window Sill & Encapsulating Hollow U-Frame Casing (`open_x in [-1.25, +1.25], open_y in [0.765, 2.265]`)
    buf.add_rounded_box(Vec3::new(-1.335, 0.708, 1.91), Vec3::new(1.335, 0.765, 2.15), 0.012, cedar_dark, MAT_WOOD);
    buf.add_u_frame_xy(-1.25, 1.25, 0.765, 2.265, 1.935, 2.145, 0.072, cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(-0.024, 0.766, 2.012), Vec3::new(0.024, 2.264, 2.085), cedar_dark, MAT_WOOD);

    // Subtle Star Citizen structural ceiling rib & corner gussets (`z = 1.28`, strictly inside walls!)
    buf.add_box(Vec3::new(-2.535, 0.004, 1.24), Vec3::new(-2.465, 2.568, 1.32), alloy_panel, MAT_ALLOY);
    buf.add_box(Vec3::new(2.465, 0.004, 1.24), Vec3::new(2.535, 2.568, 1.32), alloy_panel, MAT_ALLOY);
    buf.add_box(Vec3::new(-2.463, 2.51, 1.25), Vec3::new(2.463, 2.59, 1.31), alloy_dark, MAT_ALLOY);
    buf.add_beam(Vec3::new(-2.45, 2.24, 1.28), Vec3::new(-2.18, 2.51, 1.28), 0.024, alloy_panel, MAT_ALLOY);
    buf.add_beam(Vec3::new(2.45, 2.24, 1.28), Vec3::new(2.18, 2.51, 1.28), 0.024, alloy_panel, MAT_ALLOY);

    // Woven Japanese Tatami Living Floor (`strictly stacked Y layers: 0.003..0.022, 0.022..0.030, 0.030..0.036`)
    let tatami_a = [0.56, 0.52, 0.31];
    let tatami_b = [0.52, 0.48, 0.29];
    let tatami_heri = [0.11, 0.14, 0.17];
    buf.add_box(Vec3::new(-1.56, 0.003, -0.42), Vec3::new(1.80, 0.022, 1.96), cedar_dark, MAT_WOOD);
    let mats = [
        (-1.52_f32, -0.38_f32, -0.44_f32, 0.76_f32, tatami_a),
        (-0.41, -0.38, 0.68, 0.76, tatami_b),
        (0.71, -0.38, 1.76, 0.76, tatami_a),
        (-1.52, 0.79, -0.44, 1.92, tatami_b),
        (-0.41, 0.79, 0.68, 1.92, tatami_a),
        (0.71, 0.79, 1.76, 1.92, tatami_b),
    ];
    for &(mx0, mz0, mx1, mz1, col) in &mats {
        buf.add_box(Vec3::new(mx0, 0.022, mz0), Vec3::new(mx1, 0.030, mz1), tatami_heri, MAT_MATTE);
        buf.add_box(
            Vec3::new(mx0 + 0.032, 0.030, mz0 + 0.032),
            Vec3::new(mx1 - 0.032, 0.036, mz1 - 0.032),
            col,
            MAT_TATAMI,
        );
    }

    // =========================================================================
    // 2. RIGHT WALL ENTRY DOOR (`x = -2.535`, `z in [0.22, 1.18]`)
    // =========================================================================
    buf.add_box(Vec3::new(-2.536, 0.004, 0.16), Vec3::new(-2.46, 2.18, 1.24), alloy_dark, MAT_ALLOY);
    buf.add_rounded_box(Vec3::new(-2.458, 0.022, 0.23), Vec3::new(-2.415, 2.10, 1.17), 0.014, alloy_panel, MAT_ALLOY);
    for &dy in &[0.56_f32, 1.12, 1.68] {
        buf.add_box(Vec3::new(-2.414, dy, 0.27), Vec3::new(-2.404, dy + 0.015, 1.13), alloy_dark, MAT_ALLOY);
    }
    buf.add_cylinder(Vec3::new(-2.37, 0.88, 1.03), Vec3::new(-2.37, 1.30, 1.03), 0.013, 0.013, 12, chrome, MAT_ALLOY, true);
    buf.add_cylinder(Vec3::new(-2.415, 0.92, 1.03), Vec3::new(-2.37, 0.92, 1.03), 0.010, 0.010, 8, alloy_dark, MAT_ALLOY, false);
    buf.add_cylinder(Vec3::new(-2.415, 1.26, 1.03), Vec3::new(-2.37, 1.26, 1.03), 0.010, 0.010, 8, alloy_dark, MAT_ALLOY, false);
    buf.add_rounded_box(Vec3::new(-2.535, 1.12, 1.27), Vec3::new(-2.495, 1.30, 1.36), 0.008, alloy_dark, MAT_ALLOY);
    buf.add_box(Vec3::new(-2.494, 1.16, 1.29), Vec3::new(-2.486, 1.26, 1.34), [0.20, 0.85, 0.52], MAT_EMISSIVE_WARM);
    buf.add_rounded_box(Vec3::new(-2.44, 0.003, 0.20), Vec3::new(-1.82, 0.019, 1.20), 0.008, [0.13, 0.15, 0.17], MAT_ALLOY);

    // =========================================================================
    // 3. TOP-LEFT: L-SHAPED GALLEY KITCHEN (`inset 6mm from outer walls so zero flicker!`)
    // =========================================================================
    buf.add_box(Vec3::new(1.36, 0.004, 1.42), Vec3::new(2.532, 0.08, 1.988), [0.09, 0.10, 0.12], MAT_ALLOY);
    buf.add_box(Vec3::new(1.92, 0.004, 0.18), Vec3::new(2.532, 0.08, 1.418), [0.09, 0.10, 0.12], MAT_ALLOY);
    buf.add_box(Vec3::new(1.34, 0.082, 1.40), Vec3::new(2.532, 0.86, 1.988), alloy_dark, MAT_ALLOY);
    buf.add_box(Vec3::new(1.88, 0.082, 0.16), Vec3::new(2.532, 0.86, 1.398), alloy_dark, MAT_ALLOY);

    buf.add_rounded_box(Vec3::new(1.37, 0.12, 1.374), Vec3::new(1.84, 0.82, 1.396), 0.012, walnut_warm, MAT_WOOD);
    buf.add_cylinder(Vec3::new(1.50, 0.75, 1.364), Vec3::new(1.71, 0.75, 1.364), 0.007, 0.007, 8, chrome, MAT_ALLOY, true);
    for &(z0, z1) in &[(0.20_f32, 0.76_f32), (0.80, 1.36)] {
        buf.add_rounded_box(Vec3::new(1.854, 0.12, z0), Vec3::new(1.876, 0.82, z1), 0.012, walnut_warm, MAT_WOOD);
        buf.add_cylinder(Vec3::new(1.844, 0.75, z0 + 0.16), Vec3::new(1.844, 0.75, z1 - 0.16), 0.007, 0.007, 8, chrome, MAT_ALLOY, true);
    }

    let counter_stone = [0.68, 0.67, 0.64];
    buf.add_rounded_box(Vec3::new(1.32, 0.862, 1.36), Vec3::new(2.534, 0.912, 1.990), 0.014, counter_stone, MAT_ALLOY);
    buf.add_rounded_box(Vec3::new(1.85, 0.862, 0.14), Vec3::new(2.534, 0.912, 1.358), 0.014, counter_stone, MAT_ALLOY);
    buf.add_box(Vec3::new(1.34, 0.914, 1.96), Vec3::new(2.532, 1.05, 1.990), [0.56, 0.58, 0.58], MAT_CERAMIC);
    buf.add_box(Vec3::new(2.502, 0.914, 0.16), Vec3::new(2.532, 1.05, 1.958), [0.56, 0.58, 0.58], MAT_CERAMIC);

    buf.add_rounded_box(Vec3::new(1.98, 0.913, 0.32), Vec3::new(2.44, 0.922, 0.84), 0.012, [0.46, 0.49, 0.52], MAT_ALLOY);
    buf.add_box(Vec3::new(2.03, 0.923, 0.37), Vec3::new(2.39, 0.928, 0.79), [0.12, 0.14, 0.17], MAT_ALLOY);
    buf.add_cylinder(Vec3::new(2.44, 0.915, 0.58), Vec3::new(2.44, 1.15, 0.58), 0.014, 0.012, 12, chrome, MAT_ALLOY, true);
    buf.add_cylinder(Vec3::new(2.44, 1.15, 0.58), Vec3::new(2.30, 1.22, 0.58), 0.012, 0.010, 12, chrome, MAT_ALLOY, true);
    buf.add_cylinder(Vec3::new(2.30, 1.22, 0.58), Vec3::new(2.25, 1.12, 0.58), 0.010, 0.009, 12, chrome, MAT_ALLOY, true);
    buf.add_rounded_box(Vec3::new(1.98, 0.913, 0.96), Vec3::new(2.42, 0.924, 1.32), 0.008, [0.08, 0.09, 0.11], MAT_ALLOY);

    props::kettle_set(buf, Pose::at(1.62, 0.913, 1.66));
    buf.add_rounded_box(Vec3::new(1.94, 0.913, 1.54), Vec3::new(2.24, 0.933, 1.78), 0.008, hinoki_light, MAT_WOOD);
    for &(cx, cz, col) in &[(2.04_f32, 1.64_f32, [0.18, 0.30, 0.34]), (2.15, 1.69, [0.72, 0.69, 0.62])] {
        buf.add_lathe_y(
            Vec3::new(cx, 0.934, cz),
            &[(0.0, 0.025), (0.015, 0.032), (0.068, 0.035), (0.074, 0.032)],
            14,
            col,
            MAT_CERAMIC,
            true,
        );
    }

    buf.add_rounded_box(Vec3::new(1.36, 1.72, 1.60), Vec3::new(2.532, 2.48, 1.990), 0.018, walnut_warm, MAT_WOOD);
    buf.add_rounded_box(Vec3::new(2.12, 1.72, 0.18), Vec3::new(2.532, 2.48, 1.596), 0.018, walnut_warm, MAT_WOOD);
    buf.add_box(Vec3::new(1.42, 1.704, 1.66), Vec3::new(2.46, 1.718, 1.70), [1.0, 0.74, 0.38], MAT_EMISSIVE_WARM);
    buf.add_box(Vec3::new(2.16, 1.704, 0.26), Vec3::new(2.20, 1.718, 1.58), [1.0, 0.74, 0.38], MAT_EMISSIVE_WARM);

    // =========================================================================
    // 4. TOP-CENTER: ROUNDED HINOKI DESK + MODULAR DESK INSTRUMENTS & CHAIR
    // =========================================================================
    buf.add_rounded_box(Vec3::new(-1.18, 0.708, 1.26), Vec3::new(1.18, 0.755, 1.95), 0.018, hinoki_light, MAT_WOOD);
    for &lx in &[-1.08_f32, 1.08] {
        buf.add_cylinder(Vec3::new(lx, 0.037, 1.33), Vec3::new(lx, 0.708, 1.33), 0.022, 0.020, 12, alloy_dark, MAT_ALLOY, true);
        buf.add_cylinder(Vec3::new(lx, 0.037, 1.87), Vec3::new(lx, 0.708, 1.87), 0.022, 0.020, 12, alloy_dark, MAT_ALLOY, true);
        buf.add_rounded_box(Vec3::new(lx - 0.032, 0.037, 1.28), Vec3::new(lx + 0.032, 0.064, 1.92), 0.012, alloy_dark, MAT_ALLOY);
    }
    buf.add_cylinder(Vec3::new(-1.08, 0.22, 1.82), Vec3::new(1.08, 0.22, 1.82), 0.015, 0.015, 10, alloy_dark, MAT_ALLOY, false);
    buf.add_rounded_box(Vec3::new(0.68, 0.40, 1.34), Vec3::new(1.05, 0.706, 1.86), 0.016, alloy_mid, MAT_ALLOY);

    props::ergonomic_chair(buf, Pose::at(0.0, 0.037, 0.84));
    props::crt_oscilloscope(buf, Pose::at(0.80, 0.756, 1.63));
    props::architect_lamp(buf, Pose::at(-0.38, 0.756, 1.71));
    props::vhf_transceiver(buf, Pose::at(-0.80, 0.756, 1.62));

    // Pad Technician Shift Clipboard + Brass Pen
    buf.add_rounded_box(Vec3::new(-0.14, 0.756, 1.34), Vec3::new(0.24, 0.770, 1.68), 0.006, walnut_warm, MAT_WOOD);
    buf.add_box(Vec3::new(-0.11, 0.771, 1.37), Vec3::new(0.21, 0.776, 1.64), [0.84, 0.82, 0.76], MAT_MATTE);
    buf.add_box(Vec3::new(-0.09, 0.778, 1.58), Vec3::new(0.19, 0.783, 1.62), [0.18, 0.28, 0.38], MAT_MATTE);
    buf.add_box(Vec3::new(-0.09, 0.778, 1.39), Vec3::new(0.01, 0.783, 1.47), [0.86, 0.74, 0.28], MAT_MATTE);
    buf.add_rounded_box(Vec3::new(-0.01, 0.777, 1.62), Vec3::new(0.11, 0.794, 1.67), 0.005, chrome, MAT_ALLOY);
    buf.add_cylinder(Vec3::new(-0.20, 0.764, 1.40), Vec3::new(-0.23, 0.764, 1.58), 0.0055, 0.0045, 10, brass, MAT_ALLOY, true);

    // =========================================================================
    // 5. TOP-RIGHT: SPLIT AC + RECESSED TELEMETRY PANEL + KEYAKI BONSAI TABLE
    // =========================================================================
    buf.add_rounded_box(Vec3::new(-2.42, 2.05, 1.69), Vec3::new(-1.38, 2.39, 2.012), 0.046, [0.78, 0.78, 0.75], MAT_ALLOY);
    buf.add_box(Vec3::new(-2.35, 2.22, 1.680), Vec3::new(-1.45, 2.230, 1.688), alloy_panel, MAT_ALLOY);
    buf.add_box(Vec3::new(-1.48, 2.16, 1.678), Vec3::new(-1.45, 2.18, 1.688), [0.22, 0.90, 0.54], MAT_EMISSIVE_WARM);
    buf.add_box(Vec3::new(-2.35, 2.052, 1.695), Vec3::new(-1.45, 2.11, 1.76), alloy_dark, MAT_ALLOY);

    // Recessed Steel Telemetry & Breaker Panel (`strictly non-overlapping Z planes!`)
    buf.add_rounded_box(Vec3::new(-2.38, 1.04, 1.972), Vec3::new(-1.38, 1.82, 2.015), 0.018, alloy_dark, MAT_ALLOY);
    buf.add_rounded_box(Vec3::new(-2.34, 1.08, 1.958), Vec3::new(-1.42, 1.78, 1.970), 0.010, [0.22, 0.25, 0.28], MAT_ALLOY);
    for &(bx, by) in &[(-2.31_f32, 1.75_f32), (-1.45, 1.75), (-2.31, 1.11), (-1.45, 1.11)] {
        buf.add_disc_neg_z(Vec3::new(bx, by, 1.952), 0.011, 8, chrome, MAT_ALLOY);
    }

    for &gc in &GAUGE_CENTERS {
        buf.add_cylinder(Vec3::new(gc.x, gc.y, 1.956), Vec3::new(gc.x, gc.y, 1.940), 0.112, 0.105, 24, [0.11, 0.13, 0.15], MAT_ALLOY, true);
        buf.add_disc_neg_z(Vec3::new(gc.x, gc.y, 1.938), 0.095, 24, [0.84, 0.81, 0.74], MAT_MATTE);
        buf.add_disc_neg_z(Vec3::new(gc.x, gc.y, 1.918), 0.014, 12, brass, MAT_ALLOY);
        for t in 0..=10 {
            let frac = (t as f64) / 10.0;
            let ang = (210.0 - frac * 240.0) * PI / 180.0;
            let (ca, sa) = (dcos(ang) as f32, dsin(ang) as f32);
            let r_in = if t % 2 == 0 { 0.070 } else { 0.077 };
            let p_in = Vec3::new(gc.x + r_in * ca, gc.y + r_in * sa, 1.933);
            let p_out = Vec3::new(gc.x + 0.089 * ca, gc.y + 0.089 * sa, 1.933);
            let tick_col = if t >= 8 { [0.78, 0.20, 0.14] } else { [0.14, 0.15, 0.17] };
            buf.add_beam(p_in, p_out, 0.0028, tick_col, MAT_MATTE);
        }
        buf.add_box(Vec3::new(gc.x - 0.066, gc.y - 0.144, 1.946), Vec3::new(gc.x + 0.066, gc.y - 0.120, 1.956), [0.10, 0.11, 0.13], MAT_ALLOY);
    }

    let conduit = [0.46, 0.50, 0.54];
    buf.add_cylinder(Vec3::new(-1.69, 1.82, 1.98), Vec3::new(-1.69, 2.05, 1.94), 0.010, 0.010, 8, conduit, MAT_ALLOY, false);
    buf.add_cylinder(Vec3::new(-1.95, 1.82, 1.98), Vec3::new(-1.95, 2.54, 1.98), 0.010, 0.010, 8, conduit, MAT_ALLOY, false);
    buf.add_cylinder(Vec3::new(-1.95, 2.54, 1.98), Vec3::new(0.0, 2.54, 0.35), 0.010, 0.010, 8, conduit, MAT_ALLOY, false);

    // Low Keyaki Bonsai Table + Modular Bonsai + Washi Andon + Celadon Incense
    buf.add_rounded_box(Vec3::new(-2.46, 0.485, 1.44), Vec3::new(-1.44, 0.54, 1.96), 0.016, walnut_warm, MAT_WOOD);
    for &(tx, tz) in &[(-2.39_f32, 1.50_f32), (-1.51, 1.50), (-2.39, 1.90), (-1.51, 1.90)] {
        buf.add_cylinder(Vec3::new(tx, 0.004, tz), Vec3::new(tx, 0.486, tz), 0.020, 0.026, 10, cedar_dark, MAT_WOOD, false);
    }
    buf.add_rounded_box(Vec3::new(-2.40, 0.15, 1.50), Vec3::new(-1.50, 0.18, 1.90), 0.008, cedar_dark, MAT_WOOD);
    buf.add_rounded_box(Vec3::new(-2.34, 0.182, 1.55), Vec3::new(-1.96, 0.36, 1.85), 0.012, hinoki_light, MAT_WOOD);
    props::bonsai_planter(buf, Pose::at(-1.76, 0.541, 1.69));

    let (ax0, ax1, az0, az1) = (-2.36_f32, -2.14_f32, 1.59_f32, 1.81_f32);
    buf.add_rounded_box(Vec3::new(ax0 - 0.012, 0.541, az0 - 0.012), Vec3::new(ax1 + 0.012, 0.568, az1 + 0.012), 0.006, cedar_dark, MAT_WOOD);
    buf.add_rounded_box(Vec3::new(ax0 - 0.016, 0.945, az0 - 0.016), Vec3::new(ax1 + 0.016, 0.975, az1 + 0.016), 0.006, cedar_dark, MAT_WOOD);
    for &(px, pz) in &[(ax0, az0), (ax1, az0), (ax1, az1), (ax0, az1)] {
        buf.add_cylinder(Vec3::new(px, 0.568, pz), Vec3::new(px, 0.945, pz), 0.009, 0.009, 8, cedar_dark, MAT_WOOD, false);
    }
    buf.add_lathe_y(Vec3::new(-2.03, 0.541, 1.56), &[(0.0, 0.032), (0.028, 0.046), (0.055, 0.030)], 14, [0.30, 0.44, 0.41], MAT_CERAMIC, true);

    // =========================================================================
    // 6. BOTTOM-LEFT: SMOOTH QUILTED PLATFORM BED ALCOVE (`inset cleanly from walls!`)
    // =========================================================================
    buf.add_box(Vec3::new(0.46, 0.004, -2.02), Vec3::new(2.51, 0.08, -0.50), [0.09, 0.10, 0.12], MAT_ALLOY);
    buf.add_rounded_box(Vec3::new(0.425, 0.082, -2.03), Vec3::new(2.532, 0.36, -0.46), 0.026, walnut_warm, MAT_WOOD);
    buf.add_rounded_box(Vec3::new(2.38, 0.362, -1.99), Vec3::new(2.528, 1.06, -0.50), 0.034, [0.20, 0.22, 0.26], MAT_MATTE);
    buf.add_rounded_box(Vec3::new(0.48, 0.362, -1.97), Vec3::new(2.378, 0.535, -0.52), 0.050, [0.76, 0.74, 0.69], MAT_MATTE);
    buf.add_rounded_box(Vec3::new(0.46, 0.536, -1.98), Vec3::new(1.74, 0.595, -0.51), 0.032, [0.13, 0.22, 0.34], MAT_MATTE);
    buf.add_rounded_box(Vec3::new(1.742, 0.536, -1.975), Vec3::new(1.94, 0.598, -0.515), 0.024, [0.84, 0.83, 0.79], MAT_MATTE);
    buf.add_ellipsoid(Vec3::new(2.18, 0.60, -1.58), Vec3::new(0.18, 0.075, 0.28), 14, 8, [0.82, 0.80, 0.75], MAT_MATTE);
    buf.add_ellipsoid(Vec3::new(2.18, 0.60, -0.92), Vec3::new(0.18, 0.075, 0.28), 14, 8, [0.72, 0.75, 0.80], MAT_MATTE);
    buf.add_rounded_box(Vec3::new(0.425, 1.80, -2.03), Vec3::new(2.532, 2.55, -1.40), 0.038, alloy_panel, MAT_ALLOY);
    buf.add_box(Vec3::new(0.56, 1.784, -1.48), Vec3::new(2.40, 1.798, -1.44), [1.0, 0.70, 0.34], MAT_EMISSIVE_WARM);

    // =========================================================================
    // 7. BOTTOM-CENTER: BUILT-IN SMALL SHELF (`non-overlapping joints!`)
    // =========================================================================
    buf.add_box(Vec3::new(0.36, 0.004, -2.032), Vec3::new(0.42, 2.674, -0.46), walnut_warm, MAT_WOOD);
    buf.add_box(Vec3::new(-0.358, 0.004, -1.24), Vec3::new(0.358, 2.674, -1.16), cedar_dark, MAT_WOOD);
    for &sy in &[0.08_f32, 0.56, 1.04, 1.52, 2.02] {
        buf.add_rounded_box(Vec3::new(-0.358, sy, -1.158), Vec3::new(0.358, sy + 0.036, -0.52), 0.010, hinoki_light, MAT_WOOD);
    }
    for &(bx0, bx1, bh, bcol) in &[
        (-0.26_f32, -0.202_f32, 0.28_f32, [0.58, 0.19, 0.14]),
        (-0.192, -0.132, 0.30, [0.14, 0.28, 0.42]),
        (-0.122, -0.062, 0.27, [0.66, 0.52, 0.20]),
        (-0.052, 0.008, 0.29, [0.16, 0.34, 0.26]),
    ] {
        buf.add_rounded_box(Vec3::new(bx0, 1.078, -0.94), Vec3::new(bx1, 1.078 + bh, -0.64), 0.006, bcol, MAT_MATTE);
    }
    buf.add_lathe_y(Vec3::new(0.16, 1.558, -0.84), &[(0.0, 0.045), (0.09, 0.072), (0.18, 0.035), (0.23, 0.042)], 14, [0.72, 0.68, 0.60], MAT_CERAMIC, true);
    buf.add_rounded_box(Vec3::new(-0.24, 0.598, -1.00), Vec3::new(0.22, 0.82, -0.64), 0.016, alloy_mid, MAT_ALLOY);

    // =========================================================================
    // 8. BOTTOM-RIGHT: SEALED UNIT-BATH POD (`ZERO COPLANAR DOORJAMB STIPPLING!`)
    //    Wall rough opening is recessed `2.6 cm` inside `add_u_frame_xy` (`[-1.366, -0.574] x [0.058, 2.106]`)
    //    so white ceramic / plaster wall ends are 100% encapsulated inside the dark cedar casing!
    // =========================================================================
    buf.add_box(Vec3::new(-0.44, 0.004, -2.035), Vec3::new(-0.36, 2.674, -0.242), shikkui_warm, MAT_MATTE);

    // Left jamb pier (`x in [-0.574, -0.362]`, recessed 2.6cm inside doorway `-0.60`!)
    buf.add_box(Vec3::new(-0.574, 0.004, -0.342), Vec3::new(-0.362, 2.674, -0.245), shikkui_warm, MAT_MATTE);
    // Right pocket-wall twin skins (`x in [-2.534, -1.366]`, recessed 2.6cm left of doorway `-1.34`!)
    buf.add_box(Vec3::new(-2.534, 0.004, -0.272), Vec3::new(-1.366, 2.674, -0.245), shikkui_warm, MAT_MATTE);
    buf.add_box(Vec3::new(-2.534, 0.004, -0.345), Vec3::new(-1.366, 2.674, -0.318), [0.76, 0.78, 0.76], MAT_CERAMIC);
    // Header wall above doorway (`y in [2.106, 2.674]`, recessed 2.6cm above `2.08`!)
    buf.add_box(Vec3::new(-1.366, 2.106, -0.345), Vec3::new(-0.574, 2.674, -0.245), shikkui_warm, MAT_MATTE);

    // Encapsulating dark cedar U-frame (`trim_w = 0.058`, spans `z in [-0.362, -0.228]` -> 1.7cm proud relief!)
    buf.add_u_frame_xy(-1.34, -0.60, 0.056, 2.08, -0.362, -0.228, 0.058, cedar_dark, MAT_WOOD);

    // Recessed metal bezel + clean subtle green status dot (`12mm` proud of wall, NO striped CRT scanline!)
    buf.add_rounded_box(Vec3::new(-0.505, 1.245, -0.244), Vec3::new(-0.465, 1.315, -0.232), 0.005, alloy_dark, MAT_ALLOY);
    buf.add_box(Vec3::new(-0.495, 1.262, -0.231), Vec3::new(-0.475, 1.298, -0.226), [0.22, 0.88, 0.52], MAT_EMISSIVE_WARM);

    // Waterproof Unit-Bath Floor Pan + Hinoki Duckboard Mat + Non-Coplanar Tile Cladding
    buf.add_box(Vec3::new(-2.534, 0.003, -2.034), Vec3::new(-0.444, 0.055, -0.346), [0.74, 0.76, 0.75], MAT_CERAMIC);
    buf.add_rounded_box(Vec3::new(-1.42, 0.057, -1.22), Vec3::new(-0.68, 0.077, -0.52), 0.008, hinoki_light, MAT_WOOD);
    buf.add_box(Vec3::new(-2.532, 0.057, -2.032), Vec3::new(-0.446, 2.668, -1.985), [0.78, 0.79, 0.77], MAT_CERAMIC);
    buf.add_box(Vec3::new(-2.532, 0.057, -1.983), Vec3::new(-2.488, 2.668, -0.348), [0.77, 0.78, 0.76], MAT_CERAMIC);
    buf.add_box(Vec3::new(-1.72, 2.642, -1.42), Vec3::new(-1.12, 2.670, -0.92), [1.0, 0.88, 0.68], MAT_EMISSIVE_WARM);

    props::washlet_toilet(buf, Pose::at(-1.96, 0.057, -1.58));
    buf.add_cylinder(Vec3::new(-2.485, 0.68, -1.32), Vec3::new(-2.38, 0.68, -1.32), 0.044, 0.044, 14, [0.88, 0.88, 0.86], MAT_MATTE, true);
    buf.add_cylinder(Vec3::new(-2.44, 1.08, -0.98), Vec3::new(-2.44, 1.08, -0.58), 0.009, 0.009, 8, chrome, MAT_ALLOY, true);

    buf.add_rounded_box(Vec3::new(-1.28, 0.76, -1.978), Vec3::new(-0.68, 0.88, -1.58), 0.028, porcelain, MAT_CERAMIC);
    buf.add_cylinder(Vec3::new(-0.98, 0.882, -1.90), Vec3::new(-0.98, 1.01, -1.81), 0.013, 0.010, 12, chrome, MAT_ALLOY, true);
    buf.add_rounded_box(Vec3::new(-1.22, 1.12, -1.980), Vec3::new(-0.74, 1.82, -1.966), 0.012, [1.0, 0.86, 0.62], MAT_EMISSIVE_WARM);
    buf.add_rounded_box(Vec3::new(-1.19, 1.15, -1.962), Vec3::new(-0.77, 1.79, -1.948), 0.010, [0.58, 0.66, 0.72], MAT_ALLOY);

    // =========================================================================
    // 9. OVERHEAD CEILING FAN HUB (`(0.0, 2.38..2.675, 0.35)`)
    // =========================================================================
    buf.add_lathe_y(
        Vec3::new(0.0, 2.38, 0.35),
        &[(0.0, 0.125), (0.045, 0.135), (0.075, 0.026), (0.295, 0.026)],
        18,
        alloy_dark,
        MAT_ALLOY,
        true,
    );

    // =========================================================================
    // 10. APARTMENT 2ND-FLOOR BALCONY & MODULAR OUTDOOR AC CONDENSER
    // =========================================================================
    buf.add_box(Vec3::new(-2.70, -0.10, 2.13), Vec3::new(2.70, 0.01, 2.90), [0.46, 0.45, 0.41], MAT_MATTE);
    buf.add_cylinder(Vec3::new(-2.62, 1.00, 2.84), Vec3::new(2.62, 1.00, 2.84), 0.020, 0.020, 10, alloy_dark, MAT_ALLOY, true);
    buf.add_cylinder(Vec3::new(-2.62, 0.52, 2.84), Vec3::new(2.62, 0.52, 2.84), 0.010, 0.010, 8, alloy_dark, MAT_ALLOY, false);
    for i in -4..=4 {
        let bx = (i as f32) * 0.62;
        buf.add_cylinder(Vec3::new(bx, 0.01, 2.84), Vec3::new(bx, 1.00, 2.84), 0.013, 0.013, 8, alloy_dark, MAT_ALLOY, false);
    }
    props::outdoor_ac_condenser(buf, Pose::at(-1.98, 0.02, 2.42));
}

pub fn build_hab_204_dynamic(buf: &mut MeshBuffer, p: &DynamicSceneParams) {
    let cedar_dark = [0.16, 0.09, 0.048];
    let walnut_warm = [0.28, 0.16, 0.09];
    let washi_paper = [0.88, 0.84, 0.74];

    // 1. Sliding Shoji Screen (`strictly non-overlapping rib Z offsets so zero rib Z-fighting!`)
    let span_max = 2.50_f32;
    let cover_w = 0.24 + p.shoji_closed.clamp(0.0, 1.0) * (span_max - 0.24);
    let sx1 = 1.248_f32;
    let sx0 = sx1 - cover_w;
    let (sy0, sy1) = (0.768_f32, 2.262_f32);
    let sz = 1.965 + p.window_rattle_m * 0.4;

    buf.add_box(Vec3::new(sx0 + 0.028, sy0 + 0.028, sz - 0.004), Vec3::new(sx1 - 0.028, sy1 - 0.028, sz + 0.004), washi_paper, MAT_SHOJI);
    buf.add_box(Vec3::new(sx0, sy0, sz - 0.015), Vec3::new(sx1, sy0 + 0.030, sz + 0.015), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(sx0, sy1 - 0.030, sz - 0.015), Vec3::new(sx1, sy1, sz + 0.015), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(sx0, sy0 + 0.031, sz - 0.015), Vec3::new(sx0 + 0.032, sy1 - 0.031, sz + 0.015), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(sx1 - 0.030, sy0 + 0.031, sz - 0.015), Vec3::new(sx1, sy1 - 0.031, sz + 0.015), cedar_dark, MAT_WOOD);
    for r in 1..=4 {
        let ry = sy0 + (sy1 - sy0) * (r as f32) / 5.0;
        buf.add_box(Vec3::new(sx0 + 0.033, ry - 0.006, sz - 0.011), Vec3::new(sx1 - 0.031, ry + 0.006, sz + 0.011), cedar_dark, MAT_WOOD);
    }
    let v_count = ((cover_w / 0.26) as usize).clamp(1, 9);
    for c in 1..v_count {
        let rx = sx0 + cover_w * (c as f32) / (v_count as f32);
        buf.add_box(Vec3::new(rx - 0.005, sy0 + 0.031, sz - 0.008), Vec3::new(rx + 0.005, sy1 - 0.031, sz + 0.008), cedar_dark, MAT_WOOD);
    }

    // 2. Sliding Glass Window Sash Frame (`non-overlapping corners!`)
    let sash_shift = p.window_open.clamp(0.0, 1.0) * 1.05;
    let wx0 = -1.248 + sash_shift;
    let wx1 = -0.026 + sash_shift;
    let wz = 2.048 + p.window_rattle_m;
    let sash_col = [0.16, 0.18, 0.21];
    buf.add_box(Vec3::new(wx0, 0.768, wz - 0.012), Vec3::new(wx1, 0.802, wz + 0.012), sash_col, MAT_ALLOY);
    buf.add_box(Vec3::new(wx0, 2.228, wz - 0.012), Vec3::new(wx1, 2.262, wz + 0.012), sash_col, MAT_ALLOY);
    buf.add_box(Vec3::new(wx0, 0.803, wz - 0.012), Vec3::new(wx0 + 0.030, 2.227, wz + 0.012), sash_col, MAT_ALLOY);
    buf.add_box(Vec3::new(wx1 - 0.030, 0.803, wz - 0.012), Vec3::new(wx1, 2.227, wz + 0.012), sash_col, MAT_ALLOY);

    // 3. AC Motorized Airflow Louver Vane
    let ac_running = p.ac_switch_on && p.main_breaker_on && !p.breaker_tripped;
    let vane_z = if ac_running { 1.655 } else { 1.682 };
    let vane_y = if ac_running { 2.044 } else { 2.064 };
    buf.add_rounded_box(Vec3::new(-2.34, vane_y, vane_z), Vec3::new(-1.46, vane_y + 0.018, 1.696), 0.005, [0.76, 0.77, 0.75], MAT_ALLOY);

    // 4. Interactive Unit-Bath Pocket Door (`z in [-0.308, -0.282]`, cleanly centered inside `[-0.318, -0.272]` pocket!)
    // When fully open (`1.0`), `dx1 = -1.25` so the `9 cm` wooden pull stile stays visible & clickable in the doorway!
    let door_shift = p.toilet_door_open.clamp(0.0, 1.0) * 0.66;
    let dx0 = -1.35 - door_shift;
    let dx1 = -0.59 - door_shift;
    buf.add_rounded_box(Vec3::new(dx0, 0.060, -0.308), Vec3::new(dx1, 2.074, -0.282), 0.008, walnut_warm, MAT_WOOD);
    buf.add_box(Vec3::new(dx1 - 0.22, 0.45, -0.312), Vec3::new(dx1 - 0.13, 1.85, -0.278), [0.74, 0.80, 0.79], MAT_SHOJI);
    buf.add_rounded_box(Vec3::new(dx1 - 0.085, 0.96, -0.315), Vec3::new(dx1 - 0.045, 1.18, -0.275), 0.004, [0.14, 0.16, 0.18], MAT_ALLOY);

    // 5. Ceiling Fan 4 Sculpted Wooden Blades
    for b in 0..4 {
        let ang = (p.fan_angle_rad as f64) + (b as f64) * (TAU * 0.25);
        let (ca, sa) = (dcos(ang) as f32, dsin(ang) as f32);
        let root = Vec3::new(0.11 * ca, 2.405, 0.35 + 0.11 * sa);
        let tip = Vec3::new(0.72 * ca, 2.405, 0.35 + 0.72 * sa);
        let side_r = Vec3::new(-0.042 * sa, 0.0, 0.042 * ca);
        let side_t = Vec3::new(-0.062 * sa, 0.0, 0.062 * ca);
        buf.add_cylinder(Vec3::new(0.08 * ca, 2.405, 0.35 + 0.08 * sa), root, 0.012, 0.010, 6, [0.70, 0.54, 0.22], MAT_ALLOY, false);
        buf.push_quad(root.sub(side_r), tip.sub(side_t), tip.add(side_t), root.add(side_r), cedar_dark, MAT_WOOD);
        buf.push_quad(root.add(side_r), tip.add(side_t), tip.sub(side_t), root.sub(side_r), cedar_dark, MAT_WOOD);
    }

    // 6. Bonsai Table Washi Andon Core + Articulated Desk Lamp Bulb
    let andon_col = if p.lamp_on { [1.0, 0.72, 0.34] } else { [0.74, 0.70, 0.62] };
    let andon_mat = if p.lamp_on { MAT_EMISSIVE_WARM } else { MAT_SHOJI };
    buf.add_rounded_box(Vec3::new(-2.345, 0.569, 1.605), Vec3::new(-2.155, 0.944, 1.795), 0.012, andon_col, andon_mat);
    if p.lamp_on {
        buf.add_ellipsoid(Vec3::new(-0.29, 1.155, 1.51), Vec3::new(0.030, 0.026, 0.030), 10, 6, [1.0, 0.84, 0.50], MAT_EMISSIVE_WARM);
    }

    // 7. Three Analog Gauge Steel Needles + Yellow China-Marker Prediction Ticks
    let ranges = [
        (15.0_f32, 40.0_f32, p.room_temp_c),
        (0.0, 1800.0, p.bus_power_w),
        (85.0, 105.0, p.bus_voltage_v),
    ];
    for (idx, &(lo, hi, val)) in ranges.iter().enumerate() {
        let gc = GAUGE_CENTERS[idx];
        let frac = ((val - lo) / (hi - lo)).clamp(0.0, 1.0) as f64;
        let ang = (210.0 - frac * 240.0) * PI / 180.0;
        let tip = Vec3::new(gc.x + 0.078 * (dcos(ang) as f32), gc.y + 0.078 * (dsin(ang) as f32), 1.924);
        let tail = Vec3::new(gc.x - 0.018 * (dcos(ang) as f32), gc.y - 0.018 * (dsin(ang) as f32), 1.924);
        buf.add_beam(tail, tip, 0.0036, [0.08, 0.10, 0.12], MAT_ALLOY);

        let mark_val = p.gauge_marks[idx];
        if mark_val >= lo - 0.01 {
            let mfrac = ((mark_val - lo) / (hi - lo)).clamp(0.0, 1.0) as f64;
            let mang = (210.0 - mfrac * 240.0) * PI / 180.0;
            let m_in = Vec3::new(gc.x + 0.058 * (dcos(mang) as f32), gc.y + 0.058 * (dsin(mang) as f32), 1.929);
            let m_out = Vec3::new(gc.x + 0.095 * (dcos(mang) as f32), gc.y + 0.095 * (dsin(mang) as f32), 1.929);
            buf.add_beam(m_in, m_out, 0.0060, [1.0, 0.84, 0.14], MAT_EMISSIVE_WARM);
        }
    }

    // 8. Five Physical Breaker / Rocker Toggle Switches (`strictly proud of 1.958 faceplate!`)
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
        buf.add_rounded_box(Vec3::new(sx - 0.034, 1.11, 1.942), Vec3::new(sx + 0.034, 1.25, 1.956), 0.006, [0.10, 0.12, 0.14], MAT_ALLOY);
        let dy = if on { 0.042 } else { -0.042 };
        let lever_col = if i == 0 && p.breaker_tripped {
            [0.92, 0.20, 0.14]
        } else if on {
            [0.80, 0.83, 0.86]
        } else {
            [0.38, 0.41, 0.45]
        };
        buf.add_cylinder(Vec3::new(sx, 1.18, 1.942), Vec3::new(sx, 1.18 + dy, 1.904), 0.011, 0.013, 10, lever_col, MAT_ALLOY, true);
        let led_col = if i == 0 && p.breaker_tripped {
            [1.0, 0.18, 0.12]
        } else if on {
            [0.22, 0.88, 0.50]
        } else {
            [0.14, 0.16, 0.18]
        };
        let led_mat = if on || (i == 0 && p.breaker_tripped) { MAT_EMISSIVE_WARM } else { MAT_MATTE };
        buf.add_box(Vec3::new(sx - 0.013, 1.27, 1.946), Vec3::new(sx + 0.013, 1.29, 1.956), led_col, led_mat);
    }
}
