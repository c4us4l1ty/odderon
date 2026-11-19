#!/usr/bin/env python3
"""Generate parabolic states using CSPICE N0067 via spiceypy 8.4.0.

Run: python tools/astro-oracle/generate_parabolic_cspice.py > data/oracles/parabolic-cspice.csv
Time from periapsis uses Barker's equation; CSPICE propagates from periapsis.
"""
import math
import random
import spiceypy as sp

assert sp.__version__ == "8.4.0"
rng = random.Random(1104)
mu = 398600.4418
print("p_m,i_rad,raan_rad,argp_rad,nu_rad,x_m,y_m,z_m,vx_m_s,vy_m_s,vz_m_s")
for _ in range(50):
    rp = 10 ** rng.uniform(3.8, 5.0)
    p = 2*rp
    inc = rng.uniform(0.01, math.pi - 0.01)
    raan, argp = (rng.uniform(0, math.tau) for _ in range(2))
    d = rng.uniform(-2, 2)
    nu = 2*math.atan(d)
    et = 0.5 * math.sqrt(p**3 / mu) * (d+d**3/3)
    pv = sp.conics([rp, 1.0, inc, raan, argp, 0.0, 0.0, mu], et)
    row = [p*1000, inc, raan, argp, nu, *(v*1000 for v in pv)]
    print(",".join(format(float(x), ".17g") for x in row))
