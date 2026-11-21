//! Modular Habitation Unit #204 Assembly (`hab.rs`).
//! Composes reusable architectural modules & `props::*` placements matching the exact floorplan:
//! - Top-Left (`+X, +Z`): L-shaped Galley Kitchen
//! - Top-Center (`0, +Z`): Panoramic Window + Rounded Hinoki Desk + Chair + CRT/VHF/Lamp/Clipboard
//! - Top-Right (`-X, +Z`): Split AC + Analog Gauge/Breaker Panel + Keyaki Bonsai Table
//! - Right Wall (`-X`): Star Citizen Bulkhead Entry Door
//! - Bottom-Left (`+X, -Z`): Recessed Platform Bed Alcove
//! - Bottom-Center (`0, -Z`): Built-in Shelf Divider (sealed floor-to-ceiling!)
//! - Bottom-Right (`-X, -Z`): Sealed Floor-to-Ceiling Unit-Bath Pod with hollow U-frame Pocket Doorway!

use crate::{
    props, DynamicSceneParams, MeshBuffer, Pose, GAUGE_CENTERS, MAT_ALLOY, MAT_CERAMIC, MAT_CRT,
    MAT_EMISSIVE_WARM, MAT_MATTE, MAT_SHOJI, MAT_TATAMI, MAT_WOOD, PANEL_SWITCH_XS,
};
use odderon_dmath::{dcos, dsin, Vec3, PI, TAU};

pub fn build_hab_204_static(buf: &mut MeshBuffer) {
    let shikkui_warm = [0.90, 0.86, 0.79]; // Warm Japanese cream plaster
    let alloy_panel = [0.46, 0.49, 0.53];
    let alloy_mid = [0.33, 0.36, 0.40];
    let alloy_dark = [0.18, 0.20, 0.23];
    let chrome = [0.84, 0.86, 0.89];
    let brass = [0.84, 0.67, 0.30];
    let cedar_dark = [0.31, 0.19, 0.11];
    let walnut_warm = [0.44, 0.27, 0.16];
    let hinoki_light = [0.76, 0.58, 0.38];
    let floor_slate = [0.23, 0.24, 0.26];
    let porcelain = [0.95, 0.96, 0.95];

    // =========================================================================
    // 1. ROOM SHELL, TIMBER TRIM & STAR CITIZEN STRUCTURAL RIBS
    // =========================================================================
    buf.add_box(Vec3::new(-2.64, -0.14, -2.14), Vec3::new(2.64, 0.0, 2.10), floor_slate, MAT_ALLOY);
    buf.add_box(Vec3::new(-2.64, 2.68, -2.14), Vec3::new(2.64, 2.84, 2.10), alloy_dark, MAT_ALLOY);

    // Warm timber + alloy perimeter ceiling beam soffit
    buf.add_box(Vec3::new(-2.60, 2.58, -2.08), Vec3::new(2.60, 2.68, -1.90), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(-2.60, 2.58, 1.88), Vec3::new(2.60, 2.68, 2.06), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(-2.60, 2.58, -1.90), Vec3::new(-2.42, 2.68, 1.88), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(2.42, 2.58, -1.90), Vec3::new(2.60, 2.68, 1.88), cedar_dark, MAT_WOOD);

    // Outer walls: Back (-Z), Left (+X), Right (-X)
    buf.add_box(Vec3::new(-2.62, 0.0, -2.14), Vec3::new(2.62, 2.68, -2.04), shikkui_warm, MAT_MATTE);
    buf.add_box(Vec3::new(2.54, 0.0, -2.10), Vec3::new(2.64, 2.68, 2.08), shikkui_warm, MAT_MATTE);
    buf.add_box(Vec3::new(-2.64, 0.0, -2.10), Vec3::new(-2.54, 2.68, 2.08), shikkui_warm, MAT_MATTE);

    // Japanese cedar baseboards (`habaki`) & wall trim posts (`hashira`)
    buf.add_box(Vec3::new(-2.54, 0.0, 1.98), Vec3::new(2.54, 0.07, 2.02), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(2.50, 0.0, -2.04), Vec3::new(2.55, 0.07, 2.00), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(-2.55, 0.0, -0.24), Vec3::new(-2.50, 0.07, 2.00), cedar_dark, MAT_WOOD);

    // Window Wall (+Z = 2.02): Centered Panoramic Opening `x in [-1.26, +1.26], y in [0.76, 2.28]`
    buf.add_box(Vec3::new(-2.58, 0.0, 2.00), Vec3::new(2.58, 0.76, 2.10), shikkui_warm, MAT_MATTE);
    buf.add_box(Vec3::new(-2.58, 2.28, 2.00), Vec3::new(2.58, 2.68, 2.10), shikkui_warm, MAT_MATTE);
    buf.add_box(Vec3::new(1.26, 0.76, 2.00), Vec3::new(2.58, 2.28, 2.10), shikkui_warm, MAT_MATTE);
    buf.add_box(Vec3::new(-2.58, 0.76, 2.00), Vec3::new(-1.26, 2.28, 2.10), shikkui_warm, MAT_MATTE);

    // Deep Cedar Window Sill & Hollow U-Frame Casing
    buf.add_rounded_box(Vec3::new(-1.33, 0.715, 1.90), Vec3::new(1.33, 0.77, 2.12), 0.012, cedar_dark, MAT_WOOD);
    buf.add_u_frame_xy(-1.25, 1.25, 0.76, 2.27, 1.93, 2.12, 0.065, cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(-0.022, 0.77, 2.01), Vec3::new(0.022, 2.27, 2.07), cedar_dark, MAT_WOOD);

    // Subtle Star Citizen structural ceiling rib & corner gussets (`z = 1.28`)
    buf.add_box(Vec3::new(-2.54, 0.0, 1.24), Vec3::new(-2.47, 2.60, 1.32), alloy_panel, MAT_ALLOY);
    buf.add_box(Vec3::new(2.47, 0.0, 1.24), Vec3::new(2.54, 2.60, 1.32), alloy_panel, MAT_ALLOY);
    buf.add_box(Vec3::new(-2.50, 2.52, 1.25), Vec3::new(2.50, 2.60, 1.31), alloy_dark, MAT_ALLOY);
    buf.add_beam(Vec3::new(-2.48, 2.24, 1.28), Vec3::new(-2.18, 2.54, 1.28), 0.026, alloy_panel, MAT_ALLOY);
    buf.add_beam(Vec3::new(2.48, 2.24, 1.28), Vec3::new(2.18, 2.54, 1.28), 0.026, alloy_panel, MAT_ALLOY);

    // Woven Japanese Tatami Living Floor (`x in [-1.52, +1.78], z in [-0.40, +1.94]`)
    let tatami_a = [0.78, 0.74, 0.47];
    let tatami_b = [0.74, 0.70, 0.44];
    let tatami_heri = [0.14, 0.18, 0.22];
    buf.add_box(Vec3::new(-1.56, 0.0, -0.42), Vec3::new(1.80, 0.024, 1.96), cedar_dark, MAT_WOOD);
    let mats = [
        (-1.52_f32, -0.38_f32, -0.44_f32, 0.76_f32, tatami_a),
        (-0.41, -0.38, 0.68, 0.76, tatami_b),
        (0.71, -0.38, 1.76, 0.76, tatami_a),
        (-1.52, 0.79, -0.44, 1.92, tatami_b),
        (-0.41, 0.79, 0.68, 1.92, tatami_a),
        (0.71, 0.79, 1.76, 1.92, tatami_b),
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
    // 2. RIGHT WALL ENTRY DOOR (`x = -2.54`, `z in [0.22, 1.18]`)
    // =========================================================================
    buf.add_box(Vec3::new(-2.55, 0.0, 0.16), Vec3::new(-2.46, 2.18, 1.24), alloy_dark, MAT_ALLOY);
    buf.add_rounded_box(Vec3::new(-2.49, 0.02, 0.23), Vec3::new(-2.42, 2.10, 1.17), 0.016, alloy_panel, MAT_ALLOY);
    for &dy in &[0.56_f32, 1.12, 1.68] {
        buf.add_box(Vec3::new(-2.425, dy, 0.27), Vec3::new(-2.414, dy + 0.015, 1.13), alloy_dark, MAT_ALLOY);
    }
    buf.add_cylinder(Vec3::new(-2.38, 0.88, 1.03), Vec3::new(-2.38, 1.30, 1.03), 0.013, 0.013, 12, chrome, MAT_ALLOY, true);
    buf.add_cylinder(Vec3::new(-2.43, 0.92, 1.03), Vec3::new(-2.38, 0.92, 1.03), 0.010, 0.010, 8, alloy_dark, MAT_ALLOY, false);
    buf.add_cylinder(Vec3::new(-2.43, 1.26, 1.03), Vec3::new(-2.38, 1.26, 1.03), 0.010, 0.010, 8, alloy_dark, MAT_ALLOY, false);
    buf.add_rounded_box(Vec3::new(-2.54, 1.12, 1.27), Vec3::new(-2.50, 1.30, 1.36), 0.008, alloy_dark, MAT_ALLOY);
    buf.add_box(Vec3::new(-2.502, 1.16, 1.29), Vec3::new(-2.496, 1.26, 1.34), [0.22, 0.95, 0.62], MAT_CRT);
    buf.add_rounded_box(Vec3::new(-2.44, 0.0, 0.20), Vec3::new(-1.82, 0.018, 1.20), 0.008, [0.16, 0.18, 0.20], MAT_ALLOY);

    // =========================================================================
    // 3. TOP-LEFT: L-SHAPED GALLEY KITCHEN (`x in [1.34, 2.54], z in [0.16, 2.00]`)
    // =========================================================================
    buf.add_box(Vec3::new(1.36, 0.0, 1.42), Vec3::new(2.54, 0.08, 2.00), [0.11, 0.12, 0.14], MAT_ALLOY);
    buf.add_box(Vec3::new(1.92, 0.0, 0.18), Vec3::new(2.54, 0.08, 1.42), [0.11, 0.12, 0.14], MAT_ALLOY);
    buf.add_box(Vec3::new(1.34, 0.08, 1.40), Vec3::new(2.54, 0.86, 2.00), alloy_dark, MAT_ALLOY);
    buf.add_box(Vec3::new(1.88, 0.08, 0.16), Vec3::new(2.54, 0.86, 1.40), alloy_dark, MAT_ALLOY);

    // Smooth rounded walnut cabinet fronts
    buf.add_rounded_box(Vec3::new(1.37, 0.12, 1.380), Vec3::new(1.84, 0.82, 1.402), 0.012, walnut_warm, MAT_WOOD);
    buf.add_cylinder(Vec3::new(1.50, 0.75, 1.372), Vec3::new(1.71, 0.75, 1.372), 0.007, 0.007, 8, chrome, MAT_ALLOY, true);
    for &(z0, z1) in &[(0.20_f32, 0.76_f32), (0.80, 1.36)] {
        buf.add_rounded_box(Vec3::new(1.860, 0.12, z0), Vec3::new(1.882, 0.82, z1), 0.012, walnut_warm, MAT_WOOD);
        buf.add_cylinder(Vec3::new(1.852, 0.75, z0 + 0.16), Vec3::new(1.852, 0.75, z1 - 0.16), 0.007, 0.007, 8, chrome, MAT_ALLOY, true);
    }

    // Smooth rounded solid-surface L-countertop + ceramic backsplash
    let counter_stone = [0.84, 0.84, 0.82];
    buf.add_rounded_box(Vec3::new(1.32, 0.86, 1.36), Vec3::new(2.55, 0.912, 2.01), 0.016, counter_stone, MAT_ALLOY);
    buf.add_rounded_box(Vec3::new(1.85, 0.86, 0.14), Vec3::new(2.55, 0.912, 1.39), 0.016, counter_stone, MAT_ALLOY);
    buf.add_box(Vec3::new(1.34, 0.91, 1.96), Vec3::new(2.54, 1.05, 2.00), [0.74, 0.76, 0.76], MAT_CERAMIC);
    buf.add_box(Vec3::new(2.50, 0.91, 0.16), Vec3::new(2.54, 1.05, 1.98), [0.74, 0.76, 0.76], MAT_CERAMIC);

    // Sink + High-Arc Gooseneck Faucet + Induction Hob
    buf.add_rounded_box(Vec3::new(1.98, 0.906, 0.32), Vec3::new(2.44, 0.918, 0.84), 0.014, [0.58, 0.61, 0.64], MAT_ALLOY);
    buf.add_box(Vec3::new(2.03, 0.914, 0.37), Vec3::new(2.39, 0.920, 0.79), [0.16, 0.19, 0.22], MAT_ALLOY);
    buf.add_cylinder(Vec3::new(2.44, 0.91, 0.58), Vec3::new(2.44, 1.15, 0.58), 0.014, 0.012, 12, chrome, MAT_ALLOY, true);
    buf.add_cylinder(Vec3::new(2.44, 1.15, 0.58), Vec3::new(2.30, 1.22, 0.58), 0.012, 0.010, 12, chrome, MAT_ALLOY, true);
    buf.add_cylinder(Vec3::new(2.30, 1.22, 0.58), Vec3::new(2.25, 1.12, 0.58), 0.010, 0.009, 12, chrome, MAT_ALLOY, true);
    buf.add_rounded_box(Vec3::new(1.98, 0.908, 0.96), Vec3::new(2.42, 0.920, 1.34), 0.008, [0.10, 0.11, 0.13], MAT_ALLOY);

    // Modular Kettle + Hinoki Cutting Board + Ceramic Tea Cups
    props::kettle_set(buf, Pose::at(1.62, 0.912, 1.66));
    buf.add_rounded_box(Vec3::new(1.94, 0.912, 1.54), Vec3::new(2.24, 0.932, 1.78), 0.008, hinoki_light, MAT_WOOD);
    for &(cx, cz, col) in &[(2.04_f32, 1.64_f32, [0.24, 0.40, 0.44]), (2.15, 1.69, [0.88, 0.85, 0.78])] {
        buf.add_lathe_y(
            Vec3::new(cx, 0.932, cz),
            &[(0.0, 0.025), (0.015, 0.032), (0.068, 0.035), (0.074, 0.032)],
            14,
            col,
            MAT_CERAMIC,
            true,
        );
    }

    // Overhead Rounded Walnut Galley Cabinets + Warm 2700K LED Strips
    buf.add_rounded_box(Vec3::new(1.36, 1.72, 1.60), Vec3::new(2.54, 2.48, 2.00), 0.018, walnut_warm, MAT_WOOD);
    buf.add_rounded_box(Vec3::new(2.12, 1.72, 0.18), Vec3::new(2.54, 2.48, 1.62), 0.018, walnut_warm, MAT_WOOD);
    buf.add_box(Vec3::new(1.42, 1.706, 1.66), Vec3::new(2.46, 1.72, 1.70), [1.0, 0.76, 0.40], MAT_EMISSIVE_WARM);
    buf.add_box(Vec3::new(2.16, 1.706, 0.26), Vec3::new(2.20, 1.72, 1.60), [1.0, 0.76, 0.40], MAT_EMISSIVE_WARM);

    // =========================================================================
    // 4. TOP-CENTER: ROUNDED HINOKI DESK + MODULAR DESK INSTRUMENTS & CHAIR
    // =========================================================================
    buf.add_rounded_box(Vec3::new(-1.18, 0.708, 1.26), Vec3::new(1.18, 0.755, 1.95), 0.018, hinoki_light, MAT_WOOD);
    for &lx in &[-1.08_f32, 1.08] {
        buf.add_cylinder(Vec3::new(lx, 0.034, 1.33), Vec3::new(lx, 0.71, 1.33), 0.022, 0.020, 12, alloy_dark, MAT_ALLOY, true);
        buf.add_cylinder(Vec3::new(lx, 0.034, 1.87), Vec3::new(lx, 0.71, 1.87), 0.022, 0.020, 12, alloy_dark, MAT_ALLOY, true);
        buf.add_rounded_box(Vec3::new(lx - 0.032, 0.034, 1.28), Vec3::new(lx + 0.032, 0.062, 1.92), 0.012, alloy_dark, MAT_ALLOY);
    }
    buf.add_cylinder(Vec3::new(-1.08, 0.22, 1.82), Vec3::new(1.08, 0.22, 1.82), 0.015, 0.015, 10, alloy_dark, MAT_ALLOY, false);
    buf.add_rounded_box(Vec3::new(0.68, 0.40, 1.34), Vec3::new(1.05, 0.71, 1.86), 0.016, alloy_mid, MAT_ALLOY);

    // Modular Desk Props + Ergonomic Chair
    props::ergonomic_chair(buf, Pose::at(0.0, 0.034, 0.84));
    props::crt_oscilloscope(buf, Pose::at(0.80, 0.755, 1.63));
    props::architect_lamp(buf, Pose::at(-0.38, 0.755, 1.71));
    props::vhf_transceiver(buf, Pose::at(-0.80, 0.755, 1.62));

    // Pad Technician Shift Clipboard + Brass Pen (`x in [-0.14, 0.24], z in [1.34, 1.68]`)
    buf.add_rounded_box(Vec3::new(-0.14, 0.755, 1.34), Vec3::new(0.24, 0.770, 1.68), 0.006, walnut_warm, MAT_WOOD);
    buf.add_box(Vec3::new(-0.11, 0.770, 1.37), Vec3::new(0.21, 0.776, 1.64), [0.97, 0.96, 0.91], MAT_MATTE);
    buf.add_box(Vec3::new(-0.09, 0.777, 1.58), Vec3::new(0.19, 0.782, 1.62), [0.22, 0.34, 0.46], MAT_MATTE);
    buf.add_box(Vec3::new(-0.09, 0.777, 1.39), Vec3::new(0.01, 0.782, 1.47), [0.98, 0.88, 0.34], MAT_MATTE);
    buf.add_rounded_box(Vec3::new(-0.01, 0.774, 1.62), Vec3::new(0.11, 0.792, 1.67), 0.005, chrome, MAT_ALLOY);
    buf.add_cylinder(Vec3::new(-0.20, 0.763, 1.40), Vec3::new(-0.23, 0.763, 1.58), 0.0055, 0.0045, 10, brass, MAT_ALLOY, true);

    // =========================================================================
    // 5. TOP-RIGHT: SPLIT AC + RECESSED TELEMETRY PANEL + KEYAKI BONSAI TABLE
    // =========================================================================
    // Smooth-curved Japanese Split AC Evaporator (`x in [-2.42, -1.38], y in [2.05, 2.39]`)
    buf.add_rounded_box(Vec3::new(-2.42, 2.05, 1.68), Vec3::new(-1.38, 2.39, 2.00), 0.048, [0.92, 0.92, 0.90], MAT_ALLOY);
    buf.add_box(Vec3::new(-2.35, 2.22, 1.675), Vec3::new(-1.45, 2.230, 1.682), alloy_panel, MAT_ALLOY);
    buf.add_box(Vec3::new(-1.48, 2.16, 1.673), Vec3::new(-1.45, 2.18, 1.682), [0.22, 0.92, 0.58], MAT_CRT);
    buf.add_box(Vec3::new(-2.35, 2.052, 1.69), Vec3::new(-1.45, 2.11, 1.76), alloy_dark, MAT_ALLOY);

    // Recessed Steel Telemetry & Breaker Panel
    buf.add_rounded_box(Vec3::new(-2.38, 1.04, 1.956), Vec3::new(-1.38, 1.82, 2.00), 0.020, alloy_dark, MAT_ALLOY);
    buf.add_rounded_box(Vec3::new(-2.34, 1.08, 1.952), Vec3::new(-1.42, 1.78, 1.962), 0.010, [0.28, 0.31, 0.35], MAT_ALLOY);
    for &(bx, by) in &[(-2.34_f32, 1.78_f32), (-1.42, 1.78), (-2.34, 1.08), (-1.42, 1.08)] {
        buf.add_disc_neg_z(Vec3::new(bx, by, 1.950), 0.011, 8, chrome, MAT_ALLOY);
    }

    for &gc in &GAUGE_CENTERS {
        buf.add_cylinder(Vec3::new(gc.x, gc.y, 1.960), Vec3::new(gc.x, gc.y, 1.943), 0.112, 0.105, 24, [0.13, 0.15, 0.18], MAT_ALLOY, true);
        buf.add_disc_neg_z(Vec3::new(gc.x, gc.y, 1.942), 0.095, 24, [0.96, 0.94, 0.88], MAT_MATTE);
        buf.add_disc_neg_z(Vec3::new(gc.x, gc.y, 1.936), 0.014, 12, brass, MAT_ALLOY);
        for t in 0..=10 {
            let frac = (t as f64) / 10.0;
            let ang = (210.0 - frac * 240.0) * PI / 180.0;
            let (ca, sa) = (dcos(ang) as f32, dsin(ang) as f32);
            let r_in = if t % 2 == 0 { 0.070 } else { 0.077 };
            let p_in = Vec3::new(gc.x + r_in * ca, gc.y + r_in * sa, 1.940);
            let p_out = Vec3::new(gc.x + 0.089 * ca, gc.y + 0.089 * sa, 1.940);
            let tick_col = if t >= 8 { [0.85, 0.22, 0.16] } else { [0.16, 0.18, 0.20] };
            buf.add_beam(p_in, p_out, 0.0028, tick_col, MAT_MATTE);
        }
        buf.add_box(Vec3::new(gc.x - 0.066, gc.y - 0.144, 1.948), Vec3::new(gc.x + 0.066, gc.y - 0.120, 1.955), [0.12, 0.13, 0.15], MAT_ALLOY);
    }

    // Neat Wall Conduits
    let conduit = [0.56, 0.60, 0.64];
    buf.add_cylinder(Vec3::new(-1.69, 1.82, 1.97), Vec3::new(-1.69, 2.05, 1.93), 0.010, 0.010, 8, conduit, MAT_ALLOY, false);
    buf.add_cylinder(Vec3::new(-1.95, 1.82, 1.97), Vec3::new(-1.95, 2.55, 1.97), 0.010, 0.010, 8, conduit, MAT_ALLOY, false);
    buf.add_cylinder(Vec3::new(-1.95, 2.55, 1.97), Vec3::new(0.0, 2.55, 0.35), 0.010, 0.010, 8, conduit, MAT_ALLOY, false);

    // Low Keyaki Bonsai Table + Modular Smooth Bonsai + Washi Andon + Celadon Incense
    buf.add_rounded_box(Vec3::new(-2.46, 0.485, 1.44), Vec3::new(-1.44, 0.54, 1.96), 0.016, walnut_warm, MAT_WOOD);
    for &(tx, tz) in &[(-2.39_f32, 1.50_f32), (-1.51, 1.50), (-2.39, 1.90), (-1.51, 1.90)] {
        buf.add_cylinder(Vec3::new(tx, 0.0, tz), Vec3::new(tx, 0.49, tz), 0.020, 0.026, 10, cedar_dark, MAT_WOOD, false);
    }
    buf.add_rounded_box(Vec3::new(-2.40, 0.15, 1.50), Vec3::new(-1.50, 0.18, 1.90), 0.008, cedar_dark, MAT_WOOD);
    buf.add_rounded_box(Vec3::new(-2.34, 0.18, 1.55), Vec3::new(-1.96, 0.36, 1.85), 0.012, hinoki_light, MAT_WOOD);
    props::bonsai_planter(buf, Pose::at(-1.76, 0.54, 1.69));

    // Kumiko Table Andon Frame (`x in [-2.36, -2.14], z in [1.59, 1.81]`)
    let (ax0, ax1, az0, az1) = (-2.36_f32, -2.14_f32, 1.59_f32, 1.81_f32);
    buf.add_rounded_box(Vec3::new(ax0 - 0.012, 0.54, az0 - 0.012), Vec3::new(ax1 + 0.012, 0.568, az1 + 0.012), 0.006, cedar_dark, MAT_WOOD);
    buf.add_rounded_box(Vec3::new(ax0 - 0.016, 0.945, az0 - 0.016), Vec3::new(ax1 + 0.016, 0.975, az1 + 0.016), 0.006, cedar_dark, MAT_WOOD);
    for &(px, pz) in &[(ax0, az0), (ax1, az0), (ax1, az1), (ax0, az1)] {
        buf.add_cylinder(Vec3::new(px, 0.56, pz), Vec3::new(px, 0.955, pz), 0.009, 0.009, 8, cedar_dark, MAT_WOOD, false);
    }
    buf.add_lathe_y(Vec3::new(-2.03, 0.54, 1.56), &[(0.0, 0.032), (0.028, 0.046), (0.055, 0.030)], 14, [0.38, 0.56, 0.52], MAT_CERAMIC, true);

    // =========================================================================
    // 6. BOTTOM-LEFT: SMOOTH QUILTED PLATFORM BED ALCOVE (`x in [0.42, 2.54], z in [-2.04, -0.46]`)
    // =========================================================================
    buf.add_box(Vec3::new(0.46, 0.0, -2.02), Vec3::new(2.52, 0.08, -0.50), [0.11, 0.12, 0.14], MAT_ALLOY);
    buf.add_rounded_box(Vec3::new(0.42, 0.08, -2.04), Vec3::new(2.54, 0.36, -0.46), 0.028, walnut_warm, MAT_WOOD);
    buf.add_rounded_box(Vec3::new(2.38, 0.35, -2.00), Vec3::new(2.53, 1.06, -0.50), 0.036, [0.27, 0.30, 0.35], MAT_MATTE);
    // Soft rounded mattress, folded white sheet & quilted indigo duvet!
    buf.add_rounded_box(Vec3::new(0.48, 0.35, -1.98), Vec3::new(2.39, 0.54, -0.52), 0.055, [0.94, 0.93, 0.89], MAT_MATTE);
    buf.add_rounded_box(Vec3::new(0.46, 0.50, -1.99), Vec3::new(1.86, 0.585, -0.51), 0.038, [0.18, 0.30, 0.45], MAT_MATTE);
    buf.add_rounded_box(Vec3::new(1.72, 0.51, -1.985), Vec3::new(1.94, 0.595, -0.515), 0.026, [0.96, 0.96, 0.94], MAT_MATTE);
    // Plump smooth ellipsoid pillows (zero blocky edges!)
    buf.add_ellipsoid(Vec3::new(2.18, 0.60, -1.58), Vec3::new(0.18, 0.075, 0.28), 14, 8, [0.96, 0.95, 0.91], MAT_MATTE);
    buf.add_ellipsoid(Vec3::new(2.18, 0.60, -0.92), Vec3::new(0.18, 0.075, 0.28), 14, 8, [0.87, 0.89, 0.93], MAT_MATTE);
    // Overhead Port Olisar Rounded Storage Lockers + Warm Reading Strip
    buf.add_rounded_box(Vec3::new(0.42, 1.80, -2.04), Vec3::new(2.54, 2.56, -1.40), 0.042, alloy_panel, MAT_ALLOY);
    buf.add_box(Vec3::new(0.56, 1.785, -1.48), Vec3::new(2.40, 1.80, -1.44), [1.0, 0.72, 0.36], MAT_EMISSIVE_WARM);

    // =========================================================================
    // 7. BOTTOM-CENTER: BUILT-IN SMALL SHELF (`x in [-0.36, +0.42], z in [-2.04, -0.46]`)
    // =========================================================================
    buf.add_box(Vec3::new(0.36, 0.0, -2.04), Vec3::new(0.42, 2.68, -0.46), walnut_warm, MAT_WOOD);
    buf.add_box(Vec3::new(-0.36, 0.0, -1.24), Vec3::new(0.36, 2.68, -1.16), cedar_dark, MAT_WOOD);
    for &sy in &[0.08_f32, 0.56, 1.04, 1.52, 2.02] {
        buf.add_rounded_box(Vec3::new(-0.36, sy, -1.18), Vec3::new(0.36, sy + 0.036, -0.52), 0.010, hinoki_light, MAT_WOOD);
    }
    for &(bx0, bx1, bh, bcol) in &[
        (-0.26_f32, -0.20_f32, 0.28_f32, [0.76, 0.25, 0.19]),
        (-0.19, -0.13, 0.30, [0.19, 0.37, 0.56]),
        (-0.12, -0.06, 0.27, [0.82, 0.68, 0.28]),
        (-0.05, 0.01, 0.29, [0.22, 0.46, 0.36]),
    ] {
        buf.add_rounded_box(Vec3::new(bx0, 1.076, -0.94), Vec3::new(bx1, 1.076 + bh, -0.64), 0.006, bcol, MAT_MATTE);
    }
    buf.add_lathe_y(Vec3::new(0.16, 1.556, -0.84), &[(0.0, 0.045), (0.09, 0.072), (0.18, 0.035), (0.23, 0.042)], 14, [0.86, 0.81, 0.72], MAT_CERAMIC, true);
    buf.add_rounded_box(Vec3::new(-0.24, 0.596, -1.00), Vec3::new(0.22, 0.82, -0.64), 0.016, alloy_mid, MAT_ALLOY);

    // =========================================================================
    // 8. BOTTOM-RIGHT: SEALED FLOOR-TO-CEILING UNIT-BATH POD (`x in [-2.54, -0.36], z in [-2.04, -0.24]`)
    //    Fixes BOTH v2 bugs: (1) full floor-to-ceiling partition with 0 corner gap,
    //    and (2) hollow 3-piece `add_u_frame_xy` doorway with 0 blocking face!
    // =========================================================================
    // Full-height sealed side partition wall (`x in [-0.44, -0.36], z in [-2.06, -0.24], y in [0.0, 2.68]`)!
    buf.add_box(Vec3::new(-0.44, 0.0, -2.06), Vec3::new(-0.36, 2.68, -0.24), shikkui_warm, MAT_MATTE);

    // Front Unit-Bath Wall at `z in [-0.34, -0.24]` with CLEAR Doorway Opening `x in [-1.34, -0.60], y in [0.06, 2.08]`!
    // Left wall pier (`x in [-0.60, -0.36], y in [0.0, 2.68]`)
    buf.add_box(Vec3::new(-0.60, 0.0, -0.34), Vec3::new(-0.36, 2.68, -0.24), shikkui_warm, MAT_MATTE);
    // Right pocket-wall pier (`x in [-2.54, -1.34], y in [0.0, 2.68]`) — built as twin inner/outer skins so door slides inside pocket!
    buf.add_box(Vec3::new(-2.54, 0.0, -0.27), Vec3::new(-1.34, 2.68, -0.235), shikkui_warm, MAT_MATTE); // room-facing skin
    buf.add_box(Vec3::new(-2.54, 0.0, -0.35), Vec3::new(-1.34, 2.68, -0.315), [0.89, 0.90, 0.88], MAT_CERAMIC); // bath-facing skin
    // Header wall above doorway (`x in [-1.34, -0.60], y in [2.08, 2.68]`)
    buf.add_box(Vec3::new(-1.34, 2.08, -0.35), Vec3::new(-0.60, 2.68, -0.235), shikkui_warm, MAT_MATTE);
    // Hollow 3-piece cedar/alloy doorway casing (OPEN aperture!)
    buf.add_u_frame_xy(-1.34, -0.60, 0.05, 2.08, -0.355, -0.228, 0.036, cedar_dark, MAT_WOOD);
    // Occupied/Vacant green indicator badge beside doorway
    buf.add_box(Vec3::new(-0.52, 1.24, -0.234), Vec3::new(-0.47, 1.32, -0.228), [0.22, 0.92, 0.58], MAT_CRT);

    // Waterproof Unit-Bath Floor Pan + Hinoki Duckboard Mat + Warm Tile Cladding
    buf.add_box(Vec3::new(-2.54, 0.0, -2.04), Vec3::new(-0.44, 0.055, -0.31), [0.86, 0.88, 0.87], MAT_CERAMIC);
    buf.add_rounded_box(Vec3::new(-1.42, 0.055, -1.22), Vec3::new(-0.68, 0.075, -0.52), 0.008, hinoki_light, MAT_WOOD);
    buf.add_box(Vec3::new(-2.52, 0.05, -2.04), Vec3::new(-0.44, 2.64, -1.98), [0.90, 0.91, 0.89], MAT_CERAMIC);
    buf.add_box(Vec3::new(-2.54, 0.05, -2.04), Vec3::new(-2.48, 2.64, -0.32), [0.89, 0.90, 0.88], MAT_CERAMIC);
    // Warm diffused bathroom ceiling panel light
    buf.add_box(Vec3::new(-1.72, 2.61, -1.42), Vec3::new(-1.12, 2.64, -0.92), [1.0, 0.90, 0.72], MAT_EMISSIVE_WARM);

    // Sculpted Smooth-Porcelain Japanese Washlet Toilet (`x = -1.96, z = -1.58` facing `+Z`)
    props::washlet_toilet(buf, Pose::at(-1.96, 0.055, -1.58));
    // Wall paper holder + towel bar on `-X` bathroom wall
    buf.add_cylinder(Vec3::new(-2.48, 0.68, -1.32), Vec3::new(-2.38, 0.68, -1.32), 0.044, 0.044, 14, [0.97, 0.97, 0.96], MAT_MATTE, true);
    buf.add_cylinder(Vec3::new(-2.44, 1.08, -0.98), Vec3::new(-2.44, 1.08, -0.58), 0.009, 0.009, 8, chrome, MAT_ALLOY, true);

    // Rounded Porcelain Vanity Sink + Chrome Faucet + Backlit Mirror (`x = -0.98, z = -1.76`, clear of doorway!)
    buf.add_rounded_box(Vec3::new(-1.28, 0.76, -1.97), Vec3::new(-0.68, 0.88, -1.58), 0.028, porcelain, MAT_CERAMIC);
    buf.add_cylinder(Vec3::new(-0.98, 0.88, -1.90), Vec3::new(-0.98, 1.01, -1.81), 0.013, 0.010, 12, chrome, MAT_ALLOY, true);
    buf.add_rounded_box(Vec3::new(-1.22, 1.12, -1.978), Vec3::new(-0.74, 1.82, -1.965), 0.012, [1.0, 0.88, 0.65], MAT_EMISSIVE_WARM);
    buf.add_rounded_box(Vec3::new(-1.19, 1.15, -1.964), Vec3::new(-0.77, 1.79, -1.952), 0.010, [0.72, 0.80, 0.86], MAT_ALLOY);

    // =========================================================================
    // 9. OVERHEAD CEILING FAN HUB (`(0.0, 2.38..2.68, 0.35)`)
    // =========================================================================
    buf.add_lathe_y(
        Vec3::new(0.0, 2.38, 0.35),
        &[(0.0, 0.125), (0.045, 0.135), (0.075, 0.026), (0.30, 0.026)],
        18,
        alloy_dark,
        MAT_ALLOY,
        true,
    );

    // =========================================================================
    // 10. APARTMENT 2ND-FLOOR BALCONY & MODULAR OUTDOOR AC CONDENSER
    // =========================================================================
    buf.add_box(Vec3::new(-2.70, -0.08, 2.08), Vec3::new(2.70, 0.02, 2.90), [0.66, 0.65, 0.61], MAT_MATTE);
    buf.add_cylinder(Vec3::new(-2.62, 1.04, 2.84), Vec3::new(2.62, 1.04, 2.84), 0.022, 0.022, 10, alloy_panel, MAT_ALLOY, true);
    buf.add_cylinder(Vec3::new(-2.62, 0.56, 2.84), Vec3::new(2.62, 0.56, 2.84), 0.010, 0.010, 8, alloy_panel, MAT_ALLOY, false);
    for i in -4..=4 {
        let bx = (i as f32) * 0.62;
        buf.add_cylinder(Vec3::new(bx, 0.02, 2.84), Vec3::new(bx, 1.04, 2.84), 0.014, 0.014, 8, alloy_panel, MAT_ALLOY, false);
    }
    props::outdoor_ac_condenser(buf, Pose::at(-1.98, 0.04, 2.40));
}

pub fn build_hab_204_dynamic(buf: &mut MeshBuffer, p: &DynamicSceneParams) {
    let cedar_dark = [0.31, 0.19, 0.11];
    let walnut_warm = [0.44, 0.27, 0.16];
    let washi_paper = [0.97, 0.94, 0.87];

    // 1. Sliding Shoji Screen across centered window opening (`x in [-1.25, +1.25], y in [0.77, 2.26]`)
    let span_max = 2.50_f32;
    let cover_w = 0.24 + p.shoji_closed.clamp(0.0, 1.0) * (span_max - 0.24);
    let sx1 = 1.25_f32;
    let sx0 = sx1 - cover_w;
    let (sy0, sy1) = (0.77_f32, 2.26_f32);
    let sz = 1.965 + p.window_rattle_m * 0.4;

    buf.add_box(Vec3::new(sx0, sy0, sz - 0.005), Vec3::new(sx1, sy1, sz + 0.005), washi_paper, MAT_SHOJI);
    buf.add_box(Vec3::new(sx0, sy0, sz - 0.014), Vec3::new(sx1, sy0 + 0.032, sz + 0.014), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(sx0, sy1 - 0.032, sz - 0.014), Vec3::new(sx1, sy1, sz + 0.014), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(sx0, sy0, sz - 0.016), Vec3::new(sx0 + 0.034, sy1, sz + 0.016), cedar_dark, MAT_WOOD);
    buf.add_box(Vec3::new(sx1 - 0.030, sy0, sz - 0.014), Vec3::new(sx1, sy1, sz + 0.014), cedar_dark, MAT_WOOD);
    for r in 1..=4 {
        let ry = sy0 + (sy1 - sy0) * (r as f32) / 5.0;
        buf.add_box(Vec3::new(sx0, ry - 0.006, sz - 0.010), Vec3::new(sx1, ry + 0.006, sz + 0.010), cedar_dark, MAT_WOOD);
    }
    let v_count = ((cover_w / 0.26) as usize).clamp(1, 9);
    for c in 1..v_count {
        let rx = sx0 + cover_w * (c as f32) / (v_count as f32);
        buf.add_box(Vec3::new(rx - 0.006, sy0, sz - 0.010), Vec3::new(rx + 0.006, sy1, sz + 0.010), cedar_dark, MAT_WOOD);
    }

    // 2. Sliding Glass Window Sash Frame
    let sash_shift = p.window_open.clamp(0.0, 1.0) * 1.05;
    let wx0 = -1.25 + sash_shift;
    let wx1 = 0.00 + sash_shift;
    let wz = 2.035 + p.window_rattle_m;
    let sash_col = [0.22, 0.25, 0.29];
    buf.add_box(Vec3::new(wx0, 0.77, wz - 0.012), Vec3::new(wx1, 0.805, wz + 0.012), sash_col, MAT_ALLOY);
    buf.add_box(Vec3::new(wx0, 2.225, wz - 0.012), Vec3::new(wx1, 2.26, wz + 0.012), sash_col, MAT_ALLOY);
    buf.add_box(Vec3::new(wx0, 0.77, wz - 0.012), Vec3::new(wx0 + 0.032, 2.26, wz + 0.012), sash_col, MAT_ALLOY);
    buf.add_box(Vec3::new(wx1 - 0.032, 0.77, wz - 0.012), Vec3::new(wx1, 2.26, wz + 0.012), sash_col, MAT_ALLOY);

    // 3. AC Motorized Airflow Louver Vane
    let ac_running = p.ac_switch_on && p.main_breaker_on && !p.breaker_tripped;
    let vane_z = if ac_running { 1.652 } else { 1.678 };
    let vane_y = if ac_running { 2.045 } else { 2.065 };
    buf.add_rounded_box(Vec3::new(-2.34, vane_y, vane_z), Vec3::new(-1.46, vane_y + 0.018, 1.692), 0.005, [0.88, 0.89, 0.87], MAT_ALLOY);

    // 4. Interactive Unit-Bath Wooden-Slatted Pocket Door (`z in [-0.308, -0.278]`, slides cleanly inside pocket wall!)
    // Closed: covers `x in [-1.35, -0.59]`. Open (`1.0`): slides `-0.74m` into `[-2.09, -1.33]` pocket leaving doorway 100% open!
    let door_shift = p.toilet_door_open.clamp(0.0, 1.0) * 0.74;
    let dx0 = -1.35 - door_shift;
    let dx1 = -0.59 - door_shift;
    buf.add_rounded_box(Vec3::new(dx0, 0.058, -0.308), Vec3::new(dx1, 2.075, -0.278), 0.010, walnut_warm, MAT_WOOD);
    // Frosted vertical window strip inside bathroom door + recessed brass pull
    buf.add_box(Vec3::new(dx1 - 0.22, 0.45, -0.310), Vec3::new(dx1 - 0.13, 1.85, -0.276), [0.84, 0.90, 0.89], MAT_SHOJI);
    buf.add_rounded_box(Vec3::new(dx1 - 0.085, 0.96, -0.314), Vec3::new(dx1 - 0.045, 1.18, -0.272), 0.005, [0.18, 0.20, 0.22], MAT_ALLOY);

    // 5. Ceiling Fan 4 Sculpted Wooden Blades
    for b in 0..4 {
        let ang = (p.fan_angle_rad as f64) + (b as f64) * (TAU * 0.25);
        let (ca, sa) = (dcos(ang) as f32, dsin(ang) as f32);
        let root = Vec3::new(0.11 * ca, 2.405, 0.35 + 0.11 * sa);
        let tip = Vec3::new(0.72 * ca, 2.405, 0.35 + 0.72 * sa);
        let side_r = Vec3::new(-0.042 * sa, 0.0, 0.042 * ca);
        let side_t = Vec3::new(-0.062 * sa, 0.0, 0.062 * ca);
        buf.add_cylinder(Vec3::new(0.08 * ca, 2.405, 0.35 + 0.08 * sa), root, 0.012, 0.010, 6, [0.80, 0.64, 0.28], MAT_ALLOY, false);
        buf.push_quad(root.sub(side_r), tip.sub(side_t), tip.add(side_t), root.add(side_r), cedar_dark, MAT_WOOD);
        buf.push_quad(root.add(side_r), tip.add(side_t), tip.sub(side_t), root.sub(side_r), cedar_dark, MAT_WOOD);
    }

    // 6. Bonsai Table Washi Andon Core + Articulated Desk Lamp Bulb
    let andon_col = if p.lamp_on { [1.0, 0.74, 0.36] } else { [0.84, 0.80, 0.72] };
    let andon_mat = if p.lamp_on { MAT_EMISSIVE_WARM } else { MAT_SHOJI };
    buf.add_rounded_box(Vec3::new(-2.345, 0.568, 1.605), Vec3::new(-2.155, 0.945, 1.795), 0.012, andon_col, andon_mat);
    if p.lamp_on {
        buf.add_ellipsoid(Vec3::new(-0.29, 1.155, 1.51), Vec3::new(0.030, 0.026, 0.030), 10, 6, [1.0, 0.85, 0.52], MAT_EMISSIVE_WARM);
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
        let tip = Vec3::new(gc.x + 0.078 * (dcos(ang) as f32), gc.y + 0.078 * (dsin(ang) as f32), 1.929);
        let tail = Vec3::new(gc.x - 0.018 * (dcos(ang) as f32), gc.y - 0.018 * (dsin(ang) as f32), 1.929);
        buf.add_beam(tail, tip, 0.0036, [0.10, 0.12, 0.14], MAT_ALLOY);

        let mark_val = p.gauge_marks[idx];
        if mark_val >= lo - 0.01 {
            let mfrac = ((mark_val - lo) / (hi - lo)).clamp(0.0, 1.0) as f64;
            let mang = (210.0 - mfrac * 240.0) * PI / 180.0;
            let m_in = Vec3::new(gc.x + 0.058 * (dcos(mang) as f32), gc.y + 0.058 * (dsin(mang) as f32), 1.934);
            let m_out = Vec3::new(gc.x + 0.095 * (dcos(mang) as f32), gc.y + 0.095 * (dsin(mang) as f32), 1.934);
            buf.add_beam(m_in, m_out, 0.0060, [1.0, 0.86, 0.14], MAT_EMISSIVE_WARM);
        }
    }

    // 8. Five Physical Breaker / Rocker Toggle Switches
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
        buf.add_rounded_box(Vec3::new(sx - 0.034, 1.11, 1.942), Vec3::new(sx + 0.034, 1.25, 1.956), 0.006, [0.12, 0.14, 0.17], MAT_ALLOY);
        let dy = if on { 0.042 } else { -0.042 };
        let lever_col = if i == 0 && p.breaker_tripped {
            [0.96, 0.22, 0.16]
        } else if on {
            [0.85, 0.88, 0.91]
        } else {
            [0.45, 0.48, 0.52]
        };
        buf.add_cylinder(Vec3::new(sx, 1.18, 1.944), Vec3::new(sx, 1.18 + dy, 1.906), 0.011, 0.013, 10, lever_col, MAT_ALLOY, true);
        let led_col = if i == 0 && p.breaker_tripped {
            [1.0, 0.18, 0.12]
        } else if on {
            [0.24, 0.94, 0.54]
        } else {
            [0.18, 0.20, 0.22]
        };
        let led_mat = if on || (i == 0 && p.breaker_tripped) { MAT_CRT } else { MAT_MATTE };
        buf.add_box(Vec3::new(sx - 0.013, 1.27, 1.942), Vec3::new(sx + 0.013, 1.29, 1.952), led_col, led_mat);
    }
}
