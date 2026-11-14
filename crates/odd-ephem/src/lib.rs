//! Bounded geometric JPL DE440 ephemeris, expressed in ICRF-aligned axes.
//!
//! This crate performs no I/O. The bundled data are type-2 Chebyshev records
//! from a documented 2020–2036 SPK subset. No extrapolation is performed.
//! These are geometric coordinate states, not light-time-corrected observations.

pub mod orientation;
pub mod physical;

use odd_core::{epoch::Epoch, frames::State};

/// Identifiers used in the DE440 subset (NAIF IDs in discriminants).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Body {
    SolarSystemBarycenter = 0,
    EarthMoonBarycenter = 3,
    Sun = 10,
    Moon = 301,
    Earth = 399,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EphemerisError {
    InvalidData,
    OutsideCoverage,
}

/// A geometric state with an explicitly labelled origin, axes and epoch.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EphemerisState {
    pub target: Body,
    pub center: Body,
    pub epoch: Epoch,
    /// JPL SPK J2000 (ICRF-aligned) axes; meters and meters per second.
    pub state: State,
}

#[derive(Clone, Debug)]
struct Segment {
    target: Body,
    start: i64,
    end: i64,
    init: i64,
    interval: i64,
    ncoef: usize,
    count: usize,
    coefficients: Vec<f64>,
}

/// Validated in-memory ephemeris. Load once, then query without I/O or mutation.
#[derive(Clone, Debug)]
pub struct Ephemeris {
    segments: Vec<Segment>,
    start: i64,
    end: i64,
}

const EXPECTED: [(Body, Body); 4] = [
    (Body::EarthMoonBarycenter, Body::SolarSystemBarycenter),
    (Body::Sun, Body::SolarSystemBarycenter),
    (Body::Moon, Body::EarthMoonBarycenter),
    (Body::Earth, Body::EarthMoonBarycenter),
];
const BUNDLED: &[u8] = include_bytes!("../../../data/packed/de440s-2020-2036.ephb");

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N], EphemerisError> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or(EphemerisError::InvalidData)?;
        let b = self
            .bytes
            .get(self.offset..end)
            .ok_or(EphemerisError::InvalidData)?;
        self.offset = end;
        b.try_into().map_err(|_| EphemerisError::InvalidData)
    }
    fn i32(&mut self) -> Result<i32, EphemerisError> {
        Ok(i32::from_le_bytes(self.take()?))
    }
    fn u32(&mut self) -> Result<u32, EphemerisError> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    fn f64(&mut self) -> Result<f64, EphemerisError> {
        Ok(f64::from_le_bytes(self.take()?))
    }
    fn integer_seconds(&mut self) -> Result<i64, EphemerisError> {
        let x = self.f64()?;
        // Format v1 uses exact integral seconds within the f64 exact integer range.
        if !x.is_finite() || x.fract() != 0.0 || x.abs() > 9_007_199_254_740_992.0 {
            return Err(EphemerisError::InvalidData);
        }
        Ok(x as i64)
    }
}

impl Ephemeris {
    /// Decode and validate compact ODEPHEM1 data. Does not authenticate input.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, EphemerisError> {
        let mut r = Reader { bytes, offset: 0 };
        if &r.take::<8>()? != b"ODEPHEM1" || r.u32()? != 4 {
            return Err(EphemerisError::InvalidData);
        }
        let mut segments = Vec::with_capacity(4);
        for (target, center) in EXPECTED {
            if r.i32()? != target as i32 || r.i32()? != center as i32 {
                return Err(EphemerisError::InvalidData);
            }
            let start = r.integer_seconds()?;
            let end = r.integer_seconds()?;
            let init = r.integer_seconds()?;
            let interval = r.integer_seconds()?;
            let ncoef = r.u32()? as usize;
            let count = r.u32()? as usize;
            if start >= end
                || interval <= 0
                || ncoef == 0
                || ncoef > 64
                || count == 0
                || start < init
                || i128::from(end) > i128::from(init) + i128::from(interval) * (count as i128)
            {
                return Err(EphemerisError::InvalidData);
            }
            let len = count
                .checked_mul(ncoef)
                .and_then(|x| x.checked_mul(3))
                .ok_or(EphemerisError::InvalidData)?;
            let byte_len = len.checked_mul(8).ok_or(EphemerisError::InvalidData)?;
            if byte_len > bytes.len().saturating_sub(r.offset) {
                return Err(EphemerisError::InvalidData);
            }
            let mut coefficients = Vec::with_capacity(len);
            for _ in 0..len {
                let x = r.f64()?;
                if !x.is_finite() {
                    return Err(EphemerisError::InvalidData);
                }
                coefficients.push(x);
            }
            segments.push(Segment {
                target,
                start,
                end,
                init,
                interval,
                ncoef,
                count,
                coefficients,
            });
        }
        if r.offset != bytes.len() {
            return Err(EphemerisError::InvalidData);
        }
        let start = segments
            .iter()
            .map(|s| s.start)
            .max()
            .ok_or(EphemerisError::InvalidData)?;
        let end = segments
            .iter()
            .map(|s| s.end)
            .min()
            .ok_or(EphemerisError::InvalidData)?;
        if start >= end {
            return Err(EphemerisError::InvalidData);
        }
        Ok(Self {
            segments,
            start,
            end,
        })
    }
    /// Load the embedded 2020-01-01 to 2036-01-01 TDB DE440 subset.
    pub fn bundled() -> Result<Self, EphemerisError> {
        Self::from_bytes(BUNDLED)
    }
    /// Common coverage, as TDB seconds from J2000. Start inclusive, end exclusive.
    pub fn coverage(&self) -> (i64, i64) {
        (self.start, self.end)
    }
    /// Geometric target relative to center. Inputs outside common coverage fail closed.
    pub fn state(
        &self,
        target: Body,
        center: Body,
        epoch: Epoch,
    ) -> Result<EphemerisState, EphemerisError> {
        if epoch.seconds() < self.start || epoch.seconds() >= self.end {
            return Err(EphemerisError::OutsideCoverage);
        }
        let target_path = path(target);
        let center_path = path(center);
        let common = target_path
            .iter()
            .position(|b| center_path.contains(b))
            .ok_or(EphemerisError::InvalidData)?;
        let common_body = target_path[common];
        let mut result = State {
            position_m: [0.0; 3],
            velocity_m_s: [0.0; 3],
        };
        for (path, sign) in [(target_path, 1.0), (center_path, -1.0)] {
            for body in path.iter().take_while(|b| **b != common_body) {
                let segment = self
                    .segments
                    .iter()
                    .find(|s| s.target == *body)
                    .ok_or(EphemerisError::InvalidData)?;
                let s = segment.evaluate(epoch)?;
                for i in 0..3 {
                    result.position_m[i] += sign * s.position_m[i];
                    result.velocity_m_s[i] += sign * s.velocity_m_s[i];
                }
            }
        }
        Ok(EphemerisState {
            target,
            center,
            epoch,
            state: result,
        })
    }
}

fn path(body: Body) -> &'static [Body] {
    use Body::*;
    match body {
        SolarSystemBarycenter => &[SolarSystemBarycenter],
        EarthMoonBarycenter => &[EarthMoonBarycenter, SolarSystemBarycenter],
        Sun => &[Sun, SolarSystemBarycenter],
        Earth => &[Earth, EarthMoonBarycenter, SolarSystemBarycenter],
        Moon => &[Moon, EarthMoonBarycenter, SolarSystemBarycenter],
    }
}

impl Segment {
    fn evaluate(&self, epoch: Epoch) -> Result<State, EphemerisError> {
        if epoch.seconds() < self.start || epoch.seconds() >= self.end {
            return Err(EphemerisError::OutsideCoverage);
        }
        let delta = i128::from(epoch.seconds()) - i128::from(self.init);
        let index = usize::try_from(delta.div_euclid(i128::from(self.interval)))
            .map_err(|_| EphemerisError::InvalidData)?;
        if index >= self.count {
            return Err(EphemerisError::InvalidData);
        }
        let offset = delta.rem_euclid(i128::from(self.interval)) as f64 + epoch.fraction();
        let x = 2.0 * offset / self.interval as f64 - 1.0;
        let mut state = State {
            position_m: [0.0; 3],
            velocity_m_s: [0.0; 3],
        };
        for axis in 0..3 {
            let start = (index * 3 + axis) * self.ncoef;
            let c = &self.coefficients[start..start + self.ncoef];
            let (mut b1, mut b2, mut d1, mut d2) = (0.0, 0.0, 0.0, 0.0);
            for &coefficient in c[1..].iter().rev() {
                let b = coefficient + 2.0 * x * b1 - b2;
                let d = 2.0 * b1 + 2.0 * x * d1 - d2;
                b2 = b1;
                b1 = b;
                d2 = d1;
                d1 = d;
            }
            state.position_m[axis] = (c[0] + x * b1 - b2) * 1000.0;
            state.velocity_m_s[axis] = (b1 + x * d1 - d2) * (2000.0 / self.interval as f64);
        }
        if !state
            .position_m
            .iter()
            .chain(&state.velocity_m_s)
            .all(|v| v.is_finite())
        {
            return Err(EphemerisError::InvalidData);
        }
        Ok(state)
    }
}
