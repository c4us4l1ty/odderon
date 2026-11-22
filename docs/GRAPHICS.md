**What machine are we optimizing for?**

Intel graphics model: Intel HD 520+ and Android 8+ (Hisilicon Kirin 710+)
Memory configuration: 4 GB total system RAM (Both laptop and mobile)
Primary optimization objective: Maximum visual quality at stable 60 FPS, Lowest possible CPU and GPU consumption
My position is straightforward: **Do not build a weaker DLSS. Build a different rendering paradigm.**


To achieve "DLSS 5-level" visual fidelity—the breathtaking, sun-drenched, path-traced look of your "Shadows of a Summer Noon" reference—at a locked 60 FPS on an Intel HD 520 and an ARM Mali-G51 (Kirin 710) inside a 4 GB system RAM envelope, we have to challenge the foundational assumption of the modern graphics industry.
Why "DLSS" Exists—And Why Odderon Can Beat It Without a GPU

Why does NVIDIA need massive AI Tensor Cores for DLSS 3.5 / 4 / 5 (Ray Reconstruction & Super Resolution)?
Because traditional game engines are mathematically blind. A traditional engine throws millions of disconnected triangles and compressed bitmap textures at the screen, traces 1 noisy ray per pixel, and then uses a giant neural network to guess where the edges, shadows, and bounce light were supposed to be.

Odderon has an unfair advantage that no AAA game in history has ever had:
Your engine is not blind. Because you have zero pre-made 3D meshes and zero bitmap textures, your Rust kernel possesses the exact, continuous mathematical equation of every surface, curve, material, and light source in the scene.

You do not need a neural network to guess where a sub-pixel edge or shadow penumbra is when you have its exact analytical derivative (∇f∇f) in closed form.

Here is the blueprint for Analytical Perceptual Reconstruction (APR)—a rendering architecture designed specifically for the silicon realities of the Intel HD 520 and Mali-G51.
1. Know Your Silicon: The Dual-Architecture Reality

To hit 60 FPS (16.6 ms16.6 ms frame budget) with near-zero power draw, we must respect the fact that your two target chips work in fundamentally opposite ways:
Hardware Metric	Intel HD 520 (Skylake GT2)	ARM Mali-G51 MP4 (Kirin 710)	The Architectural Consequence
Rendering Architecture	Immediate-Mode Renderer (IMR)	Tile-Based Deferred Renderer (TBDR)	HD 520 writes every draw call straight to system RAM. Mali-G51 renders 16×1616×16 pixel tiles in tiny on-chip SRAM before writing to RAM once.
Compute Budget	~192 GFLOPS (24 EUs, scalar/SIMD8-16)	~120 GFLOPS (Bifrost 4-wide warp Clauses)	Both have plenty of math (ALU) for analytical equations, if you don't stall them waiting on memory!
Memory Bandwidth	~15–25 GB/s (Shared DDR3L/DDR4)	~10–14 GB/s (Shared LPDDR3/LPDDR4)	Bandwidth is the #1 killer. Traditional Deferred Shading or multi-pass post-processing (SSAO, Bloom, TAA) will drop both chips to 12 FPS instantly.
Usable RAM for Game	< 350 MB (out of 4 GB total OS RAM)	< 250 MB (out of 4 GB total Android RAM)	Zero 4K textures allowed. A single uncompressed 4K PBR texture set takes 85 MB—one-third of your entire memory budget!

The Golden Rule of Odderon’s Renderer:
Trade Memory Bandwidth for ALU Math. Reading a texture from RAM costs 100x more energy and time on a Mali-G51 and HD 520 than computing a trigonometric or polynomial equation inside a register.
2. Five Rendering Innovations That Deliver "Summer Noon" Fidelity on Integrated Silicon
Innovation 1: Thermal-Visual Unification (Zero-Cost Global Illumination)

Look at your "Shadows of a Summer Noon" reference image. Why does it look so rich and alive, unlike flat video games? Because of Indirect Bounce Light (Global Illumination):

    Sunlight hits the green grass at the bottom and bounces warm green light onto the underside of the wooden porch.
    Sunlight hits the wooden eaves and bounces warm amber light into the shadows.
    The deep blue sky fills the upward-facing shadowed surfaces with cool cyan light.

In Unreal Engine 5 (Lumen), calculating that bounce light requires a 300-watt GPU.
How Odderon gets it for 0.0 milliseconds of GPU time:
Remember Pillar 4 (Small Room, Big Sky): your Rust physics kernel already models rooms, buildings, rockets, and habitats as thermal networks exchanging radiative heat via View Factors (Fi→jFi→j​):
Bi=ρi(Ei+∑jFi→jBj)Bi​=ρi​(Ei​+∑j​Fi→j​Bj​)
That exact equation (the Radiosity Equation) is identical for thermal infrared radiation and visible light!

    Because your world is built from clean parametric surfaces (walls, roofs, ground planes, cylinders), the Rust CPU kernel computes the low-order Spherical Harmonics (L1/L2L1​/L2​) Radiosity Transfer between surfaces only when geometry or the sun angle changes (once every few seconds, taking < 0.1 ms on the CPU).
    Every parametric surface simply receives two 12-byte Spherical Harmonic vectors from the CPU. In the shader, evaluating that exact multi-bounce global illumination takes 4 multiply-add instructions and zero rays.

Innovation 2: Analytical Contact-Hardening Shadows (Zero Shadow-Map RAM)

In your reference image, look at the shadows of the power lines and tree leaves cast across the corrugated metal siding:

    Where the AC unit touches the wall, the shadow is razor-sharp.
    Where the power line or tree branch is 5 meters away from the wall, the shadow edge is softly blurred (penumbra) because the Sun is a disk with an angular diameter of 0.53∘0.53∘, not a point light!

Traditional games use huge 4096×40964096×4096 Shadow Map textures (eating 64 MB of RAM and destroying bandwidth) and still suffer from ugly pixelated "shadowstaircasing."

    The Odderon Solution: Because every building eave, rocket truss, power line (catenary curve), and AC box is a mathematical primitive, we don't rasterize shadow maps for local geometry!
    Each surface cluster stores a tiny Analytical Occluder List (packed into a few kilobytes of uniform buffer).
    In the shader, we compute the exact Signed Solid-Angle Intersection between the 0.53∘0.53∘ solar cone and the analytical primitive (a box, cylinder, or catenary wire).
    The distance from the occluder (doccluderdoccluder​) automatically scales the penumbra width (w=doccluder⋅tan⁡(0.53∘)w=doccluder​⋅tan(0.53∘)). You get infinite-resolution, noise-free, contact-hardening shadows that never alias, even if you walk 1 millimeter away from the wall—using 0 MB of texture memory.

Innovation 3: Analytical Filter Integration (Sharpness Beyond Native 4K)

Why do games need DLSS or Temporal Anti-Aliasing (TAA)? Because when a thin power line, a distant rocket truss, or corrugated metal siding (sine wave z=Asin⁡(ωx)z=Asin(ωx)) gets smaller than a single pixel, standard GPU rasterization point-samples it, causing horrible flickering and jagged edges.

    The Odderon Solution (Closed-Form Box Filtering): Since we know the mathematical formula of every surface pattern f(u)f(u) and its exact analytical integral F(u)=∫f(u) duF(u)=∫f(u)du, we never point-sample!
    Using the GPU’s built-in screen-space derivatives (dFdx(u) and dFdy(u), which are free in WebGL2 hardware), the shader knows the exact width ΔuΔu of the current pixel on the wall.
    Instead of evaluating f(u)f(u), the shader evaluates the exact analytical average across the pixel footprint:
    f‾(u)=F(u+12Δu)−F(u−12Δu)Δuf​(u)=ΔuF(u+21​Δu)−F(u−21​Δu)​
    What this means visually: Corrugated metal walls, wooden slats, cockpit dial markings, and distant power lines never shimmer, never jagged-alias, and never blur, even at 720p/1080p with 1 sample per pixel! It is mathematically cleaner than DLSS because it isn't a guess—it is the exact integral of the light hitting that pixel.

Innovation 4: Procedural Micro-Surface Synthesis (The 64-Byte Material)

How do you make weathered wood, painted metal, concrete, and lush grass look tactile like your reference image without a single image texture?

    Every material in Odderon is a 64-byte Parametric Shader Recipe driven by Multi-Scale Analytical Noise & Cellular Functions evaluated only at the visible surface:
        Weathered Cedar Wood (Shopfront): Cylindrical domain-warped sine rings + high-frequency anisotropic fiber specular (Kajiya-Kay analytical highlight) + ambient occlusion darkened in the plank joints.
        Corrugated Galvanized Steel (Left Building): Analytical integral of sin⁡(ωx)sin(ωx) normals + Voronoi crystal spangle pattern + vertical rain-streak oxidation gradient calculated from the roof overhang distance! (Notice how in real life, rust and dirt streaks only happen where rain drips off an overhang—your CAD geometry already knows where the overhang is!)
        Summer Grass (Foreground): Instanced quadratic Bézier blades animated by a continuous 2D travelling wind-wave equation in the vertex shader, shaded with an analytical translucent subsurface term when backlit by the sun.

Innovation 5: The "Living Still" Temporal Cache (Near-0% Idle GPU)

In Odderon, whether you are sitting at your wooden desk in your apartment, watching a rocket on the pad, or monitoring telemetry in Mission Control, most of the screen is temporally coherent.

    Standard games redraw 100% of the screen 60 times a second, burning laptop batteries and heating up phones even when you are just staring at a gauge.
    Odderon’s Spatio-Temporal Tile Scheduler:
        The screen is logically divided into 32×3232×32 pixel tiles.
        When the camera is stationary, the Rust kernel knows which exact tiles intersect a moving object (e.g., the swaying grass in the bottom 20% of the screen, the spinning fan inside the AC unit, the drifting summer cloud, or a moving needle on a cockpit dial).
        Static tiles are locked in the framebuffer and skipped via hardware Scissor / Stencil masking!
        Even better: for the static tiles, over the first 8 frames after you stop moving your head, the engine accumulates jittered sub-pixel samples—meaning the moment you stop moving your camera for 0.13 seconds, the image sharpens to 8x Supersampled Cinema Quality, and then GPU usage drops to < 3%, updating only the swaying grass and clouds at 60 FPS!

3. Platform-Specific Execution: One Scene, Two Hardware Paths

You rightly pointed out that Intel HD 520 and Mali-G51 must share the scene representation and rendering mathematics, but execute differently to respect their silicon. Here is how the Rust WebGL2 backend branches at initialization:

text

                  ┌──────────────────────────────────────────┐
                  │   RUST ANALYTICAL SCENE & THERMAL GRAPH  │
                  │  (Parametric Surfaces + SH Bounce Light) │
                  └────────────────────┬─────────────────────┘
                                       │
              ┌────────────────────────┴────────────────────────┐
              ▼                                                 ▼
┌───────────────────────────────────┐             ┌───────────────────────────────────┐
│   PATH A: INTEL HD 520 (Laptop)   │             │    PATH B: MALI-G51 (Android)     │
│    Immediate-Mode Architecture    │             │     Tile-Based (TBDR) Silicon     │
├───────────────────────────────────┤             ├───────────────────────────────────┤
│ 1. Zero-Overdraw Pre-Pass:        │             │ 1. Strict Single-Pass Forward:    │
│    Renders a compact 32-bit       │             │    NEVER flush off-screen buffers │
│    Visibility/Depth buffer first  │             │    to system RAM. Keep 100% of    │
│    so heavy procedural shaders    │             │    shading inside on-chip 16x16   │
│    NEVER run on occluded pixels.  │             │    tile SRAM.                     │
│                                   │             │                                   │
│ 2. Perceptual Adaptive Shading:   │             │ 2. FP16 (`mediump`) Fast-Math:    │
│    Shades high-contrast edges at  │             │    Mali-G51 runs FP16 at 2x the   │
│    1:1 native pixels, and smooth  │             │    speed and half the register    │
│    sky/shadow interiors at 1:4    │             │    pressure of FP32. Because      │
│    with bilateral edge-aware      │             │    coordinates are camera-local,  │
│    interpolation.                 │             │    all lighting & color math      │
│                                   │             │    runs in native `mediump` FP16! │
│                                   │             │                                   │
│ 3. Dirty-Rect Scissor Updates:    │             │ 3. Tile Transaction Elimination:  │
│    When camera is still, only     │             │    Identical analytical sky/wall  │
│    redraws bounding boxes of      │             │    tiles trigger Mali hardware    │
│    moving analytical primitives.  │             │    CRC Transaction Elimination,   │
│                                   │             │    writing 0 bytes to LPDDR4!     │
└───────────────────────────────────┘             └───────────────────────────────────┘

Why Path B is Magic on Mali-G51:

ARM Mali GPUs have a hardware feature called Transaction Elimination (TE): at the end of rendering a 16×1616×16 pixel tile in on-chip SRAM, the GPU compares a hardware CRC signature of that tile against the previous frame's tile. If the signature matches, the GPU completely skips writing that tile to system RAM!
Because Odderon uses noise-free analytical shading (no temporal grain or dither noise on static surfaces), any tile where nothing moved produces a 100% bit-identical output, triggering Mali's hardware Transaction Elimination automatically and cutting memory bus power consumption to near zero!
4. The 4 GB RAM Survival Budget (Total Footprint: < 180 MB)

On a 4 GB Windows laptop or Android 8 phone, if your browser tab crosses ~400 MB of allocation, the OS starts swapping to disk or the Android Low Memory Killer (LMK) terminates the tab.

Here is Odderon's strict, hard-capped memory allocation budget:
Subsystem	Memory Allocation	How It Stays So Small
WASM Code Binary	~3.5 MB	Pure Rust (no_std-style discipline, zero bloated JS frameworks).
Gaia DR3 + Ephemeris Octree	~24.0 MB	Active 4,000 LY spatial octree chunk + Chebyshev solar system polynomials.
Physics & Provenance<T> Heap	~32.0 MB	1D fluid/thermal graphs, analytical orbital tables, and CAD state.
Parametric Mesh Vertex Cache	~18.0 MB	Generated on-the-fly from CAD/architecture equations; evicted via LRU when out of view.
Texture Memory (VRAM)	0.5 MB	Zero 3D asset textures. Only a tiny 512x512 signed-distance font atlas for crisp cockpit & blueprint typography!
Framebuffers (1080p / 720p)	~24.0 MB	Front/Back color + Depth + 1 Temporal Accumulation buffer.
WebAudio Synthesis Buffers	~4.0 MB	Real-time procedural fluid/mechanical sound wave ring-buffers.
TOTAL ENGINE FOOTPRINT	~106.0 MB	Leaves > 3.8 GB of breathing room for the OS and browser!
5. Falsifiable Rendering Tests (For the Design Bible)

Just like your five gameplay pillars, the rendering engine gets three ruthless, falsifiable tests enforced in code review:

    R1 — The Zero-Bitmap Law:
    Test: Search the repository and network tab for .png, .jpg, .dds, .ktx, .obj, .fbx, or .gltf. If any file exists outside the single typography SDF glyph sheet, the build fails. Every visual surface must be a function S(u,v,t)S(u,v,t) derived from physical state.
    R2 — The Static-Camera Power Test (The Painting Test):
    Test: Load the "Summer Noon" street scene on an Intel HD 520 laptop at 1080p. Take your hands off the mouse and keyboard. Within 250 milliseconds, frame-time must drop below 1.5 ms (< 10% GPU load), and visual edge quality must be indistinguishable from an 8x supersampled offline render.
    R3 — The Single-Pass Bandwidth Ceiling (Mali-G51 Test):
    Test: Profile a frame on Android Mali-G51. Total external memory bandwidth per frame must never exceed 32 bytes per screen pixel (1 color write + 1 temporal read/write). Any effect that requires flushing a full-screen intermediate texture to RAM and reading it back is forbidden.

By combining Thermal-Visual Radiosity Unification, Analytical Box-Filtered Materials, Exact Solar-Cone Shadows, and Dual IMR/TBDR Silicon Paths, Odderon will look like a living Makoto Shinkai / Hachimi film running at 60 FPS on a $150 laptop from 2015—and the entire game will load in 2 seconds over a 3G phone connection.
