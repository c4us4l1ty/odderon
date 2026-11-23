# Odderon — Data licensing: the shipping gate

**Status: binding. Nothing ships with data whose terms we have not read.**

This is a gate, not a footnote (`CREDIBILITY.md`, Appendix B). It exists because Odderon's entire
premise is "a real universe, never invented" — which means we depend on other people's catalogues, and
a commercial release (Steam, mobile, per `IMPORTANT-1.md`) changes what we are allowed to do with them.

The rule that follows from that: **licensing is an architecture problem, not a paperwork problem.** If
the catalogue is a swappable backend with a provenance manifest, then a licence surprise is a data
swap; if it is baked into the engine, a licence surprise is a rewrite.

---

## 1. The trap: "pre-Gaia is commercial-safe" is false

The intuitive move is *Gaia is CC BY-NC, so ship the pre-Gaia catalogues instead*. That is wrong, and
it is wrong in the most expensive way — it looks right until a lawyer reads it.

**Hipparcos and Tycho are distributed by ESA under the same CC BY-NC 3.0 IGO licence as Gaia.** From
ESA's own catalogue page:

> "The Hipparcos and Tycho Catalogues are distributed under the CC BY-NC 3.0 IGO licence and the Terms
> and Conditions for the use of these data require users to acknowledge ESA using 'Credit: ESA'."

So the hazard is **not "Gaia"**. The hazard is **"produced by ESA and distributed by ESA/CDS"**.

The escape is narrower and more interesting: the *same* Tycho-2 tables, as mirrored by **NASA ADC**,
are published by NASA as a US government work (data.nasa.gov lists the Tycho-2 catalogue under
`usa.gov/government-works`). One dataset, two distributors, two incompatible licences — and the rights
you have depend on **which door the bytes came through**. That distinction must be recorded per file, in
the manifest, with the URL, or we cannot defend it later.

> **Lesson, in one line: licence follows the distributor, not the telescope.**

---

## 2. The verified stack

Verified 2026-11-22 against the sources named. This is the *verified* column, not the *believed* column,
and it is deliberately paranoid.

### Tier 0 — commercial-safe, planned to ship

| Data | Source | Rights position | Evidence |
| --- | --- | --- | --- |
| Planetary & lunar ephemerides | JPL **DE440 / DE441** via NAIF | **Not "public domain".** NAIF's rules page permits commercial use, **no fees or licensing required**, classifies the software TSPA for export, and provides it AS-IS with the recipient indemnifying Caltech and NASA. Permissive for our use; the indemnity clause needs a legal read before a paid release. | `naif.jpl.nasa.gov/naif/rules.html` |
| Asteroids & comets (~1.3 M) | JPL **Small-Body Database** | NASA/JPL, US government work | `ssd.jpl.nasa.gov` |
| Exoplanets (~6,000) | **NASA Exoplanet Archive** (IPAC/Caltech) | Open access; confirm the exact terms page and record it | `exoplanetarchive.ipac.caltech.edu` |
| Deep infrared stars | **2MASS** (IPAC) | Public domain per the project's own statement; NASA Open Data lists the PSC under `government-works` | `data.nasa.gov` |
| Wide-field optical stars | **Pan-STARRS1** via MAST | STScI grants a non-exclusive, royalty-free, worldwide right to use, reproduce and publicly display PS1 data — i.e. commercial use permitted with attribution. Note the underlying survey is consortium-owned, so quote the STScI grant verbatim in the manifest. | `registry.opendata.aws/mast-panstarrs` |
| Planetary surfaces | **USGS Astrogeology** (LOLA, MOLA, MESSENGER) | US government work | `astrogeology.usgs.gov` |
| Atmosphere | **US Standard Atmosphere 1976** | US government work | — |
| Thermo data | **NASA CEA / Gordon & McBride** | US government work | — |
| Tycho-2 *if needed* | **NASA ADC mirror only** | US government work **as mirrored by NASA ADC** — *not* the CDS/ESA copy | `data.nasa.gov` |

### Tier 1 — **not redistributable in a commercial build**

| Data | Source | Rights position |
| --- | --- | --- |
| Gaia DR3 (and EDR3/DR4) | ESA / DPAC / CDS | CC BY-NC 3.0 IGO. **Commercial use requires a separate arrangement with ESA.** |
| Hipparcos, Tycho-1/2 | **ESA / CDS / VizieR** | CC BY-NC 3.0 IGO — same hazard, easy to miss |
| Anything else mirrored "for convenience" | CDS / VizieR | Assume CC BY-NC until proven otherwise |

### Tier 2 — live queries, nothing redistributed

IVOA **TAP / VizieR / MAST** endpoints queried from the player's browser, with results held in session
memory and never baked into shipped content. This keeps Odderon a *client* rather than a
*redistributor*. It is a genuine legal distinction, but it is also **not load-bearing**: the offline
Tier-0 stack is sufficient for the game to exist, so a live-query feature is a garnish we can lose.

---

## 3. The architecture that makes this a non-event

Three commitments, all cheap, all worth doing before the first byte of catalogue data is fetched.

### 3.1 One manifest, one source of truth

`data/MANIFEST.json` — one row per dataset, and **no data file may exist without a row**:

```jsonc
{
  "id": "hipparcos-esa",
  "title": "Hipparcos Catalogue",
  "publisher": "ESA",
  "distributor": "ESA / CDS",
  "retrieved": "2026-11-22",
  "source_url": "https://www.cosmos.esa.int/web/hipparcos/catalogues",
  "licence_id": "CC-BY-NC-3.0-IGO",
  "licence_url": "https://creativecommons.org/licenses/by-nc/3.0/igo/",
  "commercial_ok": "needs-authorisation",
  "redistributable": false,
  "attribution": "Credit: ESA",
  "notes": "ESA distributes this under CC BY-NC 3.0 IGO. A NASA-ADC mirror of Tycho-2 is separately listed as a US government work; the mirror must be recorded explicitly if used."
}
```

`commercial_ok` is `yes` | `no` | `needs-authorisation`. `redistributable` is what the build reads.

**This gate is implemented, and the compiler of it is not the one you would guess.** `src/classes.json` is the
list, `tools/check-classes.mjs` is the checker, and it runs as a hard CI job on every push
(`docs/CLASSES.md`). It covers the case the manifest cannot: a record compiled into the kernel as a `const`,
which has no `data/` file for an orphan check to find. The manifest check below covers files the class gate
does not look at. Two gates, one question each.

### 3.2 CI enforcement, because a document is not a gate

Two checks, both mechanical:

1. **Orphan check** — every file under `data/` has a manifest row, and every row's `source_url` resolves
   to a recorded retrieval date. A data file with no provenance fails the build.
2. **Release-profile check** — a `--release` build fails if any *loaded* dataset has
   `redistributable: false`. Development and non-commercial builds may load Tier 1; the release build
   physically cannot. This is the same shape as `GRAPHICS.md` R1 (the zero-bitmap law): a property we
   can *lose* is a property nobody can quietly violate.

Attribution is **generated from the manifest** into the credits surface, so credit can never drift out
of sync with the data that requires it.

### 3.3 The catalogue is a *survey backend*, not a table

```
trait Survey {
  fn cone_search(&self, ra: Rad, dec: Rad, radius: Rad) -> StarTile;  // carries σ per field
  fn provenance(&self) -> &Prov;      // every field's uncertainty, as first-class data
}
```

`Survey` is the seam. Tier-0 backends ship; Tier-1 backends are loadable by a player who obtains them
himself (that is his licence, not ours). The engine never knows which one it is talking to — and because
every field carries a `Prov` node (`VISION.md` §3, law 2), **a star's distance uncertainty is a game
quantity, not a hidden metadata field.** Which is the point of §4.

---

## 4. The best consequence: accuracy becomes a resource instead of a freebie

This is where the licence problem pays for itself.

If Gaia DR3 were the shipped default, the player would start with all 4,000 light-years mapped to
micro-arcsecond precision. They would never have to do astrometry, and — worse — **the game would have
handed them knowledge for free, which Pillar 1 forbids**: the player is supposed to *earn* knowledge by
measuring. A licence restriction and a design principle happen to agree.

So the shipped baseline is the US-PD stack: precise directions (RA/Dec to the arcsecond, which is
genuinely known), brightness, colour, proper motion where available — and **real, large, honest distance
uncertainty along the line of sight** for anything beyond a few hundred parsecs. Distant stars are not
fogged out of the map; they are *there, at a known bearing, at an uncertain range*. That is exactly what
humanity had before 2013, and it is a far better starting condition for a game about exploration than
omniscience.

### 4.1 One correction to the long-baseline interferometry idea

The proposed mechanic — put a telescope at Saturn/Jupiter to multiply the parallax baseline and collapse
distance errors — is **scientifically sound in principle and does not work for the stars that need it**.
The arithmetic, because this is the sort of thing the game must not fudge:

- At 2,000 ly (≈ 613 pc) a star's parallax is ≈ 1.6 mas.
- Gaia's parallax error at the bright end is of order 0.02–0.03 mas, i.e. **~1–2 % distance error.** That
  is already excellent.
- A 10 AU baseline is ~5× Earth's 2 AU, so it buys a factor of 5 in *angle for a given distance*. To beat
  Gaia's 1–2 % you would need per-plate astrometric precision around **0.1 mas** from a small telescope —
  which is one to two orders of magnitude beyond what a small optical instrument can do.

**Baseline is not the binding constraint. Photon noise and baseline metrology are.** (This is why every
real µas astrometry mission — Hipparcos, FAME, SIM — needed space, huge apertures, or laser metrology,
not merely distance from the Sun.) Encoding "bigger baseline = more science points" would teach a
plausible falsehood, which is the one thing the project forbids.

**What interferometry and deep-space astrometry are genuinely for**, and what the mechanic should be:

1. **Completeness.** Gaia is magnitude-limited and dust-extincted; its completeness collapses beyond
   ~1–3 kpc and it is blind behind the galactic plane's dust. The game's own doctrine
   (`VISION.md` §4) makes that darkness a feature: *funding a survey is a real activity.* Your
   instrument goes where Gaia is blind, not where Gaia is good.
2. **Astrometric binaries and planets.** Astrometric *wobble* — not parallax — is how you find a
   companion, and it is measurable from a long baseline at any distance. That is the real science of a
   deep-space astrometry mission.
3. **Proper-motion refinement** over decades of game time, which decides whether a star is coming toward
   you or merely looks like it is.
4. **Sub-percent distances for the nearby stars you intend to visit** at 0.8c, where a 1 % error is a
   lightspeed-scale navigation error.

So the loop survives, and it is better: **you do not beat Gaia on Gaia's stars — you go where Gaia cannot
see, and you measure what parallax alone cannot tell you.** The player still starts with uncertainty and
still collapses it with instruments; the instruments are just pointed at the right problem.
