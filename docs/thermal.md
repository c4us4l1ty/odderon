# Cryogenic tanks, boil-off, and active cooling (WP 1.6)

`crates/odd-systems` is pure deterministic Rust — `f64` plus `libm`, no
`std::time`, no `web-sys`, no I/O — so the browser, CLI and the authority's
Rust/WASM replay run the identical thermal code path.

## What is modelled

A lumped single-node tank per propellant, stepped by **the same adaptive
Dormand–Prince tableau as `odd-sim`** (`odd_astro::dp45`, extracted so there is
exactly one set of coefficients in the repository). There is no separate
thermal clock and no wall-time polling: the tank ODE advances only along the
flight's trajectory spline, sampled through `integrate_along`.

Heat into the liquid:

```
Q_in  = eps_eff·sigma·A·(T_skin^4 − T_liq^4) + G_strut·(T_struct − T_liq)
mdot  = max(0, Q_in − Q_cryo) / h_vap(T)
```

The blanket's outer skin is a **quasi-steady node**, solved in closed form:

```
T_skin^4 = [ (Q_solar + Q_earthIR)/(sigma·A) + eps_out·T_sink^4 + eps_eff·T_liq^4 ]
           / (eps_out + eps_eff)
```

This matters. Sunlight and planet IR land on the *skin*, which re-radiates
almost all of it; only `eps_eff` of the imbalance reaches the propellant.
Depositing solar flux directly into the liquid gave **41.7 % loss over five
days**; with the skin node the same stage loses **3.1 %**. Solar and IR are
therefore already inside the `mli_w` term and must never be added twice.

`Q_solar = alpha·(A/4)·S_sun/r_helio²·nu_shadow`, where the quarter-area factor
is the projected area of a convex tank and `nu_shadow` comes from the real
two-occulter disc geometry (Earth and Moon) using **actual body vectors from
the Level-3 propagator** — no orbit-averaging. Earth IR is a 255 K blackbody
over the visible solid angle. A zero body vector means *body absent*, not
*inside the body*; treating the origin as an occulter extinguished sunlight in
deep space.

Pressure follows Clausius–Clapeyron anchored at the normal boiling point.
Below the relief setting the tank is closed and self-pressurizes; at relief it
vents and the temperature pins to the saturation isotherm. Every opening is
localized by 48-pass bisection and recorded as a `VentEvent` with an exact
microsecond timestamp, so a verifier reproduces the same event list. A tank
that *starts* at relief is venting at t = 0, and that opening is recorded too.

| Fluid | NBP | h_vap |
|---|---|---|
| LH2 | 20.27 K | 445.6 kJ/kg |
| LCH4 | 111.7 K | 510.8 kJ/kg |
| LOX | 90.19 K | 213.0 kJ/kg |
| NTO / MMH | storable | no boil-off |

## Active cooling and the electrical bus

`P = Q_lift / COP`, `COP = f·T_c/(T_h − T_c)`, and `T_h` follows from rejecting
`Q_lift + P + Q_absorbed` through the radiator. The three are implicit and
solved by a fixed 40-pass iteration — deterministic, no convergence history to
replay.

`Q_absorbed` is the environment the radiator itself sees: a panel mounted
edge-on to the Sun takes a quarter of the direct beam and half of the planet's
IR disc, weighted by its absorptivity and area. Without this load the hot-side
iteration could relax onto the cold tip when the radiator was oversized for
the lift, driving `T_h − T_c → 0` and `COP → ∞` — an oversized panel appeared
to refrigerate for free. A finite internal approach temperature
(`MIN_COOLER_APPROACH_K = 5 K`) floors the span as a second guard, so the
machine is always a real heat pump. The regression
`an_oversized_radiator_does_not_produce_free_cooling` pins both.

The draw comes out of the **existing bus balance** in priority order
avionics → oxidizer cooler → fuel cooler → electric propulsion, so cooling
reduces the modeled power *available* to EP. The current contracts use
chemical engines; that surplus is not coupled to electric thrust or the
trajectory. The cooler is thermostatically modulated: it lifts only what it
must to hold the setpoint, never enough to refrigerate the liquid below its
own cold tip.

## Three numbers in the work-package card are physically impossible

These were **not** tuned away. Each gate asserts the corrected physics and
pins the card's literal configuration at its true value, so a future change
that quietly made the card "pass" would have to delete an assertion.

**1. "eps_MLI = 0.03, A = 4 m², 1 AU full sun, 300 K sink → 1.5 %/day."**
Those inputs give **13.7 %/day** (70.8 W into 100 kg of LH2), not 1.5. A 300 K
sink radiates ~459 W/m² onto the blanket; no 0.03 blanket turns that into
7.7 W. The published ~1.5 %/day figure belongs to the **35 K sunshielded,
conduction-dominated** regime the same sentence names. The gate tests that
regime with conductance derived from real strut geometry — six 304-stainless
struts, 6.4 mm diameter, 100 mm long, `G = k·A/L = 0.0270 W/K` — and gets
**1.415 %/day**, inside the required 15 %. Struts carry >100× the radiative
leak there, which is what makes the scaling conduction-limited. The sunlit
case is pinned separately at 13.7 %/day.

**2. "LEO with a 60-minute orbital period."** A 60-minute Earth orbit has
a = 5076.85 km, which is **1294 km below the surface**. The shortest possible
Earth orbit is 84.3 min (surface-grazing). The gate uses a real 400 km LEO,
period **92.4143 min**.

**3. "COP ≈ 0.1 at 20 K / 300 K."** Carnot is 20/280 = **0.07143**, so 0.1
exceeds the Second Law by 40 %. The gate keeps the card's *observable*
requirement — 2 kW of lift at 20 K must draw **at least 20 kW** — which the
Carnot-fraction model satisfies with a wide margin (2 kW → **110.7 kW** at
f = 0.25, COP 0.01806, radiator 296.9 K), and separately asserts the Carnot
ceiling is never crossed.

## Measured boil-off

12.8 t LoxLh2 at O/F 5.2, five days at 1 AU, `eps_eff = 4 × 0.05/(n+1)`:

| MLI layers | eps_eff | lost | fraction |
|---|---|---|---|
| 10 | 0.01818 | 462.56 kg | 3.61 % |
| 20 | 0.00952 | 399.72 kg | **3.12 %** |
| 30 | 0.00645 | 377.10 kg | 2.95 % |
| 40 | 0.00488 | 365.45 kg | 2.86 % |

Diminishing returns are real: past ~20 layers the struts, not the blanket,
dominate. That is the design lever — more layers stop helping, so the player
must either shield the struts or pay for active cooling.

## Oracle gate

`tools/astro-oracle/boiloff_eclipse_scipy.py` re-derives the heat balance
independently (own overlapping-disc shadow, own Clausius–Clapeyron inversion
from fluid constants) and integrates with SciPy LSODA, cross-checked against
adaptive quadrature. It writes `data/oracles/boiloff-eclipse.csv`.

Rust and SciPy agree to **0.00000 %** on all four orbits — far inside the 5 %
the card allows; the gate demands 0.5 % so real drift cannot hide.

## Instantaneous bus allocation

`StageThermal::integrate` advances both tanks in **one eight-component state
vector** (`odd_systems::coupled::integrate_pair`), so the bus split is resolved
inside the right-hand side at every Dormand–Prince substep rather than from
the oxidizer's end-of-flight peak. At time `t` the generation is
`P_gen(t)·ν_shadow(t)` from the trajectory's own sun/planet vectors; base
avionics comes off the top; the oxidizer cooler is served first; the fuel is
offered **exactly** `max(0, P_bus(t) − P_ox(t))`; and electric propulsion sees
only the surplus `max(0, P_fuel_available(t) − P_fuel(t))`. In eclipse the bus
collapses to zero, the coolers receive no power, and EP surplus is zero. The
accepted steps are logged as `BusAllocation` records on
`StageThermalHistory.bus_trend`, so the priority rule is checkable in the
integration history. The full bus trend is not stored in the signed FDR.

The earlier end-state-peak approximation handed the fuel a constant
`capacity − ox_peak` for the whole mission: fictional in eclipse (the bus was
zero) and needlessly small in full sun. The gate
`oracle_instantaneous_cryo_bus_priority` flies a real sun→eclipse→sun orbit and
checks the remainder bit-exactly at every accepted step, and that the
allowance actually varies with the instantaneous sunlight.

## Recommended card and the delegated chief

`Contract::thermal_card()` is the insulation and cooling card a contract is
designed around, sized from the heat its own reference flight integrates.
Storables (101, 102) return the neutral design — there is nothing to insulate.
The cislunar tug (103) recommends 40 MLI layers, a 0.15 W/K mounting, a 2 kW
cold-tip cooler and 40 m² of radiator on a 12 kW array. The assembly panel's
**Fit recommended thermal design** button fits it only when Thermal and Power
are manual, charges ~264 kg of real fit-out dry mass and clears any stale
plan. The button is disabled for storable stages or chief-owned controls.

`oracle_multiday_contracts_are_solvable_in_both_modes` actually flies the
multi-day objectives in manual and fully delegated modes: it checks impact,
fuel, the catalog orbital tolerances and objective score, as well as thermal
loss and an ideal delta-v margin after subtracting the measured loss. For the
assembled 103 reference plans, manual loses ~1.44 kg and the chief-sized stack
~8.99 kg over the 6.94-day window. The practice-only splitter targets the
assembled engine and masses, not the baseline catalog stage. The browser gate
also flies both assembled tug designs to the objective in local practice; a
separate first-attempt manual-card ranked flight tests the server receipt (its
single-burn trap is not an objective-success claim).

The thermal chief also specifies active cooling for a cryogenic coast — a
week-long LH2 storage is zero-boil-off territory — but conservatively: a 600 W
cold tip and 30 m² of radiator on oversized 1.2 W/K mounts, with the power
chief then sizing the array for that cooler's worst-case draw plus margin. On
the stage-specific reference flight it reports ~9 kg lost at a higher dry
mass than the right-sized card; the player beats it by tuning the mounts,
cooler and panel down.

## One-way coupling and verification limits

Thermal integration samples the already-flown orbital spline; it is **not**
part of the orbital or burn mass derivative. The reported vent loss is not
subtracted from propagated mass, per-burn fuel, the displayed propagated
remaining propellant, or scoring. The calculation also does not use the burn
timeline to deplete the two thermal tanks. A post-hoc rocket-equation margin
subtracting the total loss is therefore only an idealized check, not a fully
coupled feasibility proof or a bound on trajectory error. We do not assert
that the error is negligible. For frozen replay compatibility the scoring
fuel-failure threshold and prediction term use propagated mass minus the
**catalog baseline** dry mass, whereas the browser's displayed remaining fuel
uses the actual fitted dry mass; these differ for redesigned or delegated
vehicles. A lighter fitted stage can score a fuel failure even with fuel
remaining. Correcting these assumptions requires an explicit versioned physics
and scoring migration, rather than silently changing signed v3 results.

For new v3 certificates, storable stages have no thermal block; cryogenic
stages must carry exactly one stage-0 block with the engine's propellant and
recomputed tank volumes. The verifier checks finite, non-negative bus powers
(including EP nameplate demand), valid tank ranges and emittance, and fails
closed if construction or integration fails. It replays and compares per-tank
loss, cooler energy and vent events. Schemas 1 and 2 remain replayable offline
with no thermal history, but the authority refuses new submissions of them.
This strengthens verification rules without changing the v3 byte layout.

The historical fixed-input 16 kW cooled-tug pin remains **3.843961 kg** and
**4209.826545 MJ** on its original baseline-targeted maneuver plan. The UI now
targets the actual assembled stage, so that *different* trajectory produces
different thermal numbers; its dossier is checked against its own flown result
instead of re-baselining the fixed-input physics pin.
