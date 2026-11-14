# Phase 0: accepted foundations and known limits

On 2026-11-14, the project owner agreed to proceed with work packages 0.1–0.3
as **accepted foundations with the limitations below**, rather than claiming
that every literal requirement in the original design has been met. The
original [architecture proposal](../ARCHITECTURE_LAWS.md) remains intact.
This acceptance is not proof that the code is bug-free, a guarantee of
performance on every device, or a waiver of scientific accuracy in later work.

## 0.1: coordinate and time foundation

- Delivered: split TDB epochs and hierarchical frame snapshots, with a coarse
  integer-milliparsec galactic anchor and compensated position terms. Tests
  include a 0.5 m docking separation at approximately 8,000 light-years.
- Limit: the camera's `f32` projection is **one-way**; arbitrary positions
  cannot be round-tripped through `f32` with nanometer accuracy. The tested
  compensated computational round trip is not a universal precision guarantee,
  especially across distant systems. Catalog residuals and frame metadata are
  still required. The original proposal's 2,452,809 parsecs for 8,000 light-years
  is a unit error: it should be approximately 2,452,809 milliparsecs.
- UTC conversion now extrapolates the fixed leap table outside its historical
  range. The assumed 10 s before 1972 and 37 s after 2017 are not historical
  or future truth. See [foundation.md](foundation.md) for the contract.

## 0.2: Earth–Moon ephemeris foundation

- Delivered: geometric JPL DE440s-based Sun, Earth–Moon barycenter, Earth and
  Moon states, plus bounded approximate body orientations. The 2026 Earth–Moon
  reference test and independent implementation checks are documented in
  [ephemeris.md](ephemeris.md).
- Packed DE440 coverage is 2020–2035; a lower-accuracy analytical fallback
  now covers 3000 BC–AD 3000 and identifies its model. Switching models can
  introduce jumps; other stars and planets are not exposed. Earth orientation
  is approximate, not a full EOP-based frame.
- Chebyshev coefficients are evaluated directly; the separately specified
  five-iteration Kepler solver is not implemented by this package. A local
  warmed native benchmark is not a universal sub-microsecond guarantee.

## 0.3: observatory and demand-rendering foundation

- Delivered: procedural WebGL2 Earth–Moon view with painterly filtering and a
  separate vector overlay, interactive camera, and demand-driven rendering.
  The sampled Moon path and velocity come from the ephemeris; physical body
  radii use the same scale as their positions. Surfaces and the optional
  vehicle are illustrations, not a terrain map or simulated mission.
- An optional ideal test conic with computed apsides and nodes is now
  displayed. It is not a propagated mission or a B-plane solution; targeting
  data and higher-fidelity trajectories still require later physics work.
- Idle means no continuously scheduled application frames, not a guarantee of
  zero CPU or GPU use. The browser renders a single world pass on demand;
  its deterministic idle test checks animation and worker-message counters,
  not OS CPU or hardware GPU utilization. The target laptop has not been
  measured. See [observatory.md](observatory.md) for details and caveats.

## Verification and next step

The merge to `main` is commit `06ae9c562637cf7a1c7bc412d000eadde5be5542`;
its [CI run](https://github.com/c4us4l1ty/odderon/actions/runs/43577676948)
passed. These checks cover the implemented contracts and selected reference
cases, not all epochs, hardware, browsers, or future physics.

WP 1.1 extends the ideal two-body and Hohmann code with analytical elements,
transfers and patched-conic building blocks. See [orbital-elements.md](orbital-elements.md)
for validation, assumptions and remaining limitations. Real orbit/apsis
visualization can follow when validated data is available; B-plane targeting
requires additional physics. Revisit the Phase 0 coverage, orientation,
precision and physical-device performance limits when the product needs them.
