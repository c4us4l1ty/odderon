#!/usr/bin/env python3
"""Emit CSPICE IAU frame matrices at bounded-model and fractional epochs.

Requires spiceypy and CSPICE_N0067. Output IAU-body to J2000, row-major.
Usage: python generate_orientation.py pck00011.tpc > iau-orientation-cspice.csv
"""
import hashlib
import sys
from pathlib import Path
import spiceypy as spice

source = Path(sys.argv[1])
if hashlib.sha256(source.read_bytes()).hexdigest() != "3dff7b1dbeceaa01f25467767d3fa25816051c85d162d1edf04acb310ee28bb1":
    raise SystemExit("unexpected pck00011.tpc hash")
if spice.tkvrsn("TOOLKIT") != "CSPICE_N0067":
    raise SystemExit("expected CSPICE_N0067")
spice.furnsh(str(source))
print("# CSPICE_N0067 pck00011.tpc, TDB: body,seconds,fraction,body-to-J2000 row-major")
for seconds, fraction in [(631108800, 0), (820411200, .25), (820497600, 0), (1136030399, .5)]:
    for body in ("EARTH", "MOON", "SUN"):
        m = spice.pxform("IAU_" + body, "J2000", seconds + fraction)
        print(",".join([body, str(seconds), format(fraction, '.17g')] + [format(float(x), '.17g') for row in m for x in row]))
