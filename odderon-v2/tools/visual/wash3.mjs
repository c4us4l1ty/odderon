import chromium from "@sparticuz/chromium";
import { chromium as pw } from "playwright-core";
const browser = await pw.launch({ executablePath: "/tmp/chromium", args: [...chromium.args,"--use-gl=angle","--use-angle=swiftshader","--enable-unsafe-swiftshader","--in-process-gpu"], headless: true });
const page = await browser.newPage({ viewport: { width: 1200, height: 700 } });
await page.goto("http://127.0.0.1:5173/", { waitUntil: "load" });
await page.waitForTimeout(2200);
await page.click(".launch-key");
await page.waitForTimeout(1000);
await page.keyboard.press("3");
await page.waitForTimeout(1500);
await page.keyboard.press("2");
await page.waitForTimeout(1500);
await page.keyboard.press("1");
await page.waitForTimeout(1500);
await page.screenshot({ path: "tools/visual/.shots/w3-flight.png" });
await page.evaluate(() => { const v = window.odderon.view; v.sunSprite.visible = false; v.sunCore.visible = false; });
await page.waitForTimeout(400);
await page.screenshot({ path: "tools/visual/.shots/w3-nosun.png" });
console.log(await page.evaluate(() => {
  const v = window.odderon.view;
  const d = v.camera.getWorldDirection(new (window.odderon.vessel.state.r.constructor)());
  const sun = v.sunDirection;
  return { sunDotView: +sun.dot(d).toFixed(3), camYaw: v.cameraState.yaw, camPitch: v.cameraState.pitch,
    sunSpriteSize: Math.round(v.sunSprite.scale.x), sunDist: Math.round(v.sunSprite.position.distanceTo(v.camera.position)),
    mode: v.cameraState.mode ?? v.camMode };
}));
await browser.close();
