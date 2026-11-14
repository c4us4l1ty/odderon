#!/usr/bin/env python3
"""Generate independent CSPICE evaluation fixtures from full de440s.bsp.

Requires spiceypy 8.4.0 (CSPICE N0067). Verify source SHA-256 before use.
Usage: PYTHONPATH=... python generate_cspice.py de440s.bsp > de440-cspice.csv
CSV contains TDB seconds and fractional seconds, SPK target and center IDs,
then position in meters and velocity in meters/second, geometric J2000.
"""
import hashlib
import sys
from pathlib import Path
import spiceypy as spice

source = Path(sys.argv[1])
expected = "c1c7feeab882263fc493a9d5a5b2ddd71b54826cdf65d8d17a76126b260a49f2"
if hashlib.sha256(source.read_bytes()).hexdigest() != expected:
    raise SystemExit("unexpected full DE440s kernel hash")
if spice.tkvrsn("TOOLKIT") != "CSPICE_N0067":
    raise SystemExit("expected CSPICE_N0067")
spice.furnsh(str(source))
start, end = 631108800, 1136030400
# Evenly spread epochs, plus both sides of selected 4-day and 16-day
# Chebyshev boundaries. Fractional seconds are exactly representable in ET.
times = {(start + ((end - start - 2) * i // 63), (i % 4) * 0.25) for i in range(64)}
for i in range(16):
    boundary = 631022400 + (1 + 97 * i) * 345600
    if start < boundary < end - 1:
        times.update([(boundary - 1, 0.75), (boundary, 0.0), (boundary, 0.25)])
times.update([(start, 0.0), (end - 1, 0.75)])
print("# CSPICE_N0067 full DE440s, TDB seconds; target,center,seconds,fraction,x_m,y_m,z_m,vx_m_s,vy_m_s,vz_m_s")
for seconds, fraction in sorted(times):
    for target, center in [(3, 0), (10, 0), (301, 3), (399, 3)]:
        state, _ = spice.spkez(target, seconds + fraction, "J2000", "NONE", center)
        fields = [str(target), str(center), str(seconds), format(fraction, ".17g")]
        fields.extend(format(float(x) * 1000.0, ".17g") for x in state)
        print(",".join(fields))
