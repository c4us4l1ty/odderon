# Cryogenic tanks, boil-off, and active cooling (WP 1.6)

`crates/odd-systems` is pure deterministic Rust — `f64` plus `libm`, no
`std::time`, no `web-sys`, no I/O — so the browser, the CLI and the future
Worker run the identical thermal code path.

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
`Q_lift + P` through the radiator. The three are implicit and solved by a
fixed 40-pass iteration — deterministic, no convergence history to replay.

The draw comes out of the **existing bus balance** in priority order
avionics → cryocooler → electric propulsion, so cooling genuinely steals thrust
capability from EP. The cooler is thermostatically modulated: it lifts only
what it must to hold the setpoint, never enough to refrigerate the liquid
below its own cold tip.

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

## Known approximation

`StageThermal::integrate` sizes the fuel tank's bus allowance from the
oxidizer's **end-state** temperature rather than solving both tanks
simultaneously. The oxidizer is served first and the fuel gets the remainder,
so total draw never exceeds generation, but the split is not instantaneous-
optimal. Both tanks share one flight clock.
