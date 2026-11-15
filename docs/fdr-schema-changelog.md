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

Schema 1 and 2 records still parse and verify. They carry no thermal block, so
their replay produces no vent events and no boil-off, and steps 6–7 are
skipped. No existing certificate was invalidated by this freeze.

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
