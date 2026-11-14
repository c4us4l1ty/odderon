//! Warmed native throughput only, not a per-query or cross-device guarantee.
use odd_core::epoch::Epoch;
use odd_ephem::{Body, Ephemeris};
use std::{hint::black_box, time::Instant};
fn main() {
    let eph = Ephemeris::bundled().unwrap();
    let epoch = Epoch::from_utc(2150, 6, 15, 0, 0, 0.0).unwrap();
    for _ in 0..1000 {
        black_box(
            eph.state(Body::Moon, Body::Earth, black_box(epoch))
                .unwrap(),
        );
    }
    let start = Instant::now();
    let n = 100_000;
    for _ in 0..n {
        black_box(
            eph.state(Body::Moon, Body::Earth, black_box(epoch))
                .unwrap(),
        );
    }
    println!(
        "{n} 2150 analytical Moon/Earth queries: {:.1} ns/query",
        start.elapsed().as_nanos() as f64 / n as f64
    );
}
