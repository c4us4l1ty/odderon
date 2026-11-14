# WP 1.1: ideal two-body orbit and transfer foundation

`odd-astro` provides analytical tools for preliminary mission design. They
are **osculating two-body approximations**, not flight predictions. A state is
relative to the central body in caller-supplied non-rotating axes at one common
epoch. The APIs use SI units and radians. They do not model the central body's
surface, atmosphere, oblateness, third bodies, finite burns, uncertainties or
mission constraints. The browser observatory is not yet a maneuver planner.

## Elements and singularities

`elements::Elements::from_state` converts a Cartesian `odd_core::frames::State`
and gravitational parameter μ to elements; `to_state` converts them back.
Derived true anomaly is signed in [-π, π], including inbound hyperbolic motion.
`Elements::from_classical` accepts the six conventional elements `(a, e, i,
Ω, ω, ν)`. Elliptic `a` is positive and hyperbolic `a` is negative. Internally
the size is the **semilatus rectum** `p`, so a parabola (`e = 1`) can also be
specified without an infinite semi-major axis. `semi_major_axis_m()` returns
`None` for an exactly parabolic orbit or an unrepresentable value.

Circular and equatorial orbits have undefined individual classical angles.
`special_angles()` reports true longitude, argument of latitude or longitude
of periapsis when `e < 1e-8` or the orbit is within approximately `1e-8` rad
of either 0 or π inclination. Longitudes are inertial xy-plane longitudes;
argument of latitude is measured in the direction of motion. Retrograde
orientations are handled explicitly. These alternatives are limiting combined
angles for *near*-singular cases, not a claim that small but nonzero e or i is
exactly zero. The original classical angles are retained for reconstruction;
they can be poorly conditioned. At exact singularities a conventional zero is
used for the undefined angle. Radial motion has no orbital plane and returns an
error. Invalid, nonfinite, unrepresentable or physically inaccessible inputs
return errors instead of deliberately returning NaNs.

Near a parabola, the sign and magnitude of `a`, and near-singular classical
angles, can be extremely sensitive to state errors and floating-point
roundoff. Even a state mathematically on a parabola can produce an eccentricity
slightly above or below one after floating-point conversion. Do not classify a
real trajectory on exact equality alone; retain Cartesian state and select an
application-specific error budget. See [NAIF's numerical cautions](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/cspice/oscelt_c.html).

`anomaly` provides elliptic and hyperbolic conversions and bracketed
Newton/bisection solvers for Kepler's equations. The elliptic inverse returns a
principal angle in [-π, π], not a revolution count. The hyperbolic equation is
`M = e sinh(F) - F`; its mean anomaly is unwrapped. These safeguarded solvers do
not promise convergence in five iterations. Near an asymptote or for huge
inputs, angle and phase precision are limited by floating-point representation;
errors are reported when a result cannot be represented or a solve fails.

## Transfers and patched-conic building blocks

- `vis_viva_speed` covers elliptic, hyperbolic and parabolic scalar speeds;
  `hohmann_transfer` computes two impulsive burns between coplanar circular
  orbits. `bi_elliptic_transfer` computes three burns via a caller-selected,
  finite common apoapsis at or above both circular-orbit radii. Signed burns
  follow the direction of motion; total Δv sums their magnitudes. The latter
  does not choose an optimal apoapsis. Both coast times assume ideal Keplerian
  motion and no additional waiting for orbital phasing.
- `sphere_of_influence` evaluates the conventional Laplace approximation
  `a (m/M)^(2/5)` for a smaller secondary around a primary. It is not a real
  discontinuity in gravity. `patch_to_secondary` and `patch_to_primary` change the origin of a state
  using another same-epoch, same-inertial-axis state; it does **not** find an
  SOI crossing, plan an encounter, or propagate a complete interplanetary
  trajectory. Subtracting large nearby floating-point positions may lose detail.

## Reference checks and precision

The specified Vallado input uses μ = 3.986004418 × 10¹⁴ m³/s². SatKit 0.24.3
independently produces the following values from the exact input:

| Element | Result |
| --- | ---: |
| a | 36,127.337619679 km |
| e | 0.8328533984875215 |
| i | 87.869126177° |
| Ω | 227.898260357° |
| ω | 53.384930618° |
| ν | 92.335156762° |

The original proposal rounds ω to **53.38°**, but its stated relative tolerance
of 0.001% allows only about 0.000534°, less than the 0.004931° rounding
error. Both cannot be satisfied simultaneously for the stated inputs. The
test checks the rounded value to half a displayed unit (0.005°) and checks
all six values much more tightly against the independent full-precision
calculation. The SatKit tutorial publishes this same example at rounded
precision. This is a correction to the test specification, not a claim that
0.001% was passed against the rounded ω value.

Additional committed fixtures compare 100 elliptical states and 100 anomaly
sets with SatKit, and 100 hyperbolic plus 50 parabolic states with NAIF CSPICE
N0067 (`spiceypy` 8.4.0). Generators are in `tools/astro-oracle/`; their
outputs are checked in so tests do not need Python or a network. Tests also
cover near-circular/equatorial and retrograde configurations, parabolic and
hyperbolic branches, invalid input, and published transfer examples. The
native/Node/Chrome Wasm oracle runs a tolerance-based orbit fixture; orbital
outputs are not claimed bit-identical across platforms. These checks are not a
proof of accuracy for every possible input or a real mission's dynamics.

## Performance

Run `cargo run --release --locked -p odd-astro --example elements_benchmark`
for warmed native throughput on the current machine. A local optimized sandbox
run with a temporary minimal `odd-core::State` shim measured about **214 ns**
per state-to-elements and **73 ns** per elements-to-state call over 200,000
varied inputs. This is below the proposed 0.1 ms per conversion in that run,
but it is neither a production Cargo build measurement nor a worst-case,
browser, physical-device or universal performance guarantee.

## References

- [SatKit orbital elements tutorial](https://satkit.dev/tutorials/Keplerian%20Elements/) — the published Vallado state-vector example and element definitions.
- [NAIF `oscelt_c`](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/cspice/oscelt_c.html) and [NAIF `conics_c`](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/cspice/conics_c.html) — osculating conics and numerical cautions.
- [Orbital Mechanics & Astrodynamics: sphere of influence](https://orbital-mechanics.space/interplanetary-maneuvers/sphere-of-influence.html) and [bi-elliptic transfers](https://orbital-mechanics.space/orbital-maneuvers/bielliptic-hohmann-transfer.html) — the approximations and transfer geometry.
- [Bi-elliptic transfer worked example](https://en.wikipedia.org/wiki/Bi-elliptic_transfer) — independent rounded burn values used by a test.
