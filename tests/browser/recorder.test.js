import assert from 'node:assert/strict';
import test from 'node:test';
import { stringifySigned, withReceipt } from '../../web/recorder.js';

test('signed JSON preserves negative zero without changing strings, keys or undefined metrics', () => {
  const marker = '__odderon_negative_zero__';
  const value = {
    [marker]: marker,
    nested: [-0, 0, -1, 1, NaN, null, { [marker + '_']: -0 }],
    sentence: `a quoted "${marker}" value`,
  };
  const text = stringifySigned(value);
  const parsed = JSON.parse(text);
  assert.equal(parsed[marker], marker);
  assert.equal(parsed.sentence, value.sentence);
  assert.ok(Object.is(parsed.nested[0], -0));
  assert.ok(Object.is(parsed.nested[1], 0));
  assert.deepEqual(parsed.nested.slice(2, 6), [-1, 1, null, null]);
  assert.ok(Object.is(parsed.nested[6][marker + '_'], -0));
  assert.match(text, /-0/);
});

test('attaching a receipt retains signed zeros in both envelopes', () => {
  const result = withReceipt({ document: { command_timeline: [{ delta_v_tnw: [-0, 0, 1] }] } },
    { score_s: -0, elo_after: 1200 });
  const parsed = JSON.parse(result.json);
  assert.ok(Object.is(parsed.command_timeline[0].delta_v_tnw[0], -0));
  assert.ok(Object.is(parsed.server_receipt.score_s, -0));
});
