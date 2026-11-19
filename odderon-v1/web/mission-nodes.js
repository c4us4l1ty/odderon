// Maneuver-node transport shared by the page and one-shot worker.
// Rust owns targeting. Whenever Rust adjusts a plan, read the complete node
// state back instead of rebuilding it from the page's stale JavaScript copy.
export function readMissionNodes(wasm) {
  const count = wasm.mission_node_count();
  if (!Number.isSafeInteger(count) || count < 0 || count > 12) {
    throw new Error('The targeted maneuver count is invalid.');
  }
  return Array.from({ length: count }, (_, index) => {
    const guided = wasm.mission_node_guided(index) === 1;
    const node = {
      time: wasm.mission_node_time(index),
      dv: [0, 1, 2].map(component => wasm.mission_node_dv(index, component)),
      guidance: guided
        ? Array.from({ length: 6 }, (__, component) => wasm.mission_node_guidance(index, component))
        : null,
    };
    if (!Number.isFinite(node.time)
        || node.time < 0
        || node.dv.some(value => !Number.isFinite(value))
        || (node.guidance && node.guidance.some(value => !Number.isFinite(value)))) {
      throw new Error('The targeted maneuver data is invalid.');
    }
    return node;
  });
}
