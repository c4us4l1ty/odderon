# Work Package 0.2: bounded Earth–Moon ephemeris

`odd-ephem` evaluates real JPL DE440 position coefficients, rather than
inventing circular or coplanar orbits. The bundled model covers **2020-01-01
00:00:00 TDB inclusive to 2036-01-01 00:00:00 TDB exclusive**. All queries
outside coverage return an error; there is no extrapolation. Supported objects
are the Sun, Earth–Moon barycenter, Earth, Moon, and solar-system barycenter.
The package does not yet provide other planets or stars.

States are geometric (no light-time or aberration), in meters and meters per
second, at an `odd-core` TDB `Epoch`, with an explicit target and center and
SPICE J2000 axes aligned with ICRF. Relative states use the nearest common
center, so Moon–Earth does not subtract two large barycentric positions. A
validated `Ephemeris` is immutable, evaluates without I/O or cache and can be
reused. It returns a model estimate, not an exact observation or an uncertainty
bound; the 0.1% oracle is not proof of uniform accuracy over the interval.

The JPL [SPK specification](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/req/spk.html)
describes type 2, which stores Chebyshev position coefficients in km with
velocity obtained by analytical differentiation, using TDB seconds past J2000.
This is an alternative to the blueprint's Kepler-plus-secular-rates proposal:
no Kepler solve is needed. It retains the numerical integration and lunar
perturbations embodied by the source ephemeris. JPL describes the fitted model
and uncertainties in [Park et al. (2021), DE440/DE441](https://ssd.jpl.nasa.gov/doc/Park.2021.AJ.DE440.pdf).
The specification's claims of *exact real-world positions at any date* and
≤5 Kepler iterations are not made here. Finite coverage and choice of model
are explicit instead.

## Source, provenance, and reproducibility

The upstream is JPL/NAIF [`de440s.bsp`](https://naif.jpl.nasa.gov/pub/naif/generic_kernels/spk/planets/de440s.bsp).
This repository's small packed file was produced from the 2020–2036 subset
`de440s_2020-2035.bsp` mirrored at
[laurpaul/LunarMaps](https://github.com/laurpaul/LunarMaps/tree/main/raw). The
mirror describes cutting the full DE440s SPK from Lawrence Livermore National
Laboratory's `ssa-data-lunar` 0.0.1 with SPICE SPKSUB. We independently
obtained that package's full `de440s.bsp` from PyPI and verified that clipping
it directly with `pack-ephem --range` reproduces the bundled file byte for byte.
Its SHA-256 is
`c1c7feeab882263fc493a9d5a5b2ddd71b54826cdf65d8d17a76126b260a49f2`;
this agrees with the [independent published kernel lock](https://github.com/mikezupper/learn-solar-system/blob/main/data/kernels.lock.json).
A checksum identifies a particular file, not an independent proof of its origin.
The subset's SHA-256 is
`f2ca56c14a88fa6a61d599daa40d12ae85ac25f204870fbc623d433c9a57cc37`;
the checked-in packed output has SHA-256
`8f651100deaaffbae85ae4dac2a577b805993e8e4cfbe7dd8b710913ece36335`.
The upstream subset's license and notice are preserved in `data/attribution/`.
The original large kernel is not committed; obtain it from a trusted source and
verify its hash before packing. For this exact mirrored subset, with GitHub CLI:

```sh
mkdir -p data/raw
# The blob ID pins the contents, not a moving branch.
gh api -H 'Accept: application/vnd.github.raw+json' \
  repos/laurpaul/LunarMaps/git/blobs/5613b931809458da8ef89673f95536b66a16a5de \
  > data/raw/de440s_2020-2035.bsp
sha256sum data/raw/de440s_2020-2035.bsp
cargo run --locked -p pack-ephem -- --range 631108800 1136030400 \
  data/raw/de440s_2020-2035.bsp /tmp/de440.ephb
cmp /tmp/de440.ephb data/packed/de440s-2020-2036.ephb
```

For broader use, obtain the full NAIF `de440s.bsp`, verify the SHA-256 above,
and use the same `--range` command on it, or omit `--range` to pack the full
coverage (approximately 21 MB). `Ephemeris::from_bytes` can load these files
from application-provided bytes; the application is responsible for external
storage and source integrity. Full DE440s covers 1849-12-26 through 2150-01-22
TDB; the exact packed coverage is returned by `coverage()` and remains
half-open. The separately provided orientation approximations are still limited
to 2020–2036 and do not automatically inherit external ephemeris coverage.
No large source kernels or full-range output are committed.

The offline packer deliberately reads little-endian DAF/SPK, J2000 type-2
segments (3→0, 10→0, 301→3, 399→3), with exact integer-second metadata. It
rejects missing/duplicate segments, a damaged DAF transfer sentinel, and
malformed timing and coefficients. The sentinel detects some transfer damage,
not all corruption. `--range` takes integral TDB seconds since J2000 and
selects all intersecting original records without refitting them. It
does not claim to be a general SPK parser or Horizons CSV fitter; use NAIF tools
first to subset or normalize other kernels. It does not verify source identity:
that is the operator's responsibility. The binary reader validates structure,
not authenticity.

### ODEPHEM1 binary layout

All values are little-endian. Magic `ODEPHEM1`, u32 segment count (=4), then
four segments in order: Earth–Moon barycenter, Sun, Moon, Earth. Each segment
has i32 target and center NAIF IDs, f64 start/end/record-init/record-length TDB
seconds, u32 coefficients per axis and u32 record count. The rest is f64
coefficients in record-major, xyz-axis-major, increasing-polynomial-order;
units are km. Intervals are uniform. Exact end is excluded even though SPK
normally permits its endpoint. Unknown versions and trailing bytes are rejected.

## Independent checks

- The 2026-01-01 00:00 TDB **Horizons** oracle was retrieved 2026-11-14 with
  target 301, center 500@399, vector table 2, ICRF frame, geometric correction
  NONE, and KM-S units. [Reproducible query](https://ssd.jpl.nasa.gov/api/horizons.api?format=text&COMMAND=%27301%27&EPHEM_TYPE=%27VECTORS%27&CENTER=%27500%40399%27&START_TIME=%272026-01-01%2000%3A00%27&STOP_TIME=%272026-01-02%2000%3A00%27&STEP_SIZE=%271%20d%27&REF_PLANE=%27FRAME%27&OUT_UNITS=%27KM-S%27&VEC_TABLE=%272%27&VEC_CORR=%27NONE%27&TIME_TYPE=%27TDB%27).
  Horizons identified its solution as **DE441**, not DE440. The test compares
  vector norm errors and range to <0.1%, and also exercises much tighter
  empirical tolerances at that epoch. Earth, Sun and barycenter are separately
  checked against Horizons SSB states. DE440 and DE441 are different solutions.
- CSPICE N0067 via spiceypy 8.4 evaluated the **full** DE440s kernel at
  113 epochs across the bundled coverage, including fractional instants and
  both sides of record boundaries. `data/oracles/de440s-cspice.csv` holds 452
  individual segment states; `tools/pack-ephem/reference/generate_cspice.py`
  regenerates it after verifying the full kernel's hash. The tests compare all
  position and velocity components. These checks verify the extraction,
  decoder and derivatives, not independence of the underlying ephemeris.
  The separate `de440s-cspice-pairs.csv` tests all 25 target/center pairs
  at five epochs (including the solar-system barycenter and self-states); its
  generator is `tools/pack-ephem/reference/generate_pairs.py`.
- CI checks the bundled hash and reproduces the packed bytes from the pinned,
  hash-verified SPK subset. Full-source reproduction was also verified locally.
  The 1900 and 2100 eight-day fixtures in `data/oracles/` were likewise packed
  from the hash-verified full source with `--range -3155716800 -3155025600`
  and `--range 3155716800 3156408000`; their states and half-open boundaries
  are checked against CSPICE. They test the custom-data path, not the bundled
  2020–2036 coverage.
- Native, Node wasm and headless Chrome wasm compare selected ephemeris and
  orientation outputs bit-for-bit. This fixture is not a guarantee for all
  inputs or architectures. CI also checks malformed and out-of-range input.

## Body properties and orientation

`physical` exposes selected DE440 GM estimates from Park et al. table 2 and
reference ellipsoid radii from [NAIF pck00011](https://naif.jpl.nasa.gov/pub/naif/generic_kernels/pck/pck00011.tpc)
(2022), not terrain or a complete gravity model. Barycenters have no radius.
`orientation` exposes the IAU Sun and lunar pole and prime-meridian models
from that PCK (Sun IAU 2015, Moon IAU 2009). The Moon's IAU frame is an
approximation, not the higher-precision DE440 lunar principal-axes or
mean-Earth frame. The Sun's rotating meridian is conventional; the plasma
rotates differentially. These models are restricted to the shipped coverage.
CSPICE frame matrices check the implementation. The 12 matrices in
`data/oracles/iau-orientation-cspice.csv` cover all three bodies, fractional
seconds, and the start/end of the supported interval; they can be regenerated
with `tools/pack-ephem/reference/generate_orientation.py` using pck00011.tpc
SHA-256 `3dff7b1dbeceaa01f25467767d3fa25816051c85d162d1edf04acb310ee28bb1`.
These test implementation against the same model, not the model's physical
accuracy.

For Earth, `earth_approx_orientation` supplies the NAIF IAU_EARTH low-accuracy
visualization model; NAIF warns its prime-meridian error is at least 150
arcseconds. It is not UT1-based and must not be used as a precise terrestrial
frame. `earth_mean_angles` separately computes IAU 2006 **mean** sidereal angle and
mean obliquity from separately supplied UT1 and TT days, checked against ERFA.
GMST is not *apparent* sidereal time: the latter needs nutation. A true
terrestrial-to-celestial transform additionally needs measured Earth orientation
parameters, polar motion, precession and nutation. Neither those measurements
nor a high-accuracy Earth orientation matrix are provided; do not treat GMST alone as
an accurate ICRF Earth-fixed transform. UT1 cannot be inferred exactly from TDB
or UTC, and no future Earth orientation is known exactly. This is a deliberate
correction to the blueprint's conflation of “true” rotation and GMST.

## Performance

`cargo run --release --locked -p odd-ephem --example benchmark` measures warmed
Moon/Earth throughput with varied epochs. Loading/validation is excluded. A
local optimized x86-64 sandbox run measured ~267 ns/query using a standalone
Rust build; this is illustrative only, not a browser, device, or worst-case
performance guarantee. No 1 µs claim should be assumed without profiling the
actual target and workload.
