import chromium from "@sparticuz/chromium";
import { chromium as pw } from "playwright-core";
const browser = await pw.launch({ executablePath: "/tmp/chromium", args: [...chromium.args,"--use-gl=angle","--use-angle=swiftshader","--enable-unsafe-swiftshader","--in-process-gpu"], headless: true });
const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
await page.goto("http://127.0.0.1:5173/", { waitUntil: "load" });
await page.waitForTimeout(2000);
await page.click(".launch-key");
await page.waitForTimeout(600);
const out = await page.evaluate(() => {
  const g = window.odderon;
  const v = g.vessel;
  const s = v.state;
  const el = v.elements;
  return {
    r: [s.r.x, s.r.y, s.r.z], v: [s.v.x, s.v.y, s.v.z],
    el: { a: el.a, e: el.e, i: el.i, raan: el.raan, argp: el.argp, nu: el.nu, p: el.p, pe: el.pe, ap: el.ap ?? null, period: el.period },
  };
});
console.log(JSON.stringify(out));
await browser.close();
