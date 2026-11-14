# Work Package 0.1: core foundation

`odd-core` implements the initial WP 0.1 foundation: SI quantity wrappers,
split TDB epochs, explicit UTC conversion, epoch-stamped hierarchical local
frames, coarse galactic anchors, one-way camera projection, and versioned
postcard/SHA-256 fingerprints. It is a pure computational library: it has no
clock reads, filesystem access, or network I/O. `odd-astro` additionally
contains early, ideal two-body calculators and a velocity-Verlet propagator;
these are not part of the WP 0.1 completion claim.

The design proposals remain intact in [blueprint.md](../blueprint.md) and
[ARCHITECTURE_LAWS.md](../ARCHITECTURE_LAWS.md). The clarification below is
intentional where their numerical requirements contradict real precision or
astronomical units.

## Precision and frame contract

- The 8,000-light-year test uses approximately **2,453 parsecs**, or 2,452,809
  **milliparsecs**. The blueprint's 2,452,809 *parsecs* is a factor of 1,000
  too large. The [IAU 2015 definition](https://www.iau.org/static/resolutions/IAU2015_English.pdf)
  is 648,000/π astronomical units per parsec; the
  [IAU 2012 astronomical unit](https://www.iau.org/static/resolutions/IAU2012_English.pdf)
  and the Julian year fix the relevant conversion.
- A `GalacticState` retains an integer milliparsec anchor and two f64 meter
  terms. A milliparsec is approximately 3.09 × 10¹³ m, so an anchor alone is
  not a precise stellar position. Catalog data must provide its residual,
  epoch, uncertainty, and reference-frame metadata.
- Local-to-local transforms go through their lowest common ancestor; a galactic
  round trip retains the anchor and a compensated two-term position. This avoids
  erasing docking-scale offsets when adding stellar residuals and AU-scale body
  origins, using compensated summation based on [TwoSum](https://www.tuhh.de/ti3/paper/rump/OgRuOi05.pdf).
  The test includes both at 8,000 ly. This is **not a universal
  guarantee**: rotations use f64 products, local states are eventually rounded
  to one f64, and cross-system anchor conversion loses fine precision as
  separation grows. Callers must choose suitable origins and accuracy budgets.
- Camera frames are rendering sinks: simulation transforms reject camera
  states; `project_to_camera` projects through the LCA to f32.
  Camera f32 output is deliberately one-way. A general sub-nanometer round
  trip through f32 cannot be guaranteed; tests demonstrate values that become
  indistinguishable when rendered. Camera projections must not be fed back into
  simulation state. Therefore the literal blueprint requirement to round-trip
  arbitrary f32 camera positions to nanometer accuracy is not claimed.
- Each tree is a snapshot at one TDB epoch. `KinematicTransform` accepts
  caller-supplied origins, rotations, velocities, and angular velocities at that
  epoch. Its velocity transport follows the
  [NAIF rotation/angular-velocity relationship](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/IDL/icy/cspice_rav2xf.html).
  The tree does **not** generate ephemerides or body orientation over time;
  those are later work packages. Users must label and supply correct states.

## Time and data contract

- TDB is represented as integer seconds plus a normalized fractional second
  since J2000.0 (JD 2451545.0 TDB), following
  [NAIF SPICE](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/FORTRAN/spicelib/str2et.html).
  A single f64 Julian date loses fine precision before it reaches the API; a
  two-part Julian date constructor is available for more precise input.
- UTC uses a proleptic Gregorian calendar with astronomical year numbering and
  the NAIF `naif0012.tls` positive-leap table. Before 1972 it assumes 10 s
  TAI−UTC; after 2017 it assumes 37 s. These are **extrapolations**, not
  historically correct pre-1972 UTC or predictions of future leap seconds.
  Update the table deliberately when new IERS bulletins are published.
- The NAIF periodic TT-to-TDB approximation has roughly 30 µs physical model
  accuracy, not nanosecond accuracy. Tests include CSPICE N0067 reference values
  for all 27 historical leap boundaries, plus the NAIF Cassini example.
- SHA-256 hashes include a little-endian u16 schema version followed by postcard
  bytes. **Only explicitly ordered, versioned schemas are canonical**: arbitrary
  `Serialize` implementations, unordered maps, NaNs, and signed zero can yield
  ambiguous or different encodings. Callers must validate and define their own
  schema and float policy; schemas also need distinct version/domain policies to
  prevent cross-schema ambiguity. `CanonicalValue` provides an ordered-map and finite
  float building block, but does not normalize Unicode or verify an arbitrary
  schema. A hash proves neither authorship nor real-world validity.

## Verification and remaining scope

GitHub CI runs native tests, Clippy, formatting, and wasm32 builds. A shared
fixture checks the J2000, UTC, split-frame/docking, and hash cases and compares
its byte-identical fingerprint in native Rust, Node WebAssembly, and headless
Chrome WebAssembly. This directly exercises a browser instead of relying on a
wasm build alone. Passing those fixtures is not a proof that all future physics
or all browsers are bit-reproducible.

WP 0.2 now adds a bounded Earth–Moon ephemeris; see [ephemeris notes](ephemeris.md).
No general ephemeris, propagator fidelity beyond the separate two-body experiment,
identity system, playable user interface, or real flight qualification is
implied by WP 0.1. UTC coverage, source data, cross-system precision budgets,
and richer physical models must be revisited in later packages.
