#![no_std]

use odderon_dmath::{
    clamp_f64, dsin, dsqrt, EpistemicSource, ProvGraph, ProvId, SolarState, PI,
};

/// Physical state & actuators for the 1D Habitation / Spacecraft Thermal & Power Solver.
#[derive(Clone, Copy, Debug)]
pub struct HabThermoState {
    // --- Actuators (player-controlled physical hardware) ---
    /// Sliding shoji screen closure: 0.0 = wide open, 1.0 = fully closed across glazing.
    pub shoji_closed: f64,
    /// Sliding glass window sash opening: 0.0 = sealed, 1.0 = open to coastal air.
    pub window_open: f64,
    /// Main 15A wall breaker switch.
    pub main_breaker_on: bool,
    /// Dedicated AC unit breaker / power toggle.
    pub ac_switch_on: bool,
    /// AC thermostat target temperature (Kelvin, default 297.15 K = 24.0 C).
    pub ac_setpoint_k: f64,
    /// Overhead ceiling fan switch.
    pub fan_switch_on: bool,
    /// Shoji floor andon lantern + desk task lamp switch.
    pub lamp_switch_on: bool,
    /// Stainless electric kettle switch (auto-releases at 373.15 K boil).
    pub kettle_switch_on: bool,

    // --- Conserved Dynamic State Variables ---
    /// Cabin / room air temperature (Kelvin).
    pub room_air_temp_k: f64,
    /// Interior timber/tatami structural surface temperature (Kelvin).
    pub wall_mass_temp_k: f64,
    /// Evaporator coil metal temperature (Kelvin).
    pub ac_evap_temp_k: f64,
    /// Compressor normalized modulation [0.0 .. 1.0] (with physical spool lag).
    pub ac_compressor_mod: f64,
    /// Kettle water temperature (Kelvin, 0.75 kg H2O).
    pub kettle_water_temp_k: f64,
    /// Thermal overload accumulator on the 15A bi-metallic breaker strip [0.0 .. 1.0].
    pub breaker_thermal_load: f64,
    /// Whether the main breaker tripped due to over-current (>15 A sustained).
    pub breaker_tripped: bool,

    // --- Solved Telemetry & Fluxes ---
    pub outdoor_temp_k: f64,
    pub q_solar_gain_w: f64,
    pub q_wall_leak_w: f64,
    pub q_ventilation_w: f64,
    pub q_internal_elec_w: f64,
    pub q_ac_cooling_w: f64,
    pub q_net_air_w: f64,
    pub ac_cop: f64,
    pub ac_elec_power_w: f64,
    pub total_bus_power_w: f64,
    pub bus_voltage_v: f64,
    pub bus_current_a: f64,

    // --- Eidos Analytical Partial Derivatives (Sensitivities) ---
    /// d(Q_solar) / d(shoji_closed) [Watts]
    pub sens_qsolar_d_shoji_w: f64,
    /// d(P_AC_steady) / d(shoji_closed) [Watts electrical]
    pub sens_pac_d_shoji_w: f64,
    /// d(T_room_equilibrium) / d(shoji_closed) [Kelvin]
    pub sens_teq_d_shoji_k: f64,
    /// d(V_bus) / d(I_bus) [Volts per Amp]
    pub sens_vbus_d_ibus: f64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct HabProvenanceIds {
    pub solar_flux: ProvId,
    pub q_solar: ProvId,
    pub room_temp: ProvId,
    pub ac_cop: ProvId,
    pub ac_power: ProvId,
    pub bus_power: ProvId,
    pub bus_voltage: ProvId,
}

impl Default for HabThermoState {
    fn default() -> Self {
        Self::new_summer_morning()
    }
}

impl HabThermoState {
    /// Exact geometric window aperture from odderon-geom: 2.81 m wide x 1.49 m high = 4.187 m^2.
    pub const GLAZING_AREA_M2: f64 = 4.187;
    pub const SHGC_OPEN: f64 = 0.65;
    pub const SHGC_SHOJI: f64 = 0.052;
    pub const UA_ENVELOPE_WK: f64 = 18.0;
    pub const NOMINAL_GRID_V: f64 = 100.0;
    pub const BUS_LINE_OHMS: f64 = 0.19;
    pub const BREAKER_LIMIT_A: f64 = 15.0;

    pub fn new_summer_morning() -> Self {
        let mut s = Self {
            shoji_closed: 0.18, // Partially open at wake-up so golden sunbeams slice across desk & tatami!
            window_open: 0.15,  // Cracked slightly so cicadas & breeze drift in
            main_breaker_on: true,
            ac_switch_on: true,
            ac_setpoint_k: 297.15, // 24.0 °C
            fan_switch_on: true,
            lamp_switch_on: true,
            kettle_switch_on: false,

            room_air_temp_k: 299.45, // 26.3 °C warm summer room fighting sunlight
            wall_mass_temp_k: 299.65,
            ac_evap_temp_k: 286.5,
            ac_compressor_mod: 0.86,
            kettle_water_temp_k: 299.15,
            breaker_thermal_load: 0.0,
            breaker_tripped: false,

            outdoor_temp_k: 303.85,
            q_solar_gain_w: 0.0,
            q_wall_leak_w: 0.0,
            q_ventilation_w: 0.0,
            q_internal_elec_w: 0.0,
            q_ac_cooling_w: 0.0,
            q_net_air_w: 0.0,
            ac_cop: 3.4,
            ac_elec_power_w: 0.0,
            total_bus_power_w: 0.0,
            bus_voltage_v: 100.0,
            bus_current_a: 0.0,

            sens_qsolar_d_shoji_w: 0.0,
            sens_pac_d_shoji_w: 0.0,
            sens_teq_d_shoji_k: 0.0,
            sens_vbus_d_ibus: -Self::BUS_LINE_OHMS,
        };
        let sun = odderon_dmath::sun_state_uchinoura(9.75);
        s.step(0.05, 9.75, &sun);
        s
    }

    /// Advances the coupled 1D thermal, vapour-compression, and electrical bus network by `dt_s`.
    pub fn step(&mut self, dt_s: f64, local_hour_jst: f64, solar: &SolarState) {
        let dt = clamp_f64(dt_s, 0.0, 0.25);

        // Diurnal coastal outdoor temperature curve in Uchinoura (25.5 C pre-dawn -> 31.8 C at 14:00 JST)
        let diurnal_phase = (local_hour_jst - 14.0) * (PI / 12.0);
        self.outdoor_temp_k = 298.65 + 3.15 + 3.15 * odderon_dmath::dcos(diurnal_phase);

        // 1. Solar radiative heat gain through glazing & sliding shoji screen
        let shoji = clamp_f64(self.shoji_closed, 0.0, 1.0);
        let win_open = clamp_f64(self.window_open, 0.0, 1.0);
        let eff_shgc = (1.0 - shoji) * Self::SHGC_OPEN + shoji * Self::SHGC_SHOJI;
        self.q_solar_gain_w = solar.window_incident_wm2 * Self::GLAZING_AREA_M2 * eff_shgc;

        // 2. Conductive envelope leak + open-window coastal air ventilation
        self.q_wall_leak_w = Self::UA_ENVELOPE_WK * (self.outdoor_temp_k - self.room_air_temp_k);
        let ua_vent = 4.5 + win_open * 135.0;
        self.q_ventilation_w = ua_vent * (self.outdoor_temp_k - self.room_air_temp_k);

        // 3. Electrical bus loads & vapour-compression AC cycle
        let bus_energized = self.main_breaker_on && !self.breaker_tripped;

        // Carnot-derated heat pump COP
        let t_evap_target = clamp_f64(self.room_air_temp_k - 11.5, 274.5, 295.0);
        let t_cond = self.outdoor_temp_k + 10.5;
        let lift_k = clamp_f64(t_cond - self.ac_evap_temp_k, 8.0, 55.0);
        let carnot_cop = self.ac_evap_temp_k / lift_k;
        self.ac_cop = clamp_f64(0.44 * carnot_cop, 1.8, 4.6);

        // Inverter compressor controller responds to room temp error + solar thermal load
        let target_mod = if bus_energized && self.ac_switch_on {
            let temp_err = self.room_air_temp_k - self.ac_setpoint_k;
            // Feed-forward + proportional thermal demand so closing shoji visibly throttles compressor within seconds
            let steady_heat_demand = clamp_f64(
                self.q_solar_gain_w + Self::UA_ENVELOPE_WK * (self.outdoor_temp_k - self.ac_setpoint_k) + 110.0,
                0.0,
                2600.0,
            );
            let demand_mod = steady_heat_demand / (740.0 * self.ac_cop);
            let fb_mod = temp_err * 0.13;
            if temp_err < -0.6 {
                0.0
            } else {
                clamp_f64(0.72 * demand_mod + fb_mod, 0.06, 1.0)
            }
        } else {
            0.0
        };

        // Compressor mechanical spool/thermal lag (~3.5 s time constant)
        let alpha_comp = clamp_f64(dt / 3.2, 0.0, 1.0);
        self.ac_compressor_mod += (target_mod - self.ac_compressor_mod) * alpha_comp;

        let ac_fan_w = if bus_energized && self.ac_switch_on { 36.0 } else { 0.0 };
        let ac_comp_w = if bus_energized && self.ac_switch_on {
            725.0 * self.ac_compressor_mod
        } else {
            0.0
        };
        self.ac_elec_power_w = ac_fan_w + ac_comp_w;
        self.q_ac_cooling_w = ac_comp_w * self.ac_cop;

        // Update evaporator coil temperature
        let evap_goal = if ac_comp_w > 5.0 {
            t_evap_target
        } else {
            self.room_air_temp_k
        };
        self.ac_evap_temp_k += (evap_goal - self.ac_evap_temp_k) * clamp_f64(dt / 4.0, 0.0, 1.0);

        // Other physical loads on the 100V Hab Bus
        let ceil_fan_w = if bus_energized && self.fan_switch_on { 42.0 } else { 0.0 };
        let lamp_w = if bus_energized && self.lamp_switch_on { 48.0 } else { 0.0 };
        let avionics_desk_w = if bus_energized { 64.0 } else { 0.0 }; // CRT scope + VHF receiver
        let kettle_w = if bus_energized && self.kettle_switch_on {
            1060.0
        } else {
            0.0
        };

        self.total_bus_power_w = self.ac_elec_power_w + ceil_fan_w + lamp_w + avionics_desk_w + kettle_w;

        // Solve V_bus * I_bus = P_total with V_bus = V_src - I_bus * R_line:
        // R*I^2 - V_src*I + P = 0 => I = (V_src - sqrt(V_src^2 - 4*R*P)) / (2*R)
        if bus_energized && self.total_bus_power_w > 0.01 {
            let v0 = Self::NOMINAL_GRID_V;
            let r = Self::BUS_LINE_OHMS;
            let disc = clamp_f64(v0 * v0 - 4.0 * r * self.total_bus_power_w, 1.0, v0 * v0);
            self.bus_current_a = (v0 - dsqrt(disc)) / (2.0 * r);
            self.bus_voltage_v = v0 - self.bus_current_a * r;
        } else {
            self.bus_current_a = 0.0;
            self.bus_voltage_v = if bus_energized { Self::NOMINAL_GRID_V } else { 0.0 };
        }

        // Bi-metallic 15A thermal breaker trip physics (I^2 * t heating)
        if self.bus_current_a > Self::BREAKER_LIMIT_A {
            let over_ratio = self.bus_current_a / Self::BREAKER_LIMIT_A;
            self.breaker_thermal_load += (over_ratio * over_ratio - 1.0) * dt * 0.45;
            if self.breaker_thermal_load >= 1.0 {
                self.breaker_thermal_load = 1.0;
                self.breaker_tripped = true;
                self.main_breaker_on = false;
            }
        } else {
            self.breaker_thermal_load = clamp_f64(self.breaker_thermal_load - dt * 0.18, 0.0, 1.0);
        }

        // Kettle water heating (0.45 kg water for fast tactile morning tea boil + convective cooling)
        let c_water_j_per_k = 0.45 * 4184.0;
        let q_kettle_loss = 3.8 * (self.kettle_water_temp_k - self.room_air_temp_k);
        let d_twater = (kettle_w * 0.94 - q_kettle_loss) / c_water_j_per_k;
        self.kettle_water_temp_k = clamp_f64(self.kettle_water_temp_k + d_twater * dt, 285.0, 373.4);
        if self.kettle_water_temp_k >= 373.15 && self.kettle_switch_on {
            // Bi-metal steam auto-shutoff click!
            self.kettle_switch_on = false;
        }

        // Internal electrical waste heat + human metabolic heat (85 W)
        let metabolic_w = 85.0;
        self.q_internal_elec_w = metabolic_w + ceil_fan_w + lamp_w + avionics_desk_w + ac_fan_w + q_kettle_loss + kettle_w * 0.06;

        // 2-Node Room Thermal Network:
        // Fast responsive room air volume (effective mixing zone ~18 kg air -> C_air = 18,100 J/K)
        // coupled to deeper timber/tatami thermal mass (C_wall = 95,000 J/K)
        let ua_air_wall = if bus_energized && self.fan_switch_on { 125.0 } else { 72.0 };
        let q_from_wall = ua_air_wall * (self.wall_mass_temp_k - self.room_air_temp_k);

        // 72% of solar beam heats sunlit air/desk boundary layer immediately; 28% soaks into tatami/wall mass
        self.q_net_air_w = 0.72 * self.q_solar_gain_w
            + self.q_wall_leak_w
            + self.q_ventilation_w
            + self.q_internal_elec_w
            + q_from_wall
            - self.q_ac_cooling_w;

        let q_net_wall_w = 0.28 * self.q_solar_gain_w - q_from_wall;

        let c_air = 18_100.0;
        let c_wall = 95_000.0;
        self.room_air_temp_k = clamp_f64(self.room_air_temp_k + (self.q_net_air_w / c_air) * dt, 283.0, 325.0);
        self.wall_mass_temp_k = clamp_f64(self.wall_mass_temp_k + (q_net_wall_w / c_wall) * dt, 283.0, 325.0);

        // 4. Analytical Eidos Sensitivities (Partial Derivatives at current state)
        self.sens_qsolar_d_shoji_w =
            solar.window_incident_wm2 * Self::GLAZING_AREA_M2 * (Self::SHGC_SHOJI - Self::SHGC_OPEN);
        self.sens_pac_d_shoji_w = if bus_energized && self.ac_switch_on {
            (self.sens_qsolar_d_shoji_w / self.ac_cop) * 0.55
        } else {
            0.0
        };
        let total_ua = Self::UA_ENVELOPE_WK + ua_vent + if bus_energized && self.ac_switch_on { 725.0 * self.ac_cop * 0.28 } else { 0.0 };
        self.sens_teq_d_shoji_k = self.sens_qsolar_d_shoji_w / clamp_f64(total_ua, 10.0, 2000.0);
        self.sens_vbus_d_ibus = -Self::BUS_LINE_OHMS;

        let _ = dsin(0.0);
    }

    /// Builds the zero-allocation Provenance lineage DAG on demand for Eidos / gauge inspection.
    pub fn build_provenance(&self, solar: &SolarState, graph: &mut ProvGraph) -> HabProvenanceIds {
        graph.clear();
        let solar_flux = graph.push_leaf(
            solar.window_incident_wm2,
            solar.window_incident_wm2 * 0.035, // +/- 3.5% clear-sky aerosol turbidity uncertainty
            EpistemicSource::AstronomicalCatalog,
        );
        let shoji_pos = graph.push_leaf(
            self.shoji_closed,
            0.01,
            EpistemicSource::MeasuredSensor,
        );
        let eff_shgc = (1.0 - self.shoji_closed) * Self::SHGC_OPEN + self.shoji_closed * Self::SHGC_SHOJI;
        let q_solar = graph.push_derived(
            self.q_solar_gain_w,
            18.0, // glazing optical model sigma
            EpistemicSource::DerivedConservation,
            &[
                (solar_flux, Self::GLAZING_AREA_M2 * eff_shgc),
                (shoji_pos, self.sens_qsolar_d_shoji_w),
            ],
        );
        let room_temp = graph.push_leaf(
            self.room_air_temp_k - 273.15, // Celsius display with +/- 0.15 C thermistor noise
            0.15,
            EpistemicSource::MeasuredSensor,
        );
        let ac_cop = graph.push_leaf(
            self.ac_cop,
            0.22, // Empirical Carnot-derating correlation uncertainty
            EpistemicSource::EmpiricalModel,
        );
        let ac_power = graph.push_derived(
            self.ac_elec_power_w,
            6.5,
            EpistemicSource::DerivedConservation,
            &[
                (q_solar, if self.ac_switch_on { 0.55 / self.ac_cop } else { 0.0 }),
                (ac_cop, -self.q_ac_cooling_w / (self.ac_cop * self.ac_cop)),
            ],
        );
        let bus_power = graph.push_derived(
            self.total_bus_power_w,
            4.2, // Hall-effect current shunt sigma
            EpistemicSource::MeasuredSensor,
            &[(ac_power, 1.0)],
        );
        let bus_voltage = graph.push_derived(
            self.bus_voltage_v,
            0.12,
            EpistemicSource::MeasuredSensor,
            &[(bus_power, -Self::BUS_LINE_OHMS / 100.0)],
        );

        HabProvenanceIds {
            solar_flux,
            q_solar,
            room_temp,
            ac_cop,
            ac_power,
            bus_power,
            bus_voltage,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use odderon_dmath::sun_state_uchinoura;

    #[test]
    fn closing_shoji_drops_solar_gain_and_ac_power_and_cools_room() {
        let sun = sun_state_uchinoura(9.75);
        let mut hab_open = HabThermoState::new_summer_morning();
        hab_open.shoji_closed = 0.0;
        for _ in 0..200 {
            hab_open.step(0.1, 9.75, &sun);
        }

        let mut hab_closed = hab_open;
        hab_closed.shoji_closed = 1.0;
        for _ in 0..200 {
            hab_closed.step(0.1, 9.75, &sun);
        }

        assert!(
            hab_open.q_solar_gain_w > 1100.0,
            "Expected strong summer solar gain > 1100 W, got {}",
            hab_open.q_solar_gain_w
        );
        assert!(
            hab_closed.q_solar_gain_w < 220.0,
            "Expected closed shoji gain < 220 W, got {}",
            hab_closed.q_solar_gain_w
        );
        assert!(
            hab_closed.ac_elec_power_w + 180.0 < hab_open.ac_elec_power_w,
            "AC power draw should fall substantially: open={} W vs closed={} W",
            hab_open.ac_elec_power_w,
            hab_closed.ac_elec_power_w
        );
        assert!(
            hab_closed.room_air_temp_k < hab_open.room_air_temp_k,
            "Room should cool when shoji closes"
        );
    }

    #[test]
    fn overload_trips_15a_breaker_when_ac_and_kettle_exceed_limit() {
        let sun = sun_state_uchinoura(12.0);
        let mut hab = HabThermoState::new_summer_morning();
        hab.shoji_closed = 0.0;
        hab.ac_compressor_mod = 1.0;
        hab.room_air_temp_k = 306.0;
        hab.kettle_switch_on = true;
        for _ in 0..120 {
            hab.step(0.1, 12.0, &sun);
        }
        assert!(hab.breaker_tripped, "15A breaker should trip under sustained >18A load");
        assert_eq!(hab.total_bus_power_w, 0.0);
    }
}
