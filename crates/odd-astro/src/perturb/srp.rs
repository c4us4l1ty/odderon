//! Cannonball solar radiation pressure and single-occulter conical shadow.
//! Occultation uses overlap of apparent angular disks (small-angle planar disk
//! approximation). Multiple overlapping occultors require a union calculation
//! and are not combined here.
use super::{dot, norm, PerturbationError};
const AU:f64=149_597_870_700.0;
const PRESSURE:f64=4.56e-6;

/// Fraction of the Sun's apparent disk visible at the spacecraft. `sun` and
/// `occulter` point from the craft to the respective centers, in meters.
pub fn illumination(sun:[f64;3],sun_radius:f64,occulter:[f64;3],occulter_radius:f64)->Result<f64,PerturbationError>{
    if !sun.iter().chain(occulter.iter()).all(|x|x.is_finite()) || !sun_radius.is_finite()||!occulter_radius.is_finite()||sun_radius<=0.0||occulter_radius<=0.0 { return Err(PerturbationError::InvalidInput); }
    let ds=norm(sun);let do_=norm(occulter);
    if !ds.is_finite() || !do_.is_finite() || ds<=sun_radius || do_<=occulter_radius {return Err(PerturbationError::InvalidInput);}
    if do_>=ds {return Ok(1.0);}
    let rs=libm::asin(sun_radius/ds);let ro=libm::asin(occulter_radius/do_);
    let sep=libm::acos((dot(sun,occulter)/(ds*do_)).clamp(-1.0,1.0));
    if sep>=rs+ro {return Ok(1.0);}
    if sep<= (rs-ro).abs() {return Ok(if ro>=rs {0.0} else {1.0-(ro/rs)*(ro/rs)});}
    let a=libm::acos(((sep*sep+rs*rs-ro*ro)/(2.0*sep*rs)).clamp(-1.0,1.0));
    let b=libm::acos(((sep*sep+ro*ro-rs*rs)/(2.0*sep*ro)).clamp(-1.0,1.0));
    let lens=rs*rs*a+ro*ro*b-0.5*libm::sqrt(((-sep+rs+ro)*(sep+rs-ro)*(sep-rs+ro)*(sep+rs+ro)).max(0.0));
    Ok((1.0-lens/(std::f64::consts::PI*rs*rs)).clamp(0.0,1.0))
}

/// Cannonball pressure acceleration away from the Sun. `sun` points from
/// spacecraft to Sun; reflectivity coefficient `cr` is dimensionless.
pub fn acceleration(sun:[f64;3],cr:f64,area:f64,mass:f64,visible:f64)->Result<[f64;3],PerturbationError>{
    if !sun.iter().all(|x|x.is_finite()) || !cr.is_finite()||cr<0.0||!area.is_finite()||area<0.0||!mass.is_finite()||mass<=0.0||!visible.is_finite()||!(0.0..=1.0).contains(&visible) {return Err(PerturbationError::InvalidInput);}
    let d=norm(sun);if !d.is_finite()||d<=0.0 {return Err(PerturbationError::InvalidInput);}
    let magnitude=visible*PRESSURE*(AU/d)*(AU/d)*cr*area/mass;
    let a=sun.map(|x|-magnitude*x/d);
    if a.iter().all(|x|x.is_finite()) {Ok(a)} else {Err(PerturbationError::NumericalRange)}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn eclipses_and_pressure_direction() {
        let sun=[AU,0.0,0.0];
        assert_eq!(illumination(sun,695_700e3,[4e8,0.0,0.0],1_737_400.0).unwrap(),0.0);
        assert_eq!(illumination(sun,695_700e3,[-4e8,0.0,0.0],1_737_400.0).unwrap(),1.0);
        let partial=illumination(sun,695_700e3,[4e8,2.0e6,0.0],1_737_400.0).unwrap();
        assert!(partial>0.0&&partial<1.0);
        let a=acceleration(sun,1.0,1.0,1.0,1.0).unwrap();assert!((a[0]+PRESSURE).abs()<1e-20);
    }
}
