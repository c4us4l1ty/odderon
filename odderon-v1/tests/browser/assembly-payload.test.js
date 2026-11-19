import assert from 'node:assert/strict';
import test from 'node:test';
import { assemblyPayload, applyAssembly, THERMAL_FIELDS } from '../../web/assembly.js';

function fakeAssembly(initial, mask = 0) {
  const fields = { ...initial };
  return {
    fields,
    mission_thermal_get: id => fields[id] ?? 0,
    mission_thermal_set: (id, value) => { fields[id] = value; return 1; },
    mission_delegation_mask: () => mask,
    mission_delegation_set: value => { mask = value; return 1; },
  };
}

test('worker snapshot restores every editable thermal setting including relief', () => {
  const source = fakeAssembly({ 0: 40, 1: 0.01, 2: 400, 3: 40, 4: 0.2, 5: 250_000, 6: 1, 7: 1, 8: 16_000 }, 5);
  const target = fakeAssembly({ 5: 200_000, 7: 1 });
  const snapshot = assemblyPayload(source);
  assert.ok(snapshot.fields.some(([id, value]) => id === THERMAL_FIELDS.relief && value === 250_000));
  assert.equal(applyAssembly(target, snapshot), true);
  for (const [id, value] of snapshot.fields) assert.equal(target.fields[id], value);
  assert.equal(target.mission_delegation_mask(), 5);
});

test('storable stages do not send cryogenic settings', () => {
  const source = fakeAssembly({ 7: 0 }, 2);
  assert.deepEqual(assemblyPayload(source), { fields: [], mask: 2 });
});
