# Flight Data Recorder — schema changelog

The FDR is a **fixed-layout canonical byte string**, not JSON. JSON is only a
transport: two documents differing in whitespace, key order or number
formatting describe the same flight and must produce the same signature.
Floats are raw IEEE-754 big-endian bits; every NaN collapses to one canonical
quiet NaN; signed zero is preserved.

Each schema has its own signing domain, so a signature from one schema can
never be replayed as another.

| Schema | Signing domain | Status |
|---|---|---|
| 1 | `odderon-flight-data-recorder-v1\0` | legacy, verifies, engine uncertified |
| 2 | `odderon-flight-data-recorder-v2\0` | verifies, engine certified, no tanks |
| 3 | `odderon-flight-data-recorder-v3\0` | **current, FROZEN** |

---

## v3 — cryogenic tanks and Chief-AI provenance (FROZEN)

**Frozen before any `odd-worker` code was written.** WP 1.9 and every later
work package build against this layout. No field may be added, removed or
reordered without a new schema version and a migration note here.

Everything from v2 is unchanged. The v3 block is **appended after
`elo_after`**, so every historical byte offset keeps its meaning.

### Added to the signed bytes, in order

| Field | Width | Notes |
|---|---|---|
| `delegation_mask` | u8 | bit 0 Propulsion, 1 Thermal, 2 Power, 3 GNC |
| `thermal_specs.len` | u16 | max 16 |
| per stage: `stage` | u8 | |
| `tank_volumes_m3` | 2 × f64 | oxidizer then fuel |
| `propellant_id` | u8 | same frozen wire ids as the engine block |
| `mli_layers` | u32 | |
| `epsilon_eff` | f64 | blanket emittance |
| `tank_shell_material` | u8 | 0 Al-2219, 1 304 stainless, 2 composite |
| `absorptivity` | f64 | outer-skin solar absorptivity |
| `outer_epsilon` | f64 | outer-skin IR emittance |
| `strut_conductance_w_k` | f64 | |
| `relief_pressure_pa` | f64 | |
| `radiator_area_m2` | f64 | |
| `radiator_epsilon` | f64 | |
| `cryocooler_w` | f64 | commanded lift at the cold tip |
| `cryocooler_carnot_fraction` | f64 | in (0, 1] |
| `array_w_at_1au` | f64 | |
| `avionics_w` | f64 | |
| `electric_propulsion_w` | f64 | |
| `structure_temperature_k` | f64 | warm end of the struts |
| `sink_temperature_k` | f64 | |
| `boil_off_kg` | 2 × f64 | integrated loss, oxidizer then fuel |
| `cryocooler_energy_j` | f64 | bus joules drawn over the flight |
| `vent_events.len` | u16 | max 256 |
| per event: `time_us` | i64 | exact microsecond of the opening |
| `tank` | u8 | 0 oxidizer, 1 fuel |
| `pressure_pa` | f64 | |
| `temperature_k` | f64 | |

### Two deliberate departures from the work-package card

**1. `cryocooler_cop` is not signed; `cryocooler_carnot_fraction` is.**
A COP is a *derived performance number*. Trusting one from the client is
exactly what the no-magic-lookup law forbids — and the card's own example,
COP ≈ 0.1 between 20 K and 300 K, exceeds the Carnot ceiling of 0.07143 by
40 %. The signed field is the machine's Carnot fraction; the verifier
recomputes `COP = f·T_c/(T_h − T_c)` with `T_h` from the radiator balance. The
dossier prints the resulting COP for readability.

**2. `boil_off_kg` and `cryocooler_energy_j` are signed alongside the events.**
The card asked only for `vent_events`. That turned out to be insufficient, and
the anti-tamper oracle proved it: a tank starting at its relief setting opens
at t = 0 and stays open, so downgrading the blanket from 20 layers to 4
produced **the same two openings** while losing far more propellant. A
re-signed downgrade passed signature *and* vent-list comparison. Signing the
integrated magnitude closes that hole; the oracle now rejects it with
`PhysicsReplayMismatch { field: "boil_off_kg" }`.

This correction was made **before** the freeze was declared, not after.

### Validation added

Rejected by name, before any replay runs: delegation bits outside `0b1111`;
unknown shell material or propellant id; emittance/absorptivity/epsilon out of
range; Carnot fraction outside (0, 1]; non-finite or negative thermal values;
non-positive relief pressure or temperatures; vent events out of time order
or beyond the flight window; tank id ≥ 2; more than 256 events; a pre-v3
record carrying a thermal block.

### Verification order

1. **Signature** over canonical bytes rebuilt from typed fields.
2. **`certify_engine`** — recompute thrust and Isp from the physical design.
3. **`certify_contract`** — the framing conditions are canonical constants.
4. **Level-3 replay** through the same `compute` the live flight used.
5. **`compare_metrics`** and **`compare_final_state`**, bit for bit.
6. **`compare_vent_events`** — timestamps to the microsecond, states to the bit.
7. **`compare_boil_off`** — integrated loss and bus energy, to the bit.

### Backward compatibility

Schema 1 and 2 records still parse and replay offline. They carry no thermal
block, so their replay produces no vent events or boil-off, and steps 6–7 are
skipped. New server submissions require schema 3 in both Ranked and Practice
modes; this is an authority rule, not a schema change.

### Verification-rule addendum (2026-11-14, no schema change)

The v3 byte layout is unchanged and remains frozen. One **verification rule**
was tightened while closing the acceptance audit:

`certify_engine` now recomputes the stage's dry mass with
`Contract::vehicle_delegated(spec, DelegationMask(fdr.delegation_mask), design)`,
where `design` is rebuilt from the signed `StageThermalSpec`. Cryogenic
fit-out beyond the contract baseline (blanket layers, shell, relief pressure,
radiator panels, solar-array output) and every delegated discipline's +35 %
margin are real dry mass, so a record cannot claim the hardware, or the
cooling it buys, without carrying the kilograms.

For a manual stage with the baseline design the computed mass is
**bit-identical** to the previous `contract.vehicle(spec)`, so no existing
certificate is invalidated. Pre-v3 records have no thermal block and are
recomputed with `ThermalDesign::baseline()`, which is also unchanged.

Guard: `crates/odd-verify/tests/thermal_fitout_anti_cheat.rs`
(`oracle_cooling_cannot_be_claimed_without_its_mass`).

A subsequent fail-closed verification rule requires one stage-0 thermal
block for a v3 cryogenic engine, with the same propellant and bit-exact
volumes derived from the engine and load; storables must have no block.
Invalid tank ranges, bus powers or thermal integration fail the replay rather
than becoming zero loss. This deliberately rejects previously accepted
malformed v3 records, even if the pilot signed them. The frozen byte layout
and valid historical v1/v2 offline replay are unchanged. See
`resigned_cryogenic_records_cannot_omit_or_invalidate_the_tank_model`.

The launch commitment now uses `odderon-design-hash-v2`, adding the ordered
maneuver plan and guidance to the committed vehicle. It does not alter FDR
canonical bytes; v1 launch tickets cannot be used for a new submission or retry
under the new rule. See [server authority](server-authority.md) for the
upgrade and hindsight limits.

A later validation hardening fixes the cryogenic environment to the playable
contract baseline: the signed structure temperature must be 293 K and the
radiative sink 3 K. The modeled saturation temperature at the relief setting
must also be finite, positive and strictly below that fluid's critical
temperature. These checks close fictitious cold-support and supercritical
liquid claims, without changing v3 bytes or normal baseline/card calculations.
They **can reject previously accepted v3 records**, including custom records
with non-baseline boundary temperatures or relief points outside the model;
a previously signed receipt does not make such a record pass the new replay
rules. Coordinate browser and authority upgrades and retain an appropriate
historical verifier if those prior records must remain independently replayable.
Schemas 1/2 have no thermal block and remain readable offline.

JSON transport now preserves negative zero, which was already significant to
the frozen signature and was previously lost when JavaScript re-serialized a
recorder. Noncanonical base64url tails of length 1 modulo 4 are rejected;
valid `#verify=` fragments and bare payloads are unchanged.

---

## v2 — physical engine design

Replaced the bare `thrust_n` / `specific_impulse_s` claims with a six-field
physical design: propellant id, O/F, chamber pressure, throat radius, exit
radius, bell length fraction. The derived floats remain in the record for
readability but are never trusted; `certify_engine` recomputes them.

## v1 — initial signed recorder

Callsign, epoch, duration, masses, initial state, locked prediction, command
timeline, 58 metrics, terminal-state hash and the Elo change, signed with
Ed25519. Thrust and Isp were recorded as claims, which is why v1 records are
labelled legacy.
