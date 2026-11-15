//! Three playable Block-1 contracts and an explicit local scoring rule.
//! Challenge ratings are catalog labels; the ranked ladder is local.
//! SI units internally; the browser performs display conversions only.
//!
//! Each contract owns a *stage*, not an engine lookup: a fixed airframe mass,
//! a fixed tank volume and a structural propellant budget. The baseline
//! engine is a physical `EngineSpec`, and thrust, specific impulse and engine
//! dry mass are derived from it by `odd-propulsion`. A player may redesign
//! the engine before locking a prediction; the recorder signs the design.
use odd_astro::{
    elements::Elements,
    force_model::{FlightLimits, ForceModel, Radiation, Zonal},
    integrator::{IntegrationError, Options},
    perturb::drag::DragParameters,
    spline::Keyframe,
};
use odd_core::frames::State;
use odd_propulsion::{solve_engine, EngineSolution, EngineSpec, Propellant, PropulsionError};
use odd_sim::{
    mission::{apsides, PredictionBaseline},
    planner::ManeuverNode,
    Engine,
};

pub const EARTH_MU: f64 = 3.986_004_418e14;
pub const EARTH_EQUATORIAL_RADIUS: f64 = 6_378_137.0;
pub const EARTH_POLAR_RADIUS: f64 = 6_356_752.314;
pub const EARTH_J2: f64 = 1.082_626_68e-3;
pub const EARTH_ROTATION: f64 = 7.292_115e-5;
pub const START_EPOCH: i64 = 820_497_600; // 2026-01-01 00:00:00 TDB
pub const DRAG_AREA: f64 = 4.0;
pub const DRAG_CD: f64 = 2.2;
pub const SRP_CR: f64 = 1.3;
pub const ENTRY_ALTITUDE: f64 = 120_000.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Objective {
    CircularRescue,
    SunSynchronous,
    TransLunarApoapsis,
}

#[derive(Clone, Copy, Debug)]
pub struct Contract {
    pub id: u32,
    pub title: &'static str,
    pub rank: &'static str,
    pub rating: f64,
    pub objective: Objective,
    pub initial: Keyframe,
    pub duration_us: i64,
    /// Derived: airframe plus the solved baseline engine mass.
    pub dry_mass_kg: f64,
    /// Derived from `engine_spec` at zero ambient pressure.
    pub engine: Engine,
    /// The baseline physical engine design.
    pub engine_spec: EngineSpec,
    /// Structure, tanks, avionics and payload. Excludes the engine.
    pub airframe_mass_kg: f64,
    /// Usable propellant volume. A low-density pair cannot fill the mass
    /// budget, which is the whole point of offering hydrolox.
    pub tank_volume_m3: f64,
    /// Structural load limit on propellant mass.
    pub propellant_budget_kg: f64,
    /// Derived from the documented reference geometry, not a performance lookup.
    pub reference_delta_v_m_s: f64,
}

/// A stage with a chosen engine: everything the propagator needs.
#[derive(Clone, Copy, Debug)]
pub struct Vehicle {
    pub spec: EngineSpec,
    pub solution: EngineSolution,
    pub engine: Engine,
    pub dry_mass_kg: f64,
    pub propellant_kg: f64,
    pub initial_mass_kg: f64,
    /// True when the tank volume, not the structure, limited the load.
    pub volume_limited: bool,
}

/// Baseline orbital-maneuvering engine: a storable pressure-fed-class tug
/// engine sized to the card's 2.4 kN. Solving it gives 2,396.8 N and 310.6 s.
pub const STORABLE_BASELINE: EngineSpec = EngineSpec {
    propellant: Propellant::NtoMmh,
    of_ratio: 2.0,
    p_c_pa: 1.5e6,
    r_throat_m: 0.0164,
    r_exit_m: 0.0164 * 7.416_198_487_930_862, // sqrt(55)
    length_fraction: 0.8,
};

/// Baseline cislunar tug engine: fuel-rich hydrolox, 3.5 MPa, expansion 120.
/// Solving it gives 18,087 N and 449.9 s, within half a percent of the card.
pub const TUG_BASELINE: EngineSpec = EngineSpec {
    propellant: Propellant::LoxLh2,
    of_ratio: 5.2,
    p_c_pa: 3.5e6,
    r_throat_m: 0.0288,
    r_exit_m: 0.0288 * 10.954_451_150_103_322, // sqrt(120)
    length_fraction: 0.8,
};

/// Overwritten by `with_baseline_engine` before a contract escapes this
/// module; it exists only because `Contract` is built as a literal.
const STORABLE_PLACEHOLDER: Engine = Engine {
    thrust_newtons: 0.0,
    specific_impulse_seconds: 0.0,
    stage: 0,
    engine: 0,
};

impl Contract {
    pub fn get(id: u32) -> Option<Self> {
        match id {
            101 => Some(Self::rescue()),
            102 => Some(Self::sso()),
            103 => Some(Self::tug()),
            _ => None,
        }
    }
    pub fn all() -> [Self; 3] {
        [Self::rescue(), Self::sso(), Self::tug()]
    }
    fn rescue() -> Self {
        let r = EARTH_EQUATORIAL_RADIUS + 260_000.0;
        let target = EARTH_EQUATORIAL_RADIUS + 550_000.0;
        let transfer = odd_astro::hohmann_transfer(EARTH_MU, r, target).unwrap();
        Self {
            id: 101,
            title: "LEO Drag Rescue",
            rank: "CADET",
            rating: 800.0,
            objective: Objective::CircularRescue,
            initial: Keyframe {
                time: 0.0,
                position: [r, 0.0, 0.0],
                velocity: [0.0, libm::sqrt(EARTH_MU / r), 0.0],
                mass: 1200.0,
            },
            duration_us: 10_800_000_000,
            dry_mass_kg: 0.0,
            engine: STORABLE_PLACEHOLDER,
            engine_spec: STORABLE_BASELINE,
            airframe_mass_kg: 1016.9,
            tank_volume_m3: 0.152,
            propellant_budget_kg: 180.0,
            reference_delta_v_m_s: transfer.total_delta_v_m_s,
        }
        .with_baseline_engine()
    }
    fn sso() -> Self {
        let rp = EARTH_EQUATORIAL_RADIUS + 250_000.0;
        let ra = EARTH_EQUATORIAL_RADIUS + 800_000.0;
        let a = (rp + ra) * 0.5;
        let inc = 96.0 * std::f64::consts::PI / 180.0;
        let speed = libm::sqrt(EARTH_MU * (2.0 / rp - 1.0 / a));
        let va = libm::sqrt(EARTH_MU * (2.0 / ra - 1.0 / a));
        let vc = libm::sqrt(EARTH_MU / ra);
        let delta = 2.6 * std::f64::consts::PI / 180.0;
        let dv_t = vc * libm::cos(delta) - va;
        let dv_w = -vc * libm::sin(delta);
        Self {
            id: 102,
            title: "Sun-Synchronous J2 Precession",
            rank: "FLIGHT DYNAMICS OFFICER",
            rating: 1250.0,
            objective: Objective::SunSynchronous,
            initial: Keyframe {
                time: 0.0,
                position: [rp, 0.0, 0.0],
                velocity: [0.0, speed * libm::cos(inc), speed * libm::sin(inc)],
                mass: 1200.0,
            },
            duration_us: 432_000_000_000,
            // Unspecified in the original card; explicitly borrowed from 101.
            dry_mass_kg: 0.0,
            engine: STORABLE_PLACEHOLDER,
            engine_spec: STORABLE_BASELINE,
            airframe_mass_kg: 1016.9,
            tank_volume_m3: 0.152,
            propellant_budget_kg: 180.0,
            reference_delta_v_m_s: libm::hypot(dv_t, dv_w),
        }
        .with_baseline_engine()
    }
    fn tug() -> Self {
        let r = EARTH_EQUATORIAL_RADIUS + 200_000.0;
        Self {
            id: 103,
            title: "Cislunar TLI: The Perigee-Kick Trap",
            rank: "SENIOR FLIGHT DYNAMICS OFFICER",
            rating: 1650.0,
            objective: Objective::TransLunarApoapsis,
            initial: Keyframe {
                time: 0.0,
                position: [r, 0.0, 0.0],
                velocity: [0.0, libm::sqrt(EARTH_MU / r), 0.0],
                mass: 18_000.0,
            },
            duration_us: 600_000_000_000,
            dry_mass_kg: 0.0,
            engine: STORABLE_PLACEHOLDER,
            engine_spec: TUG_BASELINE,
            airframe_mass_kg: 5182.1,
            tank_volume_m3: 38.6,
            propellant_budget_kg: 12_800.0,
            // Demonstrated three-kick reference budget. The separate 3,120 m/s
            // oracle is a gravity-loss regression, not a target-reaching claim.
            reference_delta_v_m_s: 3240.0,
        }
        .with_baseline_engine()
    }
    /// Solve the stage with its baseline engine and fill the derived fields.
    fn with_baseline_engine(mut self) -> Self {
        let vehicle = self
            .vehicle(self.engine_spec)
            .expect("the baseline engine of every contract must solve");
        self.engine = vehicle.engine;
        self.dry_mass_kg = vehicle.dry_mass_kg;
        self.initial.mass = vehicle.initial_mass_kg;
        self
    }

    /// Design the stage around a player's engine. Thrust, specific impulse,
    /// engine mass and the propellant load all follow from physics; the
    /// airframe, tank volume and structural budget do not change.
    pub fn vehicle(&self, spec: EngineSpec) -> Result<Vehicle, PropulsionError> {
        // Orbital contracts fly in vacuum. Ambient performance belongs to the
        // test stand, not to the flight.
        let solution = solve_engine(&spec, 0.0)?;
        if !solution.flight_ready() {
            return Err(PropulsionError::NumericalRange);
        }
        let capacity = self.tank_volume_m3 * solution.bulk_density_kg_m3;
        let propellant_kg = libm::fmin(capacity, self.propellant_budget_kg);
        let dry_mass_kg = self.airframe_mass_kg + solution.dry_mass_kg;
        Ok(Vehicle {
            spec,
            solution,
            engine: Engine {
                thrust_newtons: solution.thrust_n,
                specific_impulse_seconds: solution.specific_impulse_s,
                stage: 0,
                engine: 0,
            },
            dry_mass_kg,
            propellant_kg,
            initial_mass_kg: dry_mass_kg + propellant_kg,
            volume_limited: capacity < self.propellant_budget_kg,
        })
    }

    pub fn limits(&self) -> FlightLimits {
        FlightLimits {
            pole: [0.0, 0.0, 1.0],
            equatorial_radius_m: EARTH_EQUATORIAL_RADIUS,
            polar_radius_m: EARTH_POLAR_RADIUS,
            entry_altitude_m: Some(ENTRY_ALTITUDE),
        }
    }
    pub fn force_model(&self) -> ForceModel {
        ForceModel {
            mu: EARTH_MU,
            zonal: Some(Zonal {
                pole: [0.0, 0.0, 1.0],
                radius: EARTH_EQUATORIAL_RADIUS,
                j2: EARTH_J2,
            }),
            drag: Some(DragParameters {
                pole: [0.0, 0.0, 1.0],
                omega: [0.0, 0.0, EARTH_ROTATION],
                equatorial_radius: EARTH_EQUATORIAL_RADIUS,
                polar_radius: EARTH_POLAR_RADIUS,
                cd: DRAG_CD,
                area: DRAG_AREA,
                mass: self.initial.mass,
            }),
            radiation: Some(Radiation {
                cr: SRP_CR,
                area: DRAG_AREA,
                central_radius: EARTH_EQUATORIAL_RADIUS,
            }),
            third_body: true,
            burn: None,
        }
    }
    pub fn options(&self) -> Options {
        Options {
            relative_tolerance: 1e-10,
            absolute_tolerance: 1e-10,
            initial_step: 10.0,
            max_step: 120.0,
            max_steps: 1_000_000,
        }
    }
    /// Reference example nodes. Contract 103's later epochs are deliberately
    /// approximate seeds; the playable splitter retargets them numerically.
    pub fn example_nodes(&self) -> Vec<ManeuverNode> {
        match self.objective {
            Objective::CircularRescue => {
                let r = self.initial.position[0];
                let target = EARTH_EQUATORIAL_RADIUS + 550_000.0;
                let h = odd_astro::hohmann_transfer(EARTH_MU, r, target).unwrap();
                vec![
                    ManeuverNode {
                        epoch_us: 200_000_000,
                        tnw_m_s: [h.departure_delta_v_m_s, 0.0, 0.0],
                    },
                    ManeuverNode {
                        epoch_us: 200_000_000 + libm::round(h.coast_time_s * 1e6) as i64,
                        tnw_m_s: [h.arrival_delta_v_m_s, 0.0, 0.0],
                    },
                ]
            }
            Objective::SunSynchronous => {
                let rp = self.initial.position[0];
                let ra = EARTH_EQUATORIAL_RADIUS + 800_000.0;
                let a = (rp + ra) * 0.5;
                let coast = std::f64::consts::PI * libm::sqrt(a * a * a / EARTH_MU);
                let va = libm::sqrt(EARTH_MU * (2.0 / ra - 1.0 / a));
                let vc = libm::sqrt(EARTH_MU / ra);
                let delta = 2.6 * std::f64::consts::PI / 180.0;
                vec![ManeuverNode {
                    epoch_us: libm::round(coast * 1e6) as i64,
                    tnw_m_s: [vc * libm::cos(delta) - va, 0.0, -vc * libm::sin(delta)],
                }]
            }
            Objective::TransLunarApoapsis => vec![ManeuverNode {
                epoch_us: 1_600_000_000,
                tnw_m_s: [3240.0, 0.0, 0.0],
            }],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MissionScore {
    pub objectives: f64,
    pub prediction: f64,
    pub efficiency: f64,
    pub s: f64,
}

fn elements(state: Keyframe) -> Result<Elements, IntegrationError> {
    Elements::from_state(
        State {
            position_m: state.position,
            velocity_m_s: state.velocity,
        },
        EARTH_MU,
    )
    .map_err(|_| IntegrationError::ForceFailure)
}

/// Local, transparent score: 50% objective quality, 25% locked-prediction
/// closeness, 25% delta-v efficiency against the contract's reference geometry.
/// This is not the later Pareto/Elo system. Hard impact, fuel or deadline
/// failures score zero. Sensitivity shares never enter the score.
pub fn score(
    contract: &Contract,
    state: Keyframe,
    evaluation_s: f64,
    prediction: PredictionBaseline,
    commanded_delta_v_m_s: f64,
    impacted: bool,
) -> Result<MissionScore, IntegrationError> {
    let fuel_ok = state.mass >= contract.dry_mass_kg && state.mass.is_finite();
    let deadline_ok = evaluation_s <= contract.duration_us as f64 / 1e6;
    if impacted || !fuel_ok || !deadline_ok || !evaluation_s.is_finite() {
        return Ok(MissionScore {
            objectives: 0.0,
            prediction: 0.0,
            efficiency: 0.0,
            s: 0.0,
        });
    }
    let ap = apsides(state, EARTH_MU, EARTH_EQUATORIAL_RADIUS)
        .map_err(|_| IntegrationError::ForceFailure)?;
    let el = elements(state)?;
    let normalized = match contract.objective {
        Objective::CircularRescue => {
            if ap.eccentricity >= 0.005 {
                10.0
            } else {
                ((ap.apoapsis_altitude_m - 550_000.0).abs() / 10_000.0)
                    .max((ap.periapsis_altitude_m - 550_000.0).abs() / 10_000.0)
            }
        }
        Objective::SunSynchronous => {
            let drift = (el.ascending_node_rad + std::f64::consts::PI)
                .rem_euclid(std::f64::consts::TAU)
                - std::f64::consts::PI;
            let drift_deg = drift * 180.0 / std::f64::consts::PI;
            let inc_deg = el.inclination_rad * 180.0 / std::f64::consts::PI;
            if ap.eccentricity >= 0.005 {
                10.0
            } else {
                ((ap.apoapsis_altitude_m - 800_000.0).abs() / 10_000.0)
                    .max((ap.periapsis_altitude_m - 800_000.0).abs() / 10_000.0)
                    .max((drift_deg - 4.93).abs() / 0.15)
                    .max((inc_deg - 98.60).abs() / 0.10)
            }
        }
        Objective::TransLunarApoapsis => {
            if ap.periapsis_altitude_m < ENTRY_ALTITUDE {
                10.0
            } else {
                // Catalog convention: 380,000 km from Earth's center, not altitude.
                let radius = ap.apoapsis_altitude_m + EARTH_EQUATORIAL_RADIUS;
                (radius - 380_000_000.0).abs() / 15_000_000.0
            }
        }
    };
    let objectives = if normalized <= 1.0 {
        1.0 - 0.2 * normalized
    } else {
        (0.8 / normalized).max(0.0)
    };
    let pred = prediction;
    let error = (pred.delta_v_m_s - commanded_delta_v_m_s).abs() / commanded_delta_v_m_s.max(1.0)
        + (pred.apoapsis_altitude_m - ap.apoapsis_altitude_m).abs() / 1_000_000.0
        + (pred.periapsis_altitude_m - ap.periapsis_altitude_m).abs() / 1_000_000.0
        + (pred.remaining_propellant_kg - (state.mass - contract.dry_mass_kg)).abs()
            / (contract.initial.mass - contract.dry_mass_kg).max(1.0)
        + (pred.time_of_flight_s - evaluation_s).abs() / evaluation_s.max(1.0);
    let prediction_score = libm::exp(-error);
    let efficiency = if commanded_delta_v_m_s > 0.0 {
        (contract.reference_delta_v_m_s / commanded_delta_v_m_s).clamp(0.0, 1.0)
    } else {
        0.0
    };
    Ok(MissionScore {
        objectives,
        prediction: prediction_score,
        efficiency,
        s: 0.5 * objectives + 0.25 * prediction_score + 0.25 * efficiency,
    })
}

/// A numerically targeted perigee-kick plan for Contract 103.
#[derive(Clone, Debug)]
pub struct SplitPlan {
    pub plan: odd_sim::planner::Plan,
    pub nodes: Vec<ManeuverNode>,
    pub guidance: Vec<Option<odd_core::frames::State>>,
}

/// Build an equal-delta-v perigee-kick sequence by forward numerical targeting.
/// `kicks` must be 2 or 3. Each later node is centered on the periapsis found
/// by propagating the preceding commanded burns with the full contract model.
/// This is the playable splitter; it is not a global optimizer.
pub fn split_tug(
    kicks: usize,
    total_delta_v_m_s: f64,
    first_node_s: f64,
    eph: &odd_ephem::Ephemeris,
) -> Result<SplitPlan, odd_sim::targeting::TargetError> {
    use odd_sim::{planner::Plan, targeting::next_radial_turn};
    let contract = Contract::tug();
    if !(2..=3).contains(&kicks)
        || !total_delta_v_m_s.is_finite()
        || total_delta_v_m_s <= 0.0
        || !first_node_s.is_finite()
        || first_node_s < 0.0
    {
        return Err(odd_sim::targeting::TargetError::InvalidInput);
    }
    let per_kick = total_delta_v_m_s / kicks as f64;
    let mut nodes = vec![ManeuverNode {
        epoch_us: (first_node_s * 1e6).round() as i64,
        tnw_m_s: [per_kick, 0.0, 0.0],
    }];
    let mut guidance = vec![None];
    let model = contract.force_model();
    let limits = contract.limits();
    let options = contract.options();
    for _ in 1..kicks {
        let plan = Plan::compile_guided(
            contract.initial,
            EARTH_MU,
            contract.duration_us,
            contract.dry_mass_kg,
            contract.engine,
            &nodes,
            &guidance,
        )
        .map_err(|_| odd_sim::targeting::TargetError::InvalidInput)?;
        let burns = plan
            .timeline
            .burns(&[contract.engine])
            .map_err(|_| odd_sim::targeting::TargetError::InvalidInput)?;
        let after = burns.last().unwrap().end;
        let guide = next_radial_turn(
            contract.initial,
            after,
            after + 200_000.0,
            true,
            &model,
            &burns,
            limits,
            options,
            |t| environment(eph, START_EPOCH, t),
        )?;
        nodes.push(ManeuverNode {
            epoch_us: (guide.time * 1e6).round() as i64,
            tnw_m_s: [per_kick, 0.0, 0.0],
        });
        guidance.push(Some(odd_core::frames::State {
            position_m: guide.position,
            velocity_m_s: guide.velocity,
        }));
    }
    let plan = Plan::compile_guided(
        contract.initial,
        EARTH_MU,
        contract.duration_us,
        contract.dry_mass_kg,
        contract.engine,
        &nodes,
        &guidance,
    )
    .map_err(|_| odd_sim::targeting::TargetError::InvalidInput)?;
    Ok(SplitPlan {
        plan,
        nodes,
        guidance,
    })
}

/// Earth environment helper shared by the browser bridge and native tests.
pub fn environment(
    eph: &odd_ephem::Ephemeris,
    epoch: i64,
    t: f64,
) -> Result<odd_astro::force_model::Environment, IntegrationError> {
    use odd_astro::force_model::{Environment, ThirdBody};
    use odd_core::epoch::Epoch;
    use odd_ephem::Body;
    let at = Epoch::new(epoch, t).map_err(|_| IntegrationError::ForceFailure)?;
    let position = |body| {
        eph.state(body, Body::Earth, at)
            .map(|v| v.state.position_m)
            .map_err(|_| IntegrationError::ForceFailure)
    };
    Ok(Environment {
        sun: ThirdBody {
            position: position(Body::Sun)?,
            mu: 1.327_124_400_412_794_2e20,
            radius: 695_700_000.0,
        },
        companion: ThirdBody {
            position: position(Body::Moon)?,
            mu: 4.902_800_118e12,
            radius: 1_737_400.0,
        },
    })
}
