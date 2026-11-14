#!/usr/bin/env python3
"""Reproduce the high-precision near-parabolic regression constants.

Install mpmath==1.4.1; Python floats are converted exactly into mpf values.
"""
import math
import mpmath as mp

assert mp.__version__ == "1.4.1"
mp.mp.dps = 80
e = mp.mpf(1.0 - math.ulp(1.0))
for value in [1e-8, 1e-9, 1e-7]:
    anomaly = mp.mpf(value)
    true_anomaly = 2 * mp.atan(mp.sqrt((1+e)/(1-e)) * mp.tan(anomaly/2))
    print(f"elliptic E={value!r}: ν={mp.nstr(true_anomaly, 40)}")
nu = mp.mpf(math.pi - 1e-8)
radius = mp.mpf(8_000_000) / (1 + e*mp.cos(nu))
print(f"elliptic radius={mp.nstr(radius, 40)}")
e = mp.mpf(1.0 + math.ulp(1.0))
nu = mp.mpf(math.pi - 3e-8)
f = mp.asinh(mp.sqrt(e*e-1)*mp.sin(nu)/(1+e*mp.cos(nu)))
print(f"hyperbolic F={mp.nstr(f, 40)}")

a = mp.mpf(1e18)
r = mp.mpf(math.nextafter(2e18, 0.0))
mu = mp.mpf(398600441800000.0)
speed = mp.sqrt(mu*(2/r-1/a))
print(f"near-apocenter vis-viva speed={mp.nstr(speed, 40)}")
