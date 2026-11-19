# V3 runtime and trust boundaries

## One science engine, two input histories

The distributable is static. `scripts/stations.mjs` assembles the v1 browser instruments into a local multifunction display, removes their original site navigation, substitutes a no-transport authority adapter, and supplies the **same compiled module** used in the cockpit. There are separate WASM *instances*: a bench may run a costly hypothetical design without changing the live ship. Its handoff contains engine geometry, tank choices and delegation, never asserted thrust or dry mass. Its sampled plan is a visual reference only.

The v1 `odd-view` `cockpit` feature exposes `pilot_start`, `pilot_step`, `pilot_predict` and `pilot_read`; v1's default exports and old recorder schemas remain unchanged. The manual state carries a `CoupledCheckpoint` (position, velocity, propellant pool, tank columns, cooler energy). Every accepted interval uses the contract's adaptive integrator, force field and bundled ephemeris. Cryogenics resume `odd-verify::coupled_flight::fly_segment`; storables use the same coupled integrator with inert tanks. A design is solved and scored at its fitted mass. The display converts SI units, it does not compute an alternative flight. If WASM is unavailable, the game reports failure rather than silently substituting v2 physics.

The cockpit samples controls at a fixed wall-time quantum and records the actual simulation duration and throttle/pitch/yaw/roll command sent to Rust. Warp is for coasting only. Attitude is a **rate-command approximation relative to the velocity frame**, not a six-degree-of-freedom torque or reaction-wheel simulation. The exterior Earth is illustrative procedural art. The orbit map uses measured vectors and osculating apsides, not a literal view of the surface texture. The cached trail is downsampled for display; the replay tape is not. Paused flight schedules no continuous render loop.

## Reporting

- **Manual:** the optional preflight prediction is frozen when time advances. Review reports predicted versus measured apsides, fuel, elapsed time and burn Δv. Without a prediction the objective is still measured; no composite prediction grade is invented. Filing or exporting signs a domain-separated `odderon.manual/1` payload containing design, prediction, run-length encoded inputs and terminal telemetry. Replaying in an isolated worker verifies the pilot signature, rebuilds the stage and compares the measured terminal values. The verifier enforces a file/step budget and never changes the live cockpit. These tapes are specific to this input/physics revision; a future incompatible change requires a new format and retained replay implementation.
- **Planned:** the engineering benches retain the v1 experiment, original FDR formats, deterministic verification, signed dossier, attribution and old-schema replay. The bench does not misrepresent a sequence of analog throttle samples as maneuver nodes. Shapley analysis and v1 PDF/QR dossiers apply to planned experiments; the manual tape and comparison report are distinct.
- **Local storage:** the existing local pilot license is not server key custody. Exported manual tapes contain the public key and signature, not the recovery words or flight key. Lesson/playtest consent remains in the Academy. Filing explicitly saves one manual tape under a separate v3 key.

## No authority, no ranked claim

**Server-authoritative Astro-Elo is postponed.** Eliminating Cloudflare and every other online authority removes the trust anchor for commit → replay → server-signed receipt → ranked leaderboard. A locally signed and reproducible flight is *practice*, not a certified first attempt. V3 neither deploys a substitute server nor fabricates local Elo. Shared Rust may include v1 protocol/verification types; no authority service runs, and the shipped browser transport cannot make an authority call. Historical v1 deployments and docs do not describe v3.

## Distribution

CI emits the exact WASM tested by the ABI suite and ships it in the static-game artifact. No Node or Rust process runs on behalf of a player. The optional Electron host gives this directory a local secure origin; it has no Node integration in the renderer and no remote game services. Steam publishing is deliberately deferred. Keep numerical flight, contract definitions and scoring in Rust, and keep drawing, input and display formatting on the browser side.
