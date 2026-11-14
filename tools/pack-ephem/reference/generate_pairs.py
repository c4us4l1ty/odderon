#!/usr/bin/env python3
"""Emit all supported target/center pairs at five instants from full DE440s.

Requires spiceypy with CSPICE_N0067. CSV uses geometric J2000 axes and SI.
Usage: python generate_pairs.py de440s.bsp > de440s-cspice-pairs.csv
"""
import hashlib
import sys
from pathlib import Path
import spiceypy as spice

source = Path(sys.argv[1])
if hashlib.sha256(source.read_bytes()).hexdigest() != "c1c7feeab882263fc493a9d5a5b2ddd71b54826cdf65d8d17a76126b260a49f2":
    raise SystemExit("unexpected full DE440s kernel hash")
if spice.tkvrsn("TOOLKIT") != "CSPICE_N0067":
    raise SystemExit("expected CSPICE_N0067")
spice.furnsh(str(source))
print("# CSPICE_N0067 full DE440s, J2000 geometric SI: target,center,seconds,fraction,x,y,z,vx,vy,vz")
for seconds, fraction in [(631108800, 0), (820411199, .75), (820411200, 0), (820497600, .25), (1136030399, .5)]:
    for target in (0, 3, 10, 301, 399):
        for center in (0, 3, 10, 301, 399):
            state, _ = spice.spkez(target, seconds + fraction, "J2000", "NONE", center)
            fields = [str(target), str(center), str(seconds), format(fraction, '.17g')]
            fields.extend(format(float(x) * 1000.0, '.17g') for x in state)
            print(','.join(fields))
