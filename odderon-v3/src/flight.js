// JS is a driver for the v1 Rust ABI. Both the flight and the separate replay
// worker start from design inputs, not claimed thrust, fuel or a saved pose.
export const wasmUrl = () =>
  new URL("stations/odd_view.wasm", document.baseURI).href;
export async function loadCore(url) {
  const response = await fetch(url);
  if (!response.ok)
    throw Error(
      "Flight computer unavailable. Restore the CI-built WASM asset.",
    );
  return (await WebAssembly.instantiate(await response.arrayBuffer(), {}))
    .instance.exports;
}
export function reset(w, stage) {
  if (!w.mission_contract(stage.id)) throw Error("Unknown contract");
  const e = stage.engine;
  if (
    e &&
    (w.mission_engine_solve(
      e.propellant,
      e.of,
      e.pc,
      e.throat,
      e.exit,
      e.length,
      0,
    ) !== 1 ||
      w.mission_engine_apply() !== 1)
  )
    throw Error("Cannot fit engine");
  const a = stage.assembly;
  if (a) {
    if (!w.mission_delegation_set(a.mask)) throw Error("Delegation refused");
    if (
      a.fields?.length &&
      w.mission_thermal_get(2) > 0 &&
      !w.mission_thermal_set(2, 0)
    )
      throw Error("Cooler reset refused");
    for (const [field, value] of a.fields) {
      if (!w.mission_thermal_set(field, value))
        throw Error("Invalid thermal fit-out");
    }
  }
  if (w.pilot_start() !== 1) throw Error("Flight computer refused start");
}
export function telemetry(w) {
  return Array.from({ length: 32 }, (_, i) => w.pilot_read(i));
}
export function record(tape, cmd) {
  const last = tape.at(-1);
  if (last && cmd.every((v, i) => v === last[i + 1])) last[0]++;
  else tape.push([1, ...cmd]);
}
export const bytes = (body) =>
  new TextEncoder().encode("ODDERON-MANUAL-1\n" + JSON.stringify(body));
export async function replay(w, file) {
  const { body, signature, publicKey } = file;
  if (
    body.format !== "odderon.manual/1" ||
    signature?.length !== 64 ||
    publicKey?.length !== 32 ||
    !Array.isArray(body.commands) ||
    !Array.isArray(body.terminal)
  )
    throw Error("Not a signed manual flight tape");
  const message = bytes(body);
  if (message.length > 1 << 20) throw Error("Tape too large");
  w.mission_input_reset();
  w.mission_verify_reset();
  for (const x of message)
    if (!w.mission_input_absorb(x)) throw Error("Rejected input");
  for (const [channel, data] of [
    [0, publicKey],
    [1, signature],
  ])
    for (const x of data)
      if (!w.mission_verify_absorb(channel, x))
        throw Error("Invalid signature field");
  if (!w.mission_verify()) throw Error("Invalid pilot signature");
  reset(w, body.stage);
  if (body.prediction && !w.pilot_predict(...body.prediction))
    throw Error("Invalid prediction");
  let count = 0;
  for (const row of body.commands) {
    if (!Array.isArray(row) || row.length !== 6)
      throw Error("Invalid controls");
    const [n, ...cmd] = row;
    count += n;
    if (!Number.isSafeInteger(n) || n < 1 || count > 200000)
      throw Error("Replay step budget exceeded");
    for (let i = 0; i < n; i++)
      if (!w.pilot_step(...cmd)) throw Error("Replay diverged during flight");
  }
  if (JSON.stringify(telemetry(w)) !== JSON.stringify(body.terminal))
    throw Error("Terminal telemetry mismatch");
  return count;
}
