/* Headless visual harness for UI review. Development tool only. */

import chromium from "@sparticuz/chromium";
import { chromium as playwright } from "playwright-core";
import { mkdir } from "node:fs/promises";
import { resolve } from "node:path";

const url = process.argv[2] ?? "http://127.0.0.1:5173/";
const outDir = resolve(process.argv[3] ?? "tools/visual/.shots");
await mkdir(outDir, { recursive: true });

const browser = await playwright.launch({
  executablePath: process.env.CHROME_PATH ?? "/tmp/chromium",
  args: [...chromium.args, "--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--in-process-gpu"],
  headless: true,
  timeout: 90_000,
});

const page = await browser.newPage({ viewport: { width: 1600, height: 900 }, deviceScaleFactor: 1 });
const errors = [];
page.on("console", (msg) => {
  if (msg.type() === "error" || msg.type() === "warning") errors.push(`${msg.type()}: ${msg.text()}`.slice(0, 500));
});
page.on("pageerror", (err) => errors.push(`pageerror: ${err.message}`.slice(0, 800)));

const shot = async (name) => page.screenshot({ path: `${outDir}/${name}.png` });

await page.goto(url, { waitUntil: "load", timeout: 90_000 });
await page.waitForTimeout(3500);
await shot("01-menu");

await page.click(".launch-key");
await page.waitForTimeout(2500);
await shot("02-flight-idle");

// Light the engine and open the throttle for the hero frame.
await page.evaluate(() => {
  const g = window.odderon;
  g.engineArmed = true;
  g.throttle = 1;
  g.setWarp(1);
});
await page.waitForTimeout(2200);
await shot("03-engine");

// Close-up of the vehicle.
await page.evaluate(() => {
  const g = window.odderon;
  g.view.setCameraInput({ distance: 14, yaw: 1.35, pitch: 0.10 });
});
await page.waitForTimeout(1200);
await shot("04-closeup");
await page.evaluate(() => {
  const g = window.odderon;
  g.engineArmed = false;
  g.throttle = 0;
  g.view.setCameraInput({ distance: 30, yaw: 0.98, pitch: 0.34 });
});
await page.waitForTimeout(600);

// Restart from the clean launch state so the demo burn is textbook.
await page.keyboard.press("Escape");
await page.waitForTimeout(400);
await page.click('[data-pause="restart"]');
await page.waitForTimeout(1200);

// Plan and execute a node.
await page.click("#btn-add-node");
await page.waitForTimeout(700);
await shot("05-manoeuvre");
await page.click("#btn-execute");
await page.waitForTimeout(1800);
await shot("06-autopilot");

// Catch the coast at high warp, then the manoeuvre itself.
await page.keyboard.press("3");
await page.waitForTimeout(2200);
await shot("07-map");
await page.keyboard.press("2");
await page.waitForTimeout(1500);
await shot("08-chase");
await page.keyboard.press("1");
await page.waitForTimeout(400);

// Wait for the closed-loop burn to finish.
for (let i = 0; i < 60; i++) {
  const state = await page.evaluate(() => window.odderon.autopilot);
  if (state === "idle") break;
  if (state === "burning") await shot("09-burn");
  await page.waitForTimeout(1500);
}
await page.waitForTimeout(600);
await shot("09-burn-complete");

await page.keyboard.press("Escape");
await page.waitForTimeout(500);
await shot("10-pause");
await page.keyboard.press("Escape");
await page.waitForTimeout(300);

// The settings and operations panels.
await page.keyboard.press("Escape");
await page.waitForTimeout(300);
await page.click('[data-pause="settings"]');
await page.waitForTimeout(300);
await shot("11-settings");
await page.click('[data-close="settings"]');
await page.waitForTimeout(250);
const pauseOpen = await page.evaluate(() => !document.querySelector("#pause")?.classList.contains("is-hidden"));
if (!pauseOpen) {
  await page.keyboard.press("Escape");
  await page.waitForTimeout(300);
}
await page.click('[data-pause="controls"]');
await page.waitForTimeout(300);
await shot("12-controls");
await page.click('[data-close="controls"]');
await page.waitForTimeout(250);

// File the report (leave the pause overlay first).
await page.keyboard.press("Escape");
await page.waitForTimeout(450);
await page.click("#btn-end-flight");
await page.waitForTimeout(800);
await shot("13-report");

const data = await page.evaluate(() => ({
  ap: document.querySelector("#debrief-ap")?.textContent,
  pe: document.querySelector("#debrief-pe")?.textContent,
  grade: document.querySelector("#debrief-grade")?.textContent,
  hiddenHud: document.querySelector("#hud")?.classList.contains("is-hidden"),
}));
console.log("report:", data);
console.log("console issues:", errors.length ? JSON.stringify(errors, null, 2) : "none");
console.log("screenshots in", outDir);
await browser.close();
