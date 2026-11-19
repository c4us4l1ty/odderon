#!/usr/bin/env python3
"""Generate hyperbolic conics from NAIF CSPICE N0067 via spiceypy 8.4.0.

Run: python -m pip install spiceypy==8.4.0
     python tools/astro-oracle/generate_cspice.py > data/oracles/hyperbolic-cspice.csv
Units are converted from the CSPICE convention of km to SI in the output.
"""
import math
import random
import spiceypy as sp

assert sp.__version__ == "8.4.0"
rng = random.Random(10401)
mu_km = 398600.4418
print("p_m,e,i_rad,raan_rad,argp_rad,mean_rad,x_m,y_m,z_m,vx_m_s,vy_m_s,vz_m_s")
for _ in range(100):
    rp_km = 10 ** rng.uniform(3.8, 5.0)
    e = rng.uniform(1.05, 4.0)
    inc = rng.uniform(0.01, math.pi - 0.01)
    raan, argp = (rng.uniform(0, math.tau) for _ in range(2))
    mean = rng.uniform(-3, 3)
    # CSPICE conic elements are periapsis radius, e, i, node, argument,
    # mean anomaly at epoch, epoch, and μ.
    pv = sp.conics([rp_km, e, inc, raan, argp, mean, 0.0, mu_km], 0.0)
    row = [rp_km*(1+e)*1000, e, inc, raan, argp, mean, *(v*1000 for v in pv)]
    print(",".join(format(float(x), ".17g") for x in row))
