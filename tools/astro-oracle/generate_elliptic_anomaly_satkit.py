#!/usr/bin/env python3
"""Generate elliptic anomaly fixtures using SatKit 0.24.3.

Run: python tools/astro-oracle/generate_elliptic_anomaly_satkit.py > data/oracles/elliptic-anomaly-satkit.csv
"""
import math
import random
import satkit

assert satkit.__version__ == "0.24.3"
rng = random.Random(1105)
print("e,eccentric_rad,mean_rad,true_rad")
for _ in range(100):
    e = rng.uniform(0, 0.95)
    eccentric = rng.uniform(-math.pi, math.pi)
    orbit = satkit.kepler(10000000.0, e, 0.4, 0.8, 0.9, eccentric_anomaly=eccentric)
    row = [e, eccentric, orbit.mean_anomaly, orbit.true_anomaly]
    print(",".join(format(float(x), ".17g") for x in row))
