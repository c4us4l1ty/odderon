# Odderon — the class-certification gate

**Status: binding, and enforced by a checker rather than by this file.**

`docs/DATA-LICENSING.md` says what we may ship. This document says **what the compiled game is allowed to
contain at all** — and, unlike every other document in `docs/`, it is not a statement of intent that a
reviewer has to remember. It is a list a program reads. `tools/check-classes.mjs` fails the build if the
list and the code disagree in either direction.

---

## Why a second gate, when the licence gate already exists

`DATA-LICENSING.md` answers *"may these bytes be redistributed?"* That question only becomes live when a
catalogue file exists. It does **not** answer the question the user actually made a Day-1 requirement:

> "100% commercially legal from Day 1."

The gap is this: a compiled Rust kernel does not need a data file to carry somebody's rights. If a star
table, or an orbital-element table derived from a restricted source, is written into a `const` array in
`crates/`, then the binary ships it, and there is no `data/` directory for the manifest checker to
inspect. The licence gate would pass and the game would still be shipping CC BY-NC data inside its
executable.

So the two gates are complements:

| Gate | Question | Enforced by | Fails on |
| --- | --- | --- | --- |
| `DATA-LICENSING.md` §3.2 | May these **files** ship? | `data/MANIFEST.json` checks | an orphan data file; a non-redistributable dataset in a release build |
| **this document** | May the **kernel** contain this at all? | `tools/check-classes.mjs` | a class marked `forbidden` appearing in `crates/`; a class marked `must-exist` disappearing |

---

## 1. The taxonomy, and the one thing it is for

The taxonomy is `OPEN` / `GATED`. **Not** "free" / "expensive". The distinction is not price, it is
**whether the licence of the record is ours to give**:

- **`OPEN`** — the record's licence permits redistribution in a commercial product, and can be attributed.
  It may live in `crates/` as compiled data or arrive through the `Survey` seam. Tier 0 in
  `DATA-LICENSING.md`.
- **`GATED`** — the record's licence does **not** permit us to redistribute it. Tier 1. It may exist only
  behind a seam that a *player* populates with their own access. It may never be a `const` in the kernel,
  and it may never be the shipped default.

There is no third class, and there is deliberately no "probably fine" class. `DATA-LICENSING.md` §1 is the
reason: the intuition "pre-Gaia is safe" is false — Hipparcos and Tycho are *also* CC BY-NC 3.0 IGO — so a
class whose justification is a hunch would be worse than no class at all. If a record cannot be classified
from a source we have read, it does not ship.

### 1.1 The classifier is a person, and is named

`commercial_ok` is a legal conclusion, not a computed value. `tools/check-classes.mjs` enforces **the
consistency of the classification**, not its correctness — it cannot tell whether a lawyer would agree.
What it can do is make an unclassified record unwritable, and make a silent downgrade impossible. The name
of whoever classified a class is recorded in the list and printed by the checker.

---

## 2. The rules that are checked

### R1 — Every class is classified, once

Every record must belong to exactly one class in `src/classes.json`. The checker's input is the class list
plus the kernel tree; a dataset mentioned in the two licensing documents but absent from the list is a
finding. That is the failure mode this exists for: `DATA-LICENSING.md` describes the tier system in prose
and no program has ever read it, so a new dataset could be added to the game and the tier system would not
notice.

### R2 — `GATED` records cannot appear in the kernel

A class marked `GATED` with `kernel_const_expressible: true` is a contradiction and fails immediately: it
says a non-redistributable record is compiled into the shipped binary. Every `GATED` record must have
`kernel_const_expressible: false`, and no `GATED` record may appear as a Rust identifier in `crates/`.

The identifier check is a **name** check, not a data check: it cannot prove a `const` array is a star
catalogue. It proves that the one seam we rely on for gated data is not bypassed by someone typing the
source's name into a struct. That is a smaller claim than "we are compliant", and it is the largest claim a
grep can honestly make. The rest of the compliance argument lives in the seam itself
(`DATA-LICENSING.md` §3.3): gated data arrives through a trait the player implements, and the shipped
binary has no implementation of it.

### R3 — The parts that must exist are named

A build that satisfies every prohibition by containing nothing is not a pass, it is a missing feature. So
each `OPEN` class declares whether it *must* exist yet (`state.must_exist`), and the checker fails both
ways:

- `must_exist: true` but nothing implements it → **fail**, "the class is promised and absent".
- `must_exist: false` but something implements it → **fail**, "the class appeared without being
  classified", which is exactly how an unvetted source gets into a build.

This is the rule that makes the list load-bearing instead of decorative. It is also why the list has a
`state` field with three values rather than a boolean: `present`, `seam_only`, `absent` are different
claims, and collapsing them loses the one distinction that matters (`seam_only` means "the player supplies
the rows, we supply the type" — which is a *feature*, not a gap).

### R4 — Bodies are open; star records are not, yet

The kernel currently ships **opaque-body constants only** — `mu`, equatorial radius, `J2`, rotation rate,
flattening, for four bodies. Those come from JPL/IAU and are class `OPEN` with attribution. No star
position, no orbital element table for the planets, and no ephemeris is compiled in. This is not a
placeholder in the list; it is the state of `crates/odderon-astro/src/catalogue.rs`, and R3 makes the
distinction between "we have not built it" and "we decided not to" visible in the diff.

---

## 3. What the checker cannot do, stated plainly

The honest boundary matters more here than anywhere else in this repository, because "compliance" is the
one claim that can cost real money if it is wrong.

1. **It cannot verify a licence.** It compares a list to a tree. Whether `OPEN` is the right answer for a
   given record is a reading of a terms page, recorded with a URL and a date, and re-read by a person.
2. **It cannot see data that arrived as a literal array with a neutral name.** A star table called
   `TABLE` is invisible to a name check. The defence is not the checker; it is `DATA-LICENSING.md` §3.3 —
   gated data has no reason to be in the kernel because the seam exists.
3. **It cannot make the build release-safe on its own.** A release build must also refuse to *load* a
   non-redistributable dataset at runtime (`DATA-LICENSING.md` §3.2 rule 2). That check belongs to the
   loader, and the loader does not exist yet. This document says so rather than implying otherwise.

---

## 4. The records, as shipped

The table below is generated by `tools/check-classes.mjs --table`, so it cannot drift from the list the
build actually reads: the checker regenerates this block and fails on any difference. A stale table is a
failing build, not a stale document.

The `commercial` column is the verdict a reader is actually looking for; `licence_id` and the notes are in
`src/classes.json` beside it, where the checker can read them.

<!-- BEGIN GENERATED: classes -->
| id | class | state | source | commercial |
| --- | --- | --- | --- | --- |
| `atmosphere-1976` | OPEN | absent | US Government | yes |
| `body-constants` | OPEN | present | IAU Working Group on Cartographic Coordinates; JPL/EGM | yes |
| `deep-space-ephemeris` | OPEN | absent | JPL / NAIF | yes-with-indemnity |
| `exoplanets` | OPEN | absent | NASA/IPAC / NASA Exoplanet Archive | needs-authorisation |
| `gaia-dr3` | GATED | absent | ESA / DPAC / ESA / CDS | needs-authorisation |
| `hipparcos-tycho-esa` | GATED | absent | ESA / ESA / CDS / VizieR | needs-authorisation |
| `ir-stars-2mass` | OPEN | absent | UMass/IPAC / IPAC; NASA Open Data | yes |
| `optical-stars-panstarrs` | OPEN | absent | Pan-STARRS consortium / STScI / MAST | yes |
| `optical-stars-tycho2-adc` | OPEN | absent | ESA / NASA ADC | yes |
| `planetary-surfaces` | OPEN | absent | NASA / USGS Astrogeology | yes |
| `small-bodies` | OPEN | absent | JPL / JPL SSD | yes |
| `thermal-network` | OPEN | present | derived: first-principles energy balance + Copeland COP correlation / none: no data is shipped | yes |
| `thermo-cea` | OPEN | absent | NASA | yes |
| `vizier-mirrors` | GATED | absent | various / CDS / VizieR | needs-authorisation |
<!-- END GENERATED: classes -->
