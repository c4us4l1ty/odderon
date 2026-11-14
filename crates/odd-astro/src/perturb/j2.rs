//! J2 acceleration around an arbitrary inertial unit spin axis.
use super::{dot, norm, PerturbationError};

/// Position and pole are in the same inertial axes. `radius` is the
/// equatorial reference radius, `mu` the central GM, and `j2` dimensionless.
pub fn acceleration(position:[f64;3], pole:[f64;3], mu:f64, radius:f64, j2:f64) -> Result<[f64;3],PerturbationError> {
    if !position.iter().chain(pole.iter()).all(|x|x.is_finite()) || !mu.is_finite() || mu <= 0.0 || !radius.is_finite() || radius <= 0.0 || !j2.is_finite() {
        return Err(PerturbationError::InvalidInput);
    }
    let r=norm(position); let plen=norm(pole);
    if !r.is_finite() || r<=0.0 || !plen.is_finite() || plen<=0.0 { return Err(PerturbationError::InvalidInput); }
    let axis=pole.map(|x|x/plen); let u=position.map(|x|x/r); let z=dot(u,axis);
    let factor= -1.5*j2*(mu/r/r)*(radius/r)*(radius/r);
    let result=std::array::from_fn(|i|factor*((1.0-5.0*z*z)*u[i]+2.0*z*axis[i]));
    if result.iter().all(|v|v.is_finite()) { Ok(result) } else { Err(PerturbationError::NumericalRange) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn equator_pole_and_rotated_axis() {
        let a=acceleration([7e6,0.0,0.0],[0.0,0.0,1.0],3.986e14,6.378e6,0.00108263).unwrap();
        assert!(a[0]<0.0);assert_eq!(a[1],0.0);assert_eq!(a[2],0.0);
        let b=acceleration([0.0,0.0,7e6],[0.0,0.0,1.0],3.986e14,6.378e6,0.00108263).unwrap();
        assert!((b[2]/a[0]+2.0).abs()<1e-14);
        let c=acceleration([0.0,7e6,0.0],[1.0,0.0,0.0],3.986e14,6.378e6,0.00108263).unwrap();
        assert_eq!(a[0],c[1]);
    }
}
