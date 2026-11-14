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

# Difference of nearly equal circular and transfer velocities.
r1 = mp.mpf(7_000_000.0)
r2 = mp.mpf(7_000_000.000_001)
a = (r1+r2)/2
first = mp.sqrt(mu*(2/r1-1/a))-mp.sqrt(mu/r1)
second = mp.sqrt(mu/r2)-mp.sqrt(mu*(2/r2-1/a))
print(f"close-orbit Hohmann burns={mp.nstr(first, 40)}, {mp.nstr(second, 40)}")

rb = mp.mpf(7_000_000.000_002)
a1, a2 = (r1+rb)/2, (r2+rb)/2
v = lambda r, axis: mp.sqrt(mu*(2/r-1/axis))
for burn in [v(r1,a1)-mp.sqrt(mu/r1), v(rb,a2)-v(rb,a1), mp.sqrt(mu/r2)-v(r2,a2)]:
    print(f"close-orbit bi-elliptic burn={mp.nstr(burn, 40)}")

# Large equinoctial longitudes must retain their original trigonometric phase.
for f, g, longitude in [(0.4, 0.3, 1e16), (0.6, 0.4, 1e17)]:
    f, g, longitude = map(mp.mpf, (f, g, longitude))
    radius = mp.mpf(8_000_000) / (1 + f*mp.cos(longitude) + g*mp.sin(longitude))
    print(f"large-longitude equinoctial radius={mp.nstr(radius, 40)}")
