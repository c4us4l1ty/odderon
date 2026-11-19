//! A local throughput measurement, not a portability/performance guarantee.
use odd_core::epoch::Epoch;
use odd_ephem::{Body, Ephemeris};
use std::{hint::black_box, time::Instant};

fn main() {
    let ephemeris = Ephemeris::bundled().expect("bundled ephemeris");
    let samples = 1_000_000u64;
    let clock = Instant::now();
    for i in 0..samples {
        let epoch = Epoch::new(820_497_600 + (i % 86_400) as i64, 0.25).unwrap();
        black_box(
            ephemeris
                .state(Body::Moon, Body::Earth, black_box(epoch))
                .unwrap(),
        );
    }
    let elapsed = clock.elapsed();
    println!(
        "{samples} Moon/Earth queries in {elapsed:?}: {:.1} ns/query",
        elapsed.as_nanos() as f64 / samples as f64
    );
}
