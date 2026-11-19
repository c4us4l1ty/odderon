#!/usr/bin/env python3
"""Independent WP 1.4 finite-burn loss oracle (NumPy + SciPy DOP853).

Low-thrust trans-lunar injection from a 200 km circular parking orbit:
initial acceleration 0.8 m/s^2, Isp 450 s, ideal Hohmann departure delta-v
to a 384,400 km apoapsis radius. The thrust direction is frozen in inertial
axes at the parking-orbit velocity, exactly as odd-sim compiles a node.

The loss integrals are carried as additional ODE states, not reconstructed
from a spline:

    dL_steer/dt   = (F/m) (1 - u . v_hat)
    dL_gravity/dt = -(g . v_hat) = mu (r . v) / (|r|^3 |v|)

and must satisfy ideal_dv - (|v_end|-|v_start|) = L_steer + L_gravity.
Prints CSV lines consumed by crates/odd-astro/tests/finite_burn_loss.rs.
"""
import math
import numpy as np
from scipy.integrate import solve_ivp

MU = 3.986004418e14
R = 6378137.0
G0 = 9.80665


def case(name, radius, mass, thrust, isp, delta_v, centered=False):
    speed = math.sqrt(MU / radius)
    ve = isp * G0
    duration = mass * ve / thrust * (-math.expm1(-delta_v / ve))
    u = np.array([0.0, 1.0, 0.0])
    angle = -0.5 * math.sqrt(MU / radius**3) * duration if centered else 0.0

    def eom(t, y):
        r = y[:3]
        v = y[3:6]
        m = y[6]
        rr = np.linalg.norm(r)
        vv = np.linalg.norm(v)
        acc = -MU * r / rr**3 + thrust / m * u
        steer = thrust / m * (1.0 - np.dot(u, v / vv))
        gravity = MU * np.dot(r, v) / (rr**3 * vv)
        return np.r_[v, acc, -thrust / ve, steer, gravity]

    c, s = math.cos(angle), math.sin(angle)
    y0 = np.r_[
        radius * c, radius * s, 0.0,
        -speed * s, speed * c, 0.0,
        mass, 0.0, 0.0,
    ]
    sol = solve_ivp(
        eom,
        [0.0, duration],
        y0,
        method="DOP853",
        rtol=2e-13,
        atol=1e-11,
        max_step=5.0,
    )
    if not sol.success:
        raise RuntimeError(sol.message)
    y = sol.y[:, -1]
    ideal = ve * math.log(mass / y[6])
    gain = np.linalg.norm(y[3:6]) - speed
    deficit = ideal - gain
    residual = deficit - y[7] - y[8]
    print(
        f"{name},{radius:.12e},{mass:.12e},{thrust:.12e},{isp:.12e},"
        f"{delta_v:.12e},{duration:.12e},{angle:.12e},{ideal:.12e},{y[7]:.12e},"
        f"{y[8]:.12e},{deficit:.12e},{residual:.12e}"
    )


if __name__ == "__main__":
    print(
        "name,radius_m,mass_kg,thrust_n,isp_s,delta_v_m_s,duration_s,initial_angle_rad,"
        "ideal_dv_m_s,steering_loss_m_s,gravity_loss_m_s,speed_deficit_m_s,residual_m_s"
    )
    r = R + 200_000.0
    ra = 384_400_000.0
    tli = math.sqrt(MU / r) * (math.sqrt(2.0 * ra / (r + ra)) - 1.0)
    # a0 = 0.8 m/s^2 exactly at ignition.
    case("low_twr_tli", r, 1000.0, 800.0, 450.0, tli)
    # Short centered-periapsis regime where the card's cubic estimate applies.
    case("short_perigee", r, 1000.0, 800.0, 450.0, 60.0, centered=True)
