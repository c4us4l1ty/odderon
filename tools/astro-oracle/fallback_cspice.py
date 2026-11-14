#!/usr/bin/env python3
"""Reproduce selected independent DE440s fallback reference states.

Usage: PYTHONPATH=/path/to/spiceypy python fallback_cspice.py /path/to/de440s.bsp
Uses seconds relative to J2000 TDB directly; no UTC/leap-second conversion.
"""
import hashlib
import sys
from pathlib import Path
import spiceypy as spice

kernel = Path(sys.argv[1])
expected = "c1c7feeab882263fc493a9d5a5b2ddd71b54826cdf65d8d17a76126b260a49f2"
if hashlib.sha256(kernel.read_bytes()).hexdigest() != expected:
    raise SystemExit("unexpected de440s.bsp SHA-256")
spice.furnsh(str(kernel))
for seconds in [-1_577_880_000, -43_200, 4_734_763_200]:
    for target, center in [("MOON", "EARTH"), ("EARTH BARYCENTER", "SUN")]:
        vector, _ = spice.spkezr(target, seconds, "J2000", "NONE", center)
        print(seconds, target, center, [float(v) * 1000 for v in vector])
