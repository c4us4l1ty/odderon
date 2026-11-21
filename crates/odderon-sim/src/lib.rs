//! Odderon Deterministic Simulation Kernel (Checkpoint 1A — Block 1: "The Sunlit Room").
//! Enforces Law 4: all state mutations pass exclusively through physical `Actuator` variants.

use odderon_dmath::{
    clamp_f32, clamp_f64, dcos, dexp, dsin, dsqrt, Aabb3, BarycentricPos, DetHasher, ProvGraph,
    SolarState, sun_state_uchinoura, Vec3, PI, TAU,
};
use odderon_geom::{DynamicSceneParams, GAUGE_CENTERS};
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
    /// Local Japan Standard Time in fractional hours (starts at 11.65 = 11:39 AM Summer Noon light!).
    pub local_hour_jst: f64,
    pub world_pos: BarycentricPos,
    pub solar: SolarState,
    pub hab: HabThermoState,
    pub prov_graph: ProvGraph,
    pub prov_ids: HabProvenanceIds,

    // First-person astronaut/nobody physical body state inside room
    pub player_pos: Vec3, // feet x, eye y, feet z
    pub yaw_rad: f32,     // 0 = looking +Z directly at sunlit coastal window!
    pub pitch_rad: f32,
    pub head_bob_phase: f32,
    pub visor_down: bool,
    pub hovered_target: TargetId,
    pub selected_target: TargetId,
    pub clipboard_open: bool,

    // Mechanical animations & acoustic wavefronts
    pub fan_angle_rad: f32,
    pub fan_omega_rad_s: f32,
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
        let local_hour_jst = 9.75; // 09:45 AM Summer Sun slanting golden bars deep across desk & tatami!
        let solar = sun_state_uchinoura(local_hour_jst);
        let mut hab = HabThermoState::new_summer_morning();
        // Start ceiling fan OFF so Render-On-Demand settles to 0 FPS immediately when still;
        // flipping the physical Fan switch wakes continuous blade rotation!
        hab.fan_switch_on = false;
        // Pre-settle compressor to initial equilibrium so boot state starts calm & parked at 0 rAF/s
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

            // Wake up standing beside the raised tatami & bunk, looking toward the sunlit window & desk!
            player_pos: Vec3::new(-0.65, 1.66, -0.15),
            yaw_rad: 0.16,
            pitch_rad: -0.05,
            head_bob_phase: 0.0,
            visor_down: false,
            hovered_target: TargetId::None,
            selected_target: TargetId::ShojiScreen,
            clipboard_open: false,

            fan_angle_rad: 0.35,
            fan_omega_rad_s: 0.0,
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
        // c = sqrt(gamma * R_specific * T_out) = 331.3 * sqrt(T / 273.15)
        331.3 * dsqrt(self.hab.outdoor_temp_k / 273.15)
    }

    #[inline]
    pub fn eye_pos(&self) -> Vec3 {
        let floor_h = if self.player_pos.x < 0.38 && self.player_pos.z > -0.45 && self.player_pos.z < 1.72 {
            0.12 // Stepped up onto raised Japanese tatami platform!
        } else {
            0.0
        };
        let bob = 0.018 * (dsin(self.head_bob_phase as f64) as f32);
        let rattle_cam = self.window_rattle_pos * 0.35;
        Vec3::new(
            self.player_pos.x + rattle_cam,
            1.58 + floor_h + bob,
            self.player_pos.z,
        )
    }

    #[inline]
    pub fn look_dir(&self) -> Vec3 {
        let cp = dcos(self.pitch_rad as f64) as f32;
        let sp = dsin(self.pitch_rad as f64) as f32;
        let cy = dcos(self.yaw_rad as f64) as f32;
        let sy = dsin(self.yaw_rad as f64) as f32;
        // yaw = 0 looks straight along +Z (out the sunlit coastal window!)
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
                    self.mark_timer_s[gauge_index] = 6.0; // Evaluate settling accuracy after 6s!
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
                // Step smoothly 0.0 -> 0.35 -> 0.70 -> 1.0 -> 0.0 (or reverse on secondary)
                let next = if secondary {
                    if self.hab.shoji_closed <= 0.05 { 1.0 } else { clamp_f64(self.hab.shoji_closed - 0.34, 0.0, 1.0) }
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
                let next = if self.hab.window_open >= 0.90 {
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
                // Diegetic sleep / rest cycles through Morning -> Summer Noon -> Shinkai Golden Hour -> Lantern Evening!
                self.local_hour_jst += 2.75;
                if self.local_hour_jst >= 24.0 {
                    self.local_hour_jst -= 24.0;
                }
                self.solar = sun_state_uchinoura(self.local_hour_jst);
                self.hab.step(0.2, self.local_hour_jst, &self.solar);
                self.pending_click_impulse = 0.60;
                self.record_input(19, self.local_hour_jst as f32, 0.0);
            }
            TargetId::GaugeTemp | TargetId::GaugePower | TargetId::GaugeVoltage => {
                let g_idx = match target {
                    TargetId::GaugeTemp => 0,
                    TargetId::GaugePower => 1,
                    _ => 2,
                };
                if mark_mode || secondary {
                    // Place or cycle a China-marker prediction tick right on the gauge glass!
                    let current = match g_idx {
                        0 => (self.hab.room_air_temp_k - 273.15) as f32,
                        1 => self.hab.total_bus_power_w as f32,
                        _ => self.hab.bus_voltage_v as f32,
                    };
                    // Offset slightly in direction of current derivative or cycle handy tick
                    let delta = match g_idx {
                        0 => -1.2_f32,
                        1 => if self.hab.shoji_closed < 0.5 { -260.0 } else { 240.0 },
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
                        let stepped = self.gauge_marks[g_idx] + match g_idx {
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
                    // Tap gauge to inspect Provenance lineage in Eidos!
                    self.visor_down = true;
                    self.pending_click_impulse = 0.45;
                }
            }
            TargetId::None => {}
        }
    }

    pub fn trigger_pad_static_fire(&mut self) {
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
        let nx = clamp_f32(self.player_pos.x + dx, -2.02, 1.52);
        let nz = clamp_f32(self.player_pos.z + dz, -1.02, 1.06);

        // Prevent walking into Sleeping Bunk (-2.36..-0.48, -1.96..-0.44)
        let in_bunk_x = nx < -0.38 && self.player_pos.z < -0.36;
        let in_bunk_z = nz < -0.36 && nx < -0.38;

        if !in_bunk_x {
            self.player_pos.x = nx;
        }
        if !in_bunk_z {
            self.player_pos.z = nz;
        }
    }

    pub fn cast_ray_target(&self, dir: Vec3) -> TargetId {
        let eye = self.eye_pos();
        let dir = dir.normalized();
        let max_d = 3.65_f32;

        let switch_xs = [
            (1.48_f32, TargetId::MainBreaker),
            (1.63, TargetId::AcSwitch),
            (1.78, TargetId::FanSwitch),
            (1.93, TargetId::LampSwitch),
            (2.08, TargetId::KettleSwitch),
        ];

        let mut best_t = max_d;
        let mut best_id = TargetId::None;

        for &(sx, tid) in &switch_xs {
            let box_sw = Aabb3::new(
                Vec3::new(sx - 0.068, 1.05, 1.84),
                Vec3::new(sx + 0.068, 1.30, 1.96),
            );
            if let Some(t) = box_sw.ray_hit(eye, dir, best_t) {
                best_t = t;
                best_id = tid;
            }
        }

        let gauge_ids = [TargetId::GaugeTemp, TargetId::GaugePower, TargetId::GaugeVoltage];
        for (i, &gc) in GAUGE_CENTERS.iter().enumerate() {
            let box_g = Aabb3::new(
                Vec3::new(gc.x - 0.115, gc.y - 0.115, 1.87),
                Vec3::new(gc.x + 0.115, gc.y + 0.115, 1.96),
            );
            if let Some(t) = box_g.ray_hit(eye, dir, best_t) {
                best_t = t;
                best_id = gauge_ids[i];
            }
        }

        let shoji_w = 0.28 + (self.hab.shoji_closed as f32) * (2.81 - 0.28);
        let shoji_box = Aabb3::new(
            Vec3::new(-1.56, 0.78, 1.88),
            Vec3::new(-1.53 + shoji_w.max(0.45), 2.24, 1.97),
        );
        let win_box = Aabb3::new(Vec3::new(-0.14, 0.78, 1.97), Vec3::new(1.30, 2.24, 2.08));
        let radio_box = Aabb3::new(Vec3::new(0.46, 0.74, 1.38), Vec3::new(1.04, 1.04, 1.84));
        let clip_box = Aabb3::new(Vec3::new(-0.42, 0.73, 1.28), Vec3::new(0.08, 0.83, 1.72));
        let kettle_box = Aabb3::new(Vec3::new(0.94, 0.89, -1.66), Vec3::new(1.30, 1.20, -1.30));
        let ac_box = Aabb3::new(Vec3::new(1.36, 2.02, 1.62), Vec3::new(2.28, 2.40, 1.98));
        let bunk_box = Aabb3::new(Vec3::new(-2.34, 0.38, -1.92), Vec3::new(-0.56, 0.72, -0.50));

        let candidates = [
            (radio_box, TargetId::VhfPadRadio),
            (clip_box, TargetId::Clipboard),
            (kettle_box, TargetId::KettleBody),
            (ac_box, TargetId::AcUnit),
            (bunk_box, TargetId::SleepingBunk),
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
        self.hovered_target = self.cast_ray_target(self.look_dir());
    }

    pub fn actuate_world_ray(&mut self, dir: Vec3, hover_only: bool, mark_mode: bool, secondary: bool) -> TargetId {
        let hit = self.cast_ray_target(dir);
        self.hovered_target = hit;
        if !hover_only && hit != TargetId::None {
            self.actuate_target(hit, mark_mode, secondary);
            self.prov_ids = self.hab.build_provenance(&self.solar, &mut self.prov_graph);
        }
        hit
    }

    /// Advances simulation by `dt_ms` milliseconds using deterministic fixed-step sub-stepping.
    pub fn step_ms(&mut self, dt_ms: u32) {
        let clamped_ms = if dt_ms > 250 { 250 } else { dt_ms };
        if clamped_ms == 0 {
            return;
        }
        let prev_tripped = self.hab.breaker_tripped;
        self.tick_ms += clamped_ms as u64;
        let dt_s = (clamped_ms as f64) * 0.001;

        // Real-time progression of solar angle (1 real sec = 1 real sec!)
        self.local_hour_jst += dt_s / 3600.0;
        if self.local_hour_jst >= 24.0 {
            self.local_hour_jst -= 24.0;
        }
        self.solar = sun_state_uchinoura(self.local_hour_jst);
        self.hab.step(dt_s, self.local_hour_jst, &self.solar);

        if !prev_tripped && self.hab.breaker_tripped {
            self.pending_breaker_trip = true;
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

        // Evaluate China-marker predictions when timer completes (Competence Mirror, Bible §2.2a!)
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

        // Distant Coastal Pad-1 Static Fire & 343 m/s Acoustic Wavefront Propagation!
        let now_s = self.sim_time_s();
        // Trigger first unscripted morning static fire at T+9.0s (Bible §8: "count ~8s, then the windows rattle")
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
            // Exact analytical closed-form damped 18 Hz glass-pane oscillation after shockwave arrival!
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
        fan_still && plume_done && rattle_done && kettle_idle && comp_settled
    }

    pub fn dynamic_scene_params(&self) -> DynamicSceneParams {
        let now_s = self.sim_time_s();
        let age = if self.static_fire_active {
            (now_s - self.static_fire_start_s) as f32
        } else {
            99.0
        };
        // 5.2-second static-fire rocket burn profile
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
            room_temp_c: (self.hab.room_air_temp_k - 273.15) as f32,
            bus_power_w: self.hab.total_bus_power_w as f32,
            bus_voltage_v: self.hab.bus_voltage_v as f32,
            gauge_marks: self.gauge_marks,
            plume_intensity,
            plume_age_s: age,
            shockwave_radius_m: shock_r,
        }
    }

    /// Computes the deterministic 64-bit Proof-of-Flight state hash.
    pub fn state_hash(&self) -> u64 {
        let mut h = DetHasher::new();
        h.write_u64(self.tick_ms);
        h.write_f64(self.local_hour_jst);
        h.write_f64(self.hab.room_air_temp_k);
        h.write_f64(self.hab.total_bus_power_w);
        h.write_f64(self.hab.bus_voltage_v);
        h.write_f64(self.hab.shoji_closed);
        h.write_f64(self.hab.window_open);
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

        // At +7.5s, 7.5 * 349 m/s = 2617 m < 2820 m -> MUST NOT have arrived yet!
        for _ in 0..75 {
            w.step_ms(100);
        }
        assert!(!w.shockwave_arrived, "Shockwave arrived too early before 2820m / c_sound!");

        // By +8.4s, 8.4 * 349 m/s = 2931 m > 2820 m -> MUST have arrived and rattled the window!
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
}
