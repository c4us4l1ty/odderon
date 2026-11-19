//! The contract-side thermal design and its Level-3 evaluation.
//!
//! `ThermalDesign` is the player-facing, *physical* description of a stage's
//! cryogenic fit-out. It carries no boil-off number: the loss is recomputed
//! from these specs along the flown trajectory, so a client cannot claim a
//! smaller loss than its own hardware produces. This is the no-magic-lookup
//! law applied to tanks, and it is what FDR v3 signs.

use crate::{environment, Contract};
use odd_astro::integrator::IntegrationError;
use odd_astro::spline::TrajectorySpline;
use odd_ephem::Ephemeris;
use odd_propulsion::{EngineSpec, Propellant};
use odd_systems::{
    BusSpec, Fluid, ShellMaterial, StageThermal, StageThermalHistory, ThermalClock,
    ThermalEnvironment,
};

/// Wire id for the tank shell, frozen as part of FDR v3. Delegates to the
/// enum so there is exactly one mapping in the repository.
pub fn shell_from_u8(id: u8) -> Option<ShellMaterial> {
    ShellMaterial::from_u8(id)
}

/// Physical cryogenic fit-out. Every field is a design choice or a material
/// property; none is a performance claim.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThermalDesign {
    pub mli_layers: u32,
    /// Blanket emittance. Must be at least the ideal value for the layer
    /// count; the tank validator enforces that floor.
    pub epsilon_eff: f64,
    pub shell: u8,
    /// Solar absorptivity of the outermost layer.
    pub absorptivity: f64,
    pub outer_epsilon: f64,
    /// Lumped strut/penetration conductance, W/K.
    pub strut_conductance_w_k: f64,
    pub relief_pressure_pa: f64,
    pub radiator_area_m2: f64,
    pub radiator_epsilon: f64,
    /// Commanded heat lift at the cold tip, W. Zero is passive-only.
    pub cryocooler_lift_w: f64,
    /// Fraction of Carnot the machine achieves, in (0, 1].
    pub cryocooler_carnot_fraction: f64,
    pub bus: BusSpec,
    pub structure_temperature_k: f64,
    pub sink_temperature_k: f64,
}

impl ThermalDesign {
    /// A conservative flight-worthy default: 20 layers, no active cooling.
    /// This is the baseline the contracts are tuned against.
    pub fn baseline() -> Self {
        Self {
            mli_layers: 20,
            // Four times the ideal blanket: seams, penetrations and
            // compression at the supports all degrade a real installation.
            epsilon_eff: odd_systems::ideal_epsilon_for_layers(20) * 4.0,
            shell: 0,
            absorptivity: 0.2,
            outer_epsilon: 0.8,
            strut_conductance_w_k: 0.5,
            relief_pressure_pa: 200_000.0,
            radiator_area_m2: 0.0,
            radiator_epsilon: 0.8,
            cryocooler_lift_w: 0.0,
            cryocooler_carnot_fraction: 0.25,
            bus: BusSpec {
                array_w_at_1au: 3_000.0,
                avionics_w: 300.0,
                electric_propulsion_w: 0.0,
            },
            structure_temperature_k: 293.0,
            sink_temperature_k: 3.0,
        }
    }

    /// Build both tanks for a stage. Returns `None` for storable propellants
    /// or if a cryogenic stage cannot be constructed. Callers must not treat
    /// the latter as a valid zero-loss result.
    /// Resolve this design into a tank pair for a load.
    ///
    /// `revision` selects the geometry rule. v4 derives both tanks from one
    /// `TankGeometry` so the stage fits the airframe it is signed into. v3 and
    /// older records were signed against a model that padded each tank by 5 %
    /// independently, so replaying them under the new rule would move their
    /// masses and vent histories — a historical certificate must keep the
    /// physics it was certified under, so the old rule stays available and is
    /// reached only through the record's own schema.
    pub fn stage(
        &self,
        contract: &Contract,
        spec: &EngineSpec,
        load_kg: f64,
    ) -> Option<StageThermal> {
        self.stage_at(
            contract,
            spec,
            load_kg,
            odd_core::PhysicsRevision::CoupledFitted,
        )
    }

    /// Resolve this design into a tank pair under a specific physics revision.
    pub fn stage_at(
        &self,
        contract: &Contract,
        spec: &EngineSpec,
        load_kg: f64,
        revision: odd_core::PhysicsRevision,
    ) -> Option<StageThermal> {
        if !odd_systems::needs_thermal_model(spec.propellant) {
            return None;
        }
        let coupled = revision.couples_vented_mass();
        let mut stage = StageThermal::new_at(spec, load_kg, self.mli_layers, coupled).ok()?;
        if !coupled {
            // Frozen rule: the per-tank 5 % pad, and the contract volume as a
            // per-tank ceiling rather than a shared airframe budget.
            for tank in [&mut stage.oxidizer, &mut stage.fuel] {
                tank.volume_m3 = libm::fmin(tank.volume_m3, contract.tank_volume_m3);
            }
            return self.finish(stage);
        }
        // The card's volume is a *shared* airframe constraint, not a per-tank
        // ceiling. Both tanks are sized from the same load and then scaled to
        // the airframe together, so an overbooked load has to be clipped rather
        // than silently given two full airframes.
        let geometry = crate::derived_geometry(contract, load_kg)?;
        stage.oxidizer.volume_m3 = geometry.tank_volumes_m3[0];
        stage.fuel.volume_m3 = geometry.tank_volumes_m3[1];
        let airframe = contract.tank_volume_m3;
        let signed = stage.oxidizer.volume_m3 + stage.fuel.volume_m3;
        if signed > airframe {
            let scale = airframe / signed;
            stage.oxidizer.volume_m3 *= scale;
            stage.fuel.volume_m3 *= scale;
        }
        self.finish(stage)
    }

    /// Apply the design's blanket, shell and power settings to a sized pair.
    fn finish(&self, mut stage: StageThermal) -> Option<StageThermal> {
        let shell = shell_from_u8(self.shell)?;
        for tank in [&mut stage.oxidizer, &mut stage.fuel] {
            tank.epsilon_eff = self.epsilon_eff;
            tank.shell = shell;
            tank.absorptivity = self.absorptivity;
            tank.outer_epsilon = self.outer_epsilon;
            tank.strut_conductance_w_k = self.strut_conductance_w_k;
            tank.relief_pressure_pa = self.relief_pressure_pa;
            tank.radiator_area_m2 = self.radiator_area_m2;
            tank.radiator_epsilon = self.radiator_epsilon;
            tank.cryocooler_lift_w = self.cryocooler_lift_w;
            tank.cryocooler_carnot_fraction = self.cryocooler_carnot_fraction;
        }
        stage.bus = self.bus;
        stage.structure_temperature_k = self.structure_temperature_k;
        stage.sink_temperature_k = self.sink_temperature_k;
        Some(stage)
    }
}

/// Body vectors from a tank at `position` (Earth-centred, metres) to the Sun,
/// Earth centre and Moon, using the same ephemeris the propagator used.
pub fn thermal_environment(
    eph: &Ephemeris,
    epoch: i64,
    t: f64,
    position: [f64; 3],
) -> Result<ThermalEnvironment, IntegrationError> {
    let env = environment(eph, epoch, t)?;
    let to = |body: [f64; 3]| {
        [
            body[0] - position[0],
            body[1] - position[1],
            body[2] - position[2],
        ]
    };
    Ok(ThermalEnvironment {
        sun: to(env.sun.position),
        // The tank's position is Earth-centred, so the vector back to the
        // Earth's centre is simply its negation.
        earth: [-position[0], -position[1], -position[2]],
        moon: to(env.companion.position),
    })
}

/// Thermal-model loss along the flight and its bus cost. The current orbital
/// propagator does not subtract this loss from its mass or fuel budget.
#[derive(Clone, Debug)]
pub struct ThermalOutcome {
    pub lost_kg: f64,
    /// Oxidizer then fuel loss, kg. Signed separately because the vent
    /// *openings* can be identical while the integrated loss differs.
    pub tank_lost_kg: [f64; 2],
    pub cooler_energy_j: f64,
    pub vents: Vec<odd_systems::VentEvent>,
    /// True when the pair is storable and no tank model applies.
    pub inert: bool,
}

impl ThermalOutcome {
    pub fn inert() -> Self {
        Self {
            lost_kg: 0.0,
            tank_lost_kg: [0.0; 2],
            cooler_energy_j: 0.0,
            vents: Vec::new(),
            inert: true,
        }
    }
}

/// The stage whose tanks are being evaluated: its contract, its physical
/// engine design, the load aboard and the cryogenic fit-out.
#[derive(Clone, Copy, Debug)]
pub struct ThermalStage<'a> {
    pub contract: &'a Contract,
    pub spec: &'a EngineSpec,
    pub load_kg: f64,
    pub design: &'a ThermalDesign,
}

/// Evaluate boil-off along a flown trajectory. This is the single code path
/// both the live planner and replay verification use.
pub fn evaluate(
    stage_spec: ThermalStage<'_>,
    trajectory: &TrajectorySpline,
    clock: ThermalClock,
    eph: &Ephemeris,
    epoch: i64,
) -> Result<ThermalOutcome, IntegrationError> {
    evaluate_at(
        stage_spec,
        trajectory,
        clock,
        eph,
        epoch,
        odd_core::PhysicsRevision::CoupledFitted,
    )
}

/// As `evaluate`, under a specific physics revision. A historical record must
/// be evaluated with the geometry rule it was signed under, so the revision is
/// threaded rather than assumed to be the newest one.
pub fn evaluate_at(
    stage_spec: ThermalStage<'_>,
    trajectory: &TrajectorySpline,
    clock: ThermalClock,
    eph: &Ephemeris,
    epoch: i64,
    revision: odd_core::PhysicsRevision,
) -> Result<ThermalOutcome, IntegrationError> {
    let ThermalStage {
        contract,
        spec,
        load_kg,
        design,
    } = stage_spec;
    if !odd_systems::needs_thermal_model(spec.propellant) {
        return Ok(ThermalOutcome::inert());
    }
    // `stage` also returns None for an invalid cryogenic design or load. It
    // must never be mistaken for a storable stage with zero boil-off.
    let stage = design
        .stage_at(contract, spec, load_kg, revision)
        .ok_or(IntegrationError::InvalidInput)?;
    let history: StageThermalHistory = stage.integrate(spec, load_kg, clock, |t| {
        let frame = trajectory
            .sample(t)
            .map_err(|_| IntegrationError::ForceFailure)?;
        thermal_environment(eph, epoch, t, frame.position)
    })?;
    Ok(ThermalOutcome {
        lost_kg: history.lost_kg,
        tank_lost_kg: [
            history.oxidizer.state.vented_kg,
            history.fuel.state.vented_kg,
        ],
        cooler_energy_j: history.cooler_energy_j,
        vents: history.vents,
        inert: false,
    })
}

/// The fluids a propellant pair puts in the two tanks, for the signed record.
pub fn tank_fluids(propellant: Propellant) -> (Fluid, Fluid) {
    Fluid::pair(propellant)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_wire_ids_are_frozen() {
        // These ids are signed into FDR v3. Reordering them would silently
        // change what every existing certificate means.
        assert_eq!(shell_from_u8(0), Some(ShellMaterial::Aluminium2219));
        assert_eq!(shell_from_u8(1), Some(ShellMaterial::Stainless304));
        assert_eq!(shell_from_u8(2), Some(ShellMaterial::CompositeOverwrap));
        assert_eq!(shell_from_u8(3), None);
    }

    #[test]
    fn storables_have_no_tank_model() {
        let contract = Contract::get(101).unwrap();
        let design = ThermalDesign::baseline();
        assert!(design
            .stage(&contract, &contract.engine_spec, 180.0)
            .is_none());
    }

    #[test]
    fn the_tug_gets_two_real_tanks() {
        let contract = Contract::get(103).unwrap();
        let design = ThermalDesign::baseline();
        let stage = design
            .stage(&contract, &contract.engine_spec, 12_800.0)
            .expect("a hydrolox tug needs tanks");
        assert_eq!(stage.oxidizer.fluid, Fluid::Lox);
        assert_eq!(stage.fuel.fluid, Fluid::Lh2);
        assert!(stage.fuel.volume_m3 > 0.0);
        assert!(stage.oxidizer.validate().is_ok());
        assert!(stage.fuel.validate().is_ok());
    }
}
