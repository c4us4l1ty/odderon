/* Browser-level state machine checks: corrupt settings, overlay stacking,
 * keyboard attitude, restart from pause. */
import chromium from "@sparticuz/chromium";
import { chromium as pw } from "playwright-core";

const browser = await pw.launch({ executablePath: "/tmp/chromium", args: [...chromium.args, "--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--in-process-gpu"], headless: true, timeout: 90000 });
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
const errors = [];
page.on("pageerror", (e) => errors.push(e.message.slice(0, 200)));
page.on("console", (m) => { if (m.type() === "error") errors.push(m.text().slice(0, 200)); });

// 1. Corrupt settings must not break boot.
await page.addInitScript(() => {
  localStorage.setItem("odderon.settings", JSON.stringify({ sensitivity: "fast", grain: "yes", orbitLines: 3, invertY: null }));
});
await page.goto("http://127.0.0.1:5173/", { waitUntil: "load" });
await page.waitForFunction(() => Boolean(window.odderon), { timeout: 25000 });
const settings = await page.evaluate(() => ({ settings: window.odderon.screens?.settings, sens: window.odderon.sensitivity }));
console.log("corrupt settings ->", settings);

// 2. Pause -> settings -> escape must return to the pause card, not a frozen flight.
await page.evaluate(() => window.odderon.startFlight("kestrel-9"));
await page.waitForTimeout(600);
await page.keyboard.press("Escape");
await page.waitForTimeout(300);
const paused = await page.evaluate(() => ({ pauseOpen: !document.querySelector("#pause").classList.contains("is-hidden"), paused: window.odderon.paused }));
await page.click('[data-pause="settings"]');
await page.waitForTimeout(300);
const nested = await page.evaluate(() => ({
  settingsOpen: !document.querySelector("#settings").classList.contains("is-hidden"),
  pauseOpen: !document.querySelector("#pause").classList.contains("is-hidden"),
}));
await page.keyboard.press("Escape");
await page.waitForTimeout(300);
const back = await page.evaluate(() => ({
  settingsOpen: !document.querySelector("#settings").classList.contains("is-hidden"),
  pauseOpen: !document.querySelector("#pause").classList.contains("is-hidden"),
  paused: window.odderon.paused,
}));
console.log("pause/settings nesting ->", { paused, nested, back });

// 3. Escape from pause resumes.
await page.keyboard.press("Escape");
await page.waitForTimeout(300);
const resumed = await page.evaluate(() => ({ paused: window.odderon.paused, pauseOpen: !document.querySelector("#pause").classList.contains("is-hidden") }));
console.log("resume ->", resumed);

// 4. Keyboard attitude: holding W must spool the reaction wheels.
await page.evaluate(() => { document.querySelector("#viewport").focus(); });
await page.keyboard.down("w");
await page.waitForTimeout(1500);
const spin = await page.evaluate(() => window.odderon.vessel.angularVelocity.length());
await page.keyboard.up("w");
console.log("manual pitch rate ->", spin.toFixed(3), "rad/s");

// 4b. Manual input must abort a running autopilot.
await page.evaluate(() => {
  const g = window.odderon;
  document.querySelector("#btn-add-node").click();
  document.querySelector("#btn-execute").click();
});
await page.waitForTimeout(400);
const armed = await page.evaluate(() => window.odderon.autopilot);
await page.keyboard.down("a");
await page.waitForTimeout(300);
await page.keyboard.up("a");
const aborted = await page.evaluate(() => window.odderon.autopilot);
console.log("autopilot abort ->", { armed, aborted });

// 5. Restart from the pause menu resets the clock and hides overlays.
await page.evaluate(() => { window.odderon.met = 1234; });
await page.keyboard.press("Escape");
await page.waitForTimeout(300);
await page.click('[data-pause="restart"]');
await page.waitForTimeout(600);
const restarted = await page.evaluate(() => ({
  met: Math.round(window.odderon.met), paused: window.odderon.paused,
  pauseOpen: !document.querySelector("#pause").classList.contains("is-hidden"),
  prop: Math.round(window.odderon.vessel.propellant),
}));
console.log("restart ->", restarted);

console.log("errors:", errors);
await browser.close();
