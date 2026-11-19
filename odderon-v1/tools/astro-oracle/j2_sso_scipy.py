#!/usr/bin/env python3
"""Independent SSO check. Install numpy and scipy; integrates with DOP853.

The requested secular J2 formula uses mean, not instantaneous osculating,
elements. The two initial conditions demonstrate why that distinction matters.
The first, chosen osculating fixture gives mean a=7178.137 km, e=.001,
i=98.60 deg approximately over this ten-day interval.
"""
import numpy as np
from scipy.integrate import solve_ivp

MU, R, J2 = 3.986004418e14, 6378137.0, 1.08262668e-3
T = np.linspace(0.0, 864000.0, 501)

def derivative(_, y):
    p = y[:3]
    r = np.linalg.norm(p)
    z = p[2]/r
    factor = -1.5*J2*MU/r**2*(R/r)**2
    return np.r_[y[3:], -MU*p/r**3 + factor*((1-5*z*z)*p/r + [0,0,2*z])]

def run(axis, inc_deg):
    e = .001
    inc = np.deg2rad(inc_deg)
    rp = axis*(1-e)
    v = np.sqrt(MU*(1+e)/rp)
    solution = solve_ivp(derivative, (0, T[-1]), [rp,0,0,0,v*np.cos(inc),v*np.sin(inc)],
                         t_eval=T, method='DOP853', rtol=1e-10, atol=1e-10)
    assert solution.success
    y = solution.y.T
    h = np.cross(y[:,:3],y[:,3:])
    hmag = np.linalg.norm(h,axis=1)
    aa = 1/(2/np.linalg.norm(y[:,:3],axis=1)-np.sum(y[:,3:]**2,axis=1)/MU)
    ee = np.sqrt(np.maximum(0,1-hmag*hmag/(MU*aa)))
    inclination = np.rad2deg(np.arccos(h[:,2]/hmag))
    node = np.unwrap(np.arctan2(h[:,0],-h[:,1]))
    rate = np.rad2deg(np.polyfit(T,node,1)[0])*86400
    print(f'initial a={axis:.1f} m i={inc_deg:.6f}°: mean a={aa.mean():.1f} m, e={ee.mean():.8f}, i={inclination.mean():.6f}°, RAAN={rate:.8f}°/day')

run(7187122.0,98.594574)
run(7178137.0,98.60)
