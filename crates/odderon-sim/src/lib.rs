//! Odderon Deterministic Simulation Kernel (Checkpoint 1A v2 — Block 1: "The Sunlit Room").
//! Enforces Law 4: all state mutations pass exclusively through physical `Actuator` variants.

use odderon_dmath::{
    clamp_f32, clamp_f64, dcos, dexp, dsin, dsqrt, Aabb3, BarycentricPos, DetHasher, ProvGraph,
    SolarState, sun_state_uchinoura, Vec3, PI, TAU,
};
use odderon_geom::{DynamicSceneParams, GAUGE_CENTERS, PANEL_SWITCH_XS};
use odderon_thermo::{HabProvenanceIds, HabThermoState};

pub const PAD_DISTANCE_M: f64 = 2820.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TargetId {
    None = 0,
    ShojiScreen = 1,
    WindowSash = 2,
    MainBreaker = 3,
    AcSwitch = 4,
    FanSwitch = 5,
    LampSwitch = 6,
    KettleSwitch = 7,
    GaugeTemp = 8,
    GaugePower = 9,
    GaugeVoltage = 10,
    VhfPadRadio = 11,
    Clipboard = 12,
    SleepingBunk = 13,
    KettleBody = 14,
    AcUnit = 15,
    ToiletDoor = 16,
}

#[derive(Clone, Copy, Debug)]
pub enum Actuator {
    Locomotion {
        fwd: f32,
        right: f32,
        yaw_delta: f32,
        pitch_delta: f32,
        dt_s: f32,
    },
    HandRay {
        mark_mode: bool,
        secondary: bool,
    },
    SetSlider {
        target: TargetId,
        value: f32,
    },
    ToggleSwitch {
        target: TargetId,
    },
    PlaceMark {
        gauge_index: usize,
        predicted_value: f32,
    },
    FlipVisor {
        down: bool,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct LoggedInput {
    pub tick_ms: u64,
    pub code: u8,
    pub arg_a: f32,
    pub arg_b: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CalibrationRecord {
    pub count: u32,
    pub last_predicted: f32,
    pub last_actual: f32,
    pub last_rel_err_pct: f32,
    pub mean_abs_err_pct: f32,
    pub gauge_index: u8,
}

pub struct SimWorld {
    pub tick_ms: u64,
    /// Local Japan Standard Time in fractional hours (starts at 10.15 = 10:09 AM Summer Noon light!).
    pub local_hour_jst: f64,
    pub world_pos: BarycentricPos,
    pub solar: SolarState,
    pub hab: HabThermoState,
    pub prov_graph: ProvGraph,
    pub prov_ids: HabProvenanceIds,

    // First-person physical body state inside room
    pub player_pos: Vec3,
    pub yaw_rad: f32, // 0 = looking +Z directly at sunlit coastal window!
    pub pitch_rad: f32,
    pub head_bob_phase: f32,
    pub visor_down: bool,
    pub last_ray_dir: Vec3,
    pub hovered_target: TargetId,
    pub selected_target: TargetId,
    pub clipboard_open: bool,

    // Mechanical animations & acoustic wavefronts
    pub fan_angle_rad: f32,
    pub fan_omega_rad_s: f32,
    pub toilet_door_open: f32,
    pub toilet_door_target: f32,
    pub gauge_marks: [f32; 3],
    pub mark_timer_s: [f32; 3],
    pub calibration: CalibrationRecord,

    // Distant Uchinoura Pad-1 static-fire & 343 m/s acoustic shockwave state
    pub static_fire_active: bool,
    pub static_fire_start_s: f64,
    pub shockwave_arrived: bool,
    pub shockwave_hit_pulse: f32,
    pub window_rattle_pos: f32,
    pub window_rattle_vel: f32,
    pub auto_fire_triggered: bool,

    // Discrete audio impulse flags consumed by WebAudio thread (<8ms feedback)
    pub pending_click_impulse: f32,
    pub pending_breaker_trip: bool,

    // Proof-of-Flight deterministic input log
    pub input_log: Vec<LoggedInput>,
}

impl Default for SimWorld {
    fn default() -> Self {
        Self::new()
    }
}

impl SimWorld {
    pub fn new() -> Self {
        let local_hour_jst = 10.15; // 10:09 AM Summer Sun slanting vivid golden bars across desk, chair & tatami!
        let solar = sun_state_uchinoura(local_hour_jst);
        let mut hab = HabThermoState::new_summer_morning();
        // Start ceiling fan OFF so Render-On-Demand parks at 0 FPS immediately when still;
        // flipping the physical Fan switch wakes continuous 60 Hz blade rotation!
        hab.fan_switch_on = false;
        for _ in 0..80 {
            hab.step(0.2, local_hour_jst, &solar);
        }

        let mut prov_graph = ProvGraph::new();
        let prov_ids = hab.build_provenance(&solar, &mut prov_graph);

        let mut w = Self {
            tick_ms: 0,
            local_hour_jst,
            world_pos: BarycentricPos::earth_uchinoura_hab(),
            solar,
            hab,
            prov_graph,
            prov_ids,

            // Wake up standing beside the bed & tatami, facing the sunlit window, desk, L-kitchen (left) & AC/bonsai (right)!
            player_pos: Vec3::new(0.18, 1.62, -0.06),
            yaw_rad: -0.04,
            pitch_rad: -0.04,
            head_bob_phase: 0.0,
            visor_down: false,
            last_ray_dir: Vec3::new(0.0, 0.0, 1.0),
            hovered_target: TargetId::None,
            selected_target: TargetId::ShojiScreen,
            clipboard_open: false,

            fan_angle_rad: 0.38,
            fan_omega_rad_s: 0.0,
            toilet_door_open: 0.0,
            toilet_door_target: 0.0,
            gauge_marks: [-1.0, -1.0, -1.0],
            mark_timer_s: [0.0, 0.0, 0.0],
            calibration: CalibrationRecord::default(),

            static_fire_active: false,
            static_fire_start_s: -100.0,
            shockwave_arrived: true,
            shockwave_hit_pulse: 0.0,
            window_rattle_pos: 0.0,
            window_rattle_vel: 0.0,
            auto_fire_triggered: false,

            pending_click_impulse: 0.0,
            pending_breaker_trip: false,
            input_log: Vec::with_capacity(256),
        };
        w.update_hover_raycast();
        w
    }

    #[inline]
    pub fn sim_time_s(&self) -> f64 {
        (self.tick_ms as f64) * 0.001
    }

    #[inline]
    pub fn speed_of_sound_mps(&self) -> f64 {
        331.3 * dsqrt(self.hab.outdoor_temp_k / 273.15)
    }

    #[inline]
    pub fn eye_pos(&self) -> Vec3 {
        let on_tatami = self.player_pos.x > -1.55
            && self.player_pos.x < 1.78
            && self.player_pos.z > -0.42
            && self.player_pos.z < 1.94;
        let in_bath = self.player_pos.x < -0.40 && self.player_pos.z < -0.28;
        let floor_h = if in_bath {
            0.06
        } else if on_tatami {
            0.035
        } else {
            0.0
        };
        let bob = 0.016 * (dsin(self.head_bob_phase as f64) as f32);
        let rattle_cam = self.window_rattle_pos * 0.35;
        Vec3::new(
            self.player_pos.x + rattle_cam,
            1.60 + floor_h + bob,
            self.player_pos.z,
        )
    }

    #[inline]
    pub fn look_dir(&self) -> Vec3 {
        let cp = dcos(self.pitch_rad as f64) as f32;
        let sp = dsin(self.pitch_rad as f64) as f32;
        let cy = dcos(self.yaw_rad as f64) as f32;
        let sy = dsin(self.yaw_rad as f64) as f32;
        Vec3::new(sy * cp, sp, cy * cp).normalized()
    }

    /// Sole public mutation interface (Law 4: Actuator-only world control).
    pub fn actuate(&mut self, act: Actuator) {
        match act {
            Actuator::Locomotion {
                fwd,
                right,
                yaw_delta,
                pitch_delta,
                dt_s,
            } => {
                self.yaw_rad += yaw_delta;
                while self.yaw_rad > PI as f32 {
                    self.yaw_rad -= TAU as f32;
                }
                while self.yaw_rad < -(PI as f32) {
                    self.yaw_rad += TAU as f32;
                }
                self.pitch_rad = clamp_f32(self.pitch_rad + pitch_delta, -1.32, 1.32);

                let move_mag = dsqrt((fwd * fwd + right * right) as f64) as f32;
                if move_mag > 1e-4 {
                    let cy = dcos(self.yaw_rad as f64) as f32;
                    let sy = dsin(self.yaw_rad as f64) as f32;
                    let forward_xz = Vec3::new(sy, 0.0, cy);
                    let right_xz = Vec3::new(-cy, 0.0, sy);
                    let wish = forward_xz.scale(fwd).add(right_xz.scale(right));
                    let speed = 2.15_f32;
                    let step = wish.scale(speed * clamp_f32(dt_s, 0.0, 0.1));
                    self.apply_room_collision(step.x, step.z);
                    self.head_bob_phase += clamp_f32(move_mag, 0.0, 1.0) * dt_s * 9.5;
                }
                self.update_hover_raycast();
            }
            Actuator::FlipVisor { down } => {
                self.visor_down = down;
                self.pending_click_impulse = 0.65;
                self.record_input(1, if down { 1.0 } else { 0.0 }, 0.0);
            }
            Actuator::SetSlider { target, value } => {
                let v = clamp_f32(value, 0.0, 1.0) as f64;
                match target {
                    TargetId::ShojiScreen => {
                        self.hab.shoji_closed = v;
                        self.selected_target = TargetId::ShojiScreen;
                        self.pending_click_impulse = 0.45;
                        self.record_input(2, value, 0.0);
                    }
                    TargetId::WindowSash => {
                        self.hab.window_open = v;
                        self.selected_target = TargetId::WindowSash;
                        self.pending_click_impulse = 0.45;
                        self.record_input(3, value, 0.0);
                    }
                    TargetId::ToiletDoor => {
                        self.toilet_door_target = v as f32;
                        self.selected_target = TargetId::ToiletDoor;
                        self.pending_click_impulse = 0.50;
                        self.record_input(5, value, 0.0);
                    }
                    _ => {}
                }
            }
            Actuator::ToggleSwitch { target } => {
                self.actuate_target(target, false, false);
            }
            Actuator::PlaceMark {
                gauge_index,
                predicted_value,
            } => {
                if gauge_index < 3 {
                    self.gauge_marks[gauge_index] = predicted_value;
                    self.mark_timer_s[gauge_index] = 6.0;
                    self.pending_click_impulse = 0.55;
                    self.record_input(4, gauge_index as f32, predicted_value);
                }
            }
            Actuator::HandRay {
                mark_mode,
                secondary,
            } => {
                self.update_hover_raycast();
                let tgt = self.hovered_target;
                if tgt != TargetId::None {
                    self.actuate_target(tgt, mark_mode, secondary);
                }
            }
        }
        self.prov_ids = self.hab.build_provenance(&self.solar, &mut self.prov_graph);
    }

    fn actuate_target(&mut self, target: TargetId, mark_mode: bool, secondary: bool) {
        self.selected_target = target;
        match target {
            TargetId::ShojiScreen => {
                let next = if secondary {
                    if self.hab.shoji_closed <= 0.05 {
                        1.0
                    } else {
                        clamp_f64(self.hab.shoji_closed - 0.34, 0.0, 1.0)
                    }
                } else if self.hab.shoji_closed >= 0.95 {
                    0.0
                } else {
                    clamp_f64(self.hab.shoji_closed + 0.34, 0.0, 1.0)
                };
                self.hab.shoji_closed = next;
                self.pending_click_impulse = 0.55;
                self.record_input(10, next as f32, 0.0);
            }
            TargetId::WindowSash => {
                let next = if secondary {
                    if self.hab.window_open <= 0.05 {
                        0.90
                    } else {
                        clamp_f64(self.hab.window_open - 0.45, 0.0, 1.0)
                    }
                } else if self.hab.window_open >= 0.90 {
                    0.0
                } else {
                    clamp_f64(self.hab.window_open + 0.45, 0.0, 1.0)
                };
                self.hab.window_open = next;
                self.pending_click_impulse = 0.55;
                self.record_input(11, next as f32, 0.0);
            }
            TargetId::MainBreaker => {
                if self.hab.breaker_tripped {
                    self.hab.breaker_tripped = false;
                    self.hab.breaker_thermal_load = 0.0;
                    self.hab.main_breaker_on = true;
                } else {
                    self.hab.main_breaker_on = !self.hab.main_breaker_on;
                }
                self.pending_click_impulse = 0.95;
                self.record_input(12, if self.hab.main_breaker_on { 1.0 } else { 0.0 }, 0.0);
            }
            TargetId::AcSwitch | TargetId::AcUnit => {
                self.hab.ac_switch_on = !self.hab.ac_switch_on;
                self.pending_click_impulse = 0.80;
                self.record_input(13, if self.hab.ac_switch_on { 1.0 } else { 0.0 }, 0.0);
            }
            TargetId::FanSwitch => {
                self.hab.fan_switch_on = !self.hab.fan_switch_on;
                self.pending_click_impulse = 0.80;
                self.record_input(14, if self.hab.fan_switch_on { 1.0 } else { 0.0 }, 0.0);
            }
            TargetId::LampSwitch => {
                self.hab.lamp_switch_on = !self.hab.lamp_switch_on;
                self.pending_click_impulse = 0.75;
                self.record_input(15, if self.hab.lamp_switch_on { 1.0 } else { 0.0 }, 0.0);
            }
            TargetId::KettleSwitch | TargetId::KettleBody => {
                self.hab.kettle_switch_on = !self.hab.kettle_switch_on;
                self.pending_click_impulse = 0.85;
                self.record_input(16, if self.hab.kettle_switch_on { 1.0 } else { 0.0 }, 0.0);
            }
            TargetId::VhfPadRadio => {
                self.trigger_pad_static_fire();
                self.pending_click_impulse = 0.90;
                self.record_input(17, 1.0, 0.0);
            }
            TargetId::Clipboard => {
                self.clipboard_open = !self.clipboard_open;
                self.pending_click_impulse = 0.50;
                self.record_input(18, if self.clipboard_open { 1.0 } else { 0.0 }, 0.0);
            }
            TargetId::SleepingBunk => {
                // Cycle cleanly through 4 iconic Uchinoura Summer lighting presets:
                // 10:10 (Vivid Summer Noon Sunbeams) -> 12:35 (High Blazing Noon) -> 17:42 (Shinkai Golden Sunset) -> 21:30 (Japantown Lantern Night)
                self.local_hour_jst = if self.local_hour_jst < 11.5 {
                    12.58
                } else if self.local_hour_jst < 15.5 {
                    17.70
                } else if self.local_hour_jst < 19.5 {
                    21.50
                } else {
                    10.15
                };
                // Automatically click warm lamp on at dusk/night for cozy Japantown ambiance!
                if self.local_hour_jst >= 17.0 {
                    self.hab.lamp_switch_on = true;
                }
                self.solar = sun_state_uchinoura(self.local_hour_jst);
                self.hab.step(0.2, self.local_hour_jst, &self.solar);
                self.pending_click_impulse = 0.60;
                self.record_input(19, self.local_hour_jst as f32, 0.0);
            }
            TargetId::ToiletDoor => {
                self.toilet_door_target = if self.toilet_door_target < 0.5 { 1.0 } else { 0.0 };
                self.pending_click_impulse = 0.60;
                self.record_input(24, self.toilet_door_target, 0.0);
            }
            TargetId::GaugeTemp | TargetId::GaugePower | TargetId::GaugeVoltage => {
                let g_idx = match target {
                    TargetId::GaugeTemp => 0,
                    TargetId::GaugePower => 1,
                    _ => 2,
                };
                if mark_mode || secondary {
                    let current = match g_idx {
                        0 => (self.hab.room_air_temp_k - 273.15) as f32,
                        1 => self.hab.total_bus_power_w as f32,
                        _ => self.hab.bus_voltage_v as f32,
                    };
                    let delta = match g_idx {
                        0 => -1.2_f32,
                        1 => {
                            if self.hab.shoji_closed < 0.5 {
                                -260.0
                            } else {
                                240.0
                            }
                        }
                        _ => -0.8,
                    };
                    let (lo, hi) = match g_idx {
                        0 => (15.5, 39.5),
                        1 => (40.0, 1750.0),
                        _ => (86.0, 104.0),
                    };
                    let pred = if self.gauge_marks[g_idx] < 0.0 {
                        clamp_f32(current + delta, lo, hi)
                    } else {
                        let stepped = self.gauge_marks[g_idx]
                            + match g_idx {
                                0 => 1.0,
                                1 => 150.0,
                                _ => 1.0,
                            };
                        if stepped > hi { lo } else { stepped }
                    };
                    self.gauge_marks[g_idx] = pred;
                    self.mark_timer_s[g_idx] = 6.0;
                    self.pending_click_impulse = 0.65;
                    self.record_input(20 + g_idx as u8, pred, 0.0);
                } else {
                    self.visor_down = true;
                    self.pending_click_impulse = 0.45;
                }
            }
            TargetId::None => {}
        }
    }

    pub fn trigger_pad_static_fire(&mut self) {
        self.auto_fire_triggered = true;
        self.static_fire_active = true;
        self.static_fire_start_s = self.sim_time_s();
        self.shockwave_arrived = false;
    }

    fn record_input(&mut self, code: u8, arg_a: f32, arg_b: f32) {
        if self.input_log.len() < 4096 {
            self.input_log.push(LoggedInput {
                tick_ms: self.tick_ms,
                code,
                arg_a,
                arg_b,
            });
        }
    }

    fn apply_room_collision(&mut self, dx: f32, dz: f32) {
        #[derive(Clone, Copy)]
        struct Box2D {
            x0: f32,
            z0: f32,
            x1: f32,
            z1: f32,
        }

        #[inline]
        fn push_circle_out_of_box(px: &mut f32, pz: &mut f32, r: f32, b: Box2D) {
            let cx = clamp_f32(*px, b.x0, b.x1);
            let cz = clamp_f32(*pz, b.z0, b.z1);
            let vx = *px - cx;
            let vz = *pz - cz;
            let d2 = vx * vx + vz * vz;
            if d2 > 1e-8 && d2 < r * r {
                let d = dsqrt(d2 as f64) as f32;
                let push = r - d;
                *px += (vx / d) * push;
                *pz += (vz / d) * push;
            } else if d2 <= 1e-8 {
                // Center is inside box: push out along shallowest penetration face!
                let pen_l = (*px - b.x0).abs();
                let pen_r = (b.x1 - *px).abs();
                let pen_b = (*pz - b.z0).abs();
                let pen_t = (b.z1 - *pz).abs();
                let min_pen = pen_l.min(pen_r).min(pen_b).min(pen_t);
                if (min_pen - pen_l).abs() < 1e-5 {
                    *px = b.x0 - r;
                } else if (min_pen - pen_r).abs() < 1e-5 {
                    *px = b.x1 + r;
                } else if (min_pen - pen_b).abs() < 1e-5 {
                    *pz = b.z0 - r;
                } else {
                    *pz = b.z1 + r;
                }
            }
        }

        let r = 0.22_f32;
        let mut px = clamp_f32(self.player_pos.x + dx, -2.50 + r, 2.50 - r);
        let mut pz = clamp_f32(self.player_pos.z + dz, -1.96 + r, 1.96 - r);

        let door_shift = self.toilet_door_open * 0.66;
        let colliders = [
            // 1. Top-Center Desk & Chair
            Box2D { x0: -1.18, z0: 1.24, x1: 1.18, z1: 2.02 },
            Box2D { x0: -0.25, z0: 0.60, x1: 0.25, z1: 1.06 },
            // 2. Top-Left L-Kitchen
            Box2D { x0: 1.32, z0: 1.36, x1: 2.56, z1: 2.04 },
            Box2D { x0: 1.85, z0: 0.14, x1: 2.56, z1: 1.40 },
            // 3. Top-Right Bonsai Table
            Box2D { x0: -2.48, z0: 1.42, x1: -1.42, z1: 2.00 },
            // 4. Bottom-Left Bed Alcove
            Box2D { x0: 0.42, z0: -2.06, x1: 2.56, z1: -0.46 },
            // 5. Bottom-Center Small Shelf
            Box2D { x0: -0.38, z0: -2.06, x1: 0.42, z1: -0.48 },
            // 6. Bottom-Right Unit-Bath Partition, Front Wall Piers, Sliding Pocket Door & Fixtures!
            Box2D { x0: -0.45, z0: -2.06, x1: -0.35, z1: -0.23 }, // Side partition wall
            Box2D { x0: -0.60, z0: -0.35, x1: -0.36, z1: -0.23 }, // Left jamb pier
            Box2D { x0: -2.56, z0: -0.35, x1: -1.34, z1: -0.23 }, // Right pocket wall pier
            Box2D { x0: -1.35 - door_shift, z0: -0.32, x1: -0.59 - door_shift, z1: -0.26 }, // Sliding door leaf
            Box2D { x0: -2.20, z0: -1.98, x1: -1.68, z1: -1.26 }, // Washlet toilet inside bath
            Box2D { x0: -1.28, z0: -1.98, x1: -0.68, z1: -1.56 }, // Vanity sink inside bath
        ];

        // 2 solver passes guarantee clean corner sliding with zero sticking!
        for _ in 0..2 {
            for c in &colliders {
                push_circle_out_of_box(&mut px, &mut pz, r, *c);
            }
            px = clamp_f32(px, -2.50 + r, 2.50 - r);
            pz = clamp_f32(pz, -1.96 + r, 1.96 - r);
        }

        self.player_pos.x = px;
        self.player_pos.z = pz;
    }

    pub fn cast_ray_target(&self, dir: Vec3) -> TargetId {
        let eye = self.eye_pos();
        let dir = dir.normalized();
        let max_d = 4.10_f32;

        let switch_ids = [
            TargetId::MainBreaker,
            TargetId::AcSwitch,
            TargetId::FanSwitch,
            TargetId::LampSwitch,
            TargetId::KettleSwitch,
        ];

        let mut best_t = max_d;
        let mut best_id = TargetId::None;

        // First seed `best_t` with solid interior partition walls so rays cannot click targets through walls!
        let occluder_walls = [
            Aabb3::new(Vec3::new(-0.44, 0.0, -2.06), Vec3::new(-0.36, 2.68, -0.24)),   // Unit-Bath side wall
            Aabb3::new(Vec3::new(-2.55, 0.0, -0.35), Vec3::new(-1.34, 2.68, -0.235)),  // Unit-Bath pocket wall
            Aabb3::new(Vec3::new(-0.60, 0.0, -0.35), Vec3::new(-0.36, 2.68, -0.235)),  // Unit-Bath left jamb pier
            Aabb3::new(Vec3::new(-1.34, 2.08, -0.35), Vec3::new(-0.60, 2.68, -0.235)), // Unit-Bath doorway header
            Aabb3::new(Vec3::new(0.36, 0.0, -2.04), Vec3::new(0.42, 2.68, -0.46)),     // Shelf-to-bed wood divider
            Aabb3::new(Vec3::new(-0.36, 0.0, -1.24), Vec3::new(0.36, 2.68, -1.16)),    // Shelf back Spine
        ];
        for wall in &occluder_walls {
            if let Some(t) = wall.ray_hit(eye, dir, best_t) {
                best_t = t;
            }
        }

        for (i, &sx) in PANEL_SWITCH_XS.iter().enumerate() {
            let box_sw = Aabb3::new(
                Vec3::new(sx - 0.068, 1.07, 1.88),
                Vec3::new(sx + 0.068, 1.31, 1.99),
            );
            if let Some(t) = box_sw.ray_hit(eye, dir, best_t) {
                best_t = t;
                best_id = switch_ids[i];
            }
        }

        let gauge_ids = [TargetId::GaugeTemp, TargetId::GaugePower, TargetId::GaugeVoltage];
        for (i, &gc) in GAUGE_CENTERS.iter().enumerate() {
            let box_g = Aabb3::new(
                Vec3::new(gc.x - 0.12, gc.y - 0.12, 1.91),
                Vec3::new(gc.x + 0.12, gc.y + 0.12, 1.99),
            );
            if let Some(t) = box_g.ray_hit(eye, dir, best_t) {
                best_t = t;
                best_id = gauge_ids[i];
            }
        }

        let shoji_w = 0.24 + (self.hab.shoji_closed as f32) * (2.50 - 0.24);
        let shoji_box = Aabb3::new(
            Vec3::new(1.25 - shoji_w.max(0.42), 0.77, 1.92),
            Vec3::new(1.27, 2.26, 2.00),
        );
        let win_box = Aabb3::new(Vec3::new(-1.26, 0.77, 2.00), Vec3::new(1.26, 2.26, 2.10));
        let radio_box = Aabb3::new(Vec3::new(-1.08, 0.75, 1.41), Vec3::new(-0.51, 1.02, 1.82));
        let clip_box = Aabb3::new(Vec3::new(-0.22, 0.74, 1.31), Vec3::new(0.27, 0.83, 1.70));
        let kettle_box = Aabb3::new(Vec3::new(1.46, 0.90, 1.50), Vec3::new(1.80, 1.22, 1.82));
        let ac_box = Aabb3::new(Vec3::new(-2.44, 2.03, 1.65), Vec3::new(-1.36, 2.40, 2.02));
        let bunk_box = Aabb3::new(Vec3::new(0.42, 0.12, -2.02), Vec3::new(2.52, 0.70, -0.46));

        let door_shift = self.toilet_door_open * 0.66;
        let toilet_door_box = Aabb3::new(
            Vec3::new(-1.56, 0.06, -0.37),
            Vec3::new(-0.58 - door_shift, 2.10, -0.22),
        );

        let candidates = [
            (radio_box, TargetId::VhfPadRadio),
            (clip_box, TargetId::Clipboard),
            (kettle_box, TargetId::KettleBody),
            (ac_box, TargetId::AcUnit),
            (bunk_box, TargetId::SleepingBunk),
            (toilet_door_box, TargetId::ToiletDoor),
            (shoji_box, TargetId::ShojiScreen),
            (win_box, TargetId::WindowSash),
        ];

        for &(aabb, tid) in &candidates {
            if let Some(t) = aabb.ray_hit(eye, dir, best_t) {
                best_t = t;
                best_id = tid;
            }
        }
        best_id
    }

    pub fn update_hover_raycast(&mut self) {
        let dir = self.look_dir();
        self.last_ray_dir = dir;
        self.hovered_target = self.cast_ray_target(dir);
    }

    pub fn actuate_world_ray(&mut self, dir: Vec3, hover_only: bool, mark_mode: bool, secondary: bool) -> TargetId {
        let norm_dir = dir.normalized();
        self.last_ray_dir = norm_dir;
        let hit = self.cast_ray_target(norm_dir);
        self.hovered_target = hit;
        if !hover_only && hit != TargetId::None {
            self.actuate_target(hit, mark_mode, secondary);
            self.hovered_target = self.cast_ray_target(norm_dir);
            self.prov_ids = self.hab.build_provenance(&self.solar, &mut self.prov_graph);
        }
        hit
    }

    /// Advances simulation by `dt_ms` milliseconds.
    pub fn step_ms(&mut self, dt_ms: u32) {
        let clamped_ms = if dt_ms > 250 { 250 } else { dt_ms };
        if clamped_ms == 0 {
            return;
        }
        let prev_tripped = self.hab.breaker_tripped;
        self.tick_ms += clamped_ms as u64;
        let dt_s = (clamped_ms as f64) * 0.001;

        self.local_hour_jst += dt_s / 3600.0;
        if self.local_hour_jst >= 24.0 {
            self.local_hour_jst -= 24.0;
        }
        self.solar = sun_state_uchinoura(self.local_hour_jst);
        self.hab.step(dt_s, self.local_hour_jst, &self.solar);

        if !prev_tripped && self.hab.breaker_tripped {
            self.pending_breaker_trip = true;
        }

        // Smooth sliding Unit-Bath pocket door animation
        let door_diff = self.toilet_door_target - self.toilet_door_open;
        if door_diff.abs() > 0.004 {
            let step = clamp_f32((dt_s as f32) * 4.2, 0.0, 1.0);
            self.toilet_door_open += door_diff * step;
            if (self.toilet_door_target - self.toilet_door_open).abs() < 0.008 {
                self.toilet_door_open = self.toilet_door_target;
            }
            self.hovered_target = self.cast_ray_target(self.last_ray_dir);
        } else {
            self.toilet_door_open = self.toilet_door_target;
        }

        // Ceiling fan rotational inertia
        let fan_powered = self.hab.main_breaker_on && !self.hab.breaker_tripped && self.hab.fan_switch_on;
        let target_omega = if fan_powered { 8.4_f32 } else { 0.0_f32 };
        let alpha_fan = clamp_f32((dt_s as f32) * 1.8, 0.0, 1.0);
        self.fan_omega_rad_s += (target_omega - self.fan_omega_rad_s) * alpha_fan;
        if self.fan_omega_rad_s < 0.02 && !fan_powered {
            self.fan_omega_rad_s = 0.0;
        }
        self.fan_angle_rad += self.fan_omega_rad_s * (dt_s as f32);
        if self.fan_angle_rad > TAU as f32 {
            self.fan_angle_rad -= TAU as f32;
        }

        // Evaluate China-marker predictions when timer completes
        let actuals = [
            (self.hab.room_air_temp_k - 273.15) as f32,
            self.hab.total_bus_power_w as f32,
            self.hab.bus_voltage_v as f32,
        ];
        for i in 0..3 {
            if self.gauge_marks[i] >= 0.0 && self.mark_timer_s[i] > 0.0 {
                self.mark_timer_s[i] -= dt_s as f32;
                if self.mark_timer_s[i] <= 0.0 {
                    self.mark_timer_s[i] = 0.0;
                    let pred = self.gauge_marks[i];
                    let act = actuals[i];
                    let denom = if act.abs() > 1.0 { act.abs() } else { 1.0 };
                    let err_pct = ((pred - act).abs() / denom) * 100.0;
                    let n = self.calibration.count;
                    let new_mean = (self.calibration.mean_abs_err_pct * (n as f32) + err_pct) / ((n + 1) as f32);
                    self.calibration = CalibrationRecord {
                        count: n + 1,
                        last_predicted: pred,
                        last_actual: act,
                        last_rel_err_pct: err_pct,
                        mean_abs_err_pct: new_mean,
                        gauge_index: i as u8,
                    };
                }
            }
        }

        // Distant Coastal Pad-1 Static Fire & 343 m/s Acoustic Wavefront Propagation
        let now_s = self.sim_time_s();
        if !self.auto_fire_triggered && now_s >= 9.0 {
            self.auto_fire_triggered = true;
            self.trigger_pad_static_fire();
        }

        if self.static_fire_active {
            let age = now_s - self.static_fire_start_s;
            let c_sound = self.speed_of_sound_mps();
            let wave_r = age * c_sound;
            let t_arrival = PAD_DISTANCE_M / c_sound;
            if !self.shockwave_arrived && wave_r >= PAD_DISTANCE_M {
                self.shockwave_arrived = true;
                let win_coupling = 0.55 + 0.45 * (self.hab.window_open as f32);
                self.shockwave_hit_pulse = 1.0 * win_coupling;
            }
            if self.shockwave_arrived {
                let tau = (age - t_arrival).max(0.0);
                if tau < 2.2 {
                    let win_coupling = 0.55 + 0.45 * (self.hab.window_open as f64);
                    let env = 0.018 * win_coupling * dexp(-3.4 * tau);
                    self.window_rattle_pos = (env * dsin(tau * 18.0 * TAU)) as f32;
                    self.window_rattle_vel = (env * 18.0 * TAU * dcos(tau * 18.0 * TAU)) as f32;
                } else {
                    self.window_rattle_pos = 0.0;
                    self.window_rattle_vel = 0.0;
                }
            }
            if age > 18.5 {
                self.static_fire_active = false;
                self.window_rattle_pos = 0.0;
                self.window_rattle_vel = 0.0;
            }
        }

        self.prov_ids = self.hab.build_provenance(&self.solar, &mut self.prov_graph);
    }

    /// Returns `true` when no visual motion is occurring so the presenter can park at 0 rAF/s!
    pub fn is_visually_settled(&self) -> bool {
        let fan_still = self.fan_omega_rad_s < 0.01;
        let plume_done = !self.static_fire_active;
        let rattle_done = self.window_rattle_pos.abs() < 2e-5 && self.window_rattle_vel.abs() < 2e-4;
        let kettle_idle = !self.hab.kettle_switch_on;
        let door_settled = (self.toilet_door_target - self.toilet_door_open).abs() < 0.008;
        let bus_live = self.hab.main_breaker_on && !self.hab.breaker_tripped;
        let temp_err = self.hab.room_air_temp_k - self.hab.ac_setpoint_k;
        let target_mod = if bus_live && self.hab.ac_switch_on {
            if temp_err < -0.6 {
                0.0
            } else {
                let steady_heat = (self.hab.q_solar_gain_w
                    + HabThermoState::UA_ENVELOPE_WK * (self.hab.outdoor_temp_k - self.hab.ac_setpoint_k)
                    + 110.0)
                    .clamp(0.0, 2600.0);
                (0.72 * (steady_heat / (740.0 * self.hab.ac_cop)) + temp_err * 0.13).clamp(0.06, 1.0)
            }
        } else {
            0.0
        };
        let comp_settled = (target_mod - self.hab.ac_compressor_mod).abs() < 0.025;
        let mark_done = self.mark_timer_s[0] <= 0.0
            && self.mark_timer_s[1] <= 0.0
            && self.mark_timer_s[2] <= 0.0;
        fan_still && plume_done && rattle_done && kettle_idle && door_settled && comp_settled && mark_done
    }

    pub fn dynamic_scene_params(&self) -> DynamicSceneParams {
        let now_s = self.sim_time_s();
        let age = if self.static_fire_active {
            (now_s - self.static_fire_start_s) as f32
        } else {
            99.0
        };
        let plume_intensity = if self.static_fire_active && age >= 0.0 && age < 5.2 {
            let ramp_in = clamp_f32(age / 0.35, 0.0, 1.0);
            let ramp_out = clamp_f32((5.2 - age) / 0.6, 0.0, 1.0);
            ramp_in * ramp_out
        } else {
            0.0
        };
        let shock_r = if self.static_fire_active && age >= 0.0 {
            (age as f64 * self.speed_of_sound_mps()) as f32
        } else {
            0.0
        };

        DynamicSceneParams {
            shoji_closed: self.hab.shoji_closed as f32,
            window_open: self.hab.window_open as f32,
            window_rattle_m: self.window_rattle_pos,
            fan_angle_rad: self.fan_angle_rad,
            lamp_on: self.hab.main_breaker_on && !self.hab.breaker_tripped && self.hab.lamp_switch_on,
            main_breaker_on: self.hab.main_breaker_on,
            breaker_tripped: self.hab.breaker_tripped,
            ac_switch_on: self.hab.ac_switch_on,
            fan_switch_on: self.hab.fan_switch_on,
            kettle_switch_on: self.hab.kettle_switch_on,
            toilet_door_open: self.toilet_door_open,
            room_temp_c: (self.hab.room_air_temp_k - 273.15) as f32,
            bus_power_w: self.hab.total_bus_power_w as f32,
            bus_voltage_v: self.hab.bus_voltage_v as f32,
            gauge_marks: self.gauge_marks,
            plume_intensity,
            plume_age_s: age,
            shockwave_radius_m: shock_r,
        }
    }

    pub fn state_hash(&self) -> u64 {
        let mut h = DetHasher::new();
        h.write_u64(self.tick_ms);
        h.write_f64(self.local_hour_jst);
        h.write_f64(self.player_pos.x as f64);
        h.write_f64(self.player_pos.z as f64);
        h.write_f64(self.yaw_rad as f64);
        h.write_f64(self.pitch_rad as f64);
        h.write_f64(self.hab.room_air_temp_k);
        h.write_f64(self.hab.total_bus_power_w);
        h.write_f64(self.hab.bus_voltage_v);
        h.write_f64(self.hab.shoji_closed);
        h.write_f64(self.hab.window_open);
        h.write_f64(self.toilet_door_open as f64);
        h.write_u32((self.hab.breaker_tripped as u32) | ((self.visor_down as u32) << 1));
        h.write_u32(self.calibration.count);
        h.write_u32(self.input_log.len() as u32);
        h.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shockwave_arrives_at_speed_of_sound_delay() {
        let mut w = SimWorld::new();
        w.auto_fire_triggered = true;
        w.trigger_pad_static_fire();
        assert!(!w.shockwave_arrived);

        for _ in 0..75 {
            w.step_ms(100);
        }
        assert!(!w.shockwave_arrived, "Shockwave arrived too early before 2820m / c_sound!");

        for _ in 0..10 {
            w.step_ms(100);
        }
        assert!(w.shockwave_arrived, "Shockwave should have arrived by 8.5s!");
        assert!(w.window_rattle_pos.abs() > 0.0 || w.window_rattle_vel.abs() > 0.0);
    }

    #[test]
    fn deterministic_replay_produces_bit_identical_hash() {
        let run = || {
            let mut w = SimWorld::new();
            w.step_ms(1000);
            w.actuate(Actuator::SetSlider {
                target: TargetId::ShojiScreen,
                value: 0.85,
            });
            w.step_ms(2500);
            w.actuate(Actuator::ToggleSwitch {
                target: TargetId::KettleSwitch,
            });
            w.step_ms(4000);
            w.state_hash()
        };
        let h1 = run();
        let h2 = run();
        assert_eq!(h1, h2);
    }

    #[test]
    fn player_walks_into_and_out_of_bathroom_without_getting_stuck() {
        let mut w = SimWorld::new();
        // Position player in front of Unit-Bath doorway (`x = -0.96, z = 0.05`)
        w.player_pos = Vec3::new(-0.96, 0.0, 0.05);
        w.yaw_rad = PI as f32; // facing `-Z` toward bathroom doorway (`fwd = (0, 0, -1)`)

        // Open bathroom pocket door via toggle
        w.actuate(Actuator::ToggleSwitch {
            target: TargetId::ToiletDoor,
        });
        for _ in 0..25 {
            w.step_ms(50);
        }
        assert!(w.toilet_door_open > 0.99, "Toilet door should slide fully open");

        // Walk forward (`+1.0` along `fwd = -Z`) straight through doorway into bathroom!
        for _ in 0..30 {
            w.actuate(Actuator::Locomotion {
                fwd: 1.0,
                right: 0.0,
                yaw_delta: 0.0,
                pitch_delta: 0.0,
                dt_s: 0.05,
            });
        }
        assert!(
            w.player_pos.z < -0.80,
            "Player should walk freely inside Unit-Bath (got z = {})",
            w.player_pos.z
        );

        // Strafe left/right inside bathroom and walk right back out into main room!
        for _ in 0..15 {
            w.actuate(Actuator::Locomotion {
                fwd: 0.0,
                right: 0.6,
                yaw_delta: 0.0,
                pitch_delta: 0.0,
                dt_s: 0.05,
            });
        }
        // Re-align with doorway center (`x = -0.96`) and walk backward (`-1.0` -> `+Z`) out!
        w.player_pos.x = -0.96;
        for _ in 0..35 {
            w.actuate(Actuator::Locomotion {
                fwd: -1.0,
                right: 0.0,
                yaw_delta: 0.0,
                pitch_delta: 0.0,
                dt_s: 0.05,
            });
        }
        assert!(
            w.player_pos.z > 0.15,
            "Player should walk cleanly back out of Unit-Bath without ever getting stuck (got z = {})",
            w.player_pos.z
        );
    }

    #[test]
    fn solid_walls_occlude_raycasts_while_toilet_door_remains_clickable_inside_and_out() {
        let mut w = SimWorld::new();
        // Stand inside Unit-Bath behind the solid pocket wall (`x = -1.85, z = -1.10`) looking `+Z` toward breaker switches!
        w.player_pos = Vec3::new(-1.85, 0.0, -1.10);
        let hit_through_wall = w.cast_ray_target(Vec3::new(0.0, -0.10, 1.0));
        assert_eq!(
            hit_through_wall,
            TargetId::None,
            "Solid Unit-Bath wall must block raycasts to switches/gauges behind it"
        );

        // Looking at closed bathroom door from inside (`x = -0.96, z = -1.00 -> dir +Z`) must hit ToiletDoor!
        w.player_pos = Vec3::new(-0.96, 0.0, -1.00);
        assert_eq!(w.cast_ray_target(Vec3::new(0.0, -0.2, 1.0)), TargetId::ToiletDoor);
    }

    #[test]
    fn china_marker_keeps_sim_unsettled_until_scored_and_manual_fire_never_double_triggers() {
        let mut w = SimWorld::new();
        // Trigger manual VHF fire at t = 0.5s (`< 9.0s`): must NOT re-trigger at t = 9.0s!
        w.step_ms(250);
        w.step_ms(250);
        w.actuate(Actuator::ToggleSwitch {
            target: TargetId::VhfPadRadio,
        });
        assert!((w.static_fire_start_s - 0.5).abs() < 1e-6);
        for _ in 0..40 {
            w.step_ms(250);
        }
        assert!(
            (w.static_fire_start_s - 0.5).abs() < 1e-6,
            "Auto-fire at 9.0s must not overwrite manual VHF static fire!"
        );

        // Place China-marker prediction and verify `is_visually_settled()` stays false while counting down
        w.actuate(Actuator::PlaceMark {
            gauge_index: 0,
            predicted_value: 26.5,
        });
        assert!(!w.is_visually_settled(), "Sim must stay awake while 6s China-marker timer counts down");
        for _ in 0..80 {
            w.step_ms(250);
        }
        assert_eq!(w.calibration.count, 1, "China-marker prediction should be scored after 6s");
        assert!(w.is_visually_settled(), "Sim should settle to 0 rAF/s once plume & marker timer finish");
    }
}
