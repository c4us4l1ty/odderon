//! Illustrative warmed native throughput, not a device or worst-case guarantee.
use odd_astro::elements::Elements;
use odd_core::frames::State;
use std::hint::black_box;
use std::time::Instant;

fn main() {
    let state = State {
        position_m: [6_524_834.0, 6_862_875.0, 6_448_296.0],
        velocity_m_s: [4_901.327, 5_533.756, -1_976.341],
    };
    let mu = 3.986_004_418e14;
    let count = 200_000;
    let element = Elements::from_state(state, mu).unwrap();
    for _ in 0..10_000 {
        black_box(Elements::from_state(black_box(state), mu).unwrap());
        black_box(black_box(element).to_state(mu).unwrap());
    }
    let start = Instant::now();
    for n in 0..count {
        let mut state = state;
        state.position_m[0] += (n % 100) as f64;
        black_box(Elements::from_state(black_box(state), mu).unwrap());
    }
    let from = start.elapsed();
    let start = Instant::now();
    for n in 0..count {
        let mut element = element;
        element.true_anomaly_rad += (n % 100) as f64 * 1e-5;
        black_box(black_box(element).to_state(mu).unwrap());
    }
    let to = start.elapsed();
    println!(
        "{count} calls: state→elements {:.1} ns/call, elements→state {:.1} ns/call",
        from.as_nanos() as f64 / count as f64,
        to.as_nanos() as f64 / count as f64
    );
}
