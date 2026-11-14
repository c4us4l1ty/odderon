#!/usr/bin/env python3
"""Regenerate elliptical state fixtures using independent SatKit 0.24.3.

Run: python -m pip install satkit==0.24.3
     python tools/astro-oracle/generate_satkit.py > data/oracles/orbits-satkit.csv
SatKit's `kepler.to_pv()` outputs SI position/velocity in the input frame.
These cases exercise conversions, not fidelity to perturbed real trajectories.
"""
import math
import random
import satkit

assert satkit.__version__ == "0.24.3"
rng = random.Random(1103)
mu = 3.986004418e14
print("p_m,e,i_rad,raan_rad,argp_rad,nu_rad,x_m,y_m,z_m,vx_m_s,vy_m_s,vz_m_s")
for _ in range(100):
    a = 10 ** rng.uniform(6.9, 8.8)
    e = rng.uniform(0.01, 0.95)
    inc = rng.uniform(0.01, math.pi - 0.01)
    raan, argp, nu = (rng.uniform(0, math.tau) for _ in range(3))
    k = satkit.kepler(a, e, inc, raan, argp, nu, mu=mu)
    r, v = k.to_pv()
    row = [a * (1-e*e), e, inc, raan, argp, nu, *r, *v]
    print(",".join(format(float(x), ".17g") for x in row))
