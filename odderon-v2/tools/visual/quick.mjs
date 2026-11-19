import chromium from "@sparticuz/chromium";
import { chromium as playwright } from "playwright-core";
import { mkdir } from "node:fs/promises";
import { resolve } from "node:path";
const outDir = resolve("tools/visual/.shots");
await mkdir(outDir, { recursive: true });
const browser = await playwright.launch({
  executablePath: "/tmp/chromium",
  args: [...chromium.args, "--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--in-process-gpu"],
  headless: true, timeout: 90000,
});
const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
const errors = [];
page.on("console", m => { if (m.type()==="error") errors.push(m.text().slice(0,400)); });
page.on("pageerror", e => errors.push(e.message.slice(0,500)));
await page.goto("http://127.0.0.1:5173/", { waitUntil: "load" });
await page.waitForTimeout(3000);
await page.screenshot({ path: `${outDir}/q-menu.png` });
await page.click(".launch-key");
await page.waitForTimeout(1800);
await page.screenshot({ path: `${outDir}/q-flight.png` });
await page.evaluate(() => { const g=window.odderon; g.engineArmed=true; g.throttle=1; g.setWarp(1); });
await page.waitForTimeout(1400);
await page.screenshot({ path: `${outDir}/q-engine.png` });
await page.evaluate(() => { const g=window.odderon; g.engineArmed=false; g.throttle=0; g.view.setCameraInput({distance:12,yaw:1.4,pitch:0.12}); });
await page.waitForTimeout(1200);
await page.screenshot({ path: `${outDir}/q-closeup.png` });
console.log("errors:", errors.length ? errors : "none");
await browser.close();
