# UI vision and implementation boundary

This document is a forward-looking visual blueprint, not a statement that every
screen or rendering pass is shipped. The implemented root is now a portable,
full-screen game menu; the Observatory and current long-form Flight Deck remain
separate player routes. See [`docs/game-shell.md`](docs/game-shell.md) for the
exact shipped navigation, hosting boundary and browser gates. The unified
single-viewport Flight Deck described below is still future UI work.

To make **Odderon** look like a hand-painted masterpiece (like `@BlendiByl`’s *Painted Valley*) while functioning with the mathematical rigor of **NASA JPL Mission Control**—and running at **near-0% idle CPU on a laptop with no dedicated GPU**—we must design a specialized visual and UI architecture.

If you ask an AI coding agent to *"make a hand-painted space UI,"* it will normally write a heavy 60-FPS shader that drains your laptop battery in 20 minutes and makes small telemetry text blurry and unreadable.

Below is the exact **Shader Pipeline** and **Screen-by-Screen UI/UX Blueprint** designed so an AI coding agent can build it without hallucinating.

---

# Part 1: The "Painted Cosmos" Shader Architecture (Zero 3D Models, Near-Zero CPU/iGPU)

### 1. How the "Painted Valley" Aesthetic Works—and How We Adapt It for Integrated Graphics
In *Painted Valley*, every frame is composed of ~1.5 million procedural brushstrokes generated entirely in code. However, rendering 1.5 million strokes 60 times a second requires a dedicated GPU.

In **Odderon**, we achieve the exact same 1.5-million-brushstroke look on a **no-GPU laptop at 0% idle CPU** using **Two-Layer Separation + Progressive Accumulation Caching**:

```
┌─────────────────────────────────────────────────────────────────────────┐
│ LAYER 2: CRISP TELEMETRY & VECTOR OVERLAY (HTML5 Canvas2D / SVG / DOM)  │
│ • Razor-sharp 1px orbital trajectories, B-plane bullseyes, CAD lines    │
│ • Monospace numeric readouts, causal graph tooltips, maneuver handles   │
│ • Redraws ONLY the dirty 2D vector when mouse hovers/drags (<0.2% CPU)  │
├─────────────────────────────────────────────────────────────────────────┤
│ LAYER 1: THE "PAINTED COSMOS" WORLD CANVAS (WebGL2 Cached Framebuffer)  │
│ • 100% Procedural Geometry (Raymarched SDF Planets, Parametric Rockets) │
│ • 1.5 Million Instanced Brushstrokes oriented along physical flow lines │
│ • PROGRESSIVE ACCUMULATION: Renders in 4 micro-slices over 80ms when    │
│   camera stops moving, then FREEZES at 0 FPS (0.0% CPU / 0.0% iGPU!)    │
└─────────────────────────────────────────────────────────────────────────┘
```

### 2. The 3-Pass Procedural Painterly Shader Pipeline (For Your AI Agent)
Your AI agent will write zero `.obj` loaders and zero `.png` texture loaders. Every planet, moon, asteroid, habitat, and rocket engine is rendered through three lightweight WebGL2 passes:

1.  **Pass 1: Analytical Geometry & Physical Vector Field Buffer (G-Buffer)**
    *   **Planets & Moons:** Rendered as analytical spheres/ellipsoids ($J_2$ oblateness visible!) using ray-sphere intersection in the fragment shader. Surface geology (craters, basalt plains, ice caps, atmospheric cloud bands) is generated via 3D Simplex/Worley noise seeded by the body's real JPL Horizons catalog ID + physical properties (albedo, surface temperature, atmospheric pressure).
    *   **Spacecraft & Habitats:** Assembled from parametric CSG (Constructive Solid Geometry) primitives in code: truncated cones (nozzles), cylinders (tanks/O'Neill habitats), paraboloids (high-gain dishes), and flat plates (radiators/solar arrays).
    *   **The Tangent Flow Vector $(u, v)$:** Along with Color and Surface Normal $\vec{N}$, Pass 1 outputs a **2D Physical Flow Direction Vector** at every pixel:
        *   On gas giants/atmospheres: Aligned with latitude Coriolis wind bands.
        *   On rocky bodies: Aligned with crater rim tangents and elevation contours.
        *   On rocket plumes: Aligned with supersonic exhaust Mach-wave streamlines.
2.  **Pass 2: Instanced Brushstroke Splatting (`gl.drawArraysInstanced`)**
    *   Instead of standard smooth pixel shading, the shader spawns procedural brushstroke quads across screen-space tiles.
    *   Each stroke reads the **Physical Flow Vector** from Pass 1, rotates itself parallel to the physical flow, applies a subtle **OkLab color-space pigment jitter** ($\pm 4\%$ hue/luminance variation per stroke so it looks like real gouache/oil paint on canvas), and tapers at the bristle tips using a procedural quadratic falloff curve.
3.  **Pass 3: Progressive Accumulation Governor (The Zero-CPU Secret)**
    *   **While actively dragging the camera:** Render Pass 2 with **60,000 coarse strokes** at $60\text{ FPS}$ so movement feels buttery-smooth even on Intel UHD integrated graphics.
    *   **The moment the mouse stops moving:** Over the next 10 frames ($160\text{ ms}$), append **150,000 fine detail strokes per frame** directly on top of the framebuffer until reaching **1.56 million brushstrokes**, then set `webgl_loop_active = false`.
    *   **Result:** The player looks at a breathtaking, Chesley Bonestell-style hand-painted scene of Earth, the Moon, or their spacecraft while their laptop fan stays completely silent at **0.0% CPU**.

### 3. The Three Interactive Visual Lenses (Toggled with `Tab` / `1` `2` `3`)
Pressing `1`, `2`, or `3` transforms how the brushstrokes behave to teach physics visually:
*   **Lens 1: Visual / Bonestell Painterly Mode (Default):** True-color physical rendering (Rayleigh atmospheric blue-shift at the limb, blackbody thermal glow on red-hot rocket nozzles above $900\text{ K}$).
*   **Lens 2: The Schlieren & Blackbody Thermal Lens:** Turns the canvas into a high-contrast optical physics plate. Brushstrokes align with **gas density gradients ($\nabla \rho$)** inside and outside rocket nozzles (revealing shock diamonds and flow separation) and color-map spacecraft components by their exact Kelvin temperature ($20\text{ K}$ deep cryo-blue to $3,200\text{ K}$ throat incandescent white).
*   **Lens 3: The Gravity-Well & Perturbation Flow Lens:** Renders invisible space as a living Van Gogh *Starry Night* vector field! Faint, luminous brushstrokes swirl along the **Roche Equipotential Contours**, instantly making Lagrange points ($L_1\text{–}L_5$) visible as calm "paint eddies" between Earth and the Moon where gravity and centrifugal force cancel out!

---

# Part 2: The 6 Core Screen Layouts (Wireframe Blueprints for AI Agents)

To prevent UI clutter, **Odderon** uses a single unified viewport with a **Top Mode Ribbon** (switching POVs instantly) and a **Bottom Counterfactual Time-Scrubber**.

---

## Screen 1: The Astrodynamics & Mission Planning Desk (FDO View)
*This is where players design orbits, play with the Visual Physics Lens, and lock in their Prediction Contract.*

```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│ [ODD] ODDERON Aerospace │ $42.8M │ Astro-Elo: 1420 │ POV: [1:FDO Map] [2:CAD] [3:MOCR]   │
├──────────────────────┬────────────────────────────────────────────┬──────────────────────┤
│ MISSION & TELEMETRY  │                                            │ ENGINEERING CONTRACT │
│                      │         LAYER 1: PAINTED EARTH-MOON        │ (Prediction Lock-In) │
│ Target: Lunar L1     │         LAYER 2: CRISP ORBIT VECTORS       │                      │
│ Propagator:          │                                            │ [x] Overlay Lvl-1    │
│ (•) Lvl-1 Analytic   │                  . - ~ ~ - .               │     vs Lvl-3 Preview │
│ (•) Lvl-3 Perturbed  │              . '      ●      ' .           │                      │
│     [J2][Moon][SRP]  │            /       (Earth)       \         │ YOUR PREDICTIONS:    │
│                      │           │   o──► [Prograde]     │        │ Final Perigee:       │
│ CURRENT ORBIT (OSC)  │           │  (Ship)               │        │ [ 185.0 ] km         │
│ a :  6,578.14 km     │            \                     /         │ Transfer Δv:         │
│ e :  0.0012          │              .      Apoapsis    .           │ [ 3,142.5 ] m/s      │
│ i :  28.50° (Cape)   │                ' - . _ _ . - '             │ Propellant Margin:   │
│ Ω :  114.2°          │                       │                    │ [ 118.0 ] kg         │
│ ω :  42.8°           │                       ▼                    │ Time of Flight:      │
│                      │        ┌─────────────────────────────┐     │ [ 84.5 ] hrs         │
│ MANEUVER NODE #1     │        │ VISUAL PHYSICS LENS (Hover) │     │                      │
│ Δv Prograde: +3120m/s│        │ Pulling Prograde (+v) adds  │     │ ⚠ CAUSAL ALERT:      │
│ Burn Arc: 412 sec    │        │ kinetic energy here, which  │     │ Burn arc is 34° wide │
│ TWR (Start): 0.42    │        │ flings your opposite side   │     │ → Expect ~48 m/s     │
│ Finite-Burn Loss:    │        │ (Apoapsis) out to the Moon. │     │ Gravity Loss!        │
│   -48.2 m/s (1.5%)   │        └─────────────────────────────┘     │ [Split into 2 Burns] │
│                      │                                            │                      │
│                      │                                            │ [LOCK & LAUNCH (Go)] │
├──────────────────────┴────────────────────────────────────────────┴──────────────────────┤
│ TIME SCRUBBER: [◄◄] [►] 2026-04-14 18:22:10 UTC ├──────●──────────────┤ +4d 12h (Lunar)  │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

### Key UX Mechanics on Screen 1:
1.  **Dual-Line Trajectory Overlay (Blue vs. Amber):**
    *   The **Dashed Cyan Line** shows the **Level-1 Analytical Patched-Conic** orbit (instantaneous impulse burn, 2-body gravity).
    *   The **Solid Gold Line** shows the **Level-3 Perturbed Numerical** trajectory (showing the actual finite-burn spiral arc, $J_2$ precession, and Lunar pull).
    *   Beginners *visually see* the two lines diverge when their engine has low TWR, immediately grasping why long burns lose efficiency without reading a single differential equation!
2.  **1-Click "Perigee Kick" Splitter:** When the Causal Alert flags high Finite-Burn Gravity Loss, hovering over `[Split into 2 Burns]` dynamically previews splitting one long 7-minute burn into two 3.5-minute burns right at perigee.

---

## Screen 2: The 2D Parametric Engine & Spacecraft CAD Lab (Chief Engineer View)
*Instead of snapping Lego bricks blindly like KSP, the player drafts real axisymmetric cross-sections with a live supersonic Schlieren flow solver.*

```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│ POV: [2: PROPULSION & VEHICLE CAD LAB] │ Lens: [• Schlieren Shockwaves] [○ Thermal Flux] │
├───────────────────────┬──────────────────────────────────────────┬───────────────────────┤
│ THERMODYNAMIC INPUTS  │   2D PARAMETRIC AXISYMMETRIC CAD CANVAS  │ REAL-TIME OUTPUTS     │
│                       │   (Interactive Control Handles ●)        │                       │
│ Cycle Architecture:   │                                          │ Vacuum Isp : 348.2 s  │
│ [Gas-Generator Biprop]│    Injector   Chamber  Throat   Nozzle   │ Sea-Lvl Isp: 289.4 s  │
│                       │    ┌─────────●═══════╗        .─● (Re)   │ Vac Thrust : 845.0 kN │
│ Propellant Pair:      │    │░░░░░░░░░░░░░░░░░╚═══●══.'░░░░░░░░   │ Mass Flow  : 247.4kg/s│
│ [LOX / RP-1 Kerosene] │ ───┼─────────────────────┼──────░░░░░░───│                       │
│                       │    │░░░░░░░░░░░░░░░░░╔═══●══'.░░░░░░░░   │ Exit Mach  : M 3.84   │
│ Mixture Ratio (O/F):  │    └─────────●═══════╝        '─●        │ Exit Press : 0.042 MPa│
│ ├────────●────┤ 2.58  │                              ▲           │ Ambient P  : 0.101 MPa│
│ (Stoich: 3.40 - Hot!) │         ⚠ FLOW SEPARATION AT SEA LEVEL!  │                       │
│                       │         Pe (0.042 MPa) < 0.4 Pamb        │ THROAT THERMAL MARGIN │
│ Chamber Pressure (Pc):│         Oblique shockwaves detached from │ Peak Flux: 68 MW/m²   │
│ ├─────────●───┤ 9.7MPa│         nozzle wall at x = 1.42m!        │ Wall Temp: 1,140 K    │
│                       │                                          │ Max Limit: 1,350 K OK │
│ Expansion Ratio (ε):  │   TEST STAND ALTITUDE SLIDER:            │                       │
│ ├──────────●──┤ 28.0:1│   Sea Level (0 km) ├──●─────────┤ Vacuum │ AI CHIEF DELEGATION   │
│                       │                                          │ [Auto-Coolant: ON]    │
│ Regenerative Cooling: │   [FIRE TEST STAND (0.0% Idle CPU)]      │ [Auto-Turbopump: ON]  │
│ [RP-1 Jacket Channel] │                                          │ (+25% Mass Margin)    │
├───────────────────────┴──────────────────────────────────────────┴───────────────────────┤
│ COUNTERFACTUAL MISSION SCRUBBER: Drag to test battery, boil-off & saturation over 30 days│
│ Launch ├───────────●──────────────────────⚠ [Day 14: LH2 Boil-Off Vents 12% Fuel] ─────┤ │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

### Key UX Mechanics on Screen 2:
1.  **Tactile Geometry Dragging:** The player literally grabs the **Throat Radius handle ($\bullet R_t$)** and **Nozzle Exit handle ($\bullet R_e$)** with their mouse. In $< 50\text{ microseconds}$, the Rust Wasm solver recomputes the Area-Mach equation and redraws the **Schlieren supersonic shock diamonds** inside the nozzle!
2.  **Why Over-Rich Mixture Ratios Make Sense Visually:** If a beginner drags the $O/F$ mixture ratio slider to stoichiometric ($3.40$ for LOX/RP-1), they watch Chamber Temperature $T_c$ spike to $3,700\text{ K}$ (turning the nozzle throat glowing white-hot past its melting limit), whereas dragging it to fuel-rich ($2.58$) lowers molecular weight $\bar{M}$, cools the wall, and *increases* $I_{\text{sp}}$! They discover real rocket chemistry in 10 seconds with their eyes.

---

## Screen 3: The Mission Control Telemetry & Fault-Tree Console (MOCR View)
*When an anomaly occurs in flight, you don't get a "Game Over" screen. You sit at the Flight Director's console to diagnose coupled physical state variables and upload fixes across light-time delay.*

```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│ POV: [3: MISSION CONTROL (MOCR)] │ Vessel: Odyssey-2 (Mars Cruise) │ One-Way Light: 6m12s│
├──────────────────────────┬───────────────────────────────────┬───────────────────────────┤
│ ELECTRICAL & POWER BUS   │ PAINTED VIEWPORT + SCHEMATIC HUD  │ PROPULSION & CRYOGENICS   │
│                          │                                   │                           │
│ Solar Array: 1,420 W     │      [Sun Vector ───►]            │ Main Tank (LCH4):         │
│ Incidence θ: 41.2°       │                                   │ Temp : 113.8 K (Rising!)  │
│                          │         ╔═══════════╗             │ Press: 285 kPa (MAX 300!) │
│ Bus A: 28.1 V | 14.2 A   │   ══════╣  spacecraft╠══════       │ Boil-off: 0.14 kg/hr      │
│ Bus B: 19.4 V [⚠ DROP!]  │         ╚═════╤═════╝             │ [Cryocooler: OFF - Bus B] │
│ Battery SoC: 64% (-80W)  │               ▲                   │ [Vent Valve: CLOSED]      │
│                          │      (Sun Heating Tank B!)        │                           │
├──────────────────────────┤                                   ├───────────────────────────┤
│ ATTITUDE & GNC           │ CAUSAL FAULT-TREE INSPECTOR:      │ AUTONOMOUS FLIGHT RULES   │
│                          │ [Tank Pressure Near Burst]        │ (Onboard FDIR Sequencer)  │
│ Mode: [3-Axis Hold]      │       ▲                           │                           │
│ Wheel X:  18%            │ [Cryocooler Offline (0 W)]        │ Rule #1:                  │
│ Wheel Y:  98% [⚠ SAT!]   │       ▲                           │ IF [Tank_P > 290 kPa]     │
│ Wheel Z:  42%            │ [Bus B Tripped due to Wheel Y     │ THEN [Execute Passivation │
│                          │  Desaturation Heater Spike]       │       Barbecue Roll 0.5°/s│
│ [Command RCS Dump (-2kg)]│                                   │       & Cross-Tie Bus A]  │
│ [Command Barbecue Roll]  │                                   │ [UPLOAD COMMAND (6m 12s)] │
└──────────────────────────┴───────────────────────────────────┴───────────────────────────┘
```

### Key UX Mechanics on Screen 3:
*   **Interactive Fault-Tree Tracing:** Clicking on any flashing amber parameter (like `Press: 285 kPa`) draws arrows connecting the physical chain of events across the console panels (Wheel Saturation $\rightarrow$ Heater Current Spike $\rightarrow$ Bus B Trip $\rightarrow$ Cryocooler Shutdown $\rightarrow$ Solar Heating Boiling Methane).
*   **Real Rescue Solutions:** The player can save the mission by commanding a slow **Passive Thermal Control ("Barbecue Roll" at $0.5\text{ RPM}$)** so the sun no longer bakes one side of the fuel tank, cross-tying Bus A, and saving the propellant without venting it!

---

## Screen 4: The Deep-Space B-Plane & Exoplanet "Error-Bar" Observatory
*Used in Block 2 (Interplanetary Navigation) and Block 4 (4,000+ LY Exoplanet Exploration).*

```
┌────────────────────────────────────────────┬─────────────────────────────────────────────┐
│ LEFT PANEL: JPL B-PLANE TARGETING BULLSEYE │ RIGHT PANEL: EXOPLANET "PLAY THE ERROR BARS"│
│ (Approaching Mars Aerocapture Corridor)    │ Target: TRAPPIST-1e (39.46 Light-Years)     │
│                                            │ Data Source: NASA Exoplanet Archive (JWST)  │
│               B · R (South)                │                                             │
│                    │                       │ Surface Gravity (g):  0.82g ± 0.14g (1σ)    │
│         . - ~ ~ - .│. - ~ ~ - .            │   0.5g      0.82g      1.2g                 │
│      . '   SKIP    │   ZONE    ' .         │   ──────╭─────●─────╮────── (Gaussian PDF)  │
│    /     . - ~ - . │ . - ~ - .     \       │         └───┬───────┘                       │
│   │    /  GREEN AEROCAPTURE   \     │      │   Your Lander Structural Rating:            │
│   │   │   . - - .  │  . - - .  │    │      │   [Rated up to 0.95g] ──► 18% CRASH RISK!   │
│ ──┼───┼──│──────┼──●──┼──────│─┼────┼──B·T │                                             │
│   │   │  │  MARS IMPACT ZONE │ │    │      │ Atmospheric Surface Pressure:               │
│   │    \  ' - - '  │  ' - - ' /     │      │   [Unknown: 0.001 bar to 2.5 bar]           │
│    \     ' - _ _ - │ - _ _ - '     /       │                                             │
│      ' .     (3σ Covariance)   . '         │ HOW TO SHRINK ERROR BARS BEFORE LAUNCH:     │
│          ' - . _ _ │ _ _ . - '             │ • [Schedule L2 Space Telescope (40 days)]   │
│                    │                       │   → Narrows Pressure to ±0.05 bar           │
│ [Take OpNav Photo of Phobos] → Shrinks 3σ! │ • [Deploy 550 AU Solar Gravity Lens Probe]  │
│ [Execute 1.4 m/s TCM Burn]   → Centers Dot!│   → Narrows Gravity to ±0.01g               │
└────────────────────────────────────────────┴─────────────────────────────────────────────┘
```

---

## Screen 5: The Rotating Habitat & Closed-Loop ECLSS Lab (Block 3)
*Designing a rotating space station is a live structural and human-factors trade study.*

```
┌────────────────────────────────────────────┬─────────────────────────────────────────────┐
│ HABITAT PARAMETRIC CAD & HOOP STRESS       │ THE GILRUTH ARTIFICIAL GRAVITY COMFORT PLOT │
│                                            │                                             │
│ Geometry: [O'Neill Counter-Rotating Pair]  │  1.5g ┼───────────────────────────────────  │
│ Radius (r)      : [ 220.0 ] m              │       │                   /      /          │
│ Rotation (ω)    : [ 1.85  ] RPM            │  1.0g ┼─ ─ ─ ─ ─ ─ ─ ─ ─ / ─ ● ─/─ ─ ─ ─ ─  │
│ Length (L)      : [ 800.0 ] m              │       │   GREEN COMFORT / (Your /           │
│ Hull Material   : [ Carbon Fiber T1000 ]   │  0.5g ┼─────── ZONE ───/── Hab)/──────────  │
│ Wall Thickness  : [ 24.0  ] mm             │       │               /       /  CORIOLIS   │
│ Shielding Regolith: [ 2.5 ] tonnes/m²      │  0.3g ┼──────────────/───────/── SICKNESS   │
│                                            │       │             /       /    (> 4 RPM)  │
│ STRUCTURAL & ROTATIONAL CHECKS:            │    0g ┼───────┬────┴───┬───┴───┬──────────  │
│ • Hoop Stress (σ): 482 MPa / 1,200 MPa OK  │      10m     50m     200m    500m   Radius  │
│ • Rim Speed (v)  : 42.6 m/s                │                                             │
│ • Head-Turn Coriolis: 0.14 rad/s² (Safe)   │ CLOSED-LOOP BIOCHEMICAL MASS BALANCE (100p) │
│ • Tennis-Racket Stability (Iz > Ix):       │ O2 Loop : +84.0 kg/d (Electrolysis) = 100%  │
│   [STABLE: Counter-Rotating Cancel = 0 Nm] │ CO2 Loop: -100.0 kg/d (Sabatier+Algae) = OK │
└────────────────────────────────────────────┴─────────────────────────────────────────────┘
```

---

## Screen 6: The Post-Flight Debrief & 1-Click PDF PDR Dossier
*After every mission, this screen delivers the visceral "Aha!" learning moment and updates the player's Astro-Elo.*

```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│ POST-MISSION DEBRIEF: Lunar South Pole Cargo Delivery │ RESULT: SUCCESS (Pareto Top 8%)  │
├───────────────────────────────────────────┬──────────────────────────────────────────────┤
│ PREDICTION VS. REALITY COMPARISON         │ SHAPLEY PHYSICAL ERROR ATTRIBUTION           │
│                                           │ (Why Reality Deviated from Your Lvl-1 Math)  │
│ Metric          Predicted    Actual   Err │                                              │
│ ───────────────────────────────────────── │ 1. Finite-Burn Gravity Loss : 68.4% (-31m/s) │
│ Transfer Δv   : 3,142 m/s  3,187 m/s +1.4%│    (TLI burn lasted 6.8 mins; thrusting away │
│ Perilune Alt  : 50.0 km    38.4 km  -11km │     from exact perigee wasted radial energy) │
│ Prop Remaining: 118.0 kg   94.2 kg  -23kg │ 2. Lunar 3-Body Perturbation: 22.1% (-8.2km) │
│ Boil-Off Loss : 2.0 kg     7.8 kg   +5.8kg│ 3. Earth J2 Nodal Precession:  7.2% (-2.7km) │
│                                           │ 4. Solar Radiation Pressure :  2.3% (-0.7km) │
├───────────────────────────────────────────┼──────────────────────────────────────────────┤
│ ASTRO-ELO & PARETO FRONTIER RATING        │ PROFESSIONAL ENGINEERING PORTFOLIO           │
│                                           │                                              │
│ Theoretical Ideal Mass (Tsiolkovsky):     │ Your mission telemetry, nozzle CAD, and      │
│   14,210 kg IMEO                          │ error analysis have been compiled.           │
│ Your Vehicle Mass: 15,080 kg (94.2% Opt!) │                                              │
│                                           │ [ 📄 EXPORT IEEE / NASA PDR DOSSIER (.PDF) ] │
│ ASTRO-ELO UPDATE: 1420 ──► 1458 (+38 Elo) │ [ 🌐 PUBLISH REPLAY PROOF TO PORTFOLIO ]     │
└───────────────────────────────────────────┴──────────────────────────────────────────────┘
```

---

# Part 3: The "First 30 Minutes" Zero-Google Onboarding UX Flow

To prove how these screens work together so a beginner **never has to Google a single term**, here is the exact 4-step journey of a brand-new player during their first 30 minutes:

1.  **Minute 0–5 (The First Launch & The TWR Lesson):**
    *   The player accepts Contract #1: *"Launch a 250 kg weather CubeSat into a 300 km Low Earth Orbit."*
    *   Chief Engineer AI is turned ON for Thermal, Power, and Avionics, leaving the player in charge of **Rocket Staging & Trajectory**.
    *   If the player puts too much fuel in the tank and clicks `[TEST IGNITION]`, the rocket sits on the pad roaring without rising.
    *   Immediately, the **Causal Graph** highlights the weight arrow ($m \cdot g$) overpowering the thrust arrow ($F_{\text{thrust}}$) and offers a 1-slider drag to drain fuel until $F_{\text{thrust}} > m \cdot g$ ($\text{TWR} = 1.35$). The player learns Thrust-to-Weight Ratio with their eyes in 15 seconds.
2.  **Minute 5–12 (The Nozzle Bell & Schlieren Lesson):**
    *   During ascent, the player notices their rocket is sluggish at sea level. Clicking `[Inspect Engine Nozzle]` opens **Screen 2 (The Schlieren Lens)**.
    *   They visually see the exhaust plume detaching from the wide vacuum nozzle walls at sea level, shaking the engine! They drag the nozzle exit handle inward until the shockwave snaps cleanly to the lip of the bell, jumping Sea-Level $I_{\text{sp}}$ by $45\text{ seconds}$.
3.  **Minute 12–20 (Why Going Straight Up Falls Back Down — The Orbit Lesson):**
    *   The player launches straight up to $300\text{ km}$ altitude.
    *   On **Screen 1**, the **Visual Physics Lens** draws their trajectory arch falling straight back into the ocean, with a visual velocity gauge showing $v_{\text{horizontal}} = 0.4\text{ km/s}$ vs. required orbital speed $v_{\text{orbit}} = \sqrt{\mu/r} = 7.73\text{ km/s}$.
    *   They grab the **Pitch-Over / Gravity Turn curve handle** and watch the trajectory arch stretch sideways around the curvature of the painted Earth until the ends meet in a closed circle!
4.  **Minute 20–30 (Their First Prediction Contract & Astro-Elo Gain):**
    *   They lock in their first **Prediction Baseline**, watch the Level-3 flight complete in 10 seconds of smooth spline time-warp, see their first **Prediction vs. Reality Debrief** on **Screen 6**, earn $+45\text{ Astro-Elo}$, and export their first 1-page PDF Engineering Dossier.

---
