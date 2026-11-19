The Global "Anti-Hallucination Laws"

Before starting any Block, your AI agent must obey these invariant laws across the entire project:

1. The Dimensional Unit Law: Every variable and data contract must strictly use SI Base Units internally: meters (mm), kilograms (kgkg), seconds (ss), Kelvin (KK), Pascals (PaPa), Watts (WW), radians (radrad), and Joules (JJ). Display conversions (e.g., kmkm, daysdays, light-yearslight-years, RPMRPM) are permitted only in the UI formatting layer.
  
2. The Zero-Polling Idle Law: requestAnimationFrame must never run continuously by default. Rendering and physics computation are strictly Event-Driven and Dirty-Flagged. When the camera is stationary and time-warp is paused, CPU usage must drop to 0.0% and frame redraws must drop to 0 FPS.
  
3. The No-Euler Integration Law: First-order Euler integration (xn+1=xn+v⋅Δtxn+1​=xn​+v⋅Δt) and fixed-step RK4 are strictly banned for orbital mechanics because they artificially gain or lose orbital energy. All numerical trajectories must use either Symplectic Integrators (Velocity Verlet / Yoshida) or Adaptive Dormand-Prince (RK45 / RK89) cached into Hermite Cubic Splines.

4. The No-Magic-Lookup Law: No engine, habitat, or subsystem may return hardcoded performance scores (e.g., if (engine == "ion") return 3000;). Every output metric must be algebraically derived from fundamental physical inputs (temperature, molar mass, pressure, area, efficiency).
   
5. The NASA Oracle Gate: No Work Package is considered complete until it passes its numerical Ground-Truth Oracle Test against real NASA/Vallado textbook reference values.

6. The No-Invented-Truth Law: A claim in a design document is not evidence. No document may state a number, tolerance, or physical outcome as established unless a test, a measurement, or a cited source backs it, and any claim that later measurement contradicts must be corrected in place rather than quietly left standing. Prose that describes retired behaviour is a defect, not a legacy note.

7. The Card-Is-A-Hypothesis Law: Spec cards are hypotheses. Any physical number, tolerance, or design rationale in a work-package card must be checked against first principles or measurement before implementation. When a card conflicts with physics or with the measured behaviour of the code, the card loses. Report the conflict; do not implement around it.






Part 2: Decomposing Block 1 into 12 Bite-Sized AI Work Packages (Blocks 0.1 to 1.5)

Instead of giving an AI agent "Block 1" all at once, you feed it these 12 Sequential Work Packages one by one.
PHASE 0: The Scalable Universe & Zero-CPU Visual Foundation
Work Package 0.1: The 8,000+ Light-Year Hierarchical Coordinate System

    Goal: Build the spatial coordinate kernel so you never have to rewrite it when scaling from cislunar space to 4,000 LY, 8,000 LY, or the entire Milky Way.
    Exact Mathematical Spec:
        Implement a 4-tier coordinate transform tree:
            Galactic Barycentric Frame (i64 Integer Milliparsecs [x,y,z][x,y,z]): 64-bit signed integers representing milliparsecs from the Solar System Barycenter (ICRS / J2000). This supports ±9.2×1015 parsecs±9.2×1015 parsecs (billions of light-years) with zero floating-point drift.
            Stellar System Barycentric Frame (f64 meters [x,y,z][x,y,z]): Relative to the current star system's barycenter.
            Local Celestial Body SOI Frame (f64 meters [x,y,z,vx,vy,vz][x,y,z,vx​,vy​,vz​]): Relative to the dominant gravitating body (Earth, Moon, etc.).
            Camera Render Frame (f32 meters [x,y,z][x,y,z]): Always centered at (0,0,0)(0,0,0) at the camera's focal point so WebGL shaders never experience vertex jitter ("the Kraken").
        Implement time representation as a split 64-bit Integer Seconds + 64-bit Float Sub-Seconds since the J2000.0 Epoch (JD 2451545.0 TDBJD 2451545.0 TDB).
    Forbidden AI Shortcuts: Never store universal positions in a single f64 or f32 vector. Never use JavaScript Date.now() for simulation time.
    NASA Ground-Truth Oracle Test:
        Place two docking ports 0.5 meters0.5 meters apart inside a star system located 8,000 light-years8,000 light-years (2,452,809 parsecs2,452,809 parsecs) from Earth.
        Transform both coordinates from the Galactic Frame down to the Camera Frame and back.
        Pass Criterion: Round-trip relative distance error between the two docking ports must be <10−9 meters<10−9 meters (1 nanometer1 nanometer).

Work Package 0.2: Analytical Ephemeris Kernel (O(1)O(1) Celestial Body Positions)

    Goal: Compute the exact real-world position, velocity, axial tilt, and rotation of the Sun, Earth, and Moon at any date without running a continuous N-body loop.
    Exact Mathematical Spec:
        Load NASA JPL Horizons J2000 Keplerian Elements + Secular Rates (or Chebyshev polynomial coefficients) for the Sun, Earth-Moon Barycenter, Earth, and Moon.
        Given any epoch tt, solve Kepler’s Equation M=E−esin⁡EM=E−esinE using Newton-Raphson iteration to machine precision (∣ΔE∣<10−13 rad∣ΔE∣<10−13 rad) in ≤5≤5 iterations.
        Compute Earth’s true sidereal rotation angle (GMST) and axial obliquity (ϵ=23.43928∘ϵ=23.43928∘).
    Forbidden AI Shortcuts: Do not place Earth and Moon in circular, co-planar orbits! The Moon's real inclination (5.145∘5.145∘ to the ecliptic) and eccentricity (0.05490.0549) must be exact. Do not use a per-frame update() loop to move planets.
    Zero-CPU Rule: Computing a planet's state vector r⃗(t),v⃗(t)r
    (t),v
    (t) must be a pure stateless function get_body_state(body_id, epoch_tdb) taking <1 microsecond<1 microsecond.
    NASA Ground-Truth Oracle Test:
        Query Earth and Moon positions at exactly 2026-01-01 00:00:00 TDB.
        Pass Criterion: Earth-Moon distance and relative velocity vector must match JPL Horizons reference ephemeris within 0.1%0.1%.

Work Package 0.3: The "Painted Cosmos" Dirty-Flag Renderer & Procedural Brushstroke Pipeline

    Goal: Achieve the Painted Valley hand-painted procedural art style in the browser with 0.0% idle CPU/iGPU usage and zero 3D .obj/.fbx model files.
    Exact Architectural Spec:
        Create a Two-Layer Viewport Architecture:
            Layer A (The Cached Painterly Canvas - WebGL2): Renders celestial bodies, atmospheric scattering halos, engine plumes, and procedural spacecraft geometry using mathematical Signed Distance Fields (SDFs) / parametric primitives passed through an Anisotropic Kuwahara & Normal-Aligned Procedural Brushstroke Fragment Shader.
            Layer B (The Live Telemetry Vector Overlay - HTML5 Canvas2D / SVG): Renders crisp orbital ellipses, velocity vectors, apsis markers, and B-plane bullseyes over Layer A.
        The Dirty-Flag Governor: Layer A renders only when camera_moved == true or scene_lighting_changed == true. While the camera hits mouse-drag, Layer A renders at 0.5×0.5× resolution; exactly 50 ms50 ms after mouse release, Layer A renders a single high-density "Masterpiece Brushstroke Pass" into an offscreen texture and freezes.
    Forbidden AI Shortcuts: No requestAnimationFrame loop that runs when is_dirty == false. No external image textures (.png/.jpg).
    Zero-CPU Oracle Test:
        Open the Earth-Moon viewport, rotate the camera, and release the mouse for 5 seconds.
        Pass Criterion: Browser Performance Profiler must record 0 frames rendered per second and <0.2%<0.2% CPU utilization while idle, and instantly wake up on mouse movement.

PHASE 1: The Three-Level Astrodynamics Engine & Visual Physics Lens
Work Package 1.1: Level-1 Analytical Patched Conics & Orbital Elements Engine

    Goal: Instantaneous (<0.1 ms<0.1 ms) conversion between Cartesian State Vectors (r⃗,v⃗)(r
    ,v
    ) and Classical Orbital Elements (a,e,i,Ω,ω,ν)(a,e,i,Ω,ω,ν), plus analytic Vis-Viva, Hohmann, and Bi-Elliptic calculators.
    Exact Mathematical Spec:
        Implement full Cartesian ↔↔ Keplerian conversions handling all edge cases without NaN errors:
            Circular Equatorial (e<10−8,i<10−8e<10−8,i<10−8): Use true longitude λtrueλtrue​.
            Circular Inclined (e<10−8,i≥10−8e<10−8,i≥10−8): Use argument of latitude uu.
            Elliptical Equatorial (0<e<1,i<10−80<e<1,i<10−8): Use longitude of periapsis ω~ω~.
            Hyperbolic (e>1e>1): Use hyperbolic eccentric anomaly FF where Mh=esinh⁡F−FMh​=esinhF−F.
        Compute Sphere of Influence (SOI) radius: rSOI=a(mM)2/5rSOI​=a(Mm​)2/5.
    Forbidden AI Shortcuts: Never divide by zero when eccentricity e=0e=0 or inclination i=0i=0.
    NASA Ground-Truth Oracle Test (Vallado Fundamentals of Astrodynamics Example 2-5):
        Input State Vector around Earth (μ=3.986004418×1014 m3/s2μ=3.986004418×1014 m3/s2):
        r⃗=[6524.834,6862.875,6448.296] kmr
        =[6524.834,6862.875,6448.296] km, v⃗=[4.901327,5.533756,−1.976341] km/sv
        =[4.901327,5.533756,−1.976341] km/s.
        Pass Criterion: Output must equal a=36,127.343 kma=36,127.343 km, e=0.832853e=0.832853, i=87.870∘i=87.870∘, Ω=227.898∘Ω=227.898∘, ω=53.38∘ω=53.38∘, ν=92.335∘ν=92.335∘ within 0.001%0.001%.

Work Package 1.2: Level-2 & Level-3 Numerical Integrator + Hermite Spline Trajectory Cache

    Goal: Compute the true physical trajectory of a spacecraft under perturbations, and cache it as a lightweight mathematical curve so time-warping uses zero CPU.
    Exact Mathematical Spec:
        Implement an Adaptive Dormand-Prince RK45(5) / RK89 Numerical Integrator solving the cowell/Encke equation of motion:
        a⃗total=−μcentralr3r⃗+a⃗J2+a⃗3rd-body+a⃗drag+a⃗SRP+F⃗thrust(t)m(t)a
        total​=−r3μcentral​​r
        +a
        J2​​+a
        3rd-body​+a
        drag​+a
        SRP​+m(t)F
        thrust​(t)​
        Perturbation Terms (Level 3):
            J2J2​ Oblateness Acceleration:
            a⃗J2=−32J2μReq2r4[xr(1−5z2r2)i^+yr(1−5z2r2)j^+zr(3−5z2r2)k^]a
            J2​​=−23​J2​r4μReq2​​[rx​(1−5r2z2​)i^+ry​(1−5r2z2​)j^​+rz​(3−5r2z2​)k^]
            (For Earth: J2=1.08262668×10−3J2​=1.08262668×10−3, Req=6,378,137 mReq​=6,378,137 m).
            Third-Body Gravity (Sun & Moon): Battin’s f(q)f(q) formulation to prevent floating-point cancellation when subtracting two nearly equal vectors.
            Atmospheric Drag: a⃗drag=−12ρ(h)vrel2CdAmv^rela
            drag​=−21​ρ(h)vrel2​mCd​A​v^rel​ using the US Standard Atmosphere 1976 multi-layer barometric/scale-height model.
            Solar Radiation Pressure (SRP): a⃗SRP=−P⊙(1 AUr⊙)2CRA⊙m⋅νshadowu^⊙a
            SRP​=−P⊙​(r⊙​1 AU​)2mCR​A⊙​​⋅νshadow​u^⊙​, including cylindrical/conical Earth shadow eclipse factor νshadow∈[0,1]νshadow​∈[0,1].
            Finite-Burn Mass Depletion: dmdt=−∣F⃗thrust∣Ispg0dtdm​=−Isp​g0​∣F
            thrust​∣​ integrated simultaneously alongside position and velocity!
        The Hermite Spline Cache (Zero-CPU Secret):
        When the player places a maneuver or finishes a burn, a background Web Worker integrates the trajectory forward for NN days, storing sparse keyframes (tk,r⃗k,v⃗k)(tk​,r
        k​,v
        k​) at each adaptive step. During real-time playback or 100,000×100,000× time warp, the main thread simply evaluates the Cubic Hermite Polynomial between tktk​ and tk+1tk+1​ in O(1)O(1) time!
    Forbidden AI Shortcuts: Never run numerical integration inside the render frame loop. Never treat a 10-minute rocket burn as an instantaneous ΔvΔv impulse in Level 3.
    NASA Ground-Truth Oracle Test (Sun-Synchronous Nodal Precession):
        Place a satellite in a circular Earth orbit at altitude h=800 kmh=800 km (a=7,178.137 kma=7,178.137 km), inclination i=98.60∘i=98.60∘.
        Propagate for 10 days using Level-3 integration.
        Pass Criterion: Right Ascension of the Ascending Node (ΩΩ) must precess eastward at +0.9856∘/day+0.9856∘/day (matching Earth's mean motion around the Sun) within 0.5%0.5%, and total specific mechanical energy ϵϵ in vacuum without drag/SRP must drift by <10−9 J/kg<10−9 J/kg.

Work Package 1.3: The "Visual Physics Lens" & First-Principles Causal Graph (Zero-Google UI)

    Goal: Allow a complete beginner to understand orbital mechanics and failure modes through interactive visual feedback without reading a textbook or googling a single term.
    Exact Functional Spec:
        Interactive Maneuver Widget: Placing a node on the trajectory spawns 3 orthogonal handles aligned to the Frenet-Serret / TNW Frame:
            Tangential (Prograde/Retrograde): Hovering displays live visual vectors showing kinetic energy gain/loss and animates the opposite side of the orbit breathing outward/inward.
            Normal (North/South of Orbital Plane): Hovering shows the gyroscopic torque vector τ⃗=r⃗×F⃗τ
            =r
            ×F
            tilting the orbital plane around the line of nodes.
            Radial (In/Out): Hovering shows the eccentricity vector rotating in place.
        The Causal Graph Engine: Every warning state (e.g., TWR < 1.0, Periapsis < Atmosphere, Finite Burn Gravity Loss > 15%) is mapped to a deterministic Directed Acyclic Graph (DAG) of plain-English physical causes and 1-click interactive micro-demonstrations.
    Forbidden AI Shortcuts: Never show an error message that contains an unexplained acronym (like "RAAN", "AOP", "TWR", "ISP") without its dynamic visual hover-projection.

Work Package 1.4: The "Prediction vs. Reality" Engineering Contract & Debrief Engine

    Goal: Turn the mathematical difference between Level-1 (Analytical Approximation) and Level-3 (Perturbed Reality) into the core learning loop.
    Exact Functional Spec:
        Pre-Flight Lock-In: Before launching or executing a transfer, the player uses the Level-1 planner (Vis-Viva, Rocket Equation Δv=Ispg0ln⁡(m0/mf)Δv=Isp​g0​ln(m0​/mf​), Hohmann equations) to commit a Prediction Baseline:
            Predicted Final Apoapsis / Periapsis (kmkm)
            Predicted Remaining Propellant (kgkg)
            Predicted Time of Flight (ss)
            Predicted Closest Approach to Target (kmkm)
        Differential Attribution Solver (How the Game Explains Why You Missed):
        After the Level-3 flight completes, the Rust kernel runs 4 rapid shadow-propagations (<5 ms<5 ms total in Web Worker) toggling off one perturbation at a time (Finite-Burn Arc, J2J2​, 3rd-Body Moon/Sun, Drag/SRP) to compute the exact Shapley Attribution Percentage of the error!
            Example Output: "Target Missed by 184.2 km. Error Attribution: 74.1% Finite-Burn Gravity Loss (Burn lasted 420s across periapsis arc), 21.3% Lunar 3rd-Body Perturbation, 4.6% Earth J2 Precession."
    NASA Ground-Truth Oracle Test:
        Run a low-TWR (a0=0.8 m/s2a0​=0.8 m/s2) Trans-Lunar Injection burn from a 200 km200 km LEO parking orbit using a single continuous burn vs. an impulsive Level-1 Hohmann calculation.
        Pass Criterion: The Differential Attribution Solver must accurately isolate and report the finite-burn gravity loss penalty (Δvloss≈μ24r3Δv⋅tburn2Δvloss​≈24r3μ​Δv⋅tburn2​) within 1%1% of the analytical finite-burn integral.

PHASE 2: The 2D Parametric Engine Lab & Coupled Spacecraft Engineering
Work Package 1.5: The 2D Parametric Engine CAD Lab & Quasi-1D Supersonic Nozzle Solver

    Goal: Let players invent chemical, nuclear-thermal, or electrothermal/electric engines by adjusting real physical dimensions and thermodynamic parameters, with live supersonic shockwave visualization.
    Exact Mathematical Spec:
        Propellant Thermochemistry Table: Store real thermodynamic properties (molecular weight MˉMˉ, specific heat ratio γ(T)γ(T), adiabatic flame temperature TcTc​ as a function of Oxidizer-to-Fuel mixture ratio O/FO/F, liquid densities ρox,ρfuelρox​,ρfuel​, and boiling points) for standard propellants:
            LOX / RP-1LOX / RP-1 (Kerosene)
            LOX / LH2LOX / LH2​ (Hydrolox)
            LOX / LCH4LOX / LCH4​ (Methalox)
            NTO / MMHNTO / MMH (Hypergolic)
            Liquid H2Liquid H2​ (Nuclear Thermal / Solar Thermal working fluid)
            Xenon / Krypton / ArgonXenon / Krypton / Argon (Electric Propulsion working fluids)
        Quasi-1D Isentropic Nozzle Solver:
        Given Chamber Pressure PcPc​, Chamber Temperature TcTc​, Throat Radius RtRt​ (At=πRt2At​=πRt2​), and Exit Radius ReRe​ (Ae=πRe2Ae​=πRe2​, expansion ratio ϵ=Ae/Atϵ=Ae​/At​):
            Solve the Area-Mach Number Relation for the supersonic exit Mach number Me>1Me​>1:
            (AeAt)2=1Me2[2γ+1(1+γ−12Me2)]γ+1γ−1(At​Ae​​)2=Me2​1​[γ+12​(1+2γ−1​Me2​)]γ−1γ+1​
            Compute Exit Pressure Pe=Pc(1+γ−12Me2)−γγ−1Pe​=Pc​(1+2γ−1​Me2​)−γ−1γ​ and Exit Velocity ve=MeγRuMˉTeve​=Me​γMˉRu​​Te​
            ​.
            Compute Mass Flow Rate m˙m˙ (Choked Flow at Throat):
            m˙=PcAtTcγMˉRu(2γ+1)γ+1γ−1m˙=Tc​
            ​Pc​At​​Ru​γMˉ​(γ+12​)γ−1γ+1​
            ​
            Flow Separation & Summerfield Criterion (The Sea-Level Nozzle Killer):
            At ambient atmospheric pressure Pamb(h)Pamb​(h), if Pe<0.35PambPe​<0.35Pamb​, flow detaches inside the nozzle bell at the axial station where P(x)≈0.35PambP(x)≈0.35Pamb​, triggering oblique shockwaves, a 30%30% thrust efficiency penalty, and severe lateral structural vibration (side-loads)!
            Compute True Altitude-Dependent Thrust & IspIsp​:
            F(h)=m˙ve+(Pe−Pamb(h))Ae,Isp(h)=F(h)m˙g0F(h)=m˙ve​+(Pe​−Pamb​(h))Ae​,Isp​(h)=m˙g0​F(h)​
            Nozzle Wall Thermal Flux (Bartz Correlation) & Engine Mass Model:
            Calculate peak heat flux at the nozzle throat qthroat∝Pc0.8/Dt0.2qthroat​∝Pc0.8​/Dt0.2​ and structural wall mass using thin-shell pressure vessel equations + turbopump power balance.
        Live Schlieren Optical Lens (2D Procedural Shader):
        Draw the 2D axisymmetric cross-section of the player's chamber, throat, and bell contour. Render expansion waves, Mach diamonds (when Pe≠PambPe​=Pamb​), and flow-separation shock detachment inside the bell in real time as the player drags the altitude slider from 0 km0 km (Sea Level) to 100 km100 km (Vacuum).
    Forbidden AI Shortcuts: Never assign a fixed IspIsp​ to a fuel type! Changing O/FO/F mixture ratio, chamber pressure PcPc​, or nozzle expansion ratio Ae/AtAe​/At​ must mathematically alter veve​, m˙m˙, throat heat flux, and engine dry mass.
    NASA Ground-Truth Oracle Test (SpaceX Merlin 1D & RS-25 Benchmarks):
        Input RS-25 (Space Shuttle Main Engine) parameters: LOX/LH2LOX/LH2​, O/F=6.0O/F=6.0, Pc=20.64 MPaPc​=20.64 MPa, ϵ=Ae/At=69.0ϵ=Ae​/At​=69.0.
        Pass Criterion: Solver must output Vacuum Isp=452 s±8 sIsp​=452 s±8 s and Sea-Level Isp=366 s±10 sIsp​=366 s±10 s purely from the thermodynamic equations.

Work Package 1.6: Coupled Spacecraft Subsystems & The Counterfactual Time-Scrubber

    Goal: Make a spacecraft an interconnected system of Mass, Power, Thermal, Attitude, and Propellant Storage, with a timeline slider that previews failures before launch.
    Exact Mathematical Spec:
        Every spacecraft state vector in the simulator carries coupled subsystem variables integrated along the trajectory spline:
            Electrical Power Balance:
            Pgen(t)=Asolar⋅S⊙(1 AUr⊙)2ηcellcos⁡θsun⋅νshadow(t)Pgen​(t)=Asolar​⋅S⊙​(r⊙​1 AU​)2ηcell​cosθsun​⋅νshadow​(t)
            dEbatterydt=Pgen(t)−(Pavionics+Pcomms+Pcryocooler+Pelectric_thruster)dtdEbattery​​=Pgen​(t)−(Pavionics​+Pcomms​+Pcryocooler​+Pelectric_thruster​)
            Thermal Node Balance & Cryogenic Boil-Off:
            CthermaldTdt=Qsolar_absorbed+Qearth_IR+Qinternal_waste−ϵradσSBAradT4Cthermal​dtdT​=Qsolar_absorbed​+Qearth_IR​+Qinternal_waste​−ϵrad​σSB​Arad​T4
            For cryogenic propellant tanks (LH2 at 20.2 KLH2​ at 20.2 K, LOX at 90.2 KLOX at 90.2 K, LCH4 at 111.6 KLCH4​ at 111.6 K):
            Any net heat leak QleakQleak​ through Multi-Layer Insulation (MLI) not removed by an active cryocooler boils liquid propellant at rate:
            m˙boiloff=max⁡(0,Qleak−Pcooling)hvapm˙boiloff​=hvap​max(0,Qleak​−Pcooling​)​
            Attitude Control & Reaction Wheel Saturation:
            Solar radiation pressure offset from Center of Mass (r⃗cp−r⃗cmr
            cp​−r
            cm​) and Gravity-Gradient torque (τ⃗gg=3μr5r⃗×(I⋅r⃗)τ
            gg​=r53μ​r
            ×(I⋅r
            )) accumulate angular momentum in the reaction wheels: H⃗rw(t)=∫τ⃗extdtH
            rw​(t)=∫τ
            ext​dt. When ∣H⃗rw∣≥Hmax∣H
            rw​∣≥Hmax​, the wheels saturate and RCS thrusters must fire to dump momentum.
        The Counterfactual Time-Scrubber UI:
        In the Spacecraft Assembly & Mission Planning screen, dragging the bottom timeline slider from T=0T=0 to T=Day 30T=Day 30 evaluates these coupled differential equations across the planned trajectory spline in <15 ms<15 ms, highlighting the exact timestamp where a battery hits 0%0% during a lunar eclipse or where LH2LH2​ boil-off vents 40%40% of the return fuel!
    Forbidden AI Shortcuts: No decorative solar panels or radiators. Every watt of electrical consumption turns into a watt of thermal heat that must be radiated via the Stefan-Boltzmann law (σ=5.670374419×10−8 W⋅m−2⋅K−4σ=5.670374419×10−8 W⋅m−2⋅K−4).

PHASE 3: The Space Company Loop, Chief Engineer AI, Astro-Elo & PDR Dossier
Work Package 1.7: The "Chief Engineer AI" Delegation Layer (Onboarding Bridge)

    Goal: Let a brand-new player enjoy launching rockets immediately by delegating complex subsystems to conservative AI Department Heads, while rewarding players who take manual control.
    Exact Mathematical Spec:
        Implement deterministic Textbook Rule-of-Thumb Sizing Algorithms for 4 AI Department Heads (Propulsion, Thermal/Cryo, Electrical Power, Attitude/GNC).
        When a subsystem is toggled to [AUTO-DELEGATED], the AI Department Head automatically sizes the solar panels, batteries, radiators, MLI layers, and reaction wheels for the player's mission profile, but applies a conservative 35%35% mass margin and 30%30% cost markup.
        The UI explicitly displays a breakdown badge: "AI Thermal Chief added 420 kg420 kg of conservative radiator margin. Switch to Manual Thermal Design to optimize and save $1.8M$1.8M."

Work Package 1.8: Cislunar Company Economics, Astro-Elo Pareto Engine & PDF PDR Generator

    Goal: Close the Block 1 gameplay loop with commercial contracts, real launch site physics, an objective physics-based Elo rating, and exportable engineering reports.
    Exact Mathematical Spec:
        Launch Site Physics: Include real coordinates and azimuth limits for Cape Canaveral (28.5∘ N28.5∘ N), Vandenberg (34.7∘ N34.7∘ N), Kourou (5.2∘ N5.2∘ N), and Mahia (39.2∘ S39.2∘ S), calculating exact rotational velocity boost vrot=ω⊕R⊕cos⁡ϕsin⁡βazvrot​=ω⊕​R⊕​cosϕsinβaz​.
        The Astro-Elo Pareto Formula:
        For any contract (delivering payload mass mpaympay​ to target orbital elements within time ΔtmaxΔtmax​), compute the theoretical ideal Tsiolkovsky minimum Initial Mass in LEO (IMEO) and minimum energy ΔvidealΔvideal​.
        Compute the player's performance score Smission∈[0,1]Smission​∈[0,1] from three weighted components:
            Prediction Accuracy (40%40%): Spred=exp⁡(−k1∣Δvpredicted−Δvactual∣Δvactual−k2∣r⃗pred−r⃗actual∣rtarget)Spred​=exp(−k1​Δvactual​∣Δvpredicted​−Δvactual​∣​−k2​rtarget​∣r
            pred​−r
            actual​∣​)
            Mass & Thermodynamic Optimality (35%35%): Ratio of theoretical Pareto-optimal vehicle dry/wet mass to the player’s actual mass.
            Cost & Fault-Recovery Efficiency (25%25%): Cost per kg delivered + bonus if flown in Certified Engineer Mode (Zero Reverts).
            Update the player's Astro-Elo against the contract's calibrated Challenge Rating (Rcontract∈[800,2400]Rcontract​∈[800,2400]) using the standard Elo update equation.
        1-Click PDR Dossier Exporter:
        Format the spacecraft's 2D CAD cross-section, stage mass table, IspIsp​ curve, trajectory plot, and Prediction-vs-Reality error attribution table into a clean, print-styled HTML/SVG template that exports directly to an IEEE / NASA Technical Memorandum PDF in the browser.

Part 3: Detailed Specifications for Blocks 2, 3, and 4 (So Your Architecture Never Hits a Dead End)

Once the 12 Work Packages of Block 1 are running in your browser, your AI coding agent can seamlessly execute Blocks 2, 3, and 4 using the exact same modular architecture:
BLOCK 2: The Solar System, Deep Space Navigation, Fault-Tree Telemetry & ISRU Economics

    Work Package 2.1: Full Solar System Ephemeris & Izzo's Lambert Solver (Porkchop Plots)
        Math Spec: Implement Izzo’s Fast Lambert Solver (2015) in Rust Wasm. Because Izzo’s algorithm solves a single Lambert boundary-value problem in ∼0.4 microseconds∼0.4 microseconds, a Web Worker can compute a 200×200200×200 grid (40,00040,000 trajectories) to draw a full high-resolution Interplanetary Porkchop Plot (C3C3​ launch energy and arrival v∞v∞​ contours across Launch Date vs. Arrival Date) in under 20 milliseconds with zero UI lag!
        Oracle Test: Reproduce the real NASA Mars 2020 (Perseverance) launch window (July 2020 launch, Feb 2021 arrival) with C3≈14.5 km2/s2C3​≈14.5 km2/s2.
    Work Package 2.2: Orbit Determination (OD), Unscented Kalman Filter & B-Plane Covariance Bullseye
        Math Spec: Propagate a 6×66×6 State Uncertainty Covariance Matrix P(t)P(t) alongside the spacecraft's nominal state vector using the State Transition Matrix Φ(t,t0)Φ(t,t0​). Project the 3D position uncertainty ellipsoid onto the target planet's B-Plane (B⋅R^,B⋅T^B⋅R^,B⋅T^ coordinates). Implement Kalman measurement updates triggered when the player schedules DSN Radiometric Range/Doppler passes or Onboard Optical Navigation (OpNav) star-planet angle sightings.
    Work Package 2.3: Light-Time Delay, Fault-Tree Telemetry Console & Flight-Rule State Machines
        Math Spec: Split the simulation state display into Spacecraft Proper Event Time (tSCtSC​) and Earth Received Telemetry Time (tERT=tSC+∣r⃗SC−r⃗⊕∣ctERT​=tSC​+c∣r
        SC​−r
        ⊕​∣​). Build the visual FDIR (Fault Detection, Isolation, and Recovery) State Machine interpreter so spacecraft execute autonomous orbital insertion, aerocapture, and safe-mode bus switching across light-minutes of communication delay.
    Work Package 2.4: DSN Link Budget Engine (Friis & Shannon-Hartley Equations)
        Math Spec: Calculate real carrier-to-noise density ratio Eb/N0Eb​/N0​ and maximum data downlink rate (bits/sec) from transmitter power PtPt​, antenna diameters Dt,DrDt​,Dr​, carrier frequency ff (S,X,Ka-bandS,X,Ka-band, or 1550 nm1550 nm Optical), pointing loss, atmospheric attenuation, and space loss (λ4πd)2(4πdλ​)2.
    Work Package 2.5: ISRU Supply Chain Economics & Live NASA Sentry/Horizons API Sync
        Math Spec: Model regolith/ice extraction energy costs (Watt-hours per kg of H2OH2​O mined and electrolyzed into LH2/LOXLH2​/LOX), cryo-depot orbital beta-angle (ββ) boil-off minimization, and Net Present Value (NPV) capital amortization. Connect a browser fetch worker to NASA JPL’s public SBDB / Sentry API to generate live Near-Earth Asteroid deflection contracts.

BLOCK 3: Rotating Habitats, Closed-Loop ECLSS & Space Traffic Management

    Work Package 3.1: Rotating Habitat Structural, Dynamical & Human-Factors Solver
        Math Spec: Calculate centripetal acceleration ac=ω2rac​=ω2r, resident Coriolis acceleration a⃗cor=−2(ω⃗×v⃗rel)a
        cor​=−2(ω
        ×v
        rel​), cross-coupled inner-ear angular acceleration during head turns (plotting directly onto the Gilruth Comfort Diagram), cylinder hoop stress σhoop=Patmrtwall+ρwallω2r2σhoop​=twall​Patm​r​+ρwall​ω2r2 against real material yield strengths (Al-7075, Ti-6Al-4V, Carbon Fiber T1000), and principal moments of inertia (Ix,Iy,Iz)(Ix​,Iy​,Iz​) to enforce the Intermediate Axis / Tennis Racket Stability Theorem (IspinIspin​ must be the maximum moment of inertia or use counter-rotating pairs).
    Work Package 3.2: Closed-Loop Life Support (ECLSS) Stoichiometric Mass-Balance Solver
        Math Spec: Enforce strict atomic conservation of C,H,O,NC,H,O,N across crew metabolic consumption (0.84 kg O2/day0.84 kg O2​/day intake, 1.00 kg CO2/day1.00 kg CO2​/day output per astronaut) and chemical/biological processors:
            Sabatier Reactor: CO2+4H2→CH4+2H2OCO2​+4H2​→CH4​+2H2​O (ΔH=−165 kJ/molΔH=−165 kJ/mol)
            Water Electrolysis: 2H2O→2H2+O22H2​O→2H2​+O2​
            Bosch / Pyrolysis Methane Cracking & Hydroponic Photosynthetic Loops.
    Work Package 3.3: Solar Weather (F10.7F10.7​), Thermospheric Breathing & Kessler Debris Catalog
        Math Spec: Modulate upper-atmospheric density ρ(h,F10.7,Ap)ρ(h,F10.7​,Ap​) across the 11-year solar cycle using the Jacchia-Bowman / NRLMSISE-00 empirical drag curve, and propagate discarded upper stages via analytical SGP4 equations to generate real Conjunction Data Messages (CDMs) and Active Debris Removal contracts.

BLOCK 4: The 4,000-to-8,000+ Light-Year Interstellar & Open-Ended Invention Era

    Work Package 4.1: Spatial Octree Streaming for Gaia DR3 & NASA Exoplanet Archive
        Math Spec: Build an offline data-packing script that converts the Gaia DR3 catalog and NASA Exoplanet Archive into static Morton-Coded (Z-Order) Spatial Octree Binary Chunks (.oct), preserving every star's 3D position, proper motion vector (μα∗,μδ,vrad)(μα∗​,μδ​,vrad​), luminosity, spectral type, and every exoplanet's real measurement uncertainty (μ±σ)(μ±σ).
    Work Package 4.2: "Playing the Error Bars" (Stochastic Bounded Realism & Interferometry)
        Math Spec: Store unknown exoplanet parameters as Truncated Gaussian / Bayesian Prior Distributions seeded deterministically by the planet's real catalog ID. Calculate telescope angular resolution θ=1.22λDbaselineθ=1.22Dbaseline​λ​ and spectroscopic Signal-to-Noise Ratio (SNR) so launching space telescopes (at Sun-Earth L2L2​ or the 550 AU550 AU Solar Gravitational Lens) mathematically narrows the ±σ±σ error bars before the player commits an interstellar fleet.
    Work Package 4.3: Open-Ended Thermodynamic & Electrodynamic Propulsion Node Graph
        Math Spec: Implement a generalized Directed Acyclic Energy-Flow Solver where players wire together Power Nodes (Fission pebble-bed, D-T / D-3HeD-3He / p-11Bp-11B Fusion, Beamed Laser Receiver, Antimatter Annihilation), Conversion Nodes (Brayton turbine, MHD generator, Cryogenic magnetic nozzle), Thrust Nodes (Thermal expansion, Electrostatic grid, Lorentz MPD, Photon reflection), and Radiator Nodes (ϵσAT4ϵσAT4). Enforce conservation of energy (Pin=Pjet+Pradiated+Pstructural_heatingPin​=Pjet​+Pradiated​+Pstructural_heating​) and conservation of momentum at every node.
    Work Package 4.4: Relativistic Astrodynamics, ISM Dust Erosion & X-Ray Pulsar Navigation
        Math Spec: Switch the trajectory propagator to the Relativistic Rocket Equation (Ackeret's Equation) and special-relativistic momentum p⃗=γmv⃗p
        ​=γmv
        whenever v>0.005cv>0.005c. Render both the Retarded Photon-Arrival Starfield (where stars appear to be based on light travel time t−d/ct−d/c, plus relativistic aberration and Doppler color shift) and the True Ephemeris Starfield (accounting for stellar proper motion r⃗(t)=r⃗0+v⃗⋆Δtr
        (t)=r
        0​+v
        ⋆​Δt). Model kinetic energy deposition of relativistic interstellar dust impacts (Ek=(γ−1)mdustc2Ek​=(γ−1)mdust​c2) on forward Whipple/plasma shields.

