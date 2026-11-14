//! Quantized, deterministic finite-burn command compilation.
//!
//! Quaternion order is [w,x,y,z]; it rotates vehicle +X into the inertial
//! thrust direction. Decoding normalizes the signed 16-bit components.
//! Commands alone do not specify the engine: replay must also retain the exact
//! engine, mass, spacecraft and force-model configuration.
pub mod planner;

use odd_astro::maneuver::{ManeuverError, TnwBasis};
use odd_core::frames::State;

pub const STANDARD_GRAVITY: f64 = 9.80665;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandError {
    InvalidInput,
    DegenerateFrame,
    NumericalRange,
    OverlappingBurns,
}

impl From<ManeuverError> for CommandError {
    fn from(value: ManeuverError) -> Self {
        match value {
            ManeuverError::InvalidInput => Self::InvalidInput,
            ManeuverError::DegenerateFrame => Self::DegenerateFrame,
            ManeuverError::NumericalRange => Self::NumericalRange,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandKind {
    Burn {
        stage: u8,
        engine: u8,
        throttle_u16: u16,
        dur_us: i64,
        quat_i16: [i16; 4],
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command {
    pub t_us: i64,
    pub kind: CommandKind,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandTimeline {
    commands: Vec<Command>,
}
impl CommandTimeline {
    pub fn commands(&self) -> &[Command] {
        &self.commands
    }
    /// Resolve engine identifiers and decode the immutable burn schedule.
    /// Epochs are relative simulation microseconds, bounded to exact integer
    /// conversion to f64 before converting to seconds.
    pub fn burns(
        &self,
        engines: &[Engine],
    ) -> Result<Vec<odd_astro::perturb::burn::Burn>, CommandError> {
        let mut burns = Vec::with_capacity(self.commands.len());
        for command in &self.commands {
            let CommandKind::Burn {
                stage,
                engine,
                throttle_u16,
                dur_us,
                quat_i16,
            } = command.kind;
            let e = engines
                .iter()
                .find(|e| e.stage == stage && e.engine == engine)
                .ok_or(CommandError::InvalidInput)?;
            if !e.thrust_newtons.is_finite()
                || e.thrust_newtons <= 0.0
                || !e.specific_impulse_seconds.is_finite()
                || e.specific_impulse_seconds <= 0.0
            {
                return Err(CommandError::InvalidInput);
            }
            let end = command
                .t_us
                .checked_add(dur_us)
                .ok_or(CommandError::NumericalRange)?;
            const MAX_EXACT: i64 = 1_i64 << 53;
            if !(-MAX_EXACT..=MAX_EXACT).contains(&command.t_us)
                || !(-MAX_EXACT..=MAX_EXACT).contains(&end)
            {
                return Err(CommandError::NumericalRange);
            }
            let thrust = e.thrust_newtons * (f64::from(throttle_u16) / f64::from(u16::MAX));
            let force_newtons = thrust_direction(quat_i16)?.map(|x| x * thrust);
            let start_s = command.t_us as f64 / 1e6;
            let end_s = end as f64 / 1e6;
            if end_s <= start_s
                || libm::round(start_s * 1e6) != command.t_us as f64
                || libm::round(end_s * 1e6) != end as f64
            {
                return Err(CommandError::NumericalRange);
            }
            burns.push(odd_astro::perturb::burn::Burn {
                start: start_s,
                end: end_s,
                force_newtons,
                specific_impulse_seconds: e.specific_impulse_seconds,
            });
        }
        Ok(burns)
    }
    /// Insert in time order, rejecting overlaps. Touching endpoints are allowed.
    pub fn insert(&mut self, command: Command) -> Result<(), CommandError> {
        let CommandKind::Burn {
            dur_us,
            throttle_u16,
            quat_i16,
            ..
        } = command.kind;
        if dur_us <= 0 || throttle_u16 == 0 || quat_i16 == [0; 4] {
            return Err(CommandError::InvalidInput);
        }
        let end = command
            .t_us
            .checked_add(dur_us)
            .ok_or(CommandError::NumericalRange)?;
        let i = self.commands.partition_point(|c| c.t_us < command.t_us);
        if i > 0 {
            let prev = self.commands[i - 1];
            let CommandKind::Burn { dur_us, .. } = prev.kind;
            if prev
                .t_us
                .checked_add(dur_us)
                .ok_or(CommandError::NumericalRange)?
                > command.t_us
            {
                return Err(CommandError::OverlappingBurns);
            }
        }
        if i < self.commands.len() && end > self.commands[i].t_us {
            return Err(CommandError::OverlappingBurns);
        }
        self.commands.insert(i, command);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Engine {
    pub thrust_newtons: f64,
    pub specific_impulse_seconds: f64,
    pub stage: u8,
    pub engine: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct CompiledBurn {
    pub command: Command,
    /// Rocket-equation mass for the *unquantized* requested delta-v.
    pub ideal_propellant_kg: f64,
    /// Mass used at constant thrust over the quantized duration.
    pub commanded_propellant_kg: f64,
}

fn unit(v: [f64; 3]) -> Result<[f64; 3], CommandError> {
    let s = v.iter().fold(0.0_f64, |a, x| a.max(x.abs()));
    if !s.is_finite() || s == 0.0 {
        return Err(CommandError::InvalidInput);
    }
    let v = v.map(|x| x / s);
    let n = libm::sqrt(v.iter().map(|x| x * x).sum());
    Ok(v.map(|x| x / n))
}

/// Decode a quantized scalar-first quaternion and rotate the +X thrust axis.
pub fn thrust_direction(quat: [i16; 4]) -> Result<[f64; 3], CommandError> {
    let q = quat.map(f64::from);
    let n = libm::sqrt(q.iter().map(|x| x * x).sum());
    if n == 0.0 {
        return Err(CommandError::InvalidInput);
    }
    let [w, x, y, z] = q.map(|v| v / n);
    Ok([
        1.0 - 2.0 * (y * y + z * z),
        2.0 * (x * y + w * z),
        2.0 * (x * z - w * y),
    ])
}

fn direction_quaternion(direction: [f64; 3]) -> [i16; 4] {
    let [x, y, z] = direction;
    // Stable shortest-arc quaternion from +X; exact opposite has a chosen axis.
    let q = if x < -0.999_999_999_999 {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        let w = libm::sqrt((1.0 + x) * 0.5);
        [w, 0.0, -z / (2.0 * w), y / (2.0 * w)]
    };
    q.map(|v| libm::round(v.clamp(-1.0, 1.0) * 32767.0) as i16)
}

/// Compile an instantaneous TNW request into a constant-inertial-direction,
/// full-throttle burn centered on `node_us`, rounded to integer microseconds.
/// The returned ideal and commanded propellant values can differ by up to
/// half a microsecond of mass flow. A sub-microsecond burn is rejected.
pub fn compile_burn(
    state: State,
    tnw_m_s: [f64; 3],
    node_us: i64,
    mass_kg: f64,
    engine: Engine,
) -> Result<CompiledBurn, CommandError> {
    if !mass_kg.is_finite()
        || mass_kg <= 0.0
        || !engine.thrust_newtons.is_finite()
        || engine.thrust_newtons <= 0.0
        || !engine.specific_impulse_seconds.is_finite()
        || engine.specific_impulse_seconds <= 0.0
    {
        return Err(CommandError::InvalidInput);
    }
    let dv = TnwBasis::from_state(state)?.inertial_delta_v(tnw_m_s)?;
    let direction = unit(dv)?;
    let dv_mag = libm::hypot(libm::hypot(dv[0], dv[1]), dv[2]);
    let exhaust = engine.specific_impulse_seconds * STANDARD_GRAVITY;
    let propellant = -mass_kg * libm::expm1(-dv_mag / exhaust);
    let duration = propellant * exhaust / engine.thrust_newtons;
    let us = libm::round(duration * 1e6);
    if !propellant.is_finite()
        || propellant <= 0.0
        || propellant >= mass_kg
        || !us.is_finite()
        || us < 1.0
        || us >= i64::MAX as f64
    {
        return Err(CommandError::NumericalRange);
    }
    let dur_us = us as i64;
    let start = node_us
        .checked_sub(dur_us / 2)
        .ok_or(CommandError::NumericalRange)?;
    start
        .checked_add(dur_us)
        .ok_or(CommandError::NumericalRange)?;
    let commanded = engine.thrust_newtons * (dur_us as f64 / 1e6) / exhaust;
    if !commanded.is_finite() || commanded >= mass_kg {
        return Err(CommandError::NumericalRange);
    }
    Ok(CompiledBurn {
        command: Command {
            t_us: start,
            kind: CommandKind::Burn {
                stage: engine.stage,
                engine: engine.engine,
                throttle_u16: u16::MAX,
                dur_us,
                quat_i16: direction_quaternion(direction),
            },
        },
        ideal_propellant_kg: propellant,
        commanded_propellant_kg: commanded,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn engine() -> Engine {
        Engine {
            thrust_newtons: 18_000.0,
            specific_impulse_seconds: 450.0,
            stage: 0,
            engine: 0,
        }
    }
    fn state() -> State {
        State {
            position_m: [6_578_137.0, 0.0, 0.0],
            velocity_m_s: [0.0, 7784.0, 0.0],
        }
    }
    #[test]
    fn compiler_conserves_quantized_time_and_rocket_mass() {
        let out = compile_burn(
            state(),
            [3120.0, 0.0, 0.0],
            2_000_000_000,
            18_000.0,
            engine(),
        )
        .unwrap();
        let CommandKind::Burn {
            dur_us, quat_i16, ..
        } = out.command.kind;
        assert!((out.ideal_propellant_kg - 9_123.844_563_655_58).abs() < 1e-9);
        assert!(
            (out.commanded_propellant_kg - out.ideal_propellant_kg).abs()
                <= engine().thrust_newtons / (engine().specific_impulse_seconds * STANDARD_GRAVITY)
                    * 0.500_001e-6
        );
        assert!((out.command.t_us as f64 + dur_us as f64 * 0.5 - 2_000_000_000.0).abs() <= 0.5);
        let d = thrust_direction(quat_i16).unwrap();
        assert!(d[0].abs() < 1e-4 && (d[1] - 1.0).abs() < 1e-4 && d[2].abs() < 1e-4);
        assert!(compile_burn(state(), [0.0; 3], 0, 1.0, engine()).is_err());
    }
    #[test]
    fn orientations_and_timeline_overlap() {
        for direction in [
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.2, -0.3, 0.7],
        ] {
            let d = unit(direction).unwrap();
            let actual = thrust_direction(direction_quaternion(d)).unwrap();
            assert!(actual.iter().zip(d).all(|(a, b)| (a - b).abs() < 0.0001));
        }
        let burn = compile_burn(state(), [10.0, 0.0, 0.0], 1_000_000_000, 1000.0, engine())
            .unwrap()
            .command;
        let mut timeline = CommandTimeline::default();
        timeline.insert(burn).unwrap();
        assert_eq!(timeline.insert(burn), Err(CommandError::OverlappingBurns));
        assert_eq!(timeline.commands().len(), 1);
    }
}
