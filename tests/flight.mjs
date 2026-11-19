// Acceptance tests against the actual distributable, not mocked DOM values.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { bytes, record, replay, reset, telemetry } from "../src/flight.js";
const blob = await readFile(
  new URL("../runtime/odd_view.wasm", import.meta.url),
);
const fresh = async () =>
  (await WebAssembly.instantiate(blob, {})).instance.exports;
function signed(w, body) {
  assert.equal(w.mission_identity_from_entropy(1, 2, 3, 4), 1);
  w.mission_input_reset();
  for (const x of bytes(body)) assert.ok(w.mission_input_absorb(x));
  assert.equal(w.mission_identity_sign(), 1);
  return {
    body,
    publicKey: Array.from({ length: 32 }, (_, i) =>
      w.mission_identity_pubkey(i),
    ),
    signature: Array.from({ length: 64 }, (_, i) =>
      w.mission_signature_byte(i),
    ),
  };
}
for (const id of [101, 102, 103]) {
  const w = await fresh(),
    stage = { id },
    commands = [];
  reset(w, stage);
  const initial = telemetry(w),
    prediction = [550000, 550000, 120, 170, 3000];
  assert.ok(w.pilot_predict(...prediction));
  assert.equal(w.pilot_step(NaN, 1, 0, 0, 0), 0);
  assert.equal(w.pilot_step(120, 1, 0, 0, 0), 0);
  assert.equal(w.pilot_read(0), 0);
  for (const cmd of [
    [0.2, 1, 0, 0, 0],
    [0.2, 0.25, 1, 0, 0],
    [0.2, 0, 0, 1, 1],
    [60, 0, 0, 0, 0],
    [60, 0, 0, 0, 0],
  ]) {
    assert.ok(w.pilot_step(...cmd));
    record(commands, cmd);
  }
  const terminal = telemetry(w);
  assert.ok(terminal[7] < initial[7]);
  assert.ok(terminal[12] > 0);
  assert.ok(terminal[13] > 0 && terminal[14] > 0 && terminal[24] > 0);
  assert.equal(w.pilot_predict(...prediction), 0);
  assert.ok(Number.isFinite(terminal[22]));
  const tape = signed(w, {
    format: "odderon.manual/1",
    stage,
    commands,
    prediction,
    terminal,
  });
  assert.equal(await replay(await fresh(), structuredClone(tape)), 5);
  tape.body.commands[0][2] = 0.1;
  await assert.rejects(replay(await fresh(), tape), /signature/);
  tape.body.terminal[7]++;
  await assert.rejects(replay(await fresh(), signed(w, tape.body)), /mismatch/);
  console.log(
    `Contract ${id}: flown controls, SI telemetry, signed replay and mismatches OK`,
  );
}
// A real rescue: two manual prograde valve openings with an unpowered coast.
// The test pilot reads instruments. It never commands an orbit or a node.
const w = await fresh();
reset(w, { id: 101 });
let burns = 0;
while (w.pilot_read(10) < 560000 && burns++ < 140)
  assert.ok(w.pilot_step(1, 1, 0, 0, 0));
assert.ok(burns < 140);
let ascending = true;
for (let i = 0; i < 140 && ascending; i++) {
  assert.ok(w.pilot_step(25, 0, 0, 0, 0));
  const t = telemetry(w);
  ascending = t[1] * t[4] + t[2] * t[5] + t[3] * t[6] > 0;
}
burns = 0;
while (w.pilot_read(11) < 542000 && burns++ < 140)
  assert.ok(w.pilot_step(1, 1, 0, 0, 0));
assert.ok(burns < 140);
assert.ok(
  w.pilot_read(25) >= 0.8,
  `outside rescue tolerance: ${telemetry(w).slice(10, 12)}`,
);
console.log("101 objective attained using only hand-flight inputs");
// Cadet fit-out must change flown acceleration. Same throttle ≠ same flight.
const stock = await fresh(),
  custom = await fresh();
reset(stock, { id: 101 });
reset(custom, {
  id: 101,
  engine: {
    propellant: 3,
    of: 2,
    pc: 1500000,
    throat: 0.019,
    exit: 0.14,
    length: 0.8,
  },
});
for (const v of [stock, custom]) assert.ok(v.pilot_step(0.2, 1, 0, 0, 0));
assert.notEqual(stock.pilot_read(12), custom.pilot_read(12));
console.log("Engine handoff changes flown delta-v");
