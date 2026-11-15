Odderon Pre-Coding Blueprint
Repository Structure, Rust/Wasm Boundary, Data Contracts, Cryptographic Identity & Proof-of-Trajectory

The identity update changes the architecture in one very good way: the physics kernel and the anti-cheat verifier are now the same Rust crate, compiled to the same .wasm binary, running in two places (the player's browser and a Cloudflare Worker). That single decision is what makes Astro-Elo trustworthy at $0/month.

Below is everything an AI coding agent needs before writing the first line: the workspace layout, which code runs where, the exact data contracts (Rust type signatures — these are contracts, not implementations), the identity/verification pipeline, and the paste-ready task card for Work Package 0.1.
Part 1: "As Much Rust As Possible" — The Final Stack Decision
Layer	Technology	Why
Physics, ephemeris, propulsion, systems, scoring	Rust (pure, no_std-friendly crates)	Deterministic, shared between client & verifier
Identity, signing, hashing, canonical serialization	Rust (ed25519-dalek, sha2, hkdf, postcard)	No JS crypto libraries; getrandom pulls entropy from WebCrypto
UI (panels, telemetry, CAD controls)	Rust → Leptos (CSR)	Fine-grained signals (Svelte/Solid-style surgical DOM updates, no virtual DOM) — zero CPU when values don't change
Painted renderer (WebGL2) & vector overlay (Canvas2D)	Rust via web-sys + GLSL shader files	GLSL is the only non-Rust code that must exist
Physics worker thread	Rust (same wasm module loaded in a Web Worker)	Keeps integrator off the UI thread
Verification server + leaderboard API	Rust → Cloudflare Workers (workers-rs)	Free tier: 100k requests/day
Database (Elo, callsigns, commitments)	Cloudflare D1 (SQLite) via workers-rs	Free tier: 5 GB
FDR archive & star-octree chunks	Cloudflare R2	Free tier: 10 GB, zero egress fees
Static hosting	Cloudflare Pages (served by trunk build)	Free
PDR dossier PDF	Rust (printpdf / SVG → PDF)	Generated client-side, embeds verification QR

Resulting JavaScript in the entire project: one ~40-line index.html wasm loader and the Web Worker bootstrap. Everything else is Rust or GLSL.
The Determinism Mandate (New Global Law #6 — add to ARCHITECTURE_LAWS.md)

Proof-of-Trajectory only works if the verifier gets bit-identical results to the player's browser. WebAssembly's f64 arithmetic is IEEE-754 deterministic, but transcendental functions are not guaranteed identical across platforms. Therefore:

    The Bit-Reproducibility Law: All physics crates must:
        Use only f64 (never f32) for simulation state.
        Call transcendental functions exclusively through the libm crate (libm::sin, libm::atan2, libm::exp, libm::pow) — never f64::sin() etc., whose implementation differs between native test runs and wasm.
        Never iterate a HashMap/HashSet where order affects numerical results (use Vec / BTreeMap).
        Never read wall-clock time, random numbers, or thread state inside a simulation.
        Quantize all player inputs to integers at the boundary (epochs in microseconds i64, throttle in u16/65535, quaternions in i16×4 normalized) so two replays can never start from "slightly different" floats.
        Canonical hashing uses postcard byte encoding (raw IEEE bit patterns), never JSON (JSON float formatting is non-canonical).

The oracle test for this law: run odd-sim on the same FDR natively (cargo test) and in wasm (wasm-pack test --headless); the final state hash must be byte-identical.
Part 2: Cargo Workspace & Folder Structure

text

odderon/
├── Cargo.toml                     # [workspace] members = ["crates/*", "tools/*"]
├── ARCHITECTURE_LAWS.md           # The 6 anti-hallucination laws (agent system prompt)
├── WORK_PACKAGES/                 # One markdown task card per WP (0.1, 0.2, ... 4.4)
│
├── crates/
│   ├── odd-core/                  # ZERO dependencies on everything else.
│   │   └── src/ units.rs          #   SI newtypes (Meters, Kg, Seconds, Kelvin, Pascals...)
│   │            epoch.rs          #   Epoch { tdb_s: i64, frac: f64 } since J2000.0
│   │            frames.rs         #   FrameId, 4-tier coordinate tree, i64 milliparsec galactic coords
│   │            canon.rs          #   Canonical postcard encode + SHA-256 hash of any schema struct
│   │            consts.rs         #   G, c, σ_SB, g0, R_u, AU — with source citations in doc comments
│   │
│   ├── odd-ephem/                 # Celestial body catalog + O(1) analytical ephemeris
│   │   └── src/ kepler.rs         #   Newton-Raphson Kepler solver (elliptic + hyperbolic)
│   │            bodies.rs         #   BodyId, physical constants (μ, R_eq, J2, rotation, obliquity)
│   │            ephb.rs           #   Reader for packed binary ephemeris (.ephb)
│   │
│   ├── odd-astro/                 # Levels 1/2/3 astrodynamics
│   │   └── src/ elements.rs       #   Cartesian ↔ Keplerian (all degenerate cases)
│   │            level1.rs         #   Vis-viva, Hohmann, bi-elliptic, plane change, SOI
│   │            perturb/          #   j2.rs, third_body.rs (Battin f(q)), drag.rs (USSA76), srp.rs (shadow)
│   │            integrator.rs     #   Adaptive Dormand-Prince RK45 / RK89, state = [r, v, m, subsystems]
│   │            spline.rs         #   Hermite keyframe cache (SoA f64 buffer)
│   │            attribution.rs    #   Shapley error attribution (toggle perturbations)
│   │
│   ├── odd-propulsion/            # Engine Lab solver
│   │   └── src/ thermochem.rs     #   Propellant tables (γ(T), M̄, Tc(O/F), ρ, h_vap) from NASA CEA exports
│   │            nozzle.rs         #   Area–Mach solver, choked flow, Summerfield separation, Bartz flux
│   │            graph.rs          #   Energy-flow node graph (Block 4 generalization lives here from day 1)
│   │
│   ├── odd-systems/               # Coupled subsystem ODEs
│   │   └── src/ power.rs thermal.rs cryo.rs attitude.rs structure.rs
│   │
│   ├── odd-sim/                   # THE SHARED KERNEL. Design + CommandTimeline → Trajectory + Outcome.
│   │   └── src/ mission.rs        #   run_mission(&Contract, &Design, &Timeline) -> MissionOutcome
│   │            fdr.rs            #   FlightDataRecorder schema, versioning, segment checkpoints
│   │            verify.rs         #   verify_fdr(&Fdr) -> VerifyResult (used by client AND worker)
│   │
│   ├── odd-contracts/             # Mission definitions, Pareto baselines, scoring, Elo math
│   │   └── src/ catalog.rs score.rs elo.rs
│   │
│   ├── odd-identity/              # Flight License: wordlist, keypair derivation, signing
│   │   └── src/ wordlist.rs       #   2048 aerospace words (11 bits each), compile-time embedded
│   │            license.rs        #   entropy → 12 words ↔ ODD-hex key; HKDF → Ed25519
│   │            sign.rs           #   sign(canonical bytes) / verify
│   │
│   ├── odd-report/                # PDR dossier: SVG layout → PDF, embeds verification QR
│   │
│   ├── odd-wasm/                  # wasm-bindgen boundary (thin!) — exports for worker + verifier page
│   │
│   ├── odd-worker/                # Cloudflare Worker (workers-rs): /commit, /submit, /verify, /leaderboard
│   │
│   └── odd-app/                   # Leptos UI + renderer
│       └── src/ screens/          #   fdo_map.rs, cad_lab.rs, mocr.rs, observatory.rs, habitat.rs, debrief.rs
│                render/           #   painted_gl.rs (WebGL2 passes), overlay_2d.rs, dirty_flag.rs
│                lens/             #   visual_physics_lens.rs, causal_graph.rs
│                worker_bridge.rs  #   postMessage protocol to physics Web Worker
│
├── shaders/                       # GLSL: gbuffer.frag, brushstroke.vert/frag, schlieren.frag, gravity_lens.frag
├── data/
│   ├── raw/                       # JPL Horizons & CEA exports (scripts fetch; not committed)
│   ├── packed/                    # earth_moon.ephb, propellants.bin, wordlist.txt
│   └── oracles/                   # vallado_2_5.toml, rs25.toml, sso_precession.toml ...
├── tools/
│   ├── pack-ephem/                # Rust CLI: Horizons CSV → .ephb Chebyshev/Kepler coefficients
│   └── pack-wordlist/             # Validates 2048 unique words, no prefix collisions in first 4 letters
├── tests/oracle/                  # Workspace-level Oracle Gate tests (one file per WP)
└── web/ index.html Trunk.toml worker.js

Dependency Direction (Enforced by cargo deny / cargo-depgraph)

text

odd-core ← odd-ephem ← odd-astro ← odd-sim ← odd-contracts
odd-core ← odd-propulsion ↗          ↑
odd-core ← odd-systems ──┘           │
odd-core ← odd-identity ─────────────┘
odd-sim + odd-identity + odd-contracts ← odd-wasm, odd-worker, odd-app

Rule for the agent: odd-core through odd-contracts must compile with --target wasm32-unknown-unknown and natively, must have zero web-sys/js-sys/wasm-bindgen dependencies, and must contain no I/O. That is what makes them verifiable on the server.
Part 3: The Rust ↔ Browser Boundary (Minimal Crossings)

Because the UI is also Rust (Leptos), there is no JS↔Wasm chatter on the hot path. The only real boundaries are:
Boundary A: UI Thread ↔ Physics Web Worker

text

┌────────────────────────┐   postMessage (postcard bytes)   ┌───────────────────────────┐
│ UI THREAD (odd-app)    │ ───── WorkerRequest ───────────► │ PHYSICS WORKER (odd-wasm) │
│ Leptos signals         │                                  │ odd-sim kernel            │
│ WebGL painted renderer │ ◄──── WorkerResponse ─────────── │ Dormand-Prince + spline   │
│ Canvas2D overlay       │   (Transferable ArrayBuffer)     │                           │
└────────────────────────┘                                  └───────────────────────────┘

    Requests (small): Propagate { design_id, timeline, until: Epoch, fidelity: L1|L2|L3 }, SolveNozzle { geometry, thermo }, Attribute { fdr }.
    Responses (bulk): the Hermite spline as a Structure-of-Arrays Float64 buffer [t₀..tₙ | rx₀..rxₙ | ry | rz | vx | vy | vz | m | subsystem channels...], sent as a transferable ArrayBuffer (zero-copy). The UI evaluates the cubic Hermite polynomial itself in O(1) while scrubbing.
    Nozzle solves (< 50 µs) run on the UI thread directly — no worker round trip, so the CAD handles feel instantaneous.

Boundary B: Browser ↔ Cloudflare Worker (HTTPS, postcard or JSON)

Only four endpoints exist (detailed in Part 6). Payloads are ≤ 4 KB.
Boundary C: Renderer ↔ GPU

web-sys::WebGl2RenderingContext with Float32Array::view() over wasm memory for instance buffers. Render is triggered only by the dirty_flag.rs governor (camera moved, lens changed, lighting epoch changed).
Part 4: Core Data Contracts (Type Signatures Only)

These are the schemas the AI agent must implement verbatim in the named crates. Each carries a SCHEMA_VERSION constant; canonical hashing includes the version.
4.1 odd-core — Time, Frames, Coordinates

Rust

/// Barycentric Dynamical Time since J2000.0 (JD 2451545.0 TDB). Split integer/fraction.
pub struct Epoch { pub tdb_s: i64, pub frac: f64 }          // frac ∈ [0, 1)

/// 4-tier reference frame tree (NASA SPICE style).
pub enum FrameId {
    Galactic,                                 // ICRS, origin = Solar System Barycenter
    StellarSystem { star: StarId },           // origin = that system's barycenter
    BodyCentered  { body: BodyId },           // origin = body center, inertial axes (J2000)
    BodyFixed     { body: BodyId },           // rotates with body (launch sites, surface ops)
    Camera,                                   // f32 render frame, origin at focal point
}

pub struct GalacticPos { pub x_mpc: i64, pub y_mpc: i64, pub z_mpc: i64 } // milliparsecs
pub struct StateVector { pub frame: FrameId, pub epoch: Epoch,
                         pub r: [f64; 3], pub v: [f64; 3] }               // m, m/s

4.2 odd-ephem — Bodies

Rust

pub struct BodyPhysical {
    pub id: BodyId, pub parent: Option<BodyId>,
    pub mu: f64, pub r_eq: f64, pub r_polar: f64, pub j2: f64,
    pub rotation_rate: f64, pub obliquity: f64, pub prime_meridian_j2000: f64,
    pub atmosphere: Option<AtmosphereModel>,   // USSA76 for Earth; None for Moon
    pub source: &'static str,                  // "JPL Horizons 2024-xx-xx" (mandatory citation)
}
pub fn body_state(id: BodyId, t: Epoch) -> StateVector;   // pure, < 1 µs, no caching needed

4.3 odd-astro — Elements, Spline, Fidelity

Rust

pub struct OrbitalElements { pub a: f64, pub e: f64, pub i: f64, pub raan: f64,
                             pub argp: f64, pub nu: f64, pub mu: f64, pub epoch: Epoch }
pub enum Fidelity { L1Analytic, L2TwoBodyNumeric, L3Perturbed(PerturbationSet) }
pub struct PerturbationSet { pub j2: bool, pub third_body: Vec<BodyId>,
                             pub drag: bool, pub srp: bool, pub finite_burn: bool }
pub struct IntegratorConfig { pub rel_tol: f64, pub abs_tol: f64, pub max_step_s: f64 } // fixed per contract

/// SoA Hermite keyframe cache. Evaluate in O(1) by binary-searching `t`.
pub struct TrajectorySpline { pub frame: FrameId, pub n: usize, pub channels: u8,
                              pub data: Vec<f64> }    // layout: [t | rx | ry | rz | vx | vy | vz | m | ch...]

4.4 odd-propulsion — Engine Geometry & Thermo (Block 1 = thermal nozzle; graph generalizes later)

Rust

pub struct NozzleGeometry { pub r_throat: f64, pub r_exit: f64, pub chamber_len: f64,
                            pub contour: Contour /* Conical{half_angle} | Bell{rao_pct} */ }
pub struct ChamberThermo  { pub propellant: PropellantPair, pub of_ratio: f64, pub p_c: f64 }
pub struct EngineSolution { pub t_c: f64, pub gamma: f64, pub m_bar: f64,
                            pub mdot: f64, pub v_e: f64, pub p_e: f64, pub mach_e: f64,
                            pub thrust_vac: f64, pub isp_vac: f64,
                            pub separation_x: Option<f64>,       // Some(x) if Summerfield criterion violated
                            pub q_throat: f64, pub dry_mass: f64 }
pub fn solve_engine(geom: &NozzleGeometry, th: &ChamberThermo, p_amb: f64) -> EngineSolution;

/// Day-1 generalization so Block 4 needs no rewrite:
pub enum EnergyNode { Chemical(ChamberThermo), Fission{..}, Fusion{..}, BeamReceiver{..}, Solar{..} }
pub enum ConversionNode { ThermalNozzle(NozzleGeometry), Electrostatic{..}, Electromagnetic{..}, Photon{..}, Radiator{area_m2, emissivity} }
pub struct EngineGraph { pub nodes: Vec<GraphNode>, pub edges: Vec<(u16, u16)> }  // DAG, energy & momentum conserved

4.5 odd-sim — Spacecraft Design & Command Timeline (The Replayable Inputs)

Rust

pub struct SpacecraftDesign {
    pub schema: u16,
    pub stages: Vec<Stage>,          // each: tanks, engines (EngineGraph), dry structure, separation rule
    pub power: PowerSubsystem,       // solar area, cell η, battery Wh, bus topology
    pub thermal: ThermalSubsystem,   // radiator area/ε, MLI layers, cryocooler W
    pub attitude: AttitudeSubsystem, // wheel H_max, RCS thrusters, cp–cm offset
    pub avionics: Avionics,          // mass, power draw, rad-hard tier
    pub delegated: DelegationFlags,  // which subsystems the AI Chief sized (+35% dry mass, +30% cost, Seff cap 0.65)
}

/// All fields are integers → bit-identical replay guaranteed.
pub struct Command {
    pub t_us: i64,                   // µs after mission T0
    pub kind: CommandKind,
}
pub enum CommandKind {
    Burn    { stage: u8, engine: u8, throttle_u16: u16, dur_us: i64, quat_i16: [i16; 4] },
    Attitude{ mode: AttitudeMode, quat_i16: [i16; 4] },
    Separate{ stage: u8 },
    PowerRoute { bus_from: u8, bus_to: u8, on: bool },
    Vent    { tank: u8, kg_x1000: u32 },
    Fdir    { program_hash: [u8; 32] },   // visual flight-rule state machine (Block 2)
}
pub struct CommandTimeline { pub t0: Epoch, pub cmds: Vec<Command> }

pub struct PredictionBaseline {       // locked BEFORE flight
    pub delta_v_mps: f64, pub prop_margin_kg: f64, pub tof_s: f64,
    pub final_elements: OrbitalElements, pub closest_approach_m: Option<f64>,
}

pub struct MissionOutcome { pub final_state: StateVector, pub prop_remaining_kg: f64,
                            pub objectives: Vec<ObjectiveResult>, pub events: Vec<SimEvent>,
                            pub state_hash: [u8; 32] }
pub fn run_mission(c: &Contract, d: &SpacecraftDesign, tl: &CommandTimeline,
                   cfg: &IntegratorConfig) -> MissionOutcome;

4.6 odd-contracts — Contract, Score, Elo

Rust

pub struct Contract { pub id: u32, pub schema: u16, pub title: String,
                      pub initial_state: StateVector, pub objectives: Vec<Objective>,
                      pub constraints: Constraints,          // budget, max g, max heat flux, deadline
                      pub integrator: IntegratorConfig,      // canonical; player cannot alter
                      pub perturbations: PerturbationSet,
                      pub pareto_baseline: ParetoBaseline,   // theoretical ideal IMEO, Δv, cost
                      pub rating: EloRating }                // the mission's own Elo
pub struct MissionScore { pub objectives: f64, pub prediction: f64, pub efficiency: f64, pub s: f64 } // s ∈ [0,1]
pub fn score(c: &Contract, o: &MissionOutcome, p: &PredictionBaseline, d: &SpacecraftDesign) -> MissionScore;
pub fn elo_update(r_player: f64, r_mission: f64, s: f64, k: f64) -> (f64, f64);

Part 5: The Flight License (Identity) — Fully in Rust
5.1 Key Derivation Chain

text

getrandom (WebCrypto CSPRNG) ──► 128-bit entropy
        │
        ├──► + 4-bit SHA-256 checksum = 132 bits ──► 12 × 11-bit indices ──► 12 words (aerospace wordlist)
        │                                           (identical structure to BIP-39, custom wordlist)
        ├──► hex display: ODD-4F9A-82C1-00E4-77B2-91D3-A6C0-19FE-3B47   (same 128 bits)
        │
        └──► HKDF-SHA512(ikm = entropy, salt = "odderon-flight-license", info = "ed25519-v1")
                     ──► 32-byte seed ──► Ed25519 keypair
                                  │
                                  └──► Public key (32 B) ──► Callsign ID = "ODD-" + base32(pk[0..10])
                                                            (public, printable on badges; never the secret)

Wordlist requirements (tool-enforced): exactly 2048 words, all lowercase ASCII, 4–9 letters, unique first 4 letters (so apog autocompletes to apogee), no two words within Levenshtein distance 1, sourced from astronautics/astronomy/thermodynamics vocabulary. Every word in the list is a term the player will encounter in the game — the passphrase itself is a tiny vocabulary lesson.
5.2 Storage & Login (No Server Involved)

    Private seed stored in IndexedDB as a non-extractable blob; optional 6-digit PIN encrypts it with AES-GCM (key via PBKDF2 through web-sys WebCrypto — the one place native WebCrypto is used because it's constant-time and free).
    Login on a new device = type 12 words / paste hex key / drop the .odderon badge file. Keypair is re-derived locally in < 1 ms. No network request is needed to log in — only to sync.
    The badge file is a PNG with the hex key in a tEXt chunk, generated in Rust (png crate), styled as a NASA-style ID card.

5.3 Callsign Registration (One of the Four Server Endpoints)

POST /callsign { name, pubkey, signature(name) } → first-come-first-served, stored in D1 (callsign PRIMARY KEY, pubkey, created_at). No email. Lose your 12 words = lose your callsign (stated plainly on the export screen, Mullvad-style).
Part 6: Proof-of-Trajectory — The Verification Pipeline
6.1 The Flight Data Recorder Packet (≈ 2–4 KB)

Rust

pub struct Fdr {
    pub schema: u16,
    pub kernel_hash: [u8; 32],         // SHA-256 of the exact odd-sim wasm build that produced it
    pub contract_id: u32,
    pub attempt_id: [u8; 16],          // issued by server at LOCK & LAUNCH (see 6.3)
    pub design: SpacecraftDesign,
    pub prediction: PredictionBaseline,
    pub timeline: CommandTimeline,
    pub claimed: MissionOutcomeSummary,  // what the client says happened (hash + key numbers)
    pub segments: Vec<SegmentCheckpoint>,// every ~30 sim-days: { t_us, state_hash } — see 6.4
    pub pubkey: [u8; 32],
    pub signature: [u8; 64],           // Ed25519 over canonical postcard bytes of all fields above
}

6.2 What Each Cheat Vector Hits
Cheat attempted in Chrome DevTools	Why it fails
Edit isp = 9999 mid-flight	Verifier recomputes engine performance from geometry/thermo (No-Magic-Lookup Law); replay runs out of fuel; claimed.state_hash ≠ recomputed → rejected
Add fuel mass in memory	Tank mass is derived from tank geometry × propellant density in the design; replay diverges
Edit the contract (easier target)	contract_id resolves to the server-canonical Contract; client copy is ignored
Write predictions after seeing results	Prediction hash was committed at LOCK & LAUNCH before any physics ran (6.3)
Submit a friend's FDR	Signature won't verify against submitter's pubkey
Replay a 3-year mission with a tampered integrator tolerance	IntegratorConfig is part of the contract, not the FDR
Hand-craft an impossible timeline	Timeline is just inputs; physics decides the outcome
Time-warp / pause to think	Not a cheat — simulation time is virtual. Only design + commands matter
6.3 The Four Endpoints (odd-worker, Cloudflare Workers, Rust)

text

POST /callsign   { name, pubkey, sig }                      → { ok }
POST /commit     { contract_id, pubkey, H(design‖prediction), sig } → { attempt_id, server_time, server_sig }
POST /submit     { fdr }                                     → { verdict, score, elo_before, elo_after, receipt_sig }
GET  /verify/{fdr_hash}                                      → { fdr (from R2), verdict, receipt }  (public, QR target)
GET  /leaderboard?contract=402&page=1                        → [ { callsign, elo, score, fdr_hash } ]

Ranked vs. practice (Certified Engineer Mode): the first /submit per (pubkey, contract_id) updates Elo. Subsequent attempts are stored and replayable but unranked — exactly how Lichess puzzle ratings work. No "revert and retry for free" on ranked missions.
6.4 Honest Note on Free-Tier CPU Limits (And the $0 Solution)

Cloudflare Workers' free tier allows ~10 ms CPU per request. A cislunar mission replays in < 5 ms, but a 3-year low-thrust Jupiter mission may need 50–200 ms. Two options, both already accommodated by the schema:

    $0 path — Chunked Hash-Chain Verification: the FDR carries segments (state hash every ~30 sim-days). The client calls /submit once per segment (?seg=i); the worker replays only segment i from checkpoint i−1 to i (≤ 8 ms), stores the intermediate verdict in D1, and the final call aggregates. Any tampered segment breaks the chain.
    $5/month path: Workers Paid raises the limit to 30 s CPU per request; single-call verification. Flip one config flag.

6.5 Verifiable Credentials (The QR Code on the PDF)

odd-report embeds https://odderon.space/v/<fdr_hash> as a QR. That page is a static Leptos page that loads the same odd-sim wasm, fetches the FDR from R2, re-runs the replay in the recruiter's own browser, checks the Ed25519 signature and the server receipt, and renders the interactive trajectory + nozzle schematic. The recruiter doesn't need to trust Odderon's server at all — the math is checkable locally.
6.6 Elo Mechanics (As You Specified, Formalized)

    Player expected score: E=11+10(Rm−Rp)/400E=1+10(Rm​−Rp​)/4001​
    Update: Rp′=Rp+Kp(S−E)Rp′​=Rp​+Kp​(S−E), with Kp=40Kp​=40 for the first 20 ranked missions, then 20; Rm′=Rm+Km(E−S)Rm′​=Rm​+Km​(E−S), Km=8Km​=8 (missions drift slowly toward their true difficulty).
    S=0.50 Sobj+0.25 Spred+0.25 SeffS=0.50Sobj​+0.25Spred​+0.25Seff​; SpredSpred​ uses the exponential closeness formula from Block 1 WP 1.8; SeffSeff​ compares against pareto_baseline. Delegating a subsystem to the AI Chief caps SeffSeff​ for that discipline at 0.65 (the 35% margin penalty becomes a visible, honest scoring cap).

Part 7: Updated Block 1 Work Package List

The identity update adds two packages. Final Block 1 sequence (each gated by its Oracle test):
WP	Crate(s)	Oracle Gate
0.1	odd-core	8,000 LY round-trip frame error < 1 nm; Epoch arithmetic exact
0.2	odd-ephem, tools/pack-ephem	Earth–Moon state at 2026-01-01 TDB within 0.1% of Horizons
0.3	odd-app/render	0 FPS / < 0.2% CPU after 5 s idle
0.4 (new)	odd-identity	Known 128-bit test vector → fixed 12 words → fixed pubkey; sign/verify roundtrip; wordlist validator passes
1.1	odd-astro/elements,level1	Vallado Ex. 2-5 within 0.001%
1.2	odd-astro/integrator,spline	SSO precession +0.9856°/day; energy drift < 1e-9 J/kg; native vs wasm state hash identical
1.3	odd-app/lens	No unexplained acronym renders without hover projection (snapshot test)
1.4	odd-astro/attribution, odd-app/debrief	Finite-burn loss isolated within 1%
1.5	odd-propulsion	RS-25: Isp_vac 452 ± 8 s, Isp_sl 366 ± 10 s — SHIPPED, see docs/propulsion.md
1.6	odd-systems	Known LH₂ tank: boil-off rate vs. published MLI heat-leak data within 15% — SHIPPED, see docs/thermal.md (card's 1.5%/day, 60-min orbit and COP 0.1 corrected; instantaneous two-tank bus priority in a coupled integrator, radiator environmental load + approach floor kill the free-cooling collapse; oracle_instantaneous_cryo_bus_priority and the multi-day solvability gate pass)
1.7	odd-contracts/chief_ai	Delegated sizing reproduces textbook margins deterministically — SHIPPED and WIRED: a set bit SUBSTITUTES the chief's textbook hardware (oracle_chief_hardware_substitution), locked read-only sliders show the chief's values, margin charged to the flown dry mass, +30% cost, Seff cap 0.65, mask signed and the deviation rejected on replay; the thermal chief now cools a cryogenic coast competently (docs/thermal.md)
1.8	odd-contracts, odd-report	Elo update matches hand calculation; PDF opens & QR decodes
1.9 (new)	odd-sim/verify, odd-worker	Server-authoritative Astro-Elo — CORE SHIPPED: pure AuthorityEngine + Store port, canonical Elo (parity-gated to the client), Ed25519 server receipts, local HTTP/SQLite adapter exercised in gate.sh with no cloud creds, Cloudflare D1/R2 adapter (sibling crate, CI-built). Oracles pass: anti-hindsight commitment, first-attempt-only ranked, chunked segment parity, DevTools localStorage tamper defeated (receipt layer). Registry/leaderboard/public-verify UI is Slice 3. See docs/server-authority.md
Part 8: Paste-Ready Task Card — Work Package 0.1

Save this as WORK_PACKAGES/WP-0.1.md and give the agent only this file plus ARCHITECTURE_LAWS.md.

    Work Package 0.1 — odd-core: Epoch, Units, Hierarchical Frames, Canonical Hashing

    Scope: Create the Cargo workspace per the folder structure. Implement crate odd-core only. It must compile for both x86_64 (tests) and wasm32-unknown-unknown, with no dependencies other than libm, postcard, sha2, serde (derive only). No I/O, no std::time, no HashMap.

    Deliverables:

        units.rs — newtype wrappers for SI quantities with From/arithmetic traits; display-layer conversions live in a separate fmt module and are the only place non-SI units appear.
        epoch.rs — Epoch { tdb_s: i64, frac: f64 }; add/sub seconds; difference returns f64 seconds; constructors from Julian Date (TDB) and from calendar UTC via a documented leap-second table (cite IERS Bulletin C). Normalize so frac ∈ [0,1) always.
        frames.rs — FrameId enum and GalacticPos in i64 milliparsecs. Implement transform(state, from, to, epoch) through the common ancestor of the frame tree. Camera frame output is f32.
        canon.rs — canonical_bytes<T: Serialize>(&T) -> Vec<u8> using postcard; canonical_hash = SHA-256 of [schema_version_le_u16 ‖ bytes].
        consts.rs — every constant has a doc comment with source and date (CODATA 2018 / IAU 2015 / DE440).

    Forbidden: f32 anywhere except Camera frame output; f64::sin/cos/... (use libm); any single global f64 position; Date.now()/SystemTime.

    Oracle Gate (write these tests FIRST; do not proceed until green):

        oracle_8000ly_docking: two points 0.5 m apart in a system at 2,452,809 pc; round-trip Galactic → StellarSystem → BodyCentered → Camera → back; relative distance error < 1e-9 m.
        oracle_epoch_j2000: Epoch::from_jd_tdb(2451545.0) == {0, 0.0}; adding 86400.0 s × 36525 lands exactly on tdb_s = 3_155_760_000.
        oracle_canon_stable: hashing the same struct twice yields identical bytes; changing one f64 by 1 ULP changes the hash.
        oracle_wasm_parity: the three tests above pass identically under wasm-pack test --headless --chrome.

    Definition of Done: cargo test -p odd-core green, cargo build -p odd-core --target wasm32-unknown-unknown green, cargo clippy -- -D warnings clean, no TODO/unimplemented! in the crate.

