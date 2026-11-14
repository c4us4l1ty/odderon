//! Cancellation-resistant differential third-body gravity (Battin form).
use super::{dot, norm, PerturbationError};
/// `r` is spacecraft relative central body; `s` is third body relative central
/// body. Both vectors share inertial axes. `mu` is the third body's GM.
pub fn acceleration(r:[f64;3],s:[f64;3],mu:f64)->Result<[f64;3],PerturbationError>{
    if !r.iter().chain(s.iter()).all(|x|x.is_finite()) || !mu.is_finite() || mu<=0.0 { return Err(PerturbationError::InvalidInput); }
    let smag=norm(s); let d=std::array::from_fn(|i|r[i]-s[i]);let dmag=norm(d);
    if !smag.is_finite()||!dmag.is_finite()||smag<=0.0||dmag<=0.0 { return Err(PerturbationError::InvalidInput); }
    // Normalize by |s| before the dot product to avoid squaring the distance.
    let rn=r.map(|x|x/smag);let sn=s.map(|x|x/smag);
    let q=dot(rn,std::array::from_fn(|i|rn[i]-2.0*sn[i]));
    // 1+q is |r-s|²/|s|²; compute it directly near collision.
    let ratio=dmag/smag;
    let f=q*(3.0+3.0*q+q*q)/(1.0+ratio*ratio*ratio);
    let scale=-(mu/dmag)/dmag/dmag;
    let a=std::array::from_fn(|i|scale*(r[i]+f*s[i]));
    if a.iter().all(|v|v.is_finite()) {Ok(a)} else {Err(PerturbationError::NumericalRange)}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stable_tidal_limit_and_regular_case() {
        let a=acceleration([1.0,0.0,0.0],[1e12,0.0,0.0],1e24).unwrap();
        assert!((a[0]/2e-12-1.0).abs()<1e-11);
        let a=acceleration([1e7,2e7,-1e7],[3.84e8,1e8,2e8],4.9e12).unwrap();
        let s=[3.84e8,1e8,2e8];let r=[1e7,2e7,-1e7];let d=std::array::from_fn(|i|s[i]-r[i]);
        let dm=norm(d);let sm=norm(s);
        for i in 0..3 {let direct=4.9e12*(d[i]/(dm*dm*dm)-s[i]/(sm*sm*sm));assert!((a[i]-direct).abs()<1e-17);}
    }
}
