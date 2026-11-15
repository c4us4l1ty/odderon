# WP 1.5: liquid engine design, nozzle flow and thermal limits

`crates/odd-propulsion` turns six physical numbers into everything the flight
simulator needs. It exists so that a signed flight record can carry an engine
**design** rather than a claimed thrust and specific impulse: a verifier
recomputes performance from the design instead of trusting the client.

```rust
EngineSpec { propellant, of_ratio, p_c_pa, r_throat_m, r_exit_m, length_fraction }
    -> solve_engine(&spec, ambient_pa) -> EngineSolution
```

The crate is `f64` + `libm` only, with fixed iteration counts and no I/O,
clock or randomness. `tools/core-oracle` hashes every field of the solution
for a 32-configuration grid and requires native, Node wasm and headless
Chrome wasm to agree bit for bit.

## What is calibrated, and against what

This is **not** a chemical-equilibrium solver. NASA CEA cannot run inside a
deterministic wasm kernel, so flame temperature, effective expansion molar
mass and specific-heat ratio are smooth closed forms anchored to published
shifting-equilibrium values and to flying-engine performance:

| pair | anchor | `T_c` | `M̄` | `γ` | stoichiometric |
|---|---|---|---|---|---|
| LOX/LH2 | O/F 6.0, 20.64 MPa | 3588 K | 13.5 g/mol | 1.195 | 7.94 |
| LOX/RP-1 | O/F 2.36, 9.7 MPa | 3664 K | 22.9 g/mol | 1.2215 | 3.40 |
| LOX/LCH4 | O/F 3.5, 20 MPa | 3606 K | 21.2 g/mol | 1.205 | 4.00 |
| NTO/MMH | O/F 2.0, 1.5 MPa | 3278 K | 22.5 g/mol | 1.235 | 2.57 |

- `T_c(O/F)` is a downward parabola peaking near the stoichiometric ratio,
  scaled by `(P_c/P_ref)^0.02` because higher pressure suppresses dissociation.
- `M̄(O/F)` is a power law. It is an **effective expansion** molar mass, not
  the chamber equilibrium value: in a real hydrolox nozzle, dissociated
  hydrogen recombines as the gas cools, so the expanding mixture is heavier
  than the chamber mixture, and increasingly so when fuel-rich. Folding that
  in is what lets a frozen-`γ` solution reproduce shifting-equilibrium `Isp`
  and what stops the optimum from running away toward pure hydrogen.
- `γ(O/F)` is linear over the calibrated band.
- Bulk density is the exact harmonic combination
  `(1+r)/(r/ρ_ox + 1/ρ_fuel)`, which is why hydrolox needs enormous tanks.

Outside a propellant's calibrated mixture band, or outside 0.2–40 MPa, the
solver returns a named error rather than extrapolating.

Two efficiency constants per propellant are calibration, not first
principles, and are labelled as such in the source: `c_star_efficiency`
(0.965–0.985) scales mass flow, and `nozzle_efficiency` (0.985–0.997) scales
momentum thrust. `c*` efficiency changes thrust but **not** specific impulse,
because it divides out of `F/(ṁ g0)`.

### Why the temperature curve is asymmetric

A single symmetric parabola in O/F cannot reproduce both the temperature
peak and the efficiency peak. Excess fuel beyond stoichiometric adds mass
to the exhaust without releasing more heat, so real shifting-equilibrium
combustion loses temperature **faster on the rich side** than on the lean
side. With one curvature the model was too flat fuel-rich, and peak
specific impulse ran to the edge of the calibrated band for RP-1, LCH4 and
MMH instead of sitting where engines actually run. `tc_curvature_rich`
carries the steeper rich-side slope; `tc_curvature` keeps the lean side.

Fitted rich-side curvatures (K per (O/F)²): LH2 43 (symmetric, hydrolox is
already well behaved), RP-1 800, LCH4 440, NTO/MMH 515. These place peak
vacuum `Isp` at O/F **5.01 / 2.44 / 3.51 / 1.89**, inside the specified
optimal bands 4.5–6.5 / 2.2–2.7 / 3.2–3.8 / 1.6–2.2. The regression test
`every_propellant_peaks_isp_inside_its_optimal_band` sweeps all four bands
so this cannot silently drift again.

## Nozzle solution

- Choked mass flow `ṁ = P_c A_t / c*`, with
  `c*_ideal = √(γ R T_c) / [γ (2/(γ+1))^((γ+1)/(2(γ-1)))]`.
- Supersonic area–Mach inversion by damped Newton with the analytic
  derivative and a bisection fallback, converged to `|ΔM| < 1e-13`, bounded
  at 120 iterations.
- Exit state from the isentropic relations; `v_e = M_e √(γ R T_e)`.
- The wall is a parabola from a 30° opening angle (capped so the contour
  stays monotonic for short bells) to the exit plane. Its length is
  `length_fraction × (R_e − R_t)/tan 15°`.
- Rao bell divergence `λ = (1 + cos α_eff)/2` with
  `α_eff = 0.6 atan((R_e − R_t)/L)`. The 0.6 factor reproduces Rao's
  divergence tables for 60–100% bells; a parabola's momentum-averaged angle
  is well below its mean cone angle.
- Attached thrust `F = η_n λ ṁ v_e + (P_e − P_amb) A_e`, `Isp = F/(ṁ g0)`.

## Flow separation: the card's criterion, corrected for contour

The card specifies Summerfield, `P_sep = 0.38 P_amb`. Applied literally to a
contoured bell it contradicts the same card's RS-25 gate: the RS-25 exit
pressure is 0.22 atm, so the bare rule calls the sea-level flow separated and
predicts `Isp_sl ≈ 390 s`, while the real engine runs full and delivers
366 s. Summerfield's data is from **conical** nozzles; a bell turns the flow
back toward axial and its boundary layer survives a much larger pressure
rise.

The implemented threshold is therefore

```text
P_sep(x) = 0.38 P_amb · min(1, tan θ_wall(x) / tan 15°)
```

In the steep part of a bell — where separation actually occurs — the factor
is 1 and this is exactly the specified criterion. Only near-axial exit
sections get relief. The detachment line is the single root of
`P_wall(x) − P_sep(x)`, bisected on the axial coordinate.

Downstream of the separation line the flow is truncated: momentum and
pressure thrust are evaluated at the separation plane and the remaining wall
is assumed to recover to ambient, contributing no net axial force. A static
side-load estimate, 5% of the pressure imbalance over the separation area, is
reported as a design warning; it is not an aeroelastic transient prediction.

## Bartz heating and the wall limit

Throat film coefficient from the Bartz correlation with the usual 0.70
preliminary-design correction (Bartz over-predicts because it ignores
boundary-layer property variation):

```text
h_g = 0.70 · (0.026/D_t^0.2) · (μ^0.2 c_p / Pr^0.6) · (P_c/c*)^0.8 · (D_t/R_c)^0.1 · σ
```

The coolant side is a Dittus–Boelter jacket sized by continuity at a fixed
200 m/s design velocity, with 3 mm channel pitch and a 2:1 aspect ratio. The
hot-wall temperature solves
`T_w = (T_cool + h_g R T_aw)/(1 + h_g R)`, `R = 1/h_c + t_liner/k_liner`, by a
fixed 40-step iteration so the answer never depends on a convergence history.
Above **1350 K** (copper-chrome-niobium liner in a nickel jacket) the solution
is flagged `thermal_limit_exceeded` and a contract will not accept the
engine.

Because `h_g ∝ D_t^-0.2` while the coolant coefficient barely moves with
scale, **small throats run hot**: a 6 mm kerolox throat at 30 MPa trips the
limit while an RS-25-class throat sits near 1160 K. Pressure alone nearly
cancels, since coolant flow rises with chamber pressure too.

## Engine dry mass

No mass lookup. Chamber radius follows a 3:1 contraction ratio, chamber
length follows the propellant's characteristic length `L*`, wall thickness
follows hoop stress with a 1.5 safety factor against a 400 MPa allowable,
bell mass follows its actual frustum area, and turbopump mass follows shaft
power `P = ṁ ΔP/(ρ_bulk η)` at 40 kW/kg with a 1.35 accessory factor on the
static structure.

## Acceptance gates

`crates/odd-propulsion/tests/benchmarks.rs`:

- **`oracle_rs25_and_merlin_benchmarks`** — RS-25 (LOX/LH2, O/F 6.0,
  20.64 MPa, ε 69, `R_t` 0.1307 m): measured **446.84 s vacuum** (gate
  452 ± 8) and **368.73 s sea level** (gate 366 ± 10). Merlin 1D (LOX/RP-1,
  O/F 2.36, 9.7 MPa, ε 16): **309.07 s vacuum** (311 ± 8) and **280.09 s sea
  level** (282 ± 8).
- **`oracle_summerfield_separation_and_fuel_rich_optimum`** — an ε 80,
  5 MPa bell separates at sea level at `x/L = 0.218` with `ε_sep = 13.79`
  and re-attaches above 9 km, so it is attached at 35 km. Hydrolox peak
  vacuum `Isp` occurs at **O/F 4.95** (gate 4.5–6.2) while peak chamber
  temperature occurs at **O/F 8.0**, near stoichiometric 7.94; running
  stoichiometric costs more than 10 s of `Isp`.
- **`oracle_wasm_bit_parity_propulsion`** — `tools/core-oracle`'s
  `propulsion_fingerprint()` hashes 24 solution fields, the separation
  record and 24 flow stations for each of 32 designs spanning all four
  propellants and 0–87.5 km ambient. Native, Node and Chrome agree.
- **`oracle_fdr_engine_recomputation_anti_cheat`** —
  `crates/odd-verify/tests/engine_anti_cheat.rs`. A custom LOX/LCH4 design
  verifies through the portable replay; records claiming more `Isp`, more
  thrust, a lighter engine or a better engine than the one signed are all
  rejected by name even though the signature is valid.
- **`oracle_browser_cad_lab_and_idle_gate`** — the lab block in
  `tests/browser/observatory.mjs`. It drags the exit-radius handle on the
  cross-section, sweeps O/F past stoichiometric, sweeps altitude from 0 to
  100 km watching the bell separate and re-attach, fits a redesign to the
  stage, flies the contract, checks the signed record carries the design
  rather than a thrust claim, and confirms Annex B prints it. The existing
  two-second idle assertions then require zero animation callbacks, zero
  redraws and zero worker messages.
- `tests/atmosphere_parity.rs` pins the test-stand pressure model against
  `odd-astro`'s drag density so the two atmospheres cannot drift.
- `src/plume.rs` unit tests pin the Prandtl–Meyer round trip, the vacuum
  and separated regimes, the exact detachment coordinate, and a bounded
  axisymmetric Schlieren raster.

## The free jet and the Schlieren field

`src/plume.rs` reconstructs what a Schlieren photograph of the jet shows, so
the browser can paint a continuous density gradient without inventing flow
features of its own. It is a reduced-order reconstruction, **not CFD**.

- **Prandtl–Meyer expansion.** `nu(M)` is the exact perfect-gas function;
  its inverse is bisected to 1e-6 in Mach. An underexpanded lip fans outward
  by `nu(M_jet) - nu(M_exit)`, where `M_jet` follows from expanding `Pe` to
  ambient.
- **Oblique lip shock.** An overexpanded or detached jet is *compressed*, not
  expanded. `oblique_shock()` solves the normal-shock pressure relation for
  the wave angle `beta` by bisection, then returns the deflection `theta`
  from the theta-beta-Mach relation. It returns `None` when a normal shock
  cannot supply the required compression, which is precisely the condition
  for the shock to detach.
- **Detachment.** When Summerfield separation fires, the free jet starts at
  `x_sep * L_nozzle` with the radius and wall pressure there, taken from the
  shared `nozzle::separation_pressure()`. The drawn line and the computed
  line therefore cannot disagree; a test pins them to 1e-12.
- **Shock-cell train.** Prandtl–Pack spacing `L = 1.306 D sqrt(M^2 - 1)`.
  The core pressure oscillates about ambient with an exponentially decaying
  amplitude while the shear layer breathes in antiphase, which is what draws
  the Mach diamonds.
- **Mach disk.** Ashkenas–Sherman standoff `x_M / D = 0.67 sqrt(Pe / Pamb)`,
  drawn only above a two-to-one pressure ratio where a disk actually forms.
- **Density and gradient.** `density_at(x, r)` is isentropic inside the bell
  and a mixing shear layer outside. `schlieren()` central-differences it on a
  regular grid, weights the radial term (a vertical knife edge) four times
  the axial one, and compresses the dynamic range with `log1p` so the wall
  discontinuity does not hide the plume. The browser receives normalized
  values in [0, 1] and paints them; it differentiates nothing.

## Limits

Quasi-1D perfect-gas flow with frozen composition: no finite-rate kinetics,
no two-phase flow, no film-cooling or ablative cooling model, no combustion
instability, no throttle transients, no start/shutdown sequence, no thrust
structure loads beyond the static side-load estimate. The Schlieren field
is a feature reconstruction, not a solved flow field: cell shape, shear-layer
thickness and the mixing length are modelled, so the image is qualitatively
right and quantitatively indicative only. The pump model uses
bulk density rather than separate oxidizer and fuel pump work, which
under-states hydrolox pump power. The wall solution assumes every gram of
fuel passes through the jacket. Calibration is anchored at the four table
points and validated at two flying engines; accuracy between and beyond
those anchors is engineering-estimate quality, not CEA output.
