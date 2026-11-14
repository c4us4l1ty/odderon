//! Cannonball solar radiation pressure with spherical conical occultations.
//! Apparent angular disks are treated as planar disks. Two-occultor union
//! includes the triple intersection rather than double-counting overlap.
use super::{dot, norm, PerturbationError};
const AU: f64 = 149_597_870_700.0;
const PRESSURE: f64 = 4.56e-6;

/// Fraction of the Sun's apparent disk visible at the spacecraft. `sun` and
/// `occulter` point from the craft to the respective centers, in meters.
pub fn illumination(
    sun: [f64; 3],
    sun_radius: f64,
    occulter: [f64; 3],
    occulter_radius: f64,
) -> Result<f64, PerturbationError> {
    if !sun.iter().chain(occulter.iter()).all(|x| x.is_finite())
        || !sun_radius.is_finite()
        || !occulter_radius.is_finite()
        || sun_radius <= 0.0
        || occulter_radius <= 0.0
    {
        return Err(PerturbationError::InvalidInput);
    }
    let ds = norm(sun);
    let do_ = norm(occulter);
    if !ds.is_finite() || !do_.is_finite() || ds <= sun_radius || do_ <= occulter_radius {
        return Err(PerturbationError::InvalidInput);
    }
    if do_ >= ds {
        return Ok(1.0);
    }
    let rs = libm::asin(sun_radius / ds);
    let ro = libm::asin(occulter_radius / do_);
    let su = sun.map(|x| x / ds);
    let ou = occulter.map(|x| x / do_);
    let cross = [su[1]*ou[2]-su[2]*ou[1], su[2]*ou[0]-su[0]*ou[2], su[0]*ou[1]-su[1]*ou[0]];
    let sep = libm::atan2(norm(cross), dot(su, ou));
    if sep >= rs + ro {
        return Ok(1.0);
    }
    if sep <= (rs - ro).abs() {
        return Ok(if ro >= rs {
            0.0
        } else {
            1.0 - (ro / rs) * (ro / rs)
        });
    }
    let a = libm::acos(((sep * sep + rs * rs - ro * ro) / (2.0 * sep * rs)).clamp(-1.0, 1.0));
    let b = libm::acos(((sep * sep + ro * ro - rs * rs) / (2.0 * sep * ro)).clamp(-1.0, 1.0));
    let lens = rs * rs * a + ro * ro * b
        - 0.5
            * libm::sqrt(
                ((-sep + rs + ro) * (sep + rs - ro) * (sep - rs + ro) * (sep + rs + ro)).max(0.0),
            );
    Ok((1.0 - lens / (std::f64::consts::PI * rs * rs)).clamp(0.0, 1.0))
}

/// Visible fraction with two spherical occultors. Their union is clipped to
/// the Sun disk, including the overlapping portion exactly within the planar
/// angular-disk approximation. Vectors point from spacecraft to each center.
pub fn illumination_two(
    sun: [f64; 3],
    sun_radius: f64,
    first: [f64; 3],
    first_radius: f64,
    second: [f64; 3],
    second_radius: f64,
) -> Result<f64, PerturbationError> {
    let a = illumination(sun, sun_radius, first, first_radius)?;
    let b = illumination(sun, sun_radius, second, second_radius)?;
    if a == 0.0 || b == 0.0 { return Ok(0.0); }
    if a == 1.0 { return Ok(b); }
    if b == 1.0 { return Ok(a); }
    let ds = norm(sun);
    let u = sun.map(|x| x / ds);
    let reference = if u[2].abs() < 0.9 { [0.0, 0.0, 1.0] } else { [1.0, 0.0, 0.0] };
    let cross = [u[1]*reference[2]-u[2]*reference[1], u[2]*reference[0]-u[0]*reference[2], u[0]*reference[1]-u[1]*reference[0]];
    let length = norm(cross);
    let x_axis = cross.map(|v| v / length);
    let y_axis = [u[1]*x_axis[2]-u[2]*x_axis[1], u[2]*x_axis[0]-u[0]*x_axis[2], u[0]*x_axis[1]-u[1]*x_axis[0]];
    let disk = |v: [f64; 3], radius: f64| {
        let distance = norm(v);
        let unit = v.map(|x| x / distance);
        let x = dot(unit, x_axis);
        let y = dot(unit, y_axis);
        let tangent = libm::sqrt(x*x + y*y);
        let sep = libm::atan2(tangent, dot(unit, u));
        let (cx, cy) = if tangent == 0.0 { (0.0, 0.0) } else { (sep*x/tangent, sep*y/tangent) };
        Circle { x: cx, y: cy, radius: libm::asin(radius / distance) }
    };
    let solar = Circle { x: 0.0, y: 0.0, radius: libm::asin(sun_radius / ds) };
    let overlap = intersection_area(&[solar, disk(first, first_radius), disk(second, second_radius)]);
    let sun_area = std::f64::consts::PI * solar.radius * solar.radius;
    Ok((a + b - 1.0 + overlap / sun_area).clamp(0.0, 1.0))
}

#[derive(Clone, Copy)]
struct Circle { x: f64, y: f64, radius: f64 }

// Boundary integral of the intersection of up to three planar circles.
// Each accepted arc is traversed counterclockwise. Duplicates are ignored;
// otherwise identical boundaries would be integrated more than once.
fn intersection_area(circles: &[Circle]) -> f64 {
    let mut area = 0.0;
    let tau = std::f64::consts::TAU;
    for (i, c) in circles.iter().enumerate() {
        if circles[..i].iter().any(|o| o.x == c.x && o.y == c.y && o.radius == c.radius) { continue; }
        let mut angles = vec![0.0, tau];
        for (j, o) in circles.iter().enumerate() {
            if i == j { continue; }
            let dx = o.x - c.x; let dy = o.y - c.y;
            let d = libm::sqrt(dx*dx + dy*dy);
            if d > (c.radius-o.radius).abs() && d < c.radius+o.radius {
                let center = libm::atan2(dy, dx);
                let offset = libm::acos(((c.radius*c.radius+d*d-o.radius*o.radius)/(2.0*c.radius*d)).clamp(-1.0,1.0));
                angles.push((center-offset).rem_euclid(tau));
                angles.push((center+offset).rem_euclid(tau));
            }
        }
        angles.sort_by(f64::total_cmp);
        for pair in angles.windows(2) {
            let (start,end) = (pair[0],pair[1]);
            let middle = (start+end)*0.5;
            let x = c.x+c.radius*libm::cos(middle); let y = c.y+c.radius*libm::sin(middle);
            if circles.iter().all(|o| (x-o.x)*(x-o.x)+(y-o.y)*(y-o.y) <= o.radius*o.radius*(1.0+1e-14)) {
                area += 0.5*(c.radius*c.radius*(end-start)
                    + c.radius*c.x*(libm::sin(end)-libm::sin(start))
                    + c.radius*c.y*(libm::cos(start)-libm::cos(end)));
            }
        }
    }
    area.max(0.0)
}

/// Cannonball pressure acceleration away from the Sun. `sun` points from
/// spacecraft to Sun; reflectivity coefficient `cr` is dimensionless.
pub fn acceleration(
    sun: [f64; 3],
    cr: f64,
    area: f64,
    mass: f64,
    visible: f64,
) -> Result<[f64; 3], PerturbationError> {
    if !sun.iter().all(|x| x.is_finite())
        || !cr.is_finite()
        || cr < 0.0
        || !area.is_finite()
        || area < 0.0
        || !mass.is_finite()
        || mass <= 0.0
        || !visible.is_finite()
        || !(0.0..=1.0).contains(&visible)
    {
        return Err(PerturbationError::InvalidInput);
    }
    let d = norm(sun);
    if !d.is_finite() || d <= 0.0 {
        return Err(PerturbationError::InvalidInput);
    }
    let magnitude = visible * PRESSURE * (AU / d) * (AU / d) * cr * area / mass;
    let a = sun.map(|x| -magnitude * x / d);
    if a.iter().all(|x| x.is_finite()) {
        Ok(a)
    } else {
        Err(PerturbationError::NumericalRange)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multiple_occultors_do_not_double_count_and_union_disjoint_shadows() {
        let sun=[1e9,0.0,0.0];
        let solar_radius=1e9*libm::sin(0.02);
        let distance=1e8;
        let radius=distance*libm::sin(0.01);
        let offset=0.015;
        let left=[distance*libm::cos(offset),distance*libm::sin(offset),0.0];
        let right=[distance*libm::cos(offset),-distance*libm::sin(offset),0.0];
        let one=illumination(sun,solar_radius,left,radius).unwrap();
        let coincident=illumination_two(sun,solar_radius,left,radius,left,radius).unwrap();
        let disjoint=illumination_two(sun,solar_radius,left,radius,right,radius).unwrap();
        assert!((one-coincident).abs()<1e-10, "{one} {coincident}");
        assert!((disjoint-(2.0*one-1.0)).abs()<1e-10, "{one} {disjoint}");
        assert!(disjoint < one && disjoint > 0.0);
    }
    #[test]
    fn overlapping_occulter_union_matches_independent_quadrature() {
        // SciPy adaptive integration of vertical disk-union chords; see
        // tools/astro-oracle/eclipse_union_scipy.py.
        let sun=[1e9,0.0,0.0];
        let solar=1e9*libm::sin(0.02);
        let distance=1e8;
        for (offset_a,offset_b,radius,expected) in [
            (0.007_f64,-0.004_f64,0.012_f64,0.437_522_047_620_394_f64),
            (0.014_f64,-0.008_f64,0.018_f64,0.077_608_326_506_371_f64),
        ] {
            let vec = |angle:f64| [distance*libm::cos(angle),distance*libm::sin(angle),0.0];
            let r=distance*libm::sin(radius);
            let visible=illumination_two(sun,solar,vec(offset_a),r,vec(offset_b),r).unwrap();
            assert!((visible-expected).abs()<2e-8, "{visible} != {expected}");
        }
    }
    #[test]
    fn eclipses_and_pressure_direction() {
        let sun = [AU, 0.0, 0.0];
        assert_eq!(
            illumination(sun, 695_700e3, [3e8, 0.0, 0.0], 1_737_400.0).unwrap(),
            0.0
        );
        assert_eq!(
            illumination(sun, 695_700e3, [-4e8, 0.0, 0.0], 1_737_400.0).unwrap(),
            1.0
        );
        let partial = illumination(sun, 695_700e3, [4e8, 2.0e6, 0.0], 1_737_400.0).unwrap();
        assert!(partial > 0.0 && partial < 1.0);
        let a = acceleration(sun, 1.0, 1.0, 1.0, 1.0).unwrap();
        assert!((a[0] + PRESSURE).abs() < 1e-20);
    }
}
