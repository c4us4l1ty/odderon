import { View } from "./view.js";
import { navball } from "./instruments.js";
import { icons } from "./icons.js";
import { catalog, stations } from "./catalog.js";
import {
  bytes,
  loadCore,
  record,
  reset,
  telemetry,
  wasmUrl,
} from "./flight.js";
const $ = (id) => document.getElementById(id),
  ic = (name) => `<i data-i="${name}"></i>`,
  sheet = $("sheet"),
  mfd = $("mfd");
$("stations").innerHTML = stations
  .map(
    ([id, ico, name]) =>
      `<button data-station="${id}" class="${id === "flight" ? "on" : ""}" title="${name} station">${ic(ico)}<small>${name.toUpperCase()}</small></button>`,
  )
  .join("");
icons();
const view = new View($("viewport")),
  held = new Set(),
  engineering = $("engine-frame"),
  obs = $("obs-frame");
let reference = [],
  track = [],
  core,
  s = [],
  contract = catalog[0],
  stage = { id: 101 },
  commands = [],
  prediction = null,
  fullFuel = 1,
  armed = false,
  power = 0,
  running = false,
  rate = 1,
  raf = 0,
  lastTick = 0,
  acc = 0,
  station = "flight",
  instrument = null,
  replayJob = null,
  steps = 0,
  ready = false,
  broken = false,
  filed = false;
const fixed = (n, d = 1) =>
  Number.isFinite(n)
    ? n.toLocaleString("en-US", {
        minimumFractionDigits: d,
        maximumFractionDigits: d,
      })
    : "—";
const clock = (t) =>
  [Math.floor(t / 3600), Math.floor(t / 60) % 60, Math.floor(t) % 60]
    .map((n) => String(n).padStart(2, "0"))
    .join(":");
const deg = (n) => (n * 180) / Math.PI;
const sign = (n) => (n >= 0 ? "+" : "") + fixed(n, 1) + "°";
function entry(text) {
  const line = document.createElement("div");
  line.className = "entry";
  const time = document.createElement("time");
  time.textContent = clock(s[0] || 0);
  const msg = document.createElement("span");
  msg.textContent = text;
  line.append(time, msg);
  $("entries").append(line);
  while ($("entries").children.length > 40) $("entries").firstChild.remove();
  $("entries").scrollTop = $("entries").scrollHeight;
}
function controls() {
  const fly = $("fly");
  fly.innerHTML =
    ic(running ? "pause" : "play") +
    (s[15] || filed
      ? "Flight review"
      : running
        ? "Pause flight"
        : s[0]
          ? "Resume flight"
          : "Take control");
  icons(fly);
  const btn = $("arm");
  btn.innerHTML =
    ic("power") +
    `<b>${armed ? "Safe engine" : "Arm engine"}</b><kbd>SPACE</kbd>`;
  icons(btn);
  $("pause").innerHTML = ic(running ? "pause" : "play");
  icons($("pause"));
  $("pause").title = running ? "Pause flight" : "Resume flight";
  $("warp").querySelector("strong").textContent = rate + "×";
  $("throttle").value = power;
  $("pct").textContent = Math.round(power) + "%";
}
function update() {
  s = telemetry(core);
  if (!track.length || s[0] - track.at(-1)[0] >= 10) {
    track.push([s[0], ...s.slice(1, 4)]);
    if (track.length > 1800) track = track.filter((_, i) => i % 2 === 0);
  }
  s.track = track;
  s.reference = reference;
  s.throttle = armed && s[27] ? power / 100 : 0;
  $("met").textContent = clock(s[0]);
  for (const [id, v] of Object.entries({
    ap: fixed(s[10] / 1000),
    pe: fixed(s[11] / 1000),
    alt: fixed(s[8] / 1000) + " km",
    speed: fixed(s[9], 0) + " m/s",
    inc: fixed(deg(s[17]), 2) + "°",
    fuel: fixed(s[7], 1),
    dv: fixed(s[16], 0) + " m/s",
    pitch: sign(deg(s[13])),
    yaw: sign(deg(s[14])),
  }))
    $(id).textContent = v;
  $("eta-ap").textContent = Number.isFinite(s[28])
    ? "T− " + clock(s[28])
    : "CIRCULAR";
  $("eta-pe").textContent = Number.isFinite(s[29])
    ? "T− " + clock(s[29])
    : "CIRCULAR";
  $("fuelbar").style.width = (100 * s[7]) / fullFuel + "%";
  $("att-label").textContent =
    Math.abs(s[13]) + Math.abs(s[14]) < 0.01 ? "PROGRADE" : "MANUAL HOLD";
  navball($("ball"), s[13], s[14], s[24]);
  view.draw(s);
}
function halt() {
  running = false;
  held.clear();
  cancelAnimationFrame(raf);
  acc = 0;
  controls();
}
function resume() {
  if (!ready || filed || broken || s[15] || running || sheet.open || mfd.open)
    return;
  running = true;
  lastTick = performance.now();
  acc = 0;
  controls();
  raf = requestAnimationFrame(loop);
}
function tick() {
  if (held.has("KeyZ") || held.has("KeyX")) {
    power = Math.min(100, Math.max(0, power + (held.has("KeyZ") ? 1 : -1) * 5));
    controls();
  }
  const throttle = armed ? power / 100 : 0,
    cmd = [
      0.2 * rate,
      throttle,
      Number(held.has("KeyW")) - Number(held.has("KeyS")),
      Number(held.has("KeyD")) - Number(held.has("KeyA")),
      Number(held.has("KeyE")) - Number(held.has("KeyQ")),
    ];
  if (steps >= 200000 || bytesEstimate() > 950000) {
    halt();
    entry("Tape capacity reached. Review flight.");
    return;
  }
  if (!core.pilot_step(...cmd)) {
    halt();
    broken = true;
    entry("Propagation stopped. Review or restart flight.");
    return;
  }
  record(commands, cmd);
  steps++;
  update();
  if (s[15]) {
    halt();
    entry(s[26] ? "Surface impact. Review available." : "Flight window closed. Review available.");
    review();
  } else if (armed && !s[27]) {
    armed = false;
    power = 0;
    controls();
    entry("Engine flameout · tank depleted.");
  }
}
function bytesEstimate() {
  return commands.length * 64;
}
function loop(now) {
  if (!running) return;
  acc += Math.min(0.4, (now - lastTick) / 1000);
  lastTick = now;
  while (acc >= 0.2 && running) {
    acc -= 0.2;
    tick();
  }
  if (running) raf = requestAnimationFrame(loop);
}
function start(fit) {
  halt();
  reset(core, fit);
  if (fit.id !== contract.id) instrument?.selectContract(fit.id);
  stage = structuredClone(fit);
  contract = catalog.find((c) => c.id === fit.id);
  commands = [];
  track = [];
  reference = [];
  steps = 0;
  prediction = null;
  armed = false;
  power = 0;
  rate = 1;
  broken = false;
  filed = false;
  fullFuel = core.pilot_read(7);
  $("entries").replaceChildren();
  $("title").textContent = contract.title;
  $("desc").textContent = contract.brief;
  $("cid").textContent = $("ship-id").textContent = fit.id;
  update();
  controls();
  entry("Flight computer initialized.");
  entry(`Contract ${fit.id} loaded.`);
  entry("Standing by for pilot input.");
}
function arm() {
  if (!ready || filed || s[15] || broken) return;
  armed = !armed;
  rate = 1;
  if (!armed) power = 0;
  entry(armed ? "Engine armed · throttle enabled." : "Engine safed.");
  controls();
  if (armed) resume();
}
function camera(map) {
  view.mode = map ? "map" : "external";
  $("external").classList.toggle("selected", !map);
  $("mapBtn").classList.toggle("selected", map);
  $("cam-label").textContent = map ? "OSCULATING ORBIT" : "EXTERNAL OPTICS";
  view.draw(s);
}
function openSheet(tag, html) {
  halt();
  if (mfd.open) mfd.close();
  $("sheet-tag").textContent = tag;
  $("sheet-content").innerHTML = html;
  icons(sheet);
  if (!sheet.open) sheet.showModal();
}
function closeSheet() {
  if (sheet.open) sheet.close();
}
sheet.querySelector("[data-close]").onclick = closeSheet;
sheet.addEventListener("close", () => {
  if (replayJob) {
    replayJob.terminate();
    replayJob = null;
  }
});
function brief() {
  if (!ready) return;
  openSheet(
    "OPERATIONS / CONTRACT CATALOG",
    `<h2>Mission brief</h2><p>${contract.objective}</p>${catalog.map((c) => `<button class="choice ${c === contract ? "selected" : ""}" data-c="${c.id}"><span><b>${c.title}</b><small>${c.rank}</small></span><em>CTR–${c.id}</em></button>`).join("")}<button class="choice" id="predict">${ic("flag")} Preflight prediction <em>${prediction ? "RECORDED" : s[0] ? "LOCKED" : "OPTIONAL"}</em></button><button class="action" id="launch">${filed || s[15] || broken ? "Restart contract" : s[0] ? "Resume flight" : "Start flight"} →</button>`,
  );
  for (const b of sheet.querySelectorAll("[data-c]"))
    b.onclick = () => {
      const id = +b.dataset.c;
      if (id !== contract.id) replacement({ id }, brief);
    };
  $("predict").onclick = forecast;
  $("launch").onclick = () => {
    if (filed || s[15] || broken) {
      replacement(stage, () => {
        closeSheet();
        resume();
      });
      return;
    }
    closeSheet();
    resume();
  };
}
function replacement(fit, done) {
  const act = () => {
    try {
      start(fit);
      done();
    } catch (e) {
      error(e);
    }
  };
  if (!s[0]) return act();
  openSheet(
    "FLIGHT CONTROL / RESET",
    `<h2>Replace this flight?</h2><p>The current manual tape will be discarded. Export it from Flight review first if you want to keep it.</p><button class="choice" id="keep">Keep current flight</button><button class="action" id="replace">Start contract ${fit.id}</button>`,
  );
  $("replace").onclick = act;
  $("keep").onclick = closeSheet;
}
const predLabels = [
  "Apoapsis · km",
  "Periapsis · km",
  "Propellant · kg",
  "Burn Δv · m/s",
  "Elapsed time · s",
];
function forecast() {
  openSheet(
    "FLIGHT PLANNING / PREDICTION",
    `<h2>Pilot prediction</h2><p>Write down the orbit you expect to fly. This log entry does not move the ship or set the throttle. It locks at first motion.</p><form id="forecast"><div class="form-grid">${predLabels.map((l, i) => `<label>${l}<input name="p${i}" type="number" step="any" ${i >= 2 ? 'min="0"' : ""} ${s[0] ? "disabled" : ""} required value="${prediction ? prediction[i] / (i < 2 ? 1000 : 1) : ""}"></label>`).join("")}</div><button class="action" ${s[0] ? "disabled" : ""}>Save prediction</button></form>`,
  );
  $("forecast").onsubmit = (e) => {
    e.preventDefault();
    const p = predLabels.map(
      (_, i) => Number(e.target.elements["p" + i].value) * (i < 2 ? 1000 : 1),
    );
    if (!core.pilot_predict(...p)) {
      entry("Prediction refused: check elapsed time.");
      return;
    }
    prediction = p;
    entry("Pilot prediction recorded.");
    brief();
  };
}
function help() {
  openSheet(
    "PILOT REFERENCE / INPUT",
    `<h2>Flight controls</h2><p>Start or resume with the play switch. Arm the engine, point the nose, and bring up throttle. No maneuver executes itself.</p>${[
      ["W / S", "Pitch up / down"],
      ["A / D", "Yaw left / right"],
      ["Q / E", "Roll left / right"],
      ["Space", "Arm / safe engine"],
      ["Z / X", "Increase / decrease throttle"],
      ["1 / 2", "External / orbit map"],
      [".", "Cycle coast warp"],
      ["P", "Pause / resume"],
      ["Drag / wheel", "Orbit camera / zoom"],
    ]
      .map(
        ([k, l]) => `<div class="row"><span>${l}</span><kbd>${k}</kbd></div>`,
      )
      .join(
        "",
      )}<button class="choice" id="academy">${ic("graduation-cap")} Open flight academy</button>`,
  );
  $("academy").onclick = () => {
    closeSheet();
    openStation("academy");
  };
}
function settings() {
  openSheet(
    "CONFIGURATION / LOCAL",
    `<h2>Display settings</h2><label class="row">Vessel identification marker<input type="checkbox" id="tags" ${$("viewport").classList.contains("no-tags") ? "" : "checked"}></label><button class="choice" id="full">${ic("scan")} Toggle fullscreen</button><button class="choice" id="zeroCam">${ic("rotate-ccw")} Reset exterior camera</button><p style="margin-top:15px">All flight systems run locally. Ranked Astro-Elo and the online leaderboard are postponed.</p>`,
  );
  $("tags").onchange = (e) =>
    $("viewport").classList.toggle("no-tags", !e.target.checked);
  $("zeroCam").onclick = () => {
    recenter();
    closeSheet();
  };
  $("full").onclick = async () => {
    try {
      if (document.fullscreenElement) await document.exitFullscreen();
      else await document.documentElement.requestFullscreen();
    } catch {
      error(Error("Fullscreen is unavailable in this browser context."));
    }
  };
}
function recenter() {
  view.yaw = view.roll = 0;
  view.zoom = 1;
  view.draw(s);
}
async function signTape() {
  // Capture at the instant of the request, not after a lazy import. The
  // pilot can close this sheet and resume while the signer is loading.
  const body = structuredClone({
    format: "odderon.manual/1",
    stage, prediction, commands, terminal: telemetry(core),
  });
  const identity = await import(
    /* @vite-ignore */ new URL("stations/identity.js", document.baseURI).href
  );
  identity.init(core);
  return {
    body,
    publicKey: identity.profile().publicKey,
    signature: [...identity.sign(bytes(body))],
  };
}
const LAST_TAPE = "odderon.manual.last.v1";
function verifyTape(file) {
  if (replayJob) replayJob.terminate();
  replayJob = new Worker(new URL("./replay-worker.js", import.meta.url), {
    type: "module",
  });
  const worker = replayJob;
  const finish = (msg) => {
    if (replayJob !== worker) return;
    if ($("verdict")) $("verdict").textContent = msg;
    worker.terminate();
    replayJob = null;
  };
  worker.onmessage = (e) => finish(e.data.message);
  worker.onerror = () => finish("Replay worker failed.");
  $("verdict").textContent = "Checking signature and replaying the controls…";
  worker.postMessage({ file, url: wasmUrl() });
}
function review() {
  if (!ready) return;
  const fields = [s[10] / 1e3, s[11] / 1e3, s[7], s[12], s[0]];
  openSheet(
    "FLIGHT DATA / LOCAL PRACTICE",
    `<h2>${filed ? "Filed flight report" : "Flight review"}</h2><p>Contract ${contract.id} · ${clock(s[0])} mission elapsed. Compare your prediction against the orbit you actually flew.</p><div class="row"><span>Instrument</span><b>Prediction &nbsp; / &nbsp; Measured</b></div>${fields.map((v, i) => `<div class="row"><span>${predLabels[i]}</span><b>${prediction ? fixed(prediction[i] / (i < 2 ? 1000 : 1)) : "—"} &nbsp; / &nbsp; ${fixed(v)}</b></div>`).join("")}<div class="row"><span>Orbital eccentricity / ascending node</span><b>${fixed(s[31], 5)} / ${fixed(deg(s[30]), 3)}°</b></div><div class="row"><span>Vented propellant</span><b>${fixed(s[21], 3)} kg</b></div>${prediction && s[0] ? `<div class="row"><span>Fitted-stage practice score</span><b>${fixed(s[22] * 100)}%</b></div>` : ""}<div class="row"><span>Contract objective</span><b>${s[26] ? "IMPACT" : s[25] >= 0.8 ? "ON TARGET" : "OUTSIDE TARGET"}</b></div>${!filed && s[0] ? '<button class="action" id="file">End flight & file locally</button>' : ""}<button class="choice" id="export">${ic("download")} Export signed manual tape</button><button class="choice" id="import">${ic("activity")} Verify a manual tape locally</button><input id="tape-file" type="file" accept="application/json,.json" hidden><p id="verdict" role="status"></p><button class="choice" id="last-tape">Verify last filed flight on this device</button><button class="choice" id="legacy">${ic("clipboard-list")} Verify a v1 signed recorder / dossier</button><button class="choice" id="restart">${ic("rotate-ccw")} Restart this contract</button>`,
  );
  if ($("file"))
    $("file").onclick = async (e) => {
      const button = e.currentTarget;
      button.disabled = true;
      try {
        const tape = await signTape();
        if (!sheet.open || !button.isConnected) return;
        localStorage.setItem(LAST_TAPE, JSON.stringify(tape));
        filed = true;
        armed = false;
        power = 0;
        controls();
        entry("Signed tape filed on this device.");
        review();
      } catch (err) {
        button.disabled = false;
        if (button.isConnected && $("verdict"))
          $("verdict").textContent =
            "Could not file on this device. Export your tape instead.";
      }
    };
  $("last-tape").onclick = () => {
    try {
      const raw = localStorage.getItem(LAST_TAPE);
      if (!raw) throw Error("No flight has been filed on this device.");
      verifyTape(JSON.parse(raw));
    } catch (err) {
      $("verdict").textContent = err.message;
    }
  };
  $("export").onclick = async () => {
    try {
      const data = await signTape();
      const a = document.createElement("a");
      a.href = URL.createObjectURL(
        new Blob([JSON.stringify(data)], { type: "application/json" }),
      );
      a.download = `ODV-${data.body.stage.id}-manual.json`;
      a.click();
      setTimeout(() => URL.revokeObjectURL(a.href), 1000);
      if ($("verdict")) $("verdict").textContent =
        "Local pilot signature applied. No ranked receipt.";
    } catch (e) {
      if ($("verdict")) $("verdict").textContent = e.message;
    }
  };
  $("import").onclick = () => $("tape-file").click();
  $("tape-file").onchange = async (e) => {
    const f = e.target.files[0];
    if (!f) return;
    try {
      if (f.size > 2e6) throw Error("Tape exceeds 2 MB");
      const file = JSON.parse(await f.text());
      verifyTape(file);
    } catch (err) {
      $("verdict").textContent = err.message;
    }
  };
  $("legacy").onclick = () => {
    closeSheet();
    openStation("verify");
  };
  $("restart").onclick = () => replacement(stage, closeSheet);
}
const labels = {
  nav: "Mission planner",
  propulsion: "Engine CAD",
  vehicle: "Vehicle assembly",
  academy: "Flight academy",
  obs: "Earth–Moon observatory",
  license: "Pilot license",
  verify: "Flight recorder",
  prediction: "Prediction lab",
  debrief: "Planned-flight debrief",
};
function activate() {
  if (!instrument || station === "obs" || !mfd.open) return;
  const doc = engineering.contentDocument;
  for (const d of doc.querySelectorAll("dialog[open]")) d.close();
  const tab = $("mfd-tabs").querySelector("[data-tab=debrief]");
  if (tab) {
    tab.disabled = doc.getElementById("debrief").hidden;
    tab.title = tab.disabled
      ? "Run a planned flight on the prediction bench first."
      : "";
  }
  const shell = engineering.contentWindow.odderonFlightShell;
  shell?.activate(
    ["nav", "license", "verify"].includes(station) ? "flight" : station,
  );
  if (station === "license") instrument.profile();
  if (station === "verify") instrument.verify();
}
window.addEventListener("odderon-instrument-ready", (e) => {
  instrument = e.detail;
  const bench = engineering.contentDocument;
  new MutationObserver(() => {
    const tab = $("mfd-tabs").querySelector("[data-tab=debrief]");
    if (tab) tab.disabled = bench.getElementById("debrief").hidden;
    if (
      !bench.getElementById("debrief").hidden &&
      mfd.open &&
      station !== "obs"
    )
      openStation("debrief");
  }).observe(bench.getElementById("debrief"), {
    attributes: true,
    attributeFilter: ["hidden"],
  });
  instrument.selectContract(contract.id);
  $("fit").disabled = false;
  activate();
});
function openStation(id) {
  if (id === "flight") {
    mfd.close();
    return;
  }
  halt();
  station = id;
  const scope = id === "obs";
  $("mfd-title").textContent = labels[id];
  $("mfd-tabs").innerHTML = (
    scope
      ? ["obs"]
      : [
          "nav",
          "propulsion",
          "vehicle",
          "academy",
          "prediction",
          "debrief",
          "license",
          "verify",
        ]
  )
    .map(
      (k) =>
        `<button class="${id === k ? "selected" : ""}" data-tab="${k}">${labels[k]}</button>`,
    )
    .join("");
  for (const b of $("mfd-tabs").querySelectorAll("button"))
    b.onclick = () => openStation(b.dataset.tab);
  engineering.hidden = scope;
  obs.hidden = !scope;
  $("fit").hidden = scope || id === "license" || id === "verify";
  $("fit").disabled = !instrument;
  if (!mfd.open) mfd.showModal();
  if (scope) {
    if (!obs.src) obs.src = "./stations/observatory.html";
  } else if (!engineering.src) engineering.src = "./stations/mission.html";
  else activate();
}
$("fit").onclick = () => {
  if (instrument) {
    const fit = instrument.stage(),
      brief = instrument.briefing();
    mfd.close();
    replacement(fit, () => {
      closeSheet();
      reference = brief.route;
      if (brief.prediction && core.pilot_predict(...brief.prediction)) {
        prediction = brief.prediction;
        entry("Planning prediction transferred.");
      }
      update();
      entry("Engineering fit-out transferred.");
    });
  }
};
$("mfd-close").onclick = () => mfd.close();
mfd.addEventListener("close", () => {
  station = "flight";
  document.activeElement?.blur();
});
function error(e) {
  if (!ready) document.querySelector(".online").textContent = "CORE OFFLINE";
  openSheet(
    "FLIGHT SYSTEM / ATTENTION",
    '<h2>Flight computer notice</h2><p id="error-msg"></p>',
  );
  $("error-msg").textContent = e.message;
  entry(e.message);
}
function warp() {
  if (!ready || armed || held.size || filed || broken || s[15]) return;
  const rates = [1, 5, 25, 100, 600];
  rate = rates[(rates.indexOf(rate) + 1) % rates.length];
  controls();
  entry(`Coast time warp ${rate}×.`);
}
$("fly").onclick = () =>
  filed || s[15] ? review() : running ? halt() : resume();
$("help").onclick = $("controls").onclick = help;
$("settings").onclick = settings;
$("contracts").onclick = brief;
$("report").onclick = review;
$("pause").onclick = () => (running ? halt() : resume());
$("warp").onclick = warp;
$("arm").onclick = arm;
$("throttle").oninput = (e) => {
  power = Number(e.target.value);
  controls();
};
$("external").onclick = () => camera(false);
$("mapBtn").onclick = () => camera(true);
$("plus").onclick = () => view.magnify(1.2);
$("minus").onclick = () => view.magnify(1 / 1.2);
$("recenter").onclick = recenter;
$("license").onclick = () => openStation("license");
for (const b of document.querySelectorAll("[data-station]"))
  b.onclick = () => openStation(b.dataset.station);
for (const key of document.querySelectorAll("[data-axis]")) {
  const release = () => held.delete(key.dataset.axis);
  key.onpointerdown = (e) => {
    if (!ready || filed) return;
    e.preventDefault();
    key.setPointerCapture(e.pointerId);
    rate = 1;
    controls();
    held.add(key.dataset.axis);
    resume();
  };
  key.onpointerup = release;
  key.onpointercancel = release;
  key.onlostpointercapture = release;
}
window.addEventListener("keydown", (e) => {
  if (
    !ready ||
    sheet.open ||
    mfd.open ||
    e.ctrlKey ||
    e.metaKey ||
    e.altKey ||
    e.target.matches("input:not([type=range]),textarea,select")
  )
    return;
  const code = e.code;
  if (
    ["KeyW", "KeyS", "KeyA", "KeyD", "KeyQ", "KeyE", "KeyZ", "KeyX"].includes(
      code,
    )
  ) {
    e.preventDefault();
    if (e.repeat) return;
    held.add(code);
    if (rate !== 1) {
      rate = 1;
      controls();
    }
    resume();
    return;
  }
  if (e.repeat) return;
  if (code === "Space") {
    e.preventDefault();
    arm();
  } else if (code === "KeyP" || code === "Escape") running ? halt() : resume();
  else if (code === "Digit1" || code === "Digit2") camera(code === "Digit2");
  else if (code === "Period") warp();
  else if (e.key === "?") help();
});
window.addEventListener("keyup", (e) => held.delete(e.code));
window.addEventListener("blur", halt);
document.addEventListener("visibilitychange", () => {
  if (document.hidden) halt();
});
Promise.resolve()
  .then(async () => {
    core = await loadCore(wasmUrl());
    start(stage);
    ready = true;
    document.querySelector(".online").textContent = "SYSTEM ONLINE";
  })
  .catch(error);
