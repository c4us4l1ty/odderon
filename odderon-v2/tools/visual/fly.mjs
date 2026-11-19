/* Fly a contract to completion with the in-game node planner and autopilot.
 *   node tools/visual/fly.mjs meridian-2
 * Prints each manoeuvre and the final orbit, so a broken solver shows up as a
 * missing node or a burn that never converges.
 */
import chromium from "@sparticuz/chromium";
import { chromium as pw } from "playwright-core";

const id = process.argv[2] ?? "kestrel-9";
const browser = await pw.launch({ executablePath: "/tmp/chromium", args: [...chromium.args, "--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--in-process-gpu"], headless: true, timeout: 90000 });
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
const errors = [];
page.on("pageerror", (e) => errors.push(e.message.slice(0, 200)));
await page.goto("http://127.0.0.1:5173/", { waitUntil: "load" });
await page.waitForFunction(() => Boolean(window.odderon), { timeout: 25000 });
await page.evaluate((id) => window.odderon.startFlight(id), id);
await page.waitForTimeout(500);

const km = (m) => Math.round((m - 6371000) / 1000);
const snapshot = () => page.evaluate(() => {
  const g = window.odderon;
  const e = g.vessel.elements;
  return {
    met: Math.round(g.met), warp: g.warp, autopilot: g.autopilot,
    pe: e.pe, ap: e.ap, prop: Math.round(g.vessel.propellant),
    dv: g.node ? Math.round(g.node.dv) : null,
    ok: g.evaluation?.inTolerance,
  };
});

for (let burn = 1; burn <= 3; burn++) {
  let s = await snapshot();
  if (s.ok) break;
  if (s.dv === null) {
    await page.click("#btn-add-node");
    await page.waitForTimeout(500);
    s = await snapshot();
    if (s.dv === null) {
      console.log(`burn ${burn}: no node planned`, { pe: km(s.pe), ap: km(s.ap) });
      break;
    }
    console.log(`burn ${burn}: node dv ${s.dv} m/s, pe ${km(s.pe)} ap ${km(s.ap)} km, met ${s.met}`);
  }
  await page.click("#btn-execute");
  // Wait for the autopilot to finish (complete -> idle) or fail.
  const deadline = Date.now() + 180000;
  let last = "";
  while (Date.now() < deadline) {
    await page.waitForTimeout(1000);
    const now = await snapshot();
    const label = `${now.autopilot} warp ${now.warp} met ${now.met}`;
    if (label !== last) { console.log(`  ${label}`); last = label; }
    if (now.autopilot === "idle" && now.dv === null) break;
  }
  const done = await snapshot();
  console.log(`  after burn ${burn}: pe ${km(done.pe)} ap ${km(done.ap)} km, prop ${done.prop}, ok ${done.ok}`);
}

const final = await page.evaluate(() => {
  const g = window.odderon;
  const e = g.vessel.elements;
  return {
    pe: Math.round((e.pe - 6371000) / 1000), ap: Math.round((e.ap - 6371000) / 1000),
    ok: g.evaluation.inTolerance, prop: Math.round(g.vessel.propellant), met: Math.round(g.met),
    target: g.contract.target, closed: g.closed,
  };
});
await page.click("#btn-end-flight");
await page.waitForTimeout(800);
const report = await page.evaluate(() => ({
  grade: document.querySelector("#debrief-grade")?.textContent,
  score: document.querySelector("#debrief-score")?.textContent,
  status: document.querySelector("#report-status")?.textContent,
}));
console.log({ id, final, report, errors });
await page.screenshot({ path: `tools/visual/.shots/fly-${id}.png` });
await browser.close();
