import chromium from "@sparticuz/chromium";
import { chromium as pw } from "playwright-core";
const browser = await pw.launch({ executablePath: "/tmp/chromium", args: [...chromium.args,"--use-gl=angle","--use-angle=swiftshader","--enable-unsafe-swiftshader","--in-process-gpu"], headless: true });
const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
page.on("pageerror", e => console.log("PAGEERROR", e.message.slice(0,500)));
page.on("console", m => { if (m.type() === "error") console.log("CONSOLE", m.text().slice(0,300)); });
await page.goto("http://127.0.0.1:5173/", { waitUntil: "load" });
await page.waitForTimeout(2500);
await page.click(".launch-key");
await page.waitForTimeout(1200);
await page.click("#btn-add-node");
await page.waitForTimeout(600);
console.log("node:", await page.evaluate(() => { const g = window.odderon; return { dv: Math.round(g.node?.dv ?? 0), t: Math.round(g.node?.time ?? 0), pe: Math.round(g.planned?.pe ?? 0), ap: Math.round(g.planned?.ap ?? 0) }; }));
await page.click("#btn-execute");
let state = null;
for (let i = 0; i < 60; i++) {
  await page.waitForTimeout(3000);
  state = await page.evaluate(() => {
    const g = window.odderon;
    const el = g.vessel.elements;
    return { ap: g.autopilot, met: Math.round(g.met), warp: g.warp, dvDone: Math.round(g.burnDone), prop: Math.round(g.vessel.propellant), peKm: Math.round((el.pe/1000-6371)), apKm: Math.round((el.ap/1000-6371)), ok: g.evaluation.inTolerance, closed: g.closed };
  });
  if (i % 3 === 0 || state.ap === "burning" || state.ap === "idle") console.log(i, JSON.stringify(state));
  if (state.ap === "idle" && state.dvDone > 0) break;
}
console.log("final:", JSON.stringify(state));
await page.click("#btn-end-flight");
await page.waitForTimeout(900);
console.log("grade:", await page.evaluate(() => ({
  grade: document.querySelector("#debrief-grade")?.textContent,
  status: document.querySelector("#debrief-status")?.textContent,
  ap: document.querySelector("#debrief-ap")?.textContent,
  pe: document.querySelector("#debrief-pe")?.textContent,
  score: document.querySelector("#debrief-score")?.textContent,
})));
await page.screenshot({ path: "tools/visual/.shots/m-report.png" });
await browser.close();
