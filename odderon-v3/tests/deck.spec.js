import { test, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";

// Watch the entire context, including station iframes and replay workers.
// A UI assertion alone can pass while an unused panel makes a backend call.
const reports = new WeakMap();
test.beforeEach(async ({ context, page, baseURL }) => {
  const failures = [];
  reports.set(context, failures);
  page.on("pageerror", (err) => failures.push(err.message));
  context.on("response", (r) => {
    if (r.status() >= 400) failures.push(`${r.status()}: ${r.url()}`);
  });
  await context.route(/^https?:/, (route) => {
    const url = new URL(route.request().url());
    if (
      url.origin !== new URL(baseURL).origin ||
      url.pathname.startsWith("/api/")
    ) {
      failures.push(url.href);
      return route.abort();
    }
    return route.continue();
  });
});
test.afterEach(async ({ context }) => {
  expect(reports.get(context)).toEqual([]);
});

test("fly, pause, compare, sign, file and replay without a network authority", async ({
  page,
}, info) => {
  await page.goto("/");
  await expect(page.locator("#fuel")).toHaveText("180.0");
  await page.click("#contracts");
  await page.click("#predict");
  for (const [i, v] of [550, 550, 120, 170, 3000].entries())
    await page.locator(`[name=p${i}]`).fill(String(v));
  await page.click("#forecast button");
  await page.click("#launch");
  await page.click("#arm");
  await page.keyboard.down("KeyZ");
  await expect(page.locator("#pct")).not.toHaveText("0%");
  await page.keyboard.up("KeyZ");
  await expect(page.locator("#fuel")).not.toHaveText("180.0");
  await page.click("#report");
  const elapsed = await page.locator("#met").textContent();
  await page.waitForTimeout(250);
  expect(await page.locator("#met").textContent()).toBe(elapsed);
  const dl = page.waitForEvent("download");
  await page.click("#export");
  const tape = await dl;
  const p = info.outputPath("manual.json");
  await tape.saveAs(p);
  await page.locator("#tape-file").setInputFiles(p);
  await expect(page.locator("#verdict")).toContainText("verified locally");
  const data = JSON.parse(await readFile(p));
  data.body.commands[0][2] += 0.01;
  await page.locator("#tape-file").setInputFiles({
    name: "modified.json",
    mimeType: "application/json",
    buffer: Buffer.from(JSON.stringify(data)),
  });
  await expect(page.locator("#verdict")).toHaveText("Invalid pilot signature");
  await page.click("#file");
  await expect(page.locator("#sheet h2")).toHaveText("Filed flight report");
  await page.reload();
  await expect(page.locator("#fuel")).toHaveText("180.0");
  await page.click("#report");
  await page.click("#last-tape");
  await expect(page.locator("#verdict")).toContainText("verified locally");
});

test("all offline stations and fitted-stage transfer", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".online")).toHaveText("SYSTEM ONLINE");
  await page.click("#contracts");
  await page.click('[data-c="103"]');
  await page.click("[data-close]");
  await expect(page.locator("#cid")).toHaveText("103");
  await page.click("[data-station=propulsion]");
  const frame = page.frameLocator("#engine-frame");
  await expect(frame.locator("#station-surface")).toHaveAttribute(
    "data-mode",
    "propulsion",
  );
  await expect(frame.locator("#mission-select")).toHaveValue("103");
  await page.click("[data-tab=vehicle]");
  await expect(frame.locator("#station-surface")).toHaveAttribute(
    "data-mode",
    "vehicle",
  );
  await page.click("[data-tab=academy]");
  await expect(frame.locator("#onboarding-start")).toBeVisible();
  await page.click("[data-tab=license]");
  await expect(frame.locator("#profile-dialog")).toBeVisible();
  await frame.locator("#profile-close").click();
  await page.click("[data-tab=nav]");
  await frame.locator("#example").click();
  await expect(frame.locator("#split-kick")).toBeVisible();
  await page.click("#fit");
  await expect(page.locator("#entries")).toContainText("fit-out transferred");
  await page.click("#mapBtn");
  await expect(page.locator("#map")).toBeVisible();
  await page.click("[data-station=obs]");
  await expect(page.frameLocator("#obs-frame").locator("#distance")).toHaveText(
    /[0-9]/,
    { timeout: 20000 },
  );
  await page.click("#mfd-close");
  await expect(page.locator("#cid")).toHaveText("103");
});

test("planned experiment → signed FDR → verified downloadable dossier", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.locator(".online")).toHaveText("SYSTEM ONLINE");
  await page.click("[data-station=nav]");
  const bench = page.frameLocator("#engine-frame");
  await expect(page.locator("#fit")).toBeEnabled();
  await bench.locator("#example").click();
  await page.click("[data-tab=prediction]");
  await bench.locator("#prefill").click();
  await bench.locator("#execute").click();
  await expect(page.locator("#mfd-title")).toHaveText(
    "Planned-flight debrief",
    { timeout: 30000 },
  );
  const fdr = page.waitForEvent("download");
  await bench.locator("#export-fdr").click();
  const recorder = await readFile(await (await fdr).path(), "utf8");
  await page.click("[data-tab=verify]");
  await bench.locator("#verify-input").fill(recorder);
  await bench.locator("#verify-run").click();
  await expect(bench.locator("#verify-verdict")).toHaveClass(/is-verified/, {
    timeout: 30000,
  });
  await expect(bench.locator("#verify-certification")).toContainText(
    "PRACTICE",
  );
  const download = page.waitForEvent("download");
  await bench.locator("#verify-download").click();
  const html = await readFile(await (await download).path(), "utf8");
  for (const marker of [
    'id="coalition-table"',
    'id="engine-table"',
    "#verify=",
  ]) {
    expect(html).toContain(marker);
  }
});

test("redesigned engine and delegation fly on contract 102 and survive signed replay", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.locator(".online")).toHaveText("SYSTEM ONLINE");
  await page.click("#contracts");
  await page.click('[data-c="102"]');
  await page.click("[data-close]");
  await page.click("[data-station=propulsion]");
  const bench = page.frameLocator("#engine-frame");
  await expect(page.locator("#fit")).toBeEnabled();
  await bench.locator("#engine-throat").fill("19");
  await bench.locator("#engine-exit").fill("140");
  await bench.locator("#engine-apply").click();
  await page.click("[data-tab=vehicle]");
  await bench.locator("[for=delegate-3],label:has(#delegate-3)").click();
  await page.click("#fit");
  await expect(page.locator("#entries")).toContainText("fit-out transferred");
  await page.click("#arm");
  await page.keyboard.down("KeyZ");
  await expect(page.locator("#pct")).not.toHaveText("0%");
  await page.keyboard.up("KeyZ");
  await expect(page.locator("#met")).not.toHaveText("00:00:00");
  await page.click("#report");
  const download = page.waitForEvent("download");
  await page.click("#export");
  const path = await (await download).path(),
    tape = JSON.parse(await readFile(path));
  expect(tape.body.stage.id).toBe(102);
  expect(tape.body.stage.engine.throat).toBeCloseTo(0.019, 8);
  expect(tape.body.stage.assembly.mask).toBe(8);
  expect(tape.body.terminal[12]).toBeGreaterThan(0);
  await page.locator("#tape-file").setInputFiles(path);
  await expect(page.locator("#verdict")).toContainText("verified locally");
});

test("compact flight deck can steer without a keyboard", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/");
  await expect(page.locator(".online")).toHaveText("SYSTEM ONLINE");
  expect(
    await page.evaluate(() => document.documentElement.scrollWidth),
  ).toBeLessThanOrEqual(390);
  const stick = page.getByRole("button", { name: "Pitch up", exact: true });
  await stick.scrollIntoViewIfNeeded();
  const box = await stick.boundingBox();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  try {
    await expect(page.locator("#pitch")).not.toHaveText("+0.0°");
  } finally {
    await page.mouse.up();
  }
  await page.click("#report");
  await expect(page.locator("#sheet h2")).toHaveText("Flight review");
});

test('warp cannot interrupt an active attitude command', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('.online')).toHaveText('SYSTEM ONLINE');
  await page.keyboard.down('KeyW');
  try {
    await page.keyboard.press('Period');
    await page.keyboard.press('Period');
    await expect(page.locator('#warp strong')).toHaveText('1×');
    await expect(page.locator('#pitch')).not.toHaveText('+0.0°');
  } finally { await page.keyboard.up('KeyW'); }
  await page.keyboard.press('Period');
  await expect(page.locator('#warp strong')).toHaveText('5×');
  await expect(page.locator('#entries')).not.toContainText('Propagation stopped');
});

test('closing a pending file dialog does not file or stop a resumed flight', async ({ page }) => {
  let release;
  const gate = new Promise(resolve => { release = resolve; });
  await page.route('**/stations/identity.js', async route => { await gate; await route.continue(); });
  await page.goto('/');
  await expect(page.locator('.online')).toHaveText('SYSTEM ONLINE');
  await page.click('#fly');
  await expect(page.locator('#met')).not.toHaveText('00:00:00');
  await page.click('#report');
  const request = page.waitForRequest('**/stations/identity.js');
  await page.click('#file');
  try {
    await request;
    await page.click('[data-close]');
    await page.click('#fly');
  } finally { release(); }
  const met = await page.locator('#met').textContent();
  await expect(page.locator('#met')).not.toHaveText(met);
  await expect(page.locator('#sheet')).not.toBeVisible();
  expect(await page.evaluate(() => localStorage.getItem('odderon.manual.last.v1'))).toBeNull();
});
