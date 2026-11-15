//! WP 1.7 — the Chief Engineer abstraction layer.
//!
//! Four deterministic rule-of-thumb department heads let a beginner fly a
//! stack they could not yet size themselves. Delegation is **never free and
//! never hidden**:
//!
//! * the delegated discipline's hardware carries **+35 % mass** and
//!   **+30 % cost**, because a rule of thumb buys margin where a worked
//!   solution buys accuracy;
//! * the vehicle assembly shows a badge naming the chief and the exact
//!   kilograms it added;
//! * the efficiency component of the score is **capped at 0.65** whenever any
//!   discipline is delegated, so an auto-sized stack can complete a contract
//!   but cannot top the board against a hand-worked one;
//! * the bitmask travels in the signed FDR, so a reader can always tell which
//!   decisions were the player's.
//!
//! There is no randomness, no model call and no hidden state: every chief is
//! a pure function of the physical inputs, so a replay reproduces the same
//! margins and the same badge text.

use crate::thermal::ThermalDesign;
use odd_systems::{BusSpec, ShellMaterial};

/// Mass margin a chief adds to the hardware it sizes.
pub const MASS_MARGIN: f64 = 0.35;
/// Cost margin a chief adds to the hardware it sizes.
pub const COST_MARGIN: f64 = 0.30;
/// Ceiling on the efficiency component when any discipline is delegated.
pub const SEFF_CAP: f64 = 0.65;

/// The four department heads. Bit positions are frozen as part of FDR v3:
/// reordering them would change what every existing certificate means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Department {
    Propulsion = 0,
    Thermal = 1,
    Power = 2,
    Gnc = 3,
}

impl Department {
    pub const ALL: [Self; 4] = [Self::Propulsion, Self::Thermal, Self::Power, Self::Gnc];

    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Propulsion),
            1 => Some(Self::Thermal),
            2 => Some(Self::Power),
            3 => Some(Self::Gnc),
            _ => None,
        }
    }

    pub fn bit(self) -> u8 {
        1u8 << (self as u8)
    }

    /// The name shown on the badge.
    pub fn chief_name(self) -> &'static str {
        match self {
            Self::Propulsion => "AI Propulsion Chief",
            Self::Thermal => "AI Thermal Chief",
            Self::Power => "AI Power Chief",
            Self::Gnc => "AI GNC Chief",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Propulsion => "Propulsion",
            Self::Thermal => "Thermal / Cryo",
            Self::Power => "Power",
            Self::Gnc => "GNC",
        }
    }
}

/// Which disciplines the player handed to a chief. Stored verbatim in the
/// signed record; only the low four bits are meaningful.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DelegationMask(pub u8);

impl DelegationMask {
    pub const MANUAL: Self = Self(0);
    pub const ALL: Self = Self(0b1111);
    /// Every bit outside the four departments must be zero.
    pub const VALID_BITS: u8 = 0b1111;

    pub fn is_valid(self) -> bool {
        self.0 & !Self::VALID_BITS == 0
    }

    pub fn contains(self, department: Department) -> bool {
        self.0 & department.bit() != 0
    }

    pub fn with(self, department: Department, delegated: bool) -> Self {
        if delegated {
            Self(self.0 | department.bit())
        } else {
            Self(self.0 & !department.bit())
        }
    }

    pub fn count(self) -> u32 {
        (self.0 & Self::VALID_BITS).count_ones()
    }

    pub fn any(self) -> bool {
        self.0 & Self::VALID_BITS != 0
    }

    /// The scoring ceiling this mask implies. Manual stacks are uncapped.
    pub fn efficiency_cap(self) -> f64 {
        if self.any() {
            SEFF_CAP
        } else {
            1.0
        }
    }

    /// Toggle label for the assembly UI.
    pub fn toggle_label(self, department: Department) -> &'static str {
        if self.contains(department) {
            "[AUTO-DELEGATED]"
        } else {
            "[MANUAL]"
        }
    }
}

/// The hardware each discipline would size, in kilograms, derived from the
/// physical design rather than asserted. A chief multiplies these by 1.35.
#[derive(Clone, Copy, Debug)]
pub struct DisciplineMass {
    pub department: Department,
    pub base_kg: f64,
}

/// Derive the four hardware masses from the stage's physical design.
///
/// * **Propulsion** — the engine's own dry mass, from the nozzle solver.
/// * **Thermal** — pressure-shell mass from thin-wall hoop stress plus the
///   blanket's areal density times its layer count.
/// * **Power** — array mass at a fixed specific power.
/// * **GNC** — avionics, harness and attitude hardware, scaled by the bus's
///   fixed draw (a bigger computer and more actuators cost more mass).
pub fn hardware_mass(
    engine_dry_kg: f64,
    tank_volume_m3: f64,
    tank_area_m2: f64,
    relief_pressure_pa: f64,
    design: &ThermalDesign,
) -> [DisciplineMass; 4] {
    let shell = crate::thermal::shell_from_u8(design.shell).unwrap_or(ShellMaterial::Aluminium2219);
    // Thin-wall spherical shell: m = 1.5·P·V·rho/sigma_allowable.
    let shell_kg = 1.5 * relief_pressure_pa * tank_volume_m3 * shell.density_kg_m3()
        / shell.allowable_stress_pa();
    // Aluminized mylar plus Dacron spacers, per layer per square metre.
    const MLI_AREAL_DENSITY_KG_M2: f64 = 0.0125;
    let blanket_kg = f64::from(design.mli_layers) * MLI_AREAL_DENSITY_KG_M2 * tank_area_m2;
    // Radiator panels, if active cooling is fitted.
    const RADIATOR_AREAL_DENSITY_KG_M2: f64 = 4.0;
    let radiator_kg = design.radiator_area_m2 * RADIATOR_AREAL_DENSITY_KG_M2;
    // Triple-junction array specific power, W/kg at 1 AU.
    const ARRAY_SPECIFIC_POWER_W_KG: f64 = 100.0;
    let array_kg = design.bus.array_w_at_1au / ARRAY_SPECIFIC_POWER_W_KG;
    // Avionics scales with the fixed load it must serve.
    const AVIONICS_KG_PER_W: f64 = 0.02;
    let gnc_kg = 12.0 + design.bus.avionics_w * AVIONICS_KG_PER_W;
    [
        DisciplineMass {
            department: Department::Propulsion,
            base_kg: engine_dry_kg,
        },
        DisciplineMass {
            department: Department::Thermal,
            base_kg: shell_kg + blanket_kg + radiator_kg,
        },
        DisciplineMass {
            department: Department::Power,
            base_kg: array_kg,
        },
        DisciplineMass {
            department: Department::Gnc,
            base_kg: gnc_kg,
        },
    ]
}

/// One department's decision, including the exact badge the player sees.
#[derive(Clone, Debug)]
pub struct ChiefDecision {
    pub department: Department,
    pub delegated: bool,
    pub base_kg: f64,
    /// Kilograms the chief added. Zero when the discipline is manual.
    pub margin_kg: f64,
    pub cost_multiplier: f64,
    /// Human-readable provenance, e.g.
    /// "AI Thermal Chief added 420 kg conservative margin (+35%)."
    pub badge: String,
}

/// What delegating this set of disciplines costs the stack.
#[derive(Clone, Debug)]
pub struct DelegationOutcome {
    pub mask: DelegationMask,
    pub decisions: Vec<ChiefDecision>,
    /// Total kilograms added across every delegated discipline.
    pub mass_penalty_kg: f64,
    /// Cost multiplier for the whole stack: delegated disciplines pay 1.30.
    pub cost_multiplier: f64,
    pub efficiency_cap: f64,
}

/// Apply the chiefs to a stack. `discipline_cost` is the player's baseline
/// cost per discipline, in the same arbitrary units the assembly displays.
pub fn apply(
    mask: DelegationMask,
    hardware: &[DisciplineMass],
    discipline_cost: &[f64],
) -> DelegationOutcome {
    let mut decisions = Vec::with_capacity(hardware.len());
    let mut mass_penalty_kg = 0.0;
    let mut base_cost = 0.0;
    let mut actual_cost = 0.0;
    for (index, mass) in hardware.iter().enumerate() {
        let delegated = mask.contains(mass.department);
        let margin_kg = if delegated {
            mass.base_kg * MASS_MARGIN
        } else {
            0.0
        };
        let cost_multiplier = if delegated { 1.0 + COST_MARGIN } else { 1.0 };
        let cost = discipline_cost.get(index).copied().unwrap_or(0.0);
        base_cost += cost;
        actual_cost += cost * cost_multiplier;
        mass_penalty_kg += margin_kg;
        let badge = if delegated {
            format!(
                "{} added {:.0} kg conservative margin (+35% mass, +30% cost). Efficiency capped at {:.2}.",
                mass.department.chief_name(),
                margin_kg,
                SEFF_CAP
            )
        } else {
            format!(
                "{}: [MANUAL] — player-sized, no margin.",
                mass.department.label()
            )
        };
        decisions.push(ChiefDecision {
            department: mass.department,
            delegated,
            base_kg: mass.base_kg,
            margin_kg,
            cost_multiplier,
            badge,
        });
    }
    DelegationOutcome {
        mask,
        decisions,
        mass_penalty_kg,
        cost_multiplier: if base_cost > 0.0 {
            actual_cost / base_cost
        } else {
            1.0
        },
        efficiency_cap: mask.efficiency_cap(),
    }
}

/// The Thermal Chief's rule of thumb: over-blanket, mount on the lowest
/// conductivity shell in the catalogue, and never fit a cooler the player
/// has not asked to pay for. Deterministic, and deliberately heavy.
pub fn thermal_choice() -> ThermalDesign {
    let mut design = ThermalDesign::baseline();
    // Twice the flight-proven layer count: a chief cannot see the seam
    // quality, so it buys layers instead of understanding them.
    design.mli_layers = 40;
    design.epsilon_eff = odd_systems::ideal_epsilon_for_layers(40) * 6.0;
    design.shell = ShellMaterial::CompositeOverwrap as u8;
    // Oversized struts: a rule of thumb picks for strength, not heat leak.
    design.strut_conductance_w_k = 1.2;
    design
}

/// The Power Chief's rule of thumb: size the array for the worst case with
/// margin, rather than integrating the actual eclipse duty cycle.
pub fn power_choice(avionics_w: f64, electric_propulsion_w: f64) -> BusSpec {
    BusSpec {
        // 1.35 over the sum of the loads, ignoring that they never all peak
        // at once — exactly the conservatism the mass penalty represents.
        array_w_at_1au: (avionics_w + electric_propulsion_w) * (1.0 + MASS_MARGIN),
        avionics_w,
        electric_propulsion_w,
    }
}

/// Cap the efficiency component and recompute the weighted score. Called by
/// the scorer so a delegated stack can never out-score a hand-worked one on
/// efficiency, no matter how little delta-v it commanded.
pub fn cap_score(score: &mut crate::MissionScore, mask: DelegationMask) {
    let cap = mask.efficiency_cap();
    if score.efficiency > cap {
        score.efficiency = cap;
    }
    score.s = 0.5 * score.objectives + 0.25 * score.prediction + 0.25 * score.efficiency;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hardware() -> [DisciplineMass; 4] {
        [
            DisciplineMass {
                department: Department::Propulsion,
                base_kg: 1_000.0,
            },
            DisciplineMass {
                department: Department::Thermal,
                base_kg: 400.0,
            },
            DisciplineMass {
                department: Department::Power,
                base_kg: 200.0,
            },
            DisciplineMass {
                department: Department::Gnc,
                base_kg: 100.0,
            },
        ]
    }

    #[test]
    fn department_bits_are_frozen() {
        // These bits are signed into FDR v3.
        assert_eq!(Department::Propulsion.bit(), 0b0001);
        assert_eq!(Department::Thermal.bit(), 0b0010);
        assert_eq!(Department::Power.bit(), 0b0100);
        assert_eq!(Department::Gnc.bit(), 0b1000);
        for (index, department) in Department::ALL.iter().enumerate() {
            assert_eq!(Department::from_u8(index as u8), Some(*department));
        }
        assert_eq!(Department::from_u8(4), None);
        assert_eq!(DelegationMask::ALL.count(), 4);
        assert!(!DelegationMask(0b1_0000).is_valid());
        assert!(DelegationMask::ALL.is_valid());
    }

    #[test]
    fn manual_costs_nothing_and_is_uncapped() {
        let out = apply(DelegationMask::MANUAL, &hardware(), &[10.0; 4]);
        assert_eq!(out.mass_penalty_kg, 0.0);
        assert_eq!(out.cost_multiplier, 1.0);
        assert_eq!(out.efficiency_cap, 1.0);
        assert!(out.decisions.iter().all(|d| !d.delegated));
        assert!(out.decisions.iter().all(|d| d.badge.contains("[MANUAL]")));
    }

    #[test]
    fn one_chief_adds_exactly_thirty_five_percent_of_its_own_hardware() {
        let mask = DelegationMask::MANUAL.with(Department::Thermal, true);
        let out = apply(mask, &hardware(), &[10.0; 4]);
        // 35% of the thermal discipline's 400 kg, not of the whole stack.
        assert!((out.mass_penalty_kg - 140.0).abs() < 1e-9);
        // 30% on one of four equal cost lines = 1.075 overall.
        assert!((out.cost_multiplier - 1.075).abs() < 1e-9);
        assert_eq!(out.efficiency_cap, SEFF_CAP);
        let thermal = out
            .decisions
            .iter()
            .find(|d| d.department == Department::Thermal)
            .unwrap();
        assert!(thermal.badge.contains("AI Thermal Chief"));
        assert!(thermal.badge.contains("140 kg"));
        println!("{}", thermal.badge);
    }

    #[test]
    fn all_four_chiefs_add_thirty_five_percent_of_everything() {
        let out = apply(DelegationMask::ALL, &hardware(), &[10.0; 4]);
        // 35% of 1700 kg.
        assert!((out.mass_penalty_kg - 595.0).abs() < 1e-9);
        assert!((out.cost_multiplier - 1.30).abs() < 1e-9);
        assert_eq!(out.efficiency_cap, SEFF_CAP);
    }

    #[test]
    fn the_cap_bites_only_above_the_ceiling() {
        let mut high = crate::MissionScore {
            objectives: 1.0,
            prediction: 1.0,
            efficiency: 1.0,
            s: 1.0,
        };
        cap_score(&mut high, DelegationMask::ALL);
        assert_eq!(high.efficiency, SEFF_CAP);
        // 0.5 + 0.25 + 0.25*0.65 = 0.9125
        assert!((high.s - 0.9125).abs() < 1e-12);
        let mut low = crate::MissionScore {
            objectives: 1.0,
            prediction: 1.0,
            efficiency: 0.4,
            s: 0.85,
        };
        cap_score(&mut low, DelegationMask::ALL);
        assert_eq!(low.efficiency, 0.4, "a cap must never raise a score");
        assert!((low.s - 0.85).abs() < 1e-12);
    }

    #[test]
    fn the_thermal_chief_is_conservative_and_deterministic() {
        let a = thermal_choice();
        let b = thermal_choice();
        assert_eq!(a, b);
        assert!(a.mli_layers > ThermalDesign::baseline().mli_layers);
        assert!(a.strut_conductance_w_k > ThermalDesign::baseline().strut_conductance_w_k);
        let bus = power_choice(300.0, 1_000.0);
        assert!((bus.array_w_at_1au - 1_755.0).abs() < 1e-9);
    }

    #[test]
    fn toggle_labels_round_trip() {
        let mask = DelegationMask::MANUAL.with(Department::Power, true);
        assert_eq!(mask.toggle_label(Department::Power), "[AUTO-DELEGATED]");
        assert_eq!(mask.toggle_label(Department::Gnc), "[MANUAL]");
        let cleared = mask.with(Department::Power, false);
        assert_eq!(cleared, DelegationMask::MANUAL);
    }
}
