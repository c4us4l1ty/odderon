//! Inertial-frame perturbation acceleration building blocks (SI units).
pub mod j2;
pub mod third_body;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerturbationError {
    InvalidInput,
    NumericalRange,
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norm(a: [f64; 3]) -> f64 {
    libm::sqrt(dot(a, a))
}
