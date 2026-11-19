import test from 'node:test';
import assert from 'node:assert/strict';
import { readMissionNodes } from '../../web/mission-nodes.js';

function bridge(nodes, count = nodes.length) {
  return {
    mission_node_count: () => count,
    mission_node_time: index => nodes[index].time,
    mission_node_dv: (index, component) => nodes[index].dv[component],
    mission_node_guided: index => nodes[index].guidance ? 1 : 0,
    mission_node_guidance: (index, component) => nodes[index].guidance[component],
  };
}

test('targeted maneuver nodes round-trip from Rust into page state', () => {
  const adjusted = [
    { time: 120, dv: [11.5, -2.25, 0.125], guidance: null },
    { time: 960, dv: [8, 1.75, -0.5], guidance: [1, 2, 3, 4, 5, 6] },
  ];
  const result = readMissionNodes(bridge(adjusted));
  assert.deepEqual(result, adjusted);
  assert.notEqual(result[0].dv, adjusted[0].dv, 'page state must own a fresh node copy');
  assert.notEqual(result[1].guidance, adjusted[1].guidance, 'guidance must also be copied');
});

test('invalid targeted maneuver transport fails closed', () => {
  assert.throws(() => readMissionNodes(bridge([], 13)), /count is invalid/);
  assert.throws(() => readMissionNodes(bridge([{ time: 1, dv: [NaN, 0, 0], guidance: null }])), /data is invalid/);
});
