use crate::trig::{clamp_f32, dsqrt};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);

    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }

    #[inline]
    pub fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }

    #[inline]
    pub fn scale(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }

    #[inline]
    pub fn dot(self, o: Self) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    #[inline]
    pub fn cross(self, o: Self) -> Self {
        Self::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    #[inline]
    pub fn len(self) -> f32 {
        dsqrt((self.dot(self)) as f64) as f32
    }

    #[inline]
    pub fn normalized(self) -> Self {
        let l = self.len();
        if l > 1e-7 {
            self.scale(1.0 / l)
        } else {
            Self::new(0.0, 1.0, 0.0)
        }
    }

    #[inline]
    pub fn to_array(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3d {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3d {
    #[inline]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub fn len(self) -> f64 {
        dsqrt(self.x * self.x + self.y * self.y + self.z * self.z)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Aabb3 {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb3 {
    pub const fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// Slab ray-AABB intersection; returns hit distance `t >= 0` if hit within `max_dist`.
    pub fn ray_hit(&self, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<f32> {
        let mut tmin = 0.0_f32;
        let mut tmax = max_dist;

        let axes = [
            (origin.x, dir.x, self.min.x, self.max.x),
            (origin.y, dir.y, self.min.y, self.max.y),
            (origin.z, dir.z, self.min.z, self.max.z),
        ];

        for &(o, d, bmin, bmax) in &axes {
            let ad = if d < 0.0 { -d } else { d };
            if ad < 1e-6 {
                if o < bmin || o > bmax {
                    return None;
                }
            } else {
                let inv = 1.0 / d;
                let mut t1 = (bmin - o) * inv;
                let mut t2 = (bmax - o) * inv;
                if t1 > t2 {
                    core::mem::swap(&mut t1, &mut t2);
                }
                if t1 > tmin {
                    tmin = t1;
                }
                if t2 < tmax {
                    tmax = t2;
                }
                if tmin > tmax {
                    return None;
                }
            }
        }
        Some(clamp_f32(tmin, 0.0, max_dist))
    }
}
