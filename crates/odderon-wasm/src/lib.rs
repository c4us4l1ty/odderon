//! Zero-allocation C-ABI WebAssembly interface for Odderon.
//! Exposes pre-allocated vertex/index buffers and a flat 64-float telemetry ring directly
//! out of WASM linear memory so the browser frame loop allocates 0 bytes per frame.

use core::fmt::Write;
use odderon_geom::{build_dynamic_parts, build_eidos_vectors, build_static_world, MeshBuffer};
use odderon_sim::{Actuator, SimWorld, TargetId, PAD_DISTANCE_M};
use std::sync::Mutex;

pub const TELEMETRY_FLOATS: usize = 64;

pub struct EngineInstance {
    pub sim: SimWorld,
    pub static_mesh: MeshBuffer,
    pub dyn_mesh: MeshBuffer,
    pub eidos_mesh: MeshBuffer,
    pub telemetry: [f32; TELEMETRY_FLOATS],
    pub export_buf: String,
}

impl EngineInstance {
    fn new() -> Self {
        let sim = SimWorld::new();
        let mut static_mesh = MeshBuffer::new();
        build_static_world(&mut static_mesh);

        let mut dyn_mesh = MeshBuffer::new();
        let dp = sim.dynamic_scene_params();
        build_dynamic_parts(&mut dyn_mesh, &dp);

        let mut eidos_mesh = MeshBuffer::new();
        build_eidos_vectors(
            &mut eidos_mesh,
            dp.shoji_closed,
            dp.ac_switch_on && dp.main_breaker_on && !dp.breaker_tripped,
            dp.kettle_switch_on && dp.main_breaker_on && !dp.breaker_tripped,
        );

        let mut inst = Self {
            sim,
            static_mesh,
            dyn_mesh,
            eidos_mesh,
            telemetry: [0.0; TELEMETRY_FLOATS],
            export_buf: String::with_capacity(4096),
        };
        inst.sync_telemetry();
        inst
    }

    fn rebuild_dynamic(&mut self) {
        let dp = self.sim.dynamic_scene_params();
        build_dynamic_parts(&mut self.dyn_mesh, &dp);
        if self.sim.visor_down {
            build_eidos_vectors(
                &mut self.eidos_mesh,
                dp.shoji_closed,
                dp.ac_switch_on && dp.main_breaker_on && !dp.breaker_tripped,
                dp.kettle_switch_on && dp.main_breaker_on && !dp.breaker_tripped,
            );
        }
    }

    fn sync_telemetry(&mut self) {
        let s = &mut self.sim;
        let eye = s.eye_pos();
        let look = s.look_dir();
        let sun = s.solar.sun_dir_room;
        let dp = s.dynamic_scene_params();

        let click_imp = s.pending_click_impulse;
        s.pending_click_impulse = 0.0;
        let shock_imp = s.shockwave_hit_pulse;
        s.shockwave_hit_pulse = 0.0;
        let trip_imp = if s.pending_breaker_trip { 1.0 } else { 0.0 };
        s.pending_breaker_trip = false;

        let t = &mut self.telemetry;
        // [0..6]: Camera eye & look direction
        t[0] = eye.x;
        t[1] = eye.y;
        t[2] = eye.z;
        t[3] = look.x;
        t[4] = look.y;
        t[5] = look.z;
        // [6..10]: Sun direction & warmth
        t[6] = sun.x;
        t[7] = sun.y;
        t[8] = sun.z;
        t[9] = s.solar.warmth;
        // [10..20]: Room & Thermo-Electrical telemetry
        t[10] = s.local_hour_jst as f32;
        t[11] = (s.hab.room_air_temp_k - 273.15) as f32;
        t[12] = (s.hab.outdoor_temp_k - 273.15) as f32;
        t[13] = s.hab.q_solar_gain_w as f32;
        t[14] = s.hab.q_ac_cooling_w as f32;
        t[15] = s.hab.q_net_air_w as f32;
        t[16] = s.hab.ac_elec_power_w as f32;
        t[17] = s.hab.total_bus_power_w as f32;
        t[18] = s.hab.bus_voltage_v as f32;
        t[19] = s.hab.bus_current_a as f32;
        // [20..30]: Actuators & Subsystem states
        t[20] = s.hab.shoji_closed as f32;
        t[21] = s.hab.window_open as f32;
        t[22] = if s.hab.main_breaker_on && !s.hab.breaker_tripped { 1.0 } else { 0.0 };
        t[23] = if s.hab.breaker_tripped { 1.0 } else { 0.0 };
        t[24] = if s.hab.ac_switch_on { 1.0 } else { 0.0 };
        t[25] = if s.hab.fan_switch_on { 1.0 } else { 0.0 };
        t[26] = if s.hab.lamp_switch_on { 1.0 } else { 0.0 };
        t[27] = if s.hab.kettle_switch_on { 1.0 } else { 0.0 };
        t[28] = (s.hab.kettle_water_temp_k - 273.15) as f32;
        t[29] = s.hab.ac_cop as f32;
        // [30..40]: Eidos Sensitivities, Hover/Visor, & Shockwave Causality
        t[30] = if s.visor_down { 1.0 } else { 0.0 };
        t[31] = (s.hovered_target as u8) as f32;
        t[32] = (s.selected_target as u8) as f32;
        t[33] = if s.clipboard_open { 1.0 } else { 0.0 };
        t[34] = s.hab.sens_qsolar_d_shoji_w as f32;
        t[35] = s.hab.sens_pac_d_shoji_w as f32;
        t[36] = s.hab.sens_teq_d_shoji_k as f32;
        t[37] = dp.plume_intensity;
        t[38] = dp.plume_age_s;
        t[39] = dp.shockwave_radius_m;
        // [40..50]: Audio physical impulses, settle flag, & Provenance sigmas
        t[40] = click_imp;
        t[41] = shock_imp;
        t[42] = trip_imp;
        t[43] = if s.is_visually_settled() { 1.0 } else { 0.0 };
        t[44] = s.speed_of_sound_mps() as f32;
        t[45] = PAD_DISTANCE_M as f32;
        t[46] = s.hab.ac_compressor_mod as f32;
        t[47] = s.prov_graph.nodes[s.prov_ids.q_solar as usize].sigma as f32;
        t[48] = s.prov_graph.nodes[s.prov_ids.room_temp as usize].sigma as f32;
        t[49] = s.prov_graph.nodes[s.prov_ids.bus_power as usize].sigma as f32;
        // [50..60]: China-marker predictions & Competence Mirror calibration
        t[50] = s.gauge_marks[0];
        t[51] = s.gauge_marks[1];
        t[52] = s.gauge_marks[2];
        t[53] = s.calibration.count as f32;
        t[54] = s.calibration.last_predicted;
        t[55] = s.calibration.last_actual;
        t[56] = s.calibration.last_rel_err_pct;
        t[57] = s.calibration.mean_abs_err_pct;
        t[58] = s.calibration.gauge_index as f32;
        let hash = s.state_hash();
        t[59] = (hash & 0xffff_ffff) as u32 as f32;
        t[60] = ((hash >> 32) & 0xffff_ffff) as u32 as f32;
        t[61] = s.solar.window_incident_wm2 as f32;
        t[62] = (s.solar.elevation_rad * 180.0 / odderon_dmath::PI) as f32;
        t[63] = s.hab.breaker_thermal_load as f32;
    }
}

static ENGINE: Mutex<Option<EngineInstance>> = Mutex::new(None);

fn with_engine<R>(f: impl FnOnce(&mut EngineInstance) -> R) -> R {
    let mut guard = ENGINE.lock().unwrap();
    if guard.is_none() {
        *guard = Some(EngineInstance::new());
    }
    f(guard.as_mut().unwrap())
}

fn target_from_u8(code: u8) -> TargetId {
    match code {
        1 => TargetId::ShojiScreen,
        2 => TargetId::WindowSash,
        3 => TargetId::MainBreaker,
        4 => TargetId::AcSwitch,
        5 => TargetId::FanSwitch,
        6 => TargetId::LampSwitch,
        7 => TargetId::KettleSwitch,
        8 => TargetId::GaugeTemp,
        9 => TargetId::GaugePower,
        10 => TargetId::GaugeVoltage,
        11 => TargetId::VhfPadRadio,
        12 => TargetId::Clipboard,
        13 => TargetId::SleepingBunk,
        14 => TargetId::KettleBody,
        15 => TargetId::AcUnit,
        16 => TargetId::ToiletDoor,
        _ => TargetId::None,
    }
}

#[no_mangle]
pub extern "C" fn odderon_init() {
    let mut guard = ENGINE.lock().unwrap();
    *guard = Some(EngineInstance::new());
}

#[no_mangle]
pub extern "C" fn odderon_step(
    dt_ms: u32,
    fwd: f32,
    right: f32,
    yaw_delta: f32,
    pitch_delta: f32,
) -> u32 {
    with_engine(|eng| {
        if fwd.abs() > 1e-4 || right.abs() > 1e-4 || yaw_delta.abs() > 1e-6 || pitch_delta.abs() > 1e-6 {
            eng.sim.actuate(Actuator::Locomotion {
                fwd,
                right,
                yaw_delta,
                pitch_delta,
                dt_s: (dt_ms as f32) * 0.001,
            });
        }
        eng.sim.step_ms(dt_ms);
        eng.rebuild_dynamic();
        eng.sync_telemetry();
        if eng.sim.is_visually_settled() { 1 } else { 0 }
    })
}

#[no_mangle]
pub extern "C" fn odderon_hand_actuate(mark_mode: u32, secondary: u32) {
    with_engine(|eng| {
        eng.sim.actuate(Actuator::HandRay {
            mark_mode: mark_mode != 0,
            secondary: secondary != 0,
        });
        eng.rebuild_dynamic();
        eng.sync_telemetry();
    });
}

#[no_mangle]
pub extern "C" fn odderon_ray_actuate(
    dx: f32,
    dy: f32,
    dz: f32,
    hover_only: u32,
    mark_mode: u32,
    secondary: u32,
) -> u32 {
    with_engine(|eng| {
        let hit = eng.sim.actuate_world_ray(
            odderon_dmath::Vec3::new(dx, dy, dz),
            hover_only != 0,
            mark_mode != 0,
            secondary != 0,
        );
        if hover_only == 0 {
            eng.rebuild_dynamic();
        }
        eng.sync_telemetry();
        (hit as u8) as u32
    })
}

#[no_mangle]
pub extern "C" fn odderon_toggle_target(target_code: u32) {
    with_engine(|eng| {
        eng.sim.actuate(Actuator::ToggleSwitch {
            target: target_from_u8(target_code as u8),
        });
        eng.rebuild_dynamic();
        eng.sync_telemetry();
    });
}

#[no_mangle]
pub extern "C" fn odderon_set_slider(target_code: u32, value: f32) {
    with_engine(|eng| {
        eng.sim.actuate(Actuator::SetSlider {
            target: target_from_u8(target_code as u8),
            value,
        });
        eng.rebuild_dynamic();
        eng.sync_telemetry();
    });
}

#[no_mangle]
pub extern "C" fn odderon_flip_visor(down: u32) {
    with_engine(|eng| {
        eng.sim.actuate(Actuator::FlipVisor { down: down != 0 });
        eng.rebuild_dynamic();
        eng.sync_telemetry();
    });
}

#[no_mangle]
pub extern "C" fn odderon_place_mark(gauge_idx: u32, predicted_val: f32) {
    with_engine(|eng| {
        eng.sim.actuate(Actuator::PlaceMark {
            gauge_index: gauge_idx as usize,
            predicted_value: predicted_val,
        });
        eng.rebuild_dynamic();
        eng.sync_telemetry();
    });
}

// --- Zero-copy linear memory buffer getters ---

#[no_mangle]
pub extern "C" fn odderon_telemetry_ptr() -> *const f32 {
    with_engine(|eng| eng.telemetry.as_ptr())
}

#[no_mangle]
pub extern "C" fn odderon_static_verts_ptr() -> *const f32 {
    with_engine(|eng| eng.static_mesh.verts.as_ptr())
}

#[no_mangle]
pub extern "C" fn odderon_static_verts_len() -> usize {
    with_engine(|eng| eng.static_mesh.verts.len())
}

#[no_mangle]
pub extern "C" fn odderon_static_indices_ptr() -> *const u32 {
    with_engine(|eng| eng.static_mesh.indices.as_ptr())
}

#[no_mangle]
pub extern "C" fn odderon_static_indices_len() -> usize {
    with_engine(|eng| eng.static_mesh.indices.len())
}

#[no_mangle]
pub extern "C" fn odderon_dyn_verts_ptr() -> *const f32 {
    with_engine(|eng| eng.dyn_mesh.verts.as_ptr())
}

#[no_mangle]
pub extern "C" fn odderon_dyn_verts_len() -> usize {
    with_engine(|eng| eng.dyn_mesh.verts.len())
}

#[no_mangle]
pub extern "C" fn odderon_dyn_indices_ptr() -> *const u32 {
    with_engine(|eng| eng.dyn_mesh.indices.as_ptr())
}

#[no_mangle]
pub extern "C" fn odderon_dyn_indices_len() -> usize {
    with_engine(|eng| eng.dyn_mesh.indices.len())
}

#[no_mangle]
pub extern "C" fn odderon_eidos_verts_ptr() -> *const f32 {
    with_engine(|eng| eng.eidos_mesh.verts.as_ptr())
}

#[no_mangle]
pub extern "C" fn odderon_eidos_verts_len() -> usize {
    with_engine(|eng| eng.eidos_mesh.verts.len())
}

#[no_mangle]
pub extern "C" fn odderon_eidos_indices_ptr() -> *const u32 {
    with_engine(|eng| eng.eidos_mesh.indices.as_ptr())
}

#[no_mangle]
pub extern "C" fn odderon_eidos_indices_len() -> usize {
    with_engine(|eng| eng.eidos_mesh.indices.len())
}

/// Generates a verifiable Proof-of-Flight & Akashi JSON bundle on demand (off the frame loop).
#[no_mangle]
pub extern "C" fn odderon_build_proof_json() -> *const u8 {
    with_engine(|eng| {
        eng.export_buf.clear();
        let hash = eng.sim.state_hash();
        let s = &eng.sim;
        let _ = write!(
            &mut eng.export_buf,
            "{{\n  \"schema\": \"odderon.proof-of-flight/1\",\n  \"checkpoint\": \"1A-the-sunlit-room\",\n  \"location\": \"Uchinoura Coastal Hab (31.2510N, 131.0791E, +94m MSL)\",\n  \"galactic_sector_nm\": [{}, {}, {}],\n  \"tick_ms\": {},\n  \"local_hour_jst\": {:.4},\n  \"state_hash_hex\": \"{:016x}\",\n  \"telemetry\": {{\n    \"room_temp_c\": {:.2},\n    \"q_solar_gain_w\": {:.1},\n    \"ac_cooling_w\": {:.1},\n    \"ac_cop\": {:.2},\n    \"bus_power_w\": {:.1},\n    \"bus_voltage_v\": {:.2}\n  }},\n  \"eidos_sensitivities\": {{\n    \"d_qsolar_d_shoji_w\": {:.1},\n    \"d_pac_d_shoji_w\": {:.1},\n    \"d_teq_d_shoji_k\": {:.2}\n  }},\n  \"calibration\": {{\n    \"predictions_scored\": {},\n    \"mean_abs_error_pct\": {:.2}\n  }},\n  \"actuator_events_count\": {}\n}}",
            s.world_pos.sector.x_nm,
            s.world_pos.sector.y_nm,
            s.world_pos.sector.z_nm,
            s.tick_ms,
            s.local_hour_jst,
            hash,
            s.hab.room_air_temp_k - 273.15,
            s.hab.q_solar_gain_w,
            s.hab.q_ac_cooling_w,
            s.hab.ac_cop,
            s.hab.total_bus_power_w,
            s.hab.bus_voltage_v,
            s.hab.sens_qsolar_d_shoji_w,
            s.hab.sens_pac_d_shoji_w,
            s.hab.sens_teq_d_shoji_k,
            s.calibration.count,
            s.calibration.mean_abs_err_pct,
            s.input_log.len()
        );
        eng.export_buf.as_ptr()
    })
}

#[no_mangle]
pub extern "C" fn odderon_proof_json_len() -> usize {
    with_engine(|eng| eng.export_buf.len())
}
