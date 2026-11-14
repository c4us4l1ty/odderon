//! Instantaneous maneuver coordinates. TNW is an orthonormal velocity frame,
//! not the usual radial-transverse-normal frame: N is only exactly radial at
//! an apsis. All vectors use one inertial frame and SI units.
use odd_core::frames::State;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManeuverError {
    InvalidInput,
    DegenerateFrame,
    NumericalRange,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TnwBasis {
    pub tangential: [f64; 3],
    pub in_plane: [f64; 3],
    pub normal: [f64; 3],
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn unit(v: [f64; 3]) -> Result<[f64; 3], ManeuverError> {
    if !v.iter().all(|x| x.is_finite()) { return Err(ManeuverError::InvalidInput); }
    let scale = v.iter().fold(0.0_f64, |a, x| a.max(x.abs()));
    if scale == 0.0 { return Err(ManeuverError::DegenerateFrame); }
    let p = v.map(|x| x / scale);
    let mag = libm::sqrt(p.iter().map(|x| x*x).sum());
    Ok(p.map(|x| x / mag))
}

impl TnwBasis {
    /// T is velocity direction, W is orbital angular momentum, N = W × T.
    /// Degenerate radial states do not define a maneuver plane.
    pub fn from_state(state: State) -> Result<Self, ManeuverError> {
        let t = unit(state.velocity_m_s)?;
        let r = unit(state.position_m)?;
        let h = cross(r, t);
        let h_len = libm::sqrt(h.iter().map(|x| x*x).sum());
        if !h_len.is_finite() || h_len <= 1e-12 { return Err(ManeuverError::DegenerateFrame); }
        let w = h.map(|x| x / h_len);
        let n = unit(cross(w, t))?;
        Ok(Self { tangential: t, in_plane: n, normal: w })
    }

    /// Components are [T, N, W], in m/s; output is an inertial delta-v.
    pub fn inertial_delta_v(self, tnw: [f64; 3]) -> Result<[f64; 3], ManeuverError> {
        if !tnw.iter().all(|x| x.is_finite()) { return Err(ManeuverError::InvalidInput); }
        let v = std::array::from_fn(|i| self.tangential[i]*tnw[0] + self.in_plane[i]*tnw[1] + self.normal[i]*tnw[2]);
        if v.iter().all(|x| x.is_finite()) { Ok(v) } else { Err(ManeuverError::NumericalRange) }
    }
}

pub fn apply_impulse(state: State, tnw: [f64; 3]) -> Result<State, ManeuverError> {
    let dv = TnwBasis::from_state(state)?.inertial_delta_v(tnw)?;
    let velocity_m_s = std::array::from_fn(|i| state.velocity_m_s[i] + dv[i]);
    if !velocity_m_s.iter().all(|x| x.is_finite()) { return Err(ManeuverError::NumericalRange); }
    Ok(State { position_m: state.position_m, velocity_m_s })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frame_signs_and_nonradial_orbit() {
        let state = State { position_m: [2.0, 0.0, 0.0], velocity_m_s: [1.0, 1.0, 0.0] };
        let basis = TnwBasis::from_state(state).unwrap();
        let s = 1.0 / libm::sqrt(2.0);
        for (actual, expected) in basis.in_plane.iter().zip([-s,s,0.0]) { assert!((actual-expected).abs() < 1e-15); }
        assert_eq!(basis.normal, [0.0,0.0,1.0]);
        let applied = apply_impulse(state, [0.0, 0.0, 4.0]).unwrap();
        assert_eq!(applied.velocity_m_s, [1.0, 1.0, 4.0]);
        assert_eq!(TnwBasis::from_state(State { velocity_m_s: [1.0,0.0,0.0], ..state }), Err(ManeuverError::DegenerateFrame));
    }
}
