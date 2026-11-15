//! Deterministic first-principles causal DAG for mission warnings.
//! Each warning maps to a plain-English cause chain and a one-click
//! micro-demonstration id. The graph is acyclic by construction: edges only
//! point from symptom to mechanism to cause to demo.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Warning {
    ThrustToWeightBelowOne,
    PeriapsisBelowAtmosphere,
    FiniteBurnGravityLossAbove15Percent,
    PropellantDeficit,
    ApoapsisShortfall,
    Impact,
    LongFiniteBurnArc,
    FlowSeparationAtAltitude,
    ThroatThermalOverload,
    StoichiometricTemperatureSpike,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Symptom,
    Mechanism,
    Cause,
    Demo,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub id: &'static str,
    pub kind: NodeKind,
    pub text: &'static str,
}

#[derive(Clone, Debug)]
pub struct CausalChain {
    pub warning: Warning,
    pub nodes: Vec<Node>,
    /// Directed edges parent -> child.
    pub edges: Vec<(usize, usize)>,
    pub demo: &'static str,
}

pub struct Inputs {
    pub thrust_to_weight: f64,
    pub periapsis_altitude_m: f64,
    pub atmosphere_altitude_m: f64,
    pub gravity_loss_fraction: f64,
    pub propellant_deficit: bool,
    pub apoapsis_shortfall_fraction: f64,
    pub impacted: bool,
    pub burn_sweep_deg: f64,
}

fn chain(
    warning: Warning,
    symptom: &'static str,
    mechanism: &'static str,
    cause: &'static str,
    demo: &'static str,
) -> CausalChain {
    let nodes = vec![
        Node {
            id: "symptom",
            kind: NodeKind::Symptom,
            text: symptom,
        },
        Node {
            id: "mechanism",
            kind: NodeKind::Mechanism,
            text: mechanism,
        },
        Node {
            id: "cause",
            kind: NodeKind::Cause,
            text: cause,
        },
        Node {
            id: "demo",
            kind: NodeKind::Demo,
            text: "Run the one-click micro-demonstration",
        },
    ];
    CausalChain {
        warning,
        nodes,
        edges: vec![(0, 1), (1, 2), (2, 3)],
        demo,
    }
}

/// Build all triggered warning chains in a fixed, deterministic order.
pub fn diagnose(i: &Inputs) -> Vec<CausalChain> {
    let mut out = Vec::new();
    if i.impacted {
        out.push(chain(
            Warning::Impact,
            "Impact: the trajectory reached the reference surface",
            "Propagation stopped at impact; no underground flight is modeled",
            "The orbit dipped below the central body before the burn could raise it",
            "impact",
        ));
    }
    if i.propellant_deficit {
        out.push(chain(
            Warning::PropellantDeficit,
            "The requested pushes exceed available propellant",
            "The rocket equation needs more mass than the tanks hold",
            "Delta-v demand is too high for this engine and tank size",
            "propellant",
        ));
    }
    if i.thrust_to_weight.is_finite() && i.thrust_to_weight < 1.0 {
        out.push(chain(
            Warning::ThrustToWeightBelowOne,
            "Thrust-to-weight ratio is below one",
            "Engine thrust cannot overcome local weight",
            "The vehicle is too heavy or the engine too weak for a fast lift",
            "twr",
        ));
    }
    if i.periapsis_altitude_m < i.atmosphere_altitude_m {
        out.push(chain(
            Warning::PeriapsisBelowAtmosphere,
            "Closest altitude is inside the atmosphere",
            "Drag removes energy and the flight may impact",
            "The planned orbit dips below the atmospheric reference surface",
            "periapsis",
        ));
    }
    if i.gravity_loss_fraction > 0.15 {
        out.push(chain(
            Warning::FiniteBurnGravityLossAbove15Percent,
            "Finite-burn gravity loss exceeds fifteen percent",
            "Thrust acts while gravity rotates the velocity vector",
            "The burn is spread over too large an arc instead of concentrated near periapsis",
            "gravity-loss",
        ));
    }
    if i.burn_sweep_deg > 25.0 {
        out.push(chain(
            Warning::LongFiniteBurnArc,
            "The finite burn sweeps a long arc",
            "Steering and gravity losses grow with swept angle",
            "A single long burn cannot stay aligned with the instantaneous velocity",
            "split-kick",
        ));
    }
    if i.apoapsis_shortfall_fraction > 0.02 {
        out.push(chain(
            Warning::ApoapsisShortfall,
            "The farthest point falls short of the ideal prediction",
            "Finite burns and perturbations change the outcome",
            "Energy was added away from the optimal point or removed by modeled forces",
            "apoapsis",
        ));
    }
    out
}

/// Engine-lab inputs, all plain numbers so this module stays independent of
/// the propulsion crate and the browser.
pub struct EngineInputs {
    pub altitude_m: f64,
    pub ambient_pa: f64,
    pub exit_pressure_pa: f64,
    /// Normalized separation point, or NaN when the flow stays attached.
    pub separation_x_norm: f64,
    pub side_load_kn: f64,
    pub wall_temperature_k: f64,
    pub wall_limit_k: f64,
    pub throat_heat_flux_w_m2: f64,
    pub chamber_temperature_k: f64,
    pub of_ratio: f64,
    pub stoichiometric_of: f64,
    pub peak_temperature_of: f64,
}

/// Diagnose the engine design. Same DAG shape and demo discipline as the
/// flight warnings, so the browser renders both with one code path.
pub fn diagnose_engine(i: &EngineInputs) -> Vec<CausalChain> {
    let mut out = Vec::new();
    if i.separation_x_norm.is_finite() {
        out.push(chain(
            Warning::FlowSeparationAtAltitude,
            "Flow separates inside the bell at this altitude",
            "Exit pressure is far below ambient, so the boundary layer cannot stay attached",
            "The expansion ratio is too large for the ambient pressure at this altitude",
            "separation",
        ));
    }
    if i.wall_temperature_k.is_finite() && i.wall_temperature_k > i.wall_limit_k {
        out.push(chain(
            Warning::ThroatThermalOverload,
            "Regeneratively cooled throat wall exceeds its material limit",
            "Convective heat flux outruns what the coolant channel can carry away",
            "Chamber pressure and throat heat flux are too high for this wall material",
            "thermal",
        ));
    }
    // "Hotter" is not "better": peak temperature sits near stoichiometric while
    // peak Isp sits fuel-rich because light exhaust molecules matter more.
    if i.of_ratio > i.peak_temperature_of * 0.9 {
        out.push(chain(
            Warning::StoichiometricTemperatureSpike,
            "Mixture ratio is near the temperature peak, not the efficiency peak",
            "Burning near stoichiometric maximizes chamber temperature and heat load",
            "Running fuel-rich lowers molar mass, which raises exhaust velocity even though the flame is cooler",
            "mixture",
        ));
    }
    out
}

/// Verify acyclicity and that every chain terminates in a demo.
pub fn is_valid_dag(chains: &[CausalChain]) -> bool {
    chains.iter().all(|c| {
        if c.nodes.last().map(|n| n.kind) != Some(NodeKind::Demo) {
            return false;
        }
        // Kahn topological check.
        let n = c.nodes.len();
        let mut indeg = vec![0usize; n];
        for &(_, b) in &c.edges {
            if b >= n {
                return false;
            }
            indeg[b] += 1;
        }
        let mut queue: Vec<usize> = (0..n).filter(|&i| indeg[i] == 0).collect();
        let mut seen = 0;
        while let Some(u) = queue.pop() {
            seen += 1;
            for &(a, b) in &c.edges {
                if a == u {
                    indeg[b] -= 1;
                    if indeg[b] == 0 {
                        queue.push(b);
                    }
                }
            }
        }
        seen == n
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_warning_has_plain_cause_and_demo() {
        let i = Inputs {
            thrust_to_weight: 0.5,
            periapsis_altitude_m: 50_000.0,
            atmosphere_altitude_m: 120_000.0,
            gravity_loss_fraction: 0.30,
            propellant_deficit: true,
            apoapsis_shortfall_fraction: 0.10,
            impacted: true,
            burn_sweep_deg: 40.0,
        };
        let chains = diagnose(&i);
        assert_eq!(chains.len(), 7);
        assert!(is_valid_dag(&chains));
        for c in &chains {
            assert!(!c.demo.is_empty());
            assert!(c.nodes.iter().any(|n| n.kind == NodeKind::Cause));
        }
    }

    #[test]
    fn healthy_inputs_trigger_nothing() {
        let i = Inputs {
            thrust_to_weight: 1.4,
            periapsis_altitude_m: 300_000.0,
            atmosphere_altitude_m: 120_000.0,
            gravity_loss_fraction: 0.01,
            propellant_deficit: false,
            apoapsis_shortfall_fraction: 0.0,
            impacted: false,
            burn_sweep_deg: 2.0,
        };
        assert!(diagnose(&i).is_empty());
    }

    fn engine(separated: bool, hot: bool, of: f64) -> EngineInputs {
        EngineInputs {
            altitude_m: 0.0,
            ambient_pa: 101_325.0,
            exit_pressure_pa: 8_000.0,
            separation_x_norm: if separated { 0.35 } else { f64::NAN },
            side_load_kn: 1.2,
            wall_temperature_k: if hot { 1500.0 } else { 900.0 },
            wall_limit_k: 1350.0,
            throat_heat_flux_w_m2: 4.0e7,
            chamber_temperature_k: 3400.0,
            of_ratio: of,
            stoichiometric_of: 7.94,
            peak_temperature_of: 8.0,
        }
    }

    #[test]
    fn engine_warnings_are_chains_with_demos() {
        let chains = diagnose_engine(&engine(true, true, 7.9));
        assert_eq!(chains.len(), 3);
        assert!(is_valid_dag(&chains));
        assert_eq!(chains[0].warning, Warning::FlowSeparationAtAltitude);
        assert_eq!(chains[1].warning, Warning::ThroatThermalOverload);
        assert_eq!(chains[2].warning, Warning::StoichiometricTemperatureSpike);
        for c in &chains {
            assert!(!c.demo.is_empty());
            assert!(c.nodes.iter().any(|n| n.kind == NodeKind::Cause));
        }
    }

    #[test]
    fn a_fuel_rich_cool_attached_engine_is_quiet() {
        assert!(diagnose_engine(&engine(false, false, 5.2)).is_empty());
    }
}
