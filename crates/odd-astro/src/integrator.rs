//! Adaptive Dormand–Prince 5(4), seven stages, forward in SI seconds.
//!
//! Force callbacks provide acceleration and mass rate in an inertial frame.
//! Event times split steps and stages on the terminating endpoint use its left
//! limit. This supports piecewise-smooth finite-burn commands. The callback is
//! responsible for body ephemerides, thrust and physical model validity.
use crate::spline::{Keyframe, TrajectorySpline};

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub relative_tolerance: f64,
    pub absolute_tolerance: f64,
    pub initial_step: f64,
    pub max_step: f64,
    pub max_steps: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntegrationError {
    InvalidInput,
    ForceFailure,
    StepUnderflow,
    StepLimit,
    NumericalRange,
}

/// Integrate until `end`, recording each accepted step in the returned spline.
/// Events must be finite and strictly ordered; events outside the interval are
/// ignored. This method is forward-only and does not locate state-triggered events.
pub fn integrate<F>(initial: Keyframe, end: f64, events: &[f64], options: Options, mut force: F) -> Result<TrajectorySpline, IntegrationError>
where F: FnMut(f64, [f64; 3], [f64; 3], f64) -> Result<([f64; 3], f64), IntegrationError> {
    let Options { relative_tolerance: rtol, absolute_tolerance: atol, initial_step, max_step, max_steps } = options;
    if !end.is_finite() || end < initial.time || !rtol.is_finite() || rtol <= 0.0 || !atol.is_finite() || atol <= 0.0 || !initial_step.is_finite() || initial_step <= 0.0 || !max_step.is_finite() || max_step <= 0.0 || max_steps == 0
        || events.iter().any(|x| !x.is_finite()) || events.windows(2).any(|w| w[0] >= w[1]) { return Err(IntegrationError::InvalidInput); }
    let mut out=TrajectorySpline::new();
    out.push(initial).map_err(|_| IntegrationError::InvalidInput)?;
    let mut t=initial.time;
    let mut y=[initial.position[0], initial.position[1], initial.position[2], initial.velocity[0], initial.velocity[1], initial.velocity[2], initial.mass];
    let mut h=initial_step.min(max_step);
    let mut event=events.partition_point(|x| *x <= t);
    let mut attempts=0;
    while t < end {
        attempts+=1;
        if attempts > max_steps { return Err(IntegrationError::StepLimit); }
        while event < events.len() && events[event] <= t { event+=1; }
        let boundary=if event < events.len() { events[event].min(end) } else { end };
        let remaining=boundary-t;
        h=h.min(max_step).min(remaining);
        if h <= 0.0 || t+h == t { return Err(IntegrationError::StepUnderflow); }
        let landed = h == remaining;
        let next_t = if landed { boundary } else { t+h };
        let at_event = landed && event < events.len() && boundary == events[event];
        // End-stage samples use the left limit at a discontinuity. The next
        // step starts at the exact event and evaluates the new force law.
        let stage_end = if at_event { next_down(boundary) } else { next_t };
        let mut k=[[0.0;7];7];
        for i in 0..7 {
            let mut z=y;
            for (j, coeff) in A[i].iter().enumerate().take(i) {
                for n in 0..7 { z[n] += h * coeff * k[j][n]; }
            }
            if z[6] <= 0.0 || !z.iter().all(|x| x.is_finite()) { return Err(IntegrationError::NumericalRange); }
            let st=if C[i]==1.0 { stage_end } else { t+C[i]*h };
            let (acc, mass_rate)=force(st,[z[0],z[1],z[2]],[z[3],z[4],z[5]],z[6])?;
            k[i]=[z[3],z[4],z[5],acc[0],acc[1],acc[2],mass_rate];
            if !k[i].iter().all(|x| x.is_finite()) { return Err(IntegrationError::ForceFailure); }
        }
        let mut next=y;
        let mut err=0.0_f64;
        for n in 0..7 {
            let mut delta=0.0;
            for i in 0..7 { next[n] += h*B5[i]*k[i][n]; delta += h*(B5[i]-B4[i])*k[i][n]; }
            if !next[n].is_finite() { return Err(IntegrationError::NumericalRange); }
            let scale=atol + rtol * y[n].abs().max(next[n].abs());
            err=err.max((delta/scale).abs());
        }
        if !err.is_finite() { return Err(IntegrationError::NumericalRange); }
        if err <= 1.0 {
            if next[6] <= 0.0 { return Err(IntegrationError::NumericalRange); }
            y=next;t=next_t;
            out.push(Keyframe {time:t,position:[y[0],y[1],y[2]],velocity:[y[3],y[4],y[5]],mass:y[6]}).map_err(|_| IntegrationError::NumericalRange)?;
        }
        let factor=if err==0.0 { 5.0 } else { (0.9*libm::pow(err,-0.2)).clamp(0.2,5.0) };
        h*=factor;
    }
    Ok(out)
}

fn next_down(x:f64)->f64 {
    if x==0.0 { -f64::from_bits(1) } else if x>0.0 { f64::from_bits(x.to_bits()-1) } else { f64::from_bits(x.to_bits()+1) }
}

const C:[f64;7]=[0.0,1.0/5.0,3.0/10.0,4.0/5.0,8.0/9.0,1.0,1.0];
const A:[[f64;7];7]=[
    [0.0;7],
    [1.0/5.0,0.0,0.0,0.0,0.0,0.0,0.0],
    [3.0/40.0,9.0/40.0,0.0,0.0,0.0,0.0,0.0],
    [44.0/45.0,-56.0/15.0,32.0/9.0,0.0,0.0,0.0,0.0],
    [19372.0/6561.0,-25360.0/2187.0,64448.0/6561.0,-212.0/729.0,0.0,0.0,0.0],
    [9017.0/3168.0,-355.0/33.0,46732.0/5247.0,49.0/176.0,-5103.0/18656.0,0.0,0.0],
    [35.0/384.0,0.0,500.0/1113.0,125.0/192.0,-2187.0/6784.0,11.0/84.0,0.0],
];
const B5:[f64;7]=[35.0/384.0,0.0,500.0/1113.0,125.0/192.0,-2187.0/6784.0,11.0/84.0,0.0];
const B4:[f64;7]=[5179.0/57600.0,0.0,7571.0/16695.0,393.0/640.0,-92097.0/339200.0,187.0/2100.0,1.0/40.0];

#[cfg(test)]
mod tests {
    use super::*;
    fn options()->Options { Options { relative_tolerance:1e-12, absolute_tolerance:1e-12, initial_step:10.0,max_step:120.0,max_steps:1_000_000 } }
    #[test]
    fn oracle_two_body_energy_conservation() {
        let mu=3.986_004_418e14_f64; let rp=6_678_000.0; let e=0.7;
        let v=libm::sqrt(mu*(1.0+e)/rp); let a=rp/(1.0-e);
        let period=2.0*std::f64::consts::PI*libm::sqrt(a*a*a/mu);
        let initial=Keyframe {time:0.0,position:[rp,0.0,0.0],velocity:[0.0,v,0.0],mass:1000.0};
        let trajectory=integrate(initial,10.0*period,&[],options(),|_,r,_,_| {
            let d=libm::sqrt(r.iter().map(|x|x*x).sum::<f64>()); let f=-mu/(d*d*d);
            Ok((r.map(|x|f*x),0.0))
        }).unwrap();
        let s=trajectory.sample(10.0*period).unwrap();
        let energy=|k:Keyframe| { let r=libm::sqrt(k.position.iter().map(|x|x*x).sum::<f64>()); k.velocity.iter().map(|x|x*x).sum::<f64>()*0.5-mu/r };
        let angular=|k:Keyframe| k.position[0]*k.velocity[1]-k.position[1]*k.velocity[0];
        assert!((energy(s)/energy(initial)-1.0).abs()<1e-9);
        assert!((angular(s)/angular(initial)-1.0).abs()<1e-9);
    }
    #[test]
    fn event_splits_force_and_mass_rate() {
        let initial=Keyframe {time:0.0,position:[1.0,0.0,0.0],velocity:[0.0;3],mass:100.0};
        let out=integrate(initial,10.0,&[2.0,4.0],options(),|t,_,_,_| Ok((if (2.0..4.0).contains(&t) {[1.0,0.0,0.0]} else {[0.0;3]}, if (2.0..4.0).contains(&t) {-1.0} else {0.0}))).unwrap();
        let state=out.sample(10.0).unwrap();
        assert!((state.velocity[0]-2.0).abs()<1e-10);
        assert!((state.position[0]-15.0).abs()<1e-9);
        assert!((state.mass-98.0).abs()<1e-10);
        assert_eq!(out.sample(2.0).unwrap().time,2.0);
        assert_eq!(out.sample(4.0).unwrap().time,4.0);
    }
}
