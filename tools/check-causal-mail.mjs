#!/usr/bin/env node
/**
 * The gate that makes causal mail a real, kernel-verifiable presence rather than a page-drawn list.
 *
 * `web/index.html` puts other ships in the player's sky on real orbital elements, and their messages
 * arrive at the speed of light -- no netcode, because the kernel is deterministic and the universe is
 * a function of time (bible §11.2). The P4 gate is "someone else can verify your work": a third party,
 * given a peer's elements, the station, and a send time, must be able to re-derive the message's
 * light-time and arrival from the kernel, and the recorded `mail` entry must verify against them.
 * This check is that claim made mechanical.
 *
 * The checks:
 *
 *   1. THE LIGHT-TIME AND ARRIVAL ARE THE KERNEL'S. `causalMailReadings` must call the kernel's
 *      `mail_light_time_s` and `mail_arrival_s`. The page computes no light-time and holds no
 *      speed-of-light constant -- the failure mode is "someone re-adds a local c = 299792".
 *
 *   2. THE INBOX SPLITS ON CAUSALITY. `causalMailInbox` must deliver a message whose `arrival <=
 *      nowS` and hold the rest in transit with a countdown. A message that arrives before it is sent
 * is the defect this guards against.
 *
 *   3. THE SEND SCHEDULE IS A PURE FUNCTION OF TIME. `causalMailSendTime` must snap to the peer's
 *      period with `Math.floor` -- deterministic, no clock read, no randomness. "No tick
 *      reconciliation" means there is nothing to reconcile.
 *
 *   4. MAIL IS RECORDED AND VERIFIED. `recordMail` records a `mail` archive entry, and
 *      `verifyArchiveEntry` has a `mail` branch that re-derives the light-time and arrival from the
 *      kernel's `mail_light_time_s` and `mail_arrival_s`.
 *
 *   5. THE MAIL ENTRY IS HASHABLE. `archiveSerialize` has a `mail` branch, so a recorded message is
 *      fingerprinted by its physics and reproducible across time and platform.
 *
 * Usage: node tools/check-causal-mail.mjs            (static: no browser, no wasm needed)
 *        node tools/check-causal-mail.mjs --selftest
 *        node tools/check-causal-mail.mjs --wasm dist/odderon_dmath_wasm.wasm
 */

import { readFile } from "node:fs/promises";
import { readFileSync } from "node:fs";

const PAGE = new URL("../web/index.html", import.meta.url);

/** Strip comments so documentation about the mail does not trip the checks that hunt for local light-times. */
function liveSource(html) {
  const script = html.match(/<script type="module">([\s\S]*?)<\/script>/);
  if (!script) throw new Error("no module script found in web/index.html");
  return script[1]
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/(^|[^:])\/\/.*$/gm, "$1");
}

/** The body of a named function, matched up to its closing brace at column 0. */
function fnBody(src, name) {
  const m = src.match(new RegExp("function " + name + "\\s*\\([^)]*\\)\\s*\\{([\\s\\S]*?)\\n\\}"));
  if (!m) throw new Error("could not find function " + name);
  return m[1];
}

const checks = [];
function check(name, fn) {
  checks.push({ name, fn });
}

check("the mail light-time and arrival are the kernel's", (src) => {
  const body = fnBody(src, "causalMailReadings");
  if (!body.includes("mail_light_time_s")) throw new Error("causalMailReadings does not call the kernel's mail_light_time_s");
  if (!body.includes("mail_arrival_s")) throw new Error("causalMailReadings does not call the kernel's mail_arrival_s");
  // The page computes no light-time: no speed-of-light constant, no distance/speed, no sqrt of a
  // squared difference. The light-time is a kernel call, full stop.
  for (const [re, why] of [
    [/299792/, "a speed-of-light constant in the page"],
    [/3e8/, "a rounded speed of light in the page"],
    [/Math\.sqrt/, "a page-computed distance"],
    [/\bc\s*=\s*299/, "a local light-speed variable"],
  ]) {
    if (re.test(body)) throw new Error("causalMailReadings " + why);
  }
});

check("the inbox splits on causality", (src) => {
  const body = fnBody(src, "causalMailInbox");
  if (!/arrival\s*<=\s*nowS/.test(body)) throw new Error("causalMailInbox does not deliver on arrival <= nowS");
  if (!/inTransit/.test(body)) throw new Error("causalMailInbox has no in-transit branch");
  if (!/countdown/.test(body)) throw new Error("causalMailInbox does not carry a countdown for in-transit messages");
});

check("the send schedule is a pure function of time", (src) => {
  const body = fnBody(src, "causalMailSendTime");
  if (!/Math\.floor/.test(body)) throw new Error("causalMailSendTime does not snap to the period with Math.floor");
  if (!/period/.test(body)) throw new Error("causalMailSendTime does not use the peer's period");
  for (const [re, why] of [
    [/Date\.now/, "a clock read in the schedule"],
    [/Math\.random/, "randomness in the schedule"],
  ]) {
    if (re.test(body)) throw new Error("causalMailSendTime " + why);
  }
});

check("mail is recorded and verified against the kernel", (src) => {
  const rec = fnBody(src, "recordMail");
  if (!/type:\s*['"]mail['"]/.test(rec)) throw new Error("recordMail does not record a 'mail' entry");
  const ver = fnBody(src, "verifyArchiveEntry");
  if (!/entry\.type\s*===\s*['"]mail['"]/.test(ver)) throw new Error("verifyArchiveEntry has no 'mail' branch");
  if (!ver.includes("mail_light_time_s")) throw new Error("verifyArchiveEntry does not re-derive via mail_light_time_s");
  if (!ver.includes("mail_arrival_s")) throw new Error("verifyArchiveEntry does not re-derive via mail_arrival_s");
});

check("the mail entry is hashable", (src) => {
  const ser = fnBody(src, "archiveSerialize");
  if (!/entry\.type\s*===\s*['"]mail['"]/.test(ser)) throw new Error("archiveSerialize has no 'mail' branch");
  if (!/lightTime/.test(ser)) throw new Error("archiveSerialize does not fingerprint the mail light-time");
  if (!/arrival/.test(ser)) throw new Error("archiveSerialize does not fingerprint the mail arrival");
});

/**
 * The exports the causal mail depends on. The mail adds new kernel exports (peer propagation and the
 * mail light-time/arrival) and reuses the primitive `light_time_s`. All must be present and physical:
 * the mail light-time must equal the primitive measured between the peer's and the station's
 * positions, the arrival must be send + light-time (causality), and invalid elements must be refused.
 */
function verifyExports(ex) {
  const need = [
    "mail_light_time_s",
    "mail_arrival_s",
    "peer_pos_x",
    "peer_pos_y",
    "peer_pos_z",
    "station_pos_x",
    "station_pos_y",
    "station_pos_z",
    "light_time_s",
  ];
  const missing = need.filter((n) => typeof ex[n] !== "function");
  if (missing.length) throw new Error("module is missing: " + missing.join(", "));

  const problems = [];
  // A real peer: a = 20000 km, e = 0.20, i = 0.9 rad, raan = 1.0, argp = 0.3, nu = 0.5. Its
  // semi-latus rectum is p = a(1 - e²).
  const a = 20000, e = 0.20, i = 0.9, raan = 1.0, argp = 0.3, nu = 0.5;
  const p = a * (1 - e * e);
  const lat = 0, lon = 0, sendS = 1000;

  const lt = ex.mail_light_time_s(p, e, i, raan, argp, nu, lat, lon, sendS);
  if (!Number.isFinite(lt) || lt <= 0) problems.push(`mail light-time ${lt} is not a positive finite seconds`);

  const arr = ex.mail_arrival_s(p, e, i, raan, argp, nu, lat, lon, sendS);
  if (!Number.isFinite(arr)) problems.push(`mail arrival ${arr} is not finite`);
  else if (arr < sendS) problems.push(`arrival ${arr} is before the send ${sendS} -- causality violated`);
  else if (Math.abs(arr - (sendS + lt)) > 1e-9) problems.push(`arrival ${arr} != send ${sendS} + light-time ${lt}`);

  // Determinism: the same send lands the same way every time.
  const lt2 = ex.mail_light_time_s(p, e, i, raan, argp, nu, lat, lon, sendS);
  if (lt2 !== lt) problems.push("mail light-time is not deterministic");

  // The mail light-time must equal the primitive measured between the peer's and the station's
  // positions -- the verifiability claim. This is what a third party recomputes.
  const px = ex.peer_pos_x(p, e, i, raan, argp, nu, sendS);
  const py = ex.peer_pos_y(p, e, i, raan, argp, nu, sendS);
  const pz = ex.peer_pos_z(p, e, i, raan, argp, nu, sendS);
  if (![px, py, pz].every(Number.isFinite)) problems.push("peer position is not finite");
  const sx = ex.station_pos_x(lat, lon, sendS);
  const sy = ex.station_pos_y(lat, lon, sendS);
  const sz = ex.station_pos_z(lat, lon, sendS);
  if (![sx, sy, sz].every(Number.isFinite)) problems.push("station position is not finite");
  const prim = ex.light_time_s(px, py, pz, sx, sy, sz);
  if (Number.isFinite(prim) && Math.abs(prim - lt) > 1e-9) {
    problems.push(`mail light-time ${lt} != primitive ${prim} between the peer and the station`);
  }

  // A second peer on a different orbit must give a different light-time -- the elements are real.
  const a2 = 42164, e2 = 0.02, p2 = a2 * (1 - e2 * e2);
  const lt2b = ex.mail_light_time_s(p2, e2, 0.0, 0.0, 0.0, 1.1, lat, lon, sendS);
  if (Number.isFinite(lt2b) && Math.abs(lt2b - lt) < 1e-12) problems.push("two peers on different orbits give the same light-time");

  // Invalid elements (p = 0) must be refused, not guessed.
  const bad = ex.mail_light_time_s(0, e, i, raan, argp, nu, lat, lon, sendS);
  if (Number.isFinite(bad)) problems.push(`invalid peer (p=0) gave a finite light-time ${bad}`);

  if (problems.length) throw new Error("boundary disagrees with physics: " + problems.join("; "));
  return `light-time=${lt.toFixed(6)}s arrival=${arr.toFixed(3)}s (send ${sendS}), primitive-match, causal, deterministic`;
}

/** Load a built module and verify the exports the causal mail depends on. */
async function runWasm(path) {
  const bytes = await readFile(path);
  const { instance } = await WebAssembly.instantiate(bytes, {});
  return verifyExports(instance.exports);
}

/**
 * `--selftest`: prove the mail's inbox logic, recording, and verification are sound, by extracting the
 * page's own functions and running them against a shim kernel. Without this, the accept and reject
 * paths would first run in a browser, and a checker whose rejection path has never executed is not
 * known to reject.
 */
function selftest() {
  const html = readFileSync(PAGE, "utf8");
  const src = liveSource(html);

  function extract(name) {
    const bodyRe = new RegExp("function " + name + "\\s*\\([^)]*\\)\\s*\\{([\\s\\S]*?)\\n\\}");
    const m = src.match(bodyRe);
    if (!m) throw new Error("could not extract " + name + " from the page");
    const paramsRe = new RegExp("function " + name + "\\s*\\(([^)]*)\\)");
    const pm = m[0].match(paramsRe);
    if (!pm) throw new Error("could not read " + name + " params");
    return "function " + name + "(" + pm[1] + ") {" + m[1] + "\n}";
  }
  const fnSource = [
    "const archive = []; let archiveSeq = 0;",
    "function renderArchive() {}",
    "function renderMuseum() {}",
    "function renderSurvey() {}",
    "function saveArchive() {}",
    extract("archiveSerialize"),
    extract("archiveHash"),
    extract("archivePush"),
    extract("close"),
    extract("peerP"),
    extract("causalMailSendTime"),
    extract("causalMailReadings"),
    extract("causalMailInbox"),
    extract("recordMail"),
    extract("verifyArchiveEntry"),
  ].join("\n");

  // A shim kernel: the light-time is a deterministic function of the peer's semi-latus rectum (so
  // different peers differ), the arrival is send + light-time, and the positions are fixed. This
  // lets the self-test exercise the page's logic -- the inbox split, recording, verification --
  // without the real kernel; the real physics is checked in --wasm.
  const shimKernel = {
    mail_light_time_s: (p) => 0.04 + p / 1e6,
    mail_arrival_s: (p, e, i, raan, argp, nu, lat, lon, sendS) => sendS + (0.04 + p / 1e6),
    peer_pos_x: () => 1000, peer_pos_y: () => 2000, peer_pos_z: () => 3000,
    station_pos_x: () => 0, station_pos_y: () => 0, station_pos_z: () => 0,
  };
  const peers = [
    { name: "aria", a: 7000, e: 0.010, i: 0.30, raan: 0.50, argp: 0.20, nu: 0.00, period: 90, epoch: -45, text: "x" },
    { name: "bolt", a: 20000, e: 0.200, i: 0.90, raan: 1.00, argp: 0.30, nu: 0.50, period: 150, epoch: -75, text: "y" },
  ];
  const sandbox = {
    kernelState: { module: shimKernel },
    groundStation: { latitudeDeg: 0, longitudeDeg: 0 },
    CAUSAL_PEERS: peers,
  };
  const fn = new Function(...Object.keys(sandbox), fnSource + "\nreturn { archive, archiveSerialize, archiveHash, causalMailSendTime, causalMailReadings, causalMailInbox, recordMail, verifyArchiveEntry };");
  const { archive, archiveSerialize, archiveHash, causalMailSendTime, causalMailReadings, causalMailInbox, recordMail, verifyArchiveEntry } = fn(...Object.values(sandbox));

  const results = [];
  // The send schedule snaps deterministically to the period and is a pure function of time.
  const s1 = causalMailSendTime(peers[0], 500);
  const s2 = causalMailSendTime(peers[0], 500);
  results.push(["the send schedule is deterministic", s1 === s2]);
  results.push(["the send schedule snaps to the period", s1 === peers[0].epoch + Math.floor((500 - peers[0].epoch) / peers[0].period) * peers[0].period]);

  // The inbox: a message whose arrival has passed is delivered; one still in flight is in transit.
  const nowEarly = 1000;
  const inb = causalMailInbox(nowEarly);
  results.push(["the inbox returns delivered and in-transit lists", Array.isArray(inb.delivered) && Array.isArray(inb.inTransit)]);
  const deliveredAll = inb.delivered.every((m) => m.arrival <= nowEarly);
  const transitAll = inb.inTransit.every((m) => m.arrival > nowEarly && Number.isFinite(m.countdown) && m.countdown > 0);
  results.push(["delivered messages have arrival <= now", deliveredAll]);
  results.push(["in-transit messages have arrival > now with a positive countdown", transitAll]);

  // Causality: the kernel's arrival is always at or after the send.
  const r = causalMailReadings(peers[1], 1000);
  results.push(["the arrival is at or after the send (causality)", r.arrival >= 1000]);
  results.push(["the arrival equals send + light-time", Math.abs(r.arrival - (1000 + r.lightTime)) < 1e-9]);

  // Different peers on different orbits give different light-times.
  const ra = causalMailReadings(peers[0], 1000);
  results.push(["different peers give different light-times", ra.lightTime !== r.lightTime]);

  // Recording + verification: a recorded mail entry verifies against the shim kernel.
  const rec = recordMail(peers[1], 1000);
  results.push(["recordMail produces a mail entry", rec && rec.type === "mail"]);
  results.push(["a recorded mail entry verifies against the kernel", verifyArchiveEntry(rec).ok === true]);

  // Tamper-sensitivity: a recorded mail entry with a wrong light-time fails verification.
  const tampered = { ...rec, readings: { ...rec.readings, lightTime: 999 } };
  tampered.hash = archiveHash(tampered);
  results.push(["a tampered mail light-time fails verification", verifyArchiveEntry(tampered).ok === false]);

  // The hash is a pure function of the mail's physics.
  const h1 = archiveHash(rec);
  const h2 = archiveHash({ ...rec, id: 999, epoch: 12345 });
  results.push(["the mail hash excludes the id and epoch", h1 === h2]);

  let bad = 0;
  for (const [name, ok] of results) {
    console.log(`  ${ok ? "ok  " : "FAIL"} ${name}`);
    if (!ok) bad++;
  }
  return bad;
}

const html = await readFile(PAGE, "utf8");
const src = liveSource(html);

let failed = 0;
for (const { name, fn } of checks) {
  try {
    fn(src);
    console.log(`  ok   ${name}`);
  } catch (e) {
    failed++;
    console.log(`  FAIL ${name}`);
    console.log(`       ${e.message}`);
  }
}

if (process.argv.includes("--selftest")) {
  const bad = selftest();
  console.log("");
  console.log(bad ? `${bad} self-test(s) FAILED` : "self-test passed: the causal mail inbox, recording, and verification are sound");
  process.exit(bad ? 1 : 0);
}

const wasmArg = process.argv.indexOf("--wasm");
if (wasmArg !== -1) {
  const path = process.argv[wasmArg + 1];
  try {
    const detail = await runWasm(path);
    console.log(`  ok   wasm boundary: ${detail}`);
  } catch (e) {
    failed++;
    console.log(`  FAIL wasm boundary`);
    console.log(`       ${e.message}`);
  }
} else {
  console.log("  ..   wasm boundary not run (pass --wasm <file> where a build exists)");
}

console.log("");
if (failed) {
  console.log(`${failed} causal-mail check(s) FAILED`);
  process.exit(1);
}
console.log(`all ${checks.length} causal-mail checks passed`);
