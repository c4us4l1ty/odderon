I located the repository and its current `main` branch. I am treating this as an architectural and research exercise, not a conventional graphics-optimization exercise.

The central idea I see emerging is considerably more interesting than simply making a low-poly game run faster:

**Odderon is attempting to decouple perceived visual fidelity from the conventional computational cost of rendering visual fidelity.**

That is the right problem to attack.

One distinction matters from the outset: reproducing the *perceptual impact* of DLSS 5-class imagery is a different objective from reproducing its underlying neural rendering, geometry, lighting and reconstruction capabilities. The former gives us a much larger design space, particularly on integrated graphics.

## **1. What I found in Odderon**

I read the Markdown files in `docs/`, read `IMPORTANT-2.md`, and skimmed the opening architecture and design material in `IMPORTANT-1.md`. I also inspected the current rendering implementation, particularly `web/shaders.js`, `web/main.js`, and `crates/odderon-geom/src/lib.rs`.

The project has an important advantage: **you are already attacking several of the correct bottlenecks at the architectural level.**

**Current rendering architecture**

Implemented

**Procedural geometry**

Rust constructs geometry mathematically. No downloaded mesh library or conventional texture collection.

Implemented

**Analytical WebGL2 shaders**

Procedural surface detail, sky, cumulus clouds, lighting, ambient contributions and shadows.

Implemented

**Batched geometry**

The static apartment and exterior town are uploaded as one mesh and rendered together.

Implemented

**Render-on-demand**

The presenter parks when the scene is settled instead of continuously scheduling frames.

Implemented

**Deterministic screenshot tooling**

Camera, sun and environmental parameters can be controlled for reproducible visual comparisons.

Proposed / needs validation

**Progressive neural-style reconstruction**

Temporal refinement, attention-driven shading and more advanced reconstruction remain areas to develop and benchmark.

Some particularly interesting implementation details:

- The geometry engine uses a 10-float vertex representation: position, normal, RGB and material ID.
- The static world is designed around a budget of approximately 64,000 triangles.
- Material details are generated analytically in shaders.
- Shadows are approximated using geometric occlusion and analytical gobos.
- The sky uses a low-resolution procedural cloud field.
- The browser presenter uses WebAssembly for the simulation and raw WebGL2 for rendering.

This is already a much more interesting foundation than simply putting a conventional 3D engine into a browser.

### **The first important correction**

Your `PERFORMANCE.md` contains an important historical measurement:

**Block 1, round 1 achieved 11 FPS at 1880 × 959 on the target class of machine.**

The subsequent architectural changes are substantial, but the document explicitly acknowledges that the revised performance remains unverified.

That distinction matters.

The theoretical GPU arithmetic budget is not a benchmark. Memory bandwidth, render-target traffic, browser scheduling, shader occupancy, CPU submission overhead and GPU synchronization can dominate the actual result.

We need to make every performance claim experimentally falsifiable.

## **2. What DLSS 5 actually changes**

There is a significant development worth incorporating into our thinking.

NVIDIA's DLSS 5 is not simply an upscaler. It introduces **3D-Guided Neural Rendering**.

Its architecture uses:

- The conventional rendered frame.
- Motion vectors.
- Temporal information.
- Scene and material information.
- Learned priors about real-world appearance.

It then synthesizes enhanced lighting and material appearance while preserving the underlying scene's structure and artistic intent.

The research paper describes it as a generative rendering stage complementary to conventional rendering. It operates locally on RTX 50-series hardware.&#x20;

[image](https://www.google.com/s2/favicons?domain=https://research.nvidia.com\&sz=32)

NVIDIA ADLR+2



This gives us a much more interesting research question:

**Can we reproduce a substantial portion of the perceptual benefits of neural rendering without running an expensive neural network?**

I think this is the direction worth investigating.

We should not attempt to reproduce NVIDIA's implementation on an Intel integrated GPU. Its hardware assumptions are fundamentally different.

We should attack the underlying problem from another direction.

## **3. The central innovation: stop treating rendering as image generation**

Traditional rendering asks:

> How do I calculate every pixel in the final image?

I want Odderon to ask:

> What is the minimum amount of computation necessary to produce an image that the human visual system cannot meaningfully distinguish from a more expensive reference?

These are fundamentally different optimization problems.

Let:

- \\(I_r\\): reference image.
- \\(I_o\\): Odderon's rendered image.
- \\(D(I_r,I_o)\\): perceptual difference.
- \\(C\\): computational cost.

Our objective becomes:

\\[ \boxed{ \min\_{\mathcal R} C(\mathcal R) \quad\text{subject to}\quad D(I_r,I_o)\leq\epsilon } \\]

But there is a deeper opportunity.

Human vision does not process every region of an image with equal importance.

A silhouette, a face, a moving object, a bright specular highlight and a distant patch of grass have different perceptual significance.

Therefore, we should not distribute computational resources uniformly.

We should allocate them according to perceptual importance.

This leads to several research directions.

# **4. Seven innovations I would investigate**

These are architectural proposals, not claims that they are already implemented.

## **I. Perceptual Computation Allocation (PCA)**

Highest priority

Instead of assigning a fixed rendering resolution to the entire screen, assign every screen region a computational priority.

Define a priority function:

\\[ P(x,y)= w_1S+w_2M+w_3A+w_4K+w_5U \\]

Where:

- \\(S\\): silhouette importance.
- \\(M\\): motion magnitude.
- \\(A\\): estimated player attention.
- \\(K\\): semantic importance.
- \\(U\\): uncertainty in the current reconstruction.

For example:

[Japanese countryside, Old House, Veranda, Summer vacation, At dusk - SeaArt AI](https://images.openai.com/static-rsc-4/GQzSJYp3xm3wX-FDPSAhmD1KVP5cBBAMlkLeRxr-rQy2TF3fCjwDUe3Gmf-akZMzPFBWYsYCLP5lPB7Pd1khaL6jmlyyMUTWNmp7nJhnT_AffKPAEJq74AtL2WWHMtN5ISBJsYlHXkVjaCkX4w0aiHyKCHEvQo6gfXBdAJ5TrBI?purpose=inline)

**High priority**

Interacting objects, nearby geometry, important silhouettes.

[Your Name. - Serene Anime Landscape HD Wallpaper | 1920x1080 | Wallpaper Abyss](https://images.openai.com/static-rsc-4/sbp4iytVCA-0trYtEZX12JAV5romnIztHboJv5vNszYLwzgDtAT0TT9qGFY-K2V7SlFcVws7RlVyTmGyeYnWrOm6-DP8qqkxqZVl99oMDtbWnC_VYXgK2-VhZP3k4SKKvBBK4-OeOmo9ya0174kBQhKzlT7mOhCW54_c6EoYpQE?purpose=inline)

**Lower priority**

Distant scenery, peripheral foliage, low-contrast atmospheric regions.

The renderer allocates more samples and refinement to high-priority regions.

A possible computational allocation:

\\[ B_i=B\_{\text{total}} \frac{P_i^\gamma}{\sum_jP_j^\gamma} \\]

Here \\(B_i\\) is the rendering budget assigned to region \\(i\\), and \\(\gamma\\) controls how aggressively we concentrate resources.

This can become a general-purpose perceptual scheduler.

It doesn't need eye-tracking hardware. Odderon already knows the camera direction, interaction target, movement direction and scene semantics.

**Important:** camera direction is only an attention proxy, not actual eye gaze. We should account for that distinction.

## **II. Temporal Radiance Memory**

Extremely promising

This is where I would challenge conventional rendering architecture.

Why calculate the same lighting information repeatedly?

A static wall does not suddenly forget its illumination between frames.

Instead of treating every frame as an independent rendering problem, maintain a persistent representation of previously calculated visual information.

Conceptually:

\\[ L_t(x)= \begin{cases} L\_{t-1}(\phi_t(x)), & \text{valid historical sample}\\\ L\_{\text{new}}(x), & \text{stale or disoccluded sample} \end{cases} \\]

Here:

- \\(L_t\\) is the radiance estimate.
- \\(\phi_t\\) is the reprojection mapping.
- Stale pixels are selectively recomputed.

But we can go further.

Store confidence alongside radiance:

\\[ \mathcal M(x)=(L,\mathbf n,d,m,c,t) \\]

Where:

- \\(\mathbf n\\): surface normal.
- \\(d\\): depth.
- \\(m\\): material identity.
- \\(c\\): reconstruction confidence.
- \\(t\\): timestamp.

This gives us a temporal memory of the rendered world.

When the player moves, we transport valid information rather than regenerate everything.

When the player turns toward a previously unseen region, we progressively resolve it.

This is more sophisticated than ordinary temporal anti-aliasing.

The crucial problem is disocclusion: newly visible surfaces have no valid history. We must reconstruct those rather than blindly smear old pixels into them.

## **III. Neural Knowledge Without Neural Inference**

This is one of my favorite directions.

Consider what neural rendering models actually learn: complex relationships between geometry, materials, lighting and appearance.

We could transfer some of that knowledge into a different computational representation.

Instead of running a neural network every frame, train models offline and distill their useful behavior into compact mathematical functions.

For example:

\\[ f\_{\theta}(n,v,l,m) \longrightarrow \sum\_{k=1}^{N}a_k(m)\phi_k(n,v,l) \\]

Where:

- \\(n\\): surface normal.
- \\(v\\): viewing direction.
- \\(l\\): illumination direction.
- \\(m\\): material identity.
- \\(\phi_k\\): inexpensive basis functions.
- \\(a_k\\): learned coefficients.

The resulting representation could be evaluated directly in GLSL.

Potential implementations include:

- Spherical harmonics.
- Low-rank lighting bases.
- Polynomial material response approximations.
- Compact neural-network distillation.
- Learned procedural noise distributions.

The important distinction:

**Train expensively. Render cheaply.**

We could effectively compile visual knowledge into a shader representation.

No Tensor Cores required.

No large neural inference model running every frame.

This would be a different approach to neural rendering, not an equivalent implementation of DLSS 5.

## **IV. Semantic Geometry Compression**

We should stop treating geometry as merely a collection of triangles.

A corrugated wall is not fundamentally 20,000 triangles.

It is a surface defined by a small number of parameters.

For example:

\\[ z(x)=z_0+A\sin(\omega x+\phi) \\]

A roof can be represented through its structural parameters, tile spacing and curvature.

A pipe can be represented as a swept curve with a cross-sectional profile.

A tree can be represented through a hierarchy of branching functions.

The key is a semantic scene representation:

```
Scene
 ├── Architecture
 │    ├── Wall
 │    │    ├── Material
 │    │    ├── Corrugation
 │    │    └── Surface response
 │    └── Roof
 ├── Vegetation
 ├── Atmospheric fields
 └── Lighting
```

The renderer should decide how to represent each object depending on its distance, screen-space footprint and importance.

A distant building might be a silhouette.

A nearby building might be a parametric mesh.

A wall directly in front of the player might receive analytical surface details.

One object, multiple representations, seamless transitions.

This is an opportunity to combine procedural geometry, impostors, mesh LOD and analytical surface evaluation under one compiler-like system.

## **V. Physics-Coupled Appearance Synthesis**

This is where Odderon could become genuinely distinctive.

Most graphics systems treat physical simulation and visual appearance as separate domains.

Odderon does not need to.

Consider the following:

- Temperature determines thermal radiation.
- Fluid velocity influences surface motion.
- Pressure determines mechanical deformation.
- Material composition influences reflectance.
- Surface roughness influences specular response.
- Atmospheric density influences scattering.
- Structural stress influences visible deformation.

Instead of manually authoring visual effects, derive them from the simulation.

For a thermal surface:

\\[ L\_{\lambda}(T)= \frac{2hc^2}{\lambda^5} \frac{1}{e^{hc/(\lambda k_BT)}-1} \\]

For atmospheric scattering, derive the visual response from the atmospheric density and optical depth.

For an operating rocket engine, derive plume appearance from the underlying exhaust conditions rather than an arbitrary particle effect.

This creates a unified physical-to-visual pipeline.

The same physical state that determines whether an engine works determines how it looks.

That is a very powerful design principle.

## **VI. Predictive View Synthesis**

Your `PERFORMANCE.md` already proposes speculative synthesis.

I would develop that into a more general system.

The renderer maintains a probability distribution over likely future views:

\\[ p(V\_{t+\Delta}\mid V_t,\dot C_t,I_t) \\]

Where:

- \\(V_t\\): current view.
- \\(\dot C_t\\): camera movement.
- \\(I_t\\): player input.

It can begin resolving likely future regions before the camera reaches them.

For instance, when the player begins turning toward a launch pad, the system starts refining that region.

When the player suddenly turns in another direction, unfinished work is discarded or deprioritized.

The challenge is ensuring speculative work never competes with immediate interaction latency.

The system must be interruptible and governed by a strict budget.

## **VII. Perceptual Error Correction**

This is the innovation I would use to tie the entire architecture together.

Every rendered region receives an estimated reconstruction error:

\\[ E(x,y)= \alpha E\_{\text{structure}} +\beta E\_{\text{motion}} +\gamma E\_{\text{appearance}} +\delta E\_{\text{temporal}} \\]

The renderer identifies regions where its approximation is no longer sufficiently accurate.

It then selectively spends computation correcting those regions.

In other words:

**Odderon should not render everything equally. It should continuously identify what it is getting wrong and correct only that.**

That creates an adaptive renderer rather than a conventional fixed-cost renderer.

# **5. The architecture I would build**

I would combine these ideas into a new rendering architecture.

Working name: **Odderon Perceptual Synthesis Engine (OPSE).**

# **OPSE**

PROPOSED RENDERING ARCHITECTURE

**1. Semantic World Representation**

Parametric geometry · Materials · Physical state · Scene relationships

**2. Perceptual Intelligence Layer**

Attention estimation · Screen-space importance · Error prediction

**3A. Geometry Synthesis**

Parametric meshes · SDF · LOD · Impostors

**3B. Appearance Synthesis**

Learned bases · Analytical lighting · Materials

**4. Temporal Radiance Memory**

Reprojection · Confidence · History · Selective refinement

**5. Perceptual Error Correction**

Detect errors · Allocate budget · Refine · Present

Feedback into the perceptual intelligence layer

The design principle is that each stage should have access to information produced by the preceding stages. We should avoid independent systems repeatedly calculating information another system already possesses.

### **The most important architectural decision**

Do not make neural inference a mandatory dependency.

I would build three rendering backends:

| Backend    | Purpose                                            |
| ---------- | -------------------------------------------------- |
| Analytical | Baseline rendering for integrated GPUs             |
| Temporal   | Reuse and selectively reconstruct previous results |
| Learned    | Optional compact learned appearance models         |

This gives us a renderer that degrades gracefully across hardware generations.

A 2015 Intel integrated GPU should not need the same rendering pipeline as an RTX 5090.

Both should produce the same underlying world, but spend their computational budgets differently.

# **6. What we need to stop assuming**

There are several things in the existing documentation that I would challenge.

**1. Zero assets does not automatically mean zero cost.**

Procedural geometry still consumes vertex processing, rasterization, memory bandwidth and fragment shading.

The absence of textures is an advantage, not a guarantee of performance.

**2. One-third pixel shading does not automatically produce full-resolution fidelity.**

Thin geometry, disocclusion, specular highlights, rapid camera motion and high-frequency detail can expose reconstruction errors.

We need to measure these independently.

**3. Exact analytical motion does not mean exact visual reconstruction.**

Knowing an object's position at every instant is valuable, but it does not automatically solve visibility, transparency, reflections or changing illumination.

**4. GPU FLOPS are not the objective function.**

The shared memory bus on integrated graphics is particularly important. We should measure bandwidth pressure and framebuffer traffic, not just arithmetic operations.

**5. Four gigabytes of RAM is not four gigabytes of dedicated VRAM.**

Intel UHD graphics generally use shared system memory. We need to know the exact GPU, processor, memory configuration and available graphics memory.

# **7. The experimental roadmap**

I would not immediately rewrite the renderer. That would be a mistake.

We have an existing implementation that provides a useful experimental baseline.

Instead, establish a sequence of independently testable breakthroughs.

1. **Experiment 1 — Establish the actual hardware baseline**

   Record:
   - Exact Intel GPU model and EU count.
   - CPU and RAM configuration.
   - Browser and WebGL2 capabilities.
   - GPU frame time and CPU submission time.
   - Memory consumption and power consumption.

   Capture the existing seven reference views and establish reproducible measurements.
2. **Experiment 2 — Perceptual computation allocation**

   Implement adaptive rendering priority.

   Compare uniform shading against attention-driven shading at identical computational budgets.

   The experiment succeeds when perceptual quality improves without increasing frame cost.
3. **Experiment 3 — Temporal radiance memory**

   Introduce persistent screen-space or world-space reconstruction history.

   Test camera translation, rotation, object movement, disocclusion and lighting changes.

   Measure ghosting, temporal stability and reconstruction latency.
4. **Experiment 4 — Learned appearance distillation**

   Train a model offline to approximate expensive material and lighting responses.

   Distill it into a compact shader representation.

   Compare its visual error, instruction count and memory consumption against the analytical baseline.
5. **Experiment 5 — Physics-coupled appearance**

   Connect physical simulation outputs directly to visual synthesis.

   Test thermal surfaces, exhaust plumes, atmospheric scattering and deformation.
6. **Experiment 6 — Integrated renderer**

   Combine successful experiments and benchmark the complete pipeline against the original renderer.

## **8. How we measure whether we have succeeded**

I want an explicit benchmark rather than an impressive screenshot.

| Metric              | What we measure                          |
| ------------------- | ---------------------------------------- |
| Frame time          | p50, p95, p99                            |
| GPU time            | Actual GPU execution                     |
| CPU time            | Main thread and worker                   |
| Memory              | Peak resident memory                     |
| Bandwidth           | Render-target and buffer traffic         |
| Temporal stability  | Flicker, ghosting, disocclusion          |
| Visual fidelity     | Reference-image perceptual difference    |
| Interaction latency | Input-to-visible-response                |
| Power               | System power under controlled conditions |

We should use several visual quality measurements:

- FLIP for perceptual image differences.
- LPIPS for learned perceptual similarity.
- SSIM for structural similarity.
- Human pairwise comparisons for actual perceived quality.

No individual metric should become our definition of visual quality.

**Proposed initial experimental targets**

RESEARCH TARGETS, NOT CURRENT PERFORMANCE CLAIMS

# **60**

**FPS**

Interactive target

# **1080p**

**Output**

Full-resolution presentation

# **0**

**Scheduled idle frames**

When genuinely settled

# **4 GB**

**System RAM**

Target hardware class

The 60 FPS target requires a 16.67 ms frame budget. Our eventual goal should be substantially lower frame cost to leave room for the simulation and browser overhead.

# **9. The bigger opportunity: graphics without conventional rendering**

I want to introduce one more idea.

What happens when we stop treating a game world as a collection of objects that must be rendered, and instead treat it as a collection of **observable phenomena**?

Consider a Japanese coastal town.

The player sees:

- A wooden building.
- A corrugated metal roof.
- Grass.
- Clouds.
- Sunlight.
- Shadows.
- Atmospheric haze.

A conventional engine represents these through meshes, textures, lighting systems, shadow maps and material pipelines.

Odderon could represent much of the same environment through a compact collection of mathematical descriptions.

\\[ \mathcal W = \\{ G,M,L,A,T,P \\} \\]

Where:

- \\(G\\): geometric structure.
- \\(M\\): material properties.
- \\(L\\): illumination.
- \\(A\\): atmospheric conditions.
- \\(T\\): temporal state.
- \\(P\\): physical state.

The displayed image is then:

\\[ I=\mathcal R(\mathcal W,C) \\]

Here \\(C\\) is the observer's camera state.

The important question becomes how much of \\(\mathcal W\\) actually needs to be evaluated to produce the current observation.

This is where the combination of mathematical rendering, perceptual scheduling, temporal memory and procedural synthesis becomes powerful.

It also fits Odderon's scientific identity: a physically meaningful world whose visual appearance emerges from its underlying state.

# **10. One question before we design the next generation**

I need to establish the exact hardware target before choosing the algorithms and numerical budgets.

**What machine are we optimizing for?**

Intel graphics model: Intel HD 520+ and Android 8+ (Hisilicon Kirin 710+)
Memory configuration: 4 GB total system RAM (Both laptop and mobile)
Primary optimization objective: Maximum visual quality at stable 60 FPS, Lowest possible CPU and GPU consumption
My position is straightforward: **Do not build a weaker DLSS. Build a different rendering paradigm.**

NVIDIA has enormous resources to make neural rendering work through specialized hardware. Odderon's opportunity is to discover how much of that visual result can emerge from mathematical structure, perceptual intelligence, temporal computation and carefully distilled knowledge.

The real breakthrough would not be making one game look impressive on an old laptop.

It would be making an entire class of games computationally inexpensive by changing what a renderer considers necessary to calculate.

That is a much more consequential objective.
