# Software design

How the pieces fit, what is shipped, and what is deliberately not built yet.

Governing principle: **Odderon's ambition is civilization-scale. Its execution
must be bottleneck-scale.** Every layer below is the smallest thing that
could be made correct and provable, not the smallest thing that could be
demoed.

---

## The five laws, and where they are enforced

`ARCHITECTURE_LAWS.md` states them; this is where each one lives in code.

| Law | Enforcement |
|---|---|
| SI everywhere | `odd-core::consts`, no unit conversions at boundaries |
| Zero-polling idle | one-shot workers; browser gate asserts 0 callbacks / 0 messages after a frame |
| No Euler | one Dormand–Prince tableau in `odd-astro::dp45`, shared by orbits *and* tanks |
| No magic lookup | signed FDR stores physical specs; `certify_engine` and the tank replay recompute |
| NASA oracle gate | `tools/astro-oracle/*_scipy.py` → `data/oracles/*.csv` → Rust tests |

The integrator deserves a note: `dp45.rs` was **extracted** from
`odd-astro::integrator` when the thermal ODE arrived, rather than copied. The
six core-oracle fingerprints were re-verified bit-identical after the
extraction. There is exactly one set of coefficients in the repository, so a
tank cannot drift from an orbit.

---

## Crate map

```
odd-core        constants, epochs, frames — zero internal dependencies
odd-ephem       packed DE440s reader; 1.1 MB include_bytes!
odd-astro       elements, Level 1/2/3, dp45, splines, SRP/drag/J2, Shapley
odd-propulsion  thermochem, nozzle, Summerfield separation, Bartz flux
odd-systems     cryogenic tanks, thermal environment, electrical bus
odd-sim         maneuver planning, targeting, finite burns, encounters
odd-contracts   the three playable contracts, scoring, thermal design,
                Chief-AI department heads
odd-app         pure UI-domain logic: lens, causal graph, debrief, Elo
odd-identity    Ed25519 flight licenses, callsigns, public ids
odd-report      canonical FDR, JSON transport, all-vector dossier + QR
odd-verify      the portable evaluator: compute, replay, certify
odd-view        the scalar WASM boundary (the only crate with web-sys)
```

`odd-verify` is the portability boundary. It has no `std::time`, `fs`, `net`,
`thread`, `println!` or `env` — only a `OnceLock` for the ephemeris. The
browser, the headless CLI and the future Worker call the **same `compute`**.
The gate proves it: a browser-signed ranked flight verifies in the native
CLI, and a one-ulp throat forgery exits 1.

---

## What is shipped

### WP 1.5 — Engine Lab (`odd-propulsion`)

Quasi-1D nozzle solve from a physical design: propellant pair, O/F, chamber
pressure, throat and exit radius, bell length fraction. Summerfield separation
scaled by `min(1, tanθ_wall/tan15°)`, Bartz wall flux, a 1350 K wall limit.
Pins: RS-25 446.836 s vacuum / 368.732 s sea level; Merlin 1D 309.072 / 280.093.

The signed record carries the **design**, never a thrust or Isp float.
`appliedEngine` is the only design that flies — the lab preview and the
committed design are separate state, and a regression test guards the desync.

### WP 1.6 — Cryogenic systems (`odd-systems`)

Lumped single-node tank per propellant, stepped by the shared DP45 along the
trajectory spline. No separate clock, no wall-time polling.

```
Q_in  = eps_eff·sigma·A·(T_skin^4 − T_liq^4) + G_strut·(T_struct − T_liq)
mdot  = max(0, Q_in − Q_cryo) / h_vap(T)
```

Sunlight and Earth IR land on a **quasi-steady outer skin**, solved in closed
form, not on the liquid. That distinction is the difference between 41.7 %
and 3.1 % loss over five days. Eclipse uses the real two-occulter disc
geometry with body vectors from the Level-3 propagator — no averaging.

Active cooling draws `P = Q_lift/COP` from the existing bus in priority order
avionics → cryocooler → electric propulsion, so cooling genuinely steals EP
capability. The cooler is thermostatically modulated.

Measured: a 12.8 t LoxLh2 tug loses **582 kg (4.55 %)** over its 6.94-day
contract with a 20-layer blanket; 40 layers gives 521 kg. See
`docs/thermal.md` for the regime table and the three corrected card numbers.

### WP 1.7 — Chief Engineer abstraction (`odd-contracts::chief_ai`)

Four deterministic department heads — Propulsion, Thermal/Cryo, Power, GNC.
Per-discipline `[AUTO-DELEGATED]` / `[MANUAL]` toggle. Delegating adds
**+35 % mass** and **+30 % cost** to that discipline's hardware, prints a
badge naming the chief and the exact kilograms, and caps the efficiency
component at **0.65**. The bitmask is signed into the FDR.

Hand-checked consequence: a perfect manual flight scores 1.0; three of four
delegated scores 0.9125; at Kp = 40 that is exactly **3.5 Elo points** less.

### WP 1.8 — dossier, identity, ranked certificates

All-vector dossier with a decodable QR (`#verify=` fragment, Medium→Low
fallback), Ed25519 flight licenses, one ranked certificate per contract,
backup/restore, damaged-storage repair. The header now prints
`Scan at: <target>`, and says "relative to the serving origin" rather than
inventing a host when the payload is a bare fragment.

### FDR v3 — FROZEN

See `docs/fdr-schema-changelog.md`. Signed thermal specs, delegation bitmask,
exact vent history **and the integrated boil-off magnitude**. v1/v2 still
verify. Frozen before any `odd-worker` code was written.

---

## The oracle gate

Every work package ships with independent oracles, not self-consistency
checks. The Python side re-derives the physics from the governing equations
and integrates with SciPy; the Rust side must agree.

| Oracle | Reference | Tolerance | Actual |
|---|---|---|---|
| `oracle_lh2_self_pressurization_rate` | closed form + chain rule | 15 % | 1.415 %/day vs 1.5 |
| `oracle_eclipse_thermal_cycle` | SciPy LSODA + quadrature | 5 % | **0.00000 %** |
| `oracle_cryocooler_bus_drain` | Carnot ceiling | ≥ 20 kW | 110.7 kW |
| `oracle_delegation_efficiency_cap` | hand Elo arithmetic | exact | 3.5 points |
| `oracle_fdr_v3_anti_tamper` | signature + physics replay | exact | 10 forgeries rejected |

The eclipse oracle is genuinely independent: it implements its own
overlapping-disc shadow and inverts Clausius–Clapeyron from fluid constants
rather than copying the Rust answer. Agreement to 0.00000 % across four
orbits means the two implementations are solving the same equations.

---

## Known approximations and sacrifices

These are deliberate, documented, and bounded — not bugs awaiting discovery.

* **Stage cooling split.** `StageThermal::integrate` sizes the fuel tank's
  bus allowance from the oxidizer's **end-state** temperature, not an
  instantaneous simultaneous solve. Total draw never exceeds generation.
* **Boil-off is one-way coupled.** Loss is integrated along the flown spline
  and reported; the vented mass is not fed back into the propagator as a
  thrust-axis perturbation. For a few percent over a week the trajectory
  effect is far below the perturbations already modelled.
* **Thermodynamics.** Quasi-1D perfect gas, frozen composition. No CEA
  equilibrium solve.
* **Rating is client-authoritative.**Forgery is *detectable* by replay, not
  impossible. A server rating authority is WP 1.9.
* **No** launch windows, B-plane targeting, LOI, landing, space weather,
  steering law or server-side rating yet.
* **Identity is single-browser** with export/restore, not an account system.
* **Contract 102 flies storables.** NTO/MMH has no boil-off by definition, so
  its tank block is empty. Contract 103 is the cryogenic 12.8 t case.

---

## What is NOT built: WP 1.9

`odd-worker` does not exist yet, by design. The schema was frozen first so
that WP 1.9 can be written against a fixed target with no schema bumps, no
invalidated certificates and no rework.

The planned surface (blueprint §6.3–6.6): `POST /callsign`, `/commit`,
`/submit`; `GET /verify/{fdr_hash}`, `/leaderboard`. Roughly 10 ms of CPU per
verification fits the free tier with a **$0 chunked hash-chain**: a state
hash every ~30 simulated days, addressable by `?seg=i`, stored in D1. The
portable evaluator is already free of I/O, so the Worker runs the identical
code path — the 1.1 MB embedded ephemeris is the only wasm cost.
