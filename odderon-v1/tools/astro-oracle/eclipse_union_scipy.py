#!/usr/bin/env python3
"""Independent apparent-disk eclipse union reference; requires scipy.

Integrate vertical chords of the occultor union clipped by the solar disk,
independently of the Rust circular-arc boundary integral. All radii and
centers below are angular-plane units (radians). For reproducibility: scipy
1.17.0, quad with 1e-14 absolute tolerance. This geometry is still the planar
angular-disk approximation, not a full spherical radiance model.
"""
import math
from scipy.integrate import quad


def visible(sun, occultors):
    def chord(x):
        half = math.sqrt(max(0.0, sun*sun-x*x))
        intervals = []
        for center, radius in occultors:
            dx = x-center
            if abs(dx) < radius:
                h = math.sqrt(radius*radius-dx*dx)
                lo, hi = max(-half, -h), min(half, h)
                if hi > lo:
                    intervals.append((lo, hi))
        if not intervals:
            return 0.0
        intervals.sort()
        lo, hi = intervals[0]
        total = 0.0
        for start, end in intervals[1:]:
            if start > hi:
                total += hi-lo
                lo, hi = start, end
            else:
                hi = max(hi, end)
        return total+hi-lo
    points = sorted(set(x for c,r in occultors for x in (c-r,c+r) if -sun < x < sun))
    area, error = quad(chord,-sun,sun,points=points,epsabs=1e-14,limit=1000)
    return 1-area/(math.pi*sun*sun),error

for circles in [[(.007,.012),(-.004,.012)],[(.014,.018),(-.008,.018)]]:
    print(circles, visible(.02,circles))
