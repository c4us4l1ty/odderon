# Foundation: first implementation slice

This is an initial subset of Work Package 0.1, not a complete game or a completed
coordinate kernel. `odd-core` is pure Rust and has no runtime dependencies. It
contains split TDB epochs, defined astronomical constants, and local frame transformations, including instantaneous rotation and velocity. CI checks native tests, formatting, Clippy, and a WebAssembly build.
`odd-astro` also provides an initial ideal two-body Hohmann transfer calculator.
Its published [FreeFlyer worked example](https://ai-solutions.com/_freeflyeruniversityguide/hohmann_transfer.htm)
is an independent rounded reference, not an ephemeris or high-fidelity simulation.
A fixed-step velocity-Verlet propagator also models ideal two-body coasts. Tests
compare against a circular analytic orbit, time reversal, energy conservation,
and second-order step convergence. It does not yet pass the proposed high-fidelity
NASA/J2 oracle gate or detect impacts between steps.

## Scientific and engineering decisions

- Simulation time is TDB seconds since J2000.0 (JD 2451545.0 TDB), as in
  [NAIF SPICE](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/FORTRAN/spicelib/str2et.html).
  No UTC conversion is supplied: leap-second data and the TDB−TT correction are
  both needed. A single floating-point Julian date also limits input precision.
- The IAU-defined parsec is 648,000/π astronomical units, and a Julian year is
  365.25 days; see [IAU Resolution B2 (2012)](https://www.iau.org/static/resolutions/IAU2012_English.pdf)
  and [IAU Resolution B2 (2015)](https://www.iau.org/static/resolutions/IAU2015_English.pdf).
  8,000 light-years is approximately **2,453 parsecs**, not 2,452,809 parsecs.
- Integer milliparsecs are a **coarse anchor**, not an exact stellar position:
  each step is about 3.09 × 10¹³ m. A future catalog must also retain residuals,
  measurement uncertainty, epoch, and reference-frame metadata. Nearby
  calculations must stay local; reconstructing and subtracting huge f64 global
  coordinates will lose precision.
- A rotating frame needs a velocity correction, not just a position rotation.
  `KinematicTransform` uses the standard rotation-plus-angular-velocity transport
  equation; see [NAIF RAV2XF](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/IDL/icy/cspice_rav2xf.html).
  Its inputs are supplied at one epoch; it does not model Earth orientation.
- f32 camera coordinates are a one-way rendering projection. They cannot in
  general preserve a nanometer-level round trip. The docking test instead checks
  a particular local f64 transform and separation; it is not a universal
  precision guarantee. Even f64 local coordinates have magnitude-dependent
  precision, so frame origins and acceptable error budgets matter.

## Remaining work before WP 0.1 is complete

Define and validate frame identity/tree, time-dependent rotation models, and
cross-system residuals. Add robust epoch/calendar conversions
with versioned leap-second and time-scale data, SI quantity types, versioned
canonical serialization and hashing, additional independent reference cases,
and native/Wasm replay-parity tests. The current crate should not be used as
an ephemeris, navigation system, or flight-qualified software.
