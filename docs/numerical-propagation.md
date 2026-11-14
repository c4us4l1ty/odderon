# Numerical propagation

`odd-astro` provides a forward adaptive Dormand–Prince 5(4)7 integrator and a
structure-of-arrays trajectory cache. Positions use cubic Hermite interpolation;
velocity is the analytic derivative and mass is interpolated linearly. Sampling
uses binary search and never reruns the integration. A callback supplies
body-centered inertial environment states at each Runge–Kutta stage. The
assembled force model combines point gravity, optional J2, Battin third-body
gravity, co-rotating atmospheric drag, cannonball SRP and constant-force burns.
Burn endpoints are automatically passed to the integrator as time events; the
step ending at an event evaluates the left limit and the next step the right.

The browser's comparison example is Earth-centered in J2000, with a 500 kg
spacecraft, 4 m² drag/SRP area, Cd 2.2 and Cr 1.3. It integrates 24 hours on a
short-lived worker using the bundled ephemeris; it displays a dashed cyan ideal
conic and a gold perturbed path. The browser example has no burn configured;
Rust callers can supply one. The model uses the J2000 z-axis as an approximate
Earth pole and a constant rotation rate, not observed Earth orientation data.
Changing the date or input cancels the old worker, and failures clear the
computed path. The API permits other force configurations, but the current
browser illustration is specifically an Earth model.

The seven standard geopotential lower-atmosphere layers end at about 86 km,
not 1000 km. Above them we use the published piecewise polynomial density fits
to the 1976 Standard Atmosphere tables through 1000 km; these fits approximate
the table, and their boundaries can have small rounding discontinuities. The
Standard Atmosphere is an idealized reference, not a time-dependent
space-weather model; actual thermospheric density can differ greatly. See
[NASA TR R-459](https://ntrs.nasa.gov/api/citations/19760017709/downloads/19760017709.pdf)
and [Braeunig's table 8 fits](http://www.braeunig.us/space/atmmodel.htm).
The upper profile uses geometric altitude; the lower model converts to
geopotential height. Height above the oblate reference ellipsoid is measured
radially, not as geodetic or terrain height.

The SRP shadow is a planar apparent-disk approximation for spherical occultors.
Earth and Moon are evaluated separately; taking the smaller illumination avoids
double-counting aligned eclipses but is not an exact union for partially
intersecting shadows. It ignores oblateness, atmospheric absorption and solar
limb darkening. Burns have fixed inertial direction and fixed Isp; there is no
dry-mass cutoff, throttle/guidance, state-triggered event detection, impact
handling, backwards integration or operational constraint model. An attempted
trajectory that reaches below the atmospheric reference surface fails, rather
than representing a landing. No model here should be used for flight operations.

Tests compare ten-period two-body conserved quantities, ten-day J2 nodal
precession, five-day drag decay with a separately integrated circular
energy-balance model, and a finite perigee burn against an impulsive transfer.
The native/Node/Chromium parity fixture hashes the final state of a ten-day
J2 + Moon + Sun + drag + SRP + burn run. Parity is tested for these builds and
inputs; it is not a claim about every compiler, CPU or configuration.
