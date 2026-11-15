#!/usr/bin/env python3
"""Independent boil-off oracle for `oracle_eclipse_thermal_cycle`.

Requires NumPy and SciPy. This is NOT the production model: it re-derives the
tank heat balance from the governing equations and integrates it with SciPy's
LSODA/Radau instead of the workspace Dormand-Prince, and it implements the
conical two-disc shadow separately. Agreement therefore means something.

Scenario: a 100 kg LH2 tank on a 400 km circular LEO, blanket outer skin
intercepting sunlight and Earth IR, support struts conducting from a 293 K
structure. The tank starts saturated at its relief setting, so it vents from
t = 0 and the liquid temperature is pinned; every net watt vaporizes liquid.

    m_dot = max(0, Q_net) / h_vap

Writes data/oracles/boiloff-eclipse.csv with the per-orbit vented mass.

NOTE ON THE PERIOD: the work-package card asked for a "60-minute orbital
period". A 60-minute orbit about Earth has a = 5076.85 km, which is 1294 km
BELOW the surface. The shortest possible Earth orbit (surface-grazing) is
84.3 minutes. This oracle uses the true 400 km period, 92.41 minutes.
"""
import csv
import math
import pathlib

import numpy as np
from scipy.integrate import solve_ivp, quad

# --- constants (SI) -------------------------------------------------------
SIGMA = 5.670374419e-8
AU = 149597870700.0
SOLAR = 1361.0
R_EARTH = 6371000.0
R_SUN = 695700000.0
T_EARTH_IR = 255.0
MU = 3.986004418e14
H_VAP = 445600.0          # LH2 latent heat, J/kg
C_LIQ = 9690.0            # LH2 liquid specific heat, J/(kg K)
M_LH2 = 0.00201588        # kg/mol
R_GAS = 8.31446261815324  # J/(mol K)
T_NBP = 20.27             # K at 101325 Pa
P_NBP = 101325.0
P_RELIEF = 200000.0


def saturation_temperature(pressure):
    """Invert Clausius-Clapeyron from the normal boiling point.

    Derived here from the fluid constants rather than copied from the Rust
    answer, so the two implementations can genuinely disagree.
    """
    scale = H_VAP * M_LH2 / R_GAS
    return 1.0 / (1.0 / T_NBP - math.log(pressure / P_NBP) / scale)


T_SAT_200KPA = saturation_temperature(P_RELIEF)

# --- tank design (must match the Rust scenario) ---------------------------
AREA = 4.0
EPS_EFF = 0.03
EPS_OUT = 0.8
ALPHA = 0.2
STRUT_G = 0.028
T_STRUCT = 293.0
T_SINK = 3.0
LIQUID_KG = 100.0

ALTITUDE = 400000.0
A_ORBIT = R_EARTH + ALTITUDE
PERIOD = 2.0 * math.pi * math.sqrt(A_ORBIT ** 3 / MU)
N_MEAN = 2.0 * math.pi / PERIOD


def disc_illumination(sun_vec, occ_vec, r_sun, r_occ):
    """Visible fraction of the solar disc behind one occulter.

    Standard overlapping-disc area on the sky, using angular radii. Written
    independently of the Rust `srp::illumination`.
    """
    ds = np.linalg.norm(sun_vec)
    do = np.linalg.norm(occ_vec)
    if do <= r_occ:
        return 0.0
    if do >= ds:
        return 1.0
    rs = math.asin(r_sun / ds)
    ro = math.asin(r_occ / do)
    su = sun_vec / ds
    ou = occ_vec / do
    sep = math.acos(float(np.clip(np.dot(su, ou), -1.0, 1.0)))
    if sep >= rs + ro:
        return 1.0
    if sep <= abs(rs - ro):
        return 0.0 if ro >= rs else 1.0 - (ro / rs) ** 2
    a = math.acos(max(-1.0, min(1.0, (sep * sep + rs * rs - ro * ro) / (2 * sep * rs))))
    b = math.acos(max(-1.0, min(1.0, (sep * sep + ro * ro - rs * rs) / (2 * sep * ro))))
    lens = (rs * rs * a + ro * ro * b
            - 0.5 * math.sqrt(max(0.0, (-sep + rs + ro) * (sep + rs - ro)
                                  * (sep - rs + ro) * (sep + rs + ro))))
    return max(0.0, min(1.0, 1.0 - lens / (math.pi * rs * rs)))


def geometry(t):
    """Tank on a circular orbit in the xy-plane; Sun fixed along +x."""
    theta = N_MEAN * t
    tank = np.array([A_ORBIT * math.cos(theta), A_ORBIT * math.sin(theta), 0.0])
    sun_vec = np.array([AU, 0.0, 0.0]) - tank
    earth_vec = -tank
    return sun_vec, earth_vec


def net_heat(t, temperature=T_SAT_200KPA):
    sun_vec, earth_vec = geometry(t)
    illum = disc_illumination(sun_vec, earth_vec, R_SUN, R_EARTH)
    d_sun = np.linalg.norm(sun_vec)
    d_earth = np.linalg.norm(earth_vec)
    solar = ALPHA * (AREA * 0.25) * SOLAR * (AU / d_sun) ** 2 * illum
    earth_ir = (EPS_OUT * AREA * 0.25 * SIGMA * T_EARTH_IR ** 4
                * (R_EARTH / d_earth) ** 2)
    absorbed = (solar + earth_ir) / (SIGMA * AREA)
    skin4 = (absorbed + EPS_OUT * T_SINK ** 4
             + EPS_EFF * temperature ** 4) / (EPS_OUT + EPS_EFF)
    mli = EPS_EFF * SIGMA * AREA * (skin4 - temperature ** 4)
    strut = STRUT_G * (T_STRUCT - temperature)
    return mli + strut, illum, solar, earth_ir


def rhs(t, y):
    """y = [vented_kg]; temperature pinned at the relief isotherm."""
    q, *_ = net_heat(t)
    return [max(0.0, q) / H_VAP]


def main():
    root = pathlib.Path(__file__).resolve().parents[2]
    out = root / "data" / "oracles" / "boiloff-eclipse.csv"
    out.parent.mkdir(parents=True, exist_ok=True)
    rows = []
    for orbit in range(1, 5):
        t0, t1 = (orbit - 1) * PERIOD, orbit * PERIOD
        # Tight integration; the eclipse entry/exit are the only sharp features.
        sol = solve_ivp(rhs, [t0, t1], [0.0], method="LSODA",
                        rtol=1e-11, atol=1e-13, max_step=PERIOD / 400.0)
        if not sol.success:
            raise SystemExit(f"integration failed: {sol.message}")
        vented = float(sol.y[0, -1])
        # Cross-check the same orbit with adaptive quadrature of Q_net.
        integral, _ = quad(lambda t: max(0.0, net_heat(t)[0]) / H_VAP,
                           t0, t1, limit=400, epsabs=1e-12, epsrel=1e-11)
        if abs(vented - integral) / integral > 1e-6:
            raise SystemExit(f"ODE/quadrature disagree: {vented} vs {integral}")
        mean_q = integral * H_VAP / PERIOD
        rows.append({
            "orbit": orbit,
            "period_s": f"{PERIOD:.9f}",
            "vented_kg": f"{vented:.12e}",
            "mean_net_w": f"{mean_q:.9f}",
            "pct_per_orbit": f"{100.0 * vented / LIQUID_KG:.9f}",
        })
        print(f"orbit {orbit}: vented {vented:.9f} kg  mean Q {mean_q:.4f} W  "
              f"({100.0 * vented / LIQUID_KG:.5f} %/orbit)")
    with out.open("w", newline="") as fh:
        writer = csv.DictWriter(fh, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    print(f"\nperiod {PERIOD:.4f} s = {PERIOD / 60.0:.4f} min")
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
