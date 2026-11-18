// Interface regression gate: the shipped screens must read as a game console,
// never as a website. This checks structure and computed style, so a later
// redesign cannot quietly reintroduce page chrome.
import { chromium } from 'playwright-core';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { resolve, extname } from 'node:path';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';

const root = resolve('web');
const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm', '.woff2': 'font/woff2', '.jpg': 'image/jpeg' };
const server = createServer(async (request, response) => {
  const path = resolve(root, `.${new URL(request.url, 'http://test').pathname.replace(/\/$/, '/index.html')}`);
  if (!path.startsWith(`${root}/`)) { response.writeHead(403).end(); return; }
  try {
    const body = await readFile(path);
    response.setHeader('content-type', types[extname(path)] ?? 'application/octet-stream');
    response.end(body);
  } catch { response.writeHead(404).end(); }
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const origin = `http://127.0.0.1:${server.address().port}`;

let browser;
try {
  const executablePath = process.env.CHROME_BIN || execFileSync('which', ['google-chrome'], { encoding: 'utf8' }).trim();
  browser = await chromium.launch({ executablePath, headless: true, args: ['--no-sandbox', '--use-angle=swiftshader', '--enable-unsafe-swiftshader', '--disable-dev-shm-usage'] });
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  const errors = [];
  page.on('pageerror', error => errors.push(String(error)));

  // ---- Every route loads the shared console system and no legacy sheet.
  for (const route of ['/', '/mission.html', '/observatory.html']) {
    await page.goto(`${origin}${route}`, { waitUntil: 'domcontentloaded' });
    const sheets = await page.evaluate(() => [...document.styleSheets].map(sheet => sheet.href?.split('/').pop()));
    assert.ok(sheets.includes('ui.css'), `${route} must load the shared console stylesheet, got ${sheets}`);
    for (const legacy of ['style.css', 'game.css', 'flight-shell.css', 'observatory-shell.css']) {
      assert.ok(!sheets.includes(legacy), `${route} must not load the retired sheet ${legacy}`);
    }
    // Boxed web form controls are forbidden: every text control uses the entry well.
    const controls = await page.evaluate(() => [...document.querySelectorAll('input,select,textarea')]
      .filter(node => !['checkbox', 'range', 'file', 'hidden'].includes(node.type))
      .map(node => {
        const style = getComputedStyle(node);
        return { id: node.id || node.name || node.tagName, appearance: style.appearance, shadow: style.boxShadow, radius: style.borderRadius };
      }));
    for (const control of controls) {
      assert.notEqual(control.appearance, 'auto', `${route} ${control.id} still uses the native control appearance`);
      assert.ok(control.shadow && control.shadow !== 'none', `${route} ${control.id} must sit in an instrument entry well`);
      assert.equal(control.radius, '0px', `${route} ${control.id} must be square, not a rounded web field`);
    }
  }

  // ---- Title screen: soft keys pinned low, camera plate, no site footer.
  await page.goto(origin, { waitUntil: 'networkidle' });
  const title = await page.evaluate(() => {
    const menu = document.querySelector('.game-menu').getBoundingClientRect();
    const footer = document.querySelector('.game-footer');
    return {
      menuBottomRatio: (menu.bottom / innerHeight),
      menuTopRatio: (menu.top / innerHeight),
      footerHeight: footer.getBoundingClientRect().height,
      footerText: footer.textContent.trim(),
      h1: document.querySelector('h1').textContent.trim(),
      plate: getComputedStyle(document.querySelector('.spacecraft')).backgroundImage,
      scroll: document.documentElement.scrollHeight <= innerHeight + 1,
      overflow: getComputedStyle(document.body).overflow,
    };
  });
  assert.ok(title.menuBottomRatio > .9, 'the main menu must be anchored to the bottom of the screen like a game front end');
  assert.ok(title.menuTopRatio > .6, `the main menu must not be a landing-page block, top at ${title.menuTopRatio}`);
  assert.ok(title.footerHeight < 40, `the status line must stay a strip, got ${title.footerHeight}px`);
  assert.doesNotMatch(title.footerText, /copyright|©|about|privacy|contact/i, 'the title screen must not carry a website footer');
  assert.equal(title.h1, 'ODDERON');
  assert.match(title.plate, /title-orbit\.jpg/);
  assert.equal(title.overflow, 'hidden');
  assert.ok(title.scroll, 'the title screen must fit one viewport');

  // ---- Flight Deck: fixed console, station rail on every station.
  await page.goto(`${origin}/mission.html`, { waitUntil: 'networkidle' });
  await page.waitForFunction(() => !document.getElementById('add-node')?.disabled, null, { timeout: 30000 });
  const deck = await page.evaluate(() => {
    const rail = document.querySelector('.flight-mode-nav').getBoundingClientRect();
    const strip = document.querySelector('.flight-command-strip').getBoundingClientRect();
    const heading = document.querySelector('.mission-main > .intro h1');
    return {
      overflow: getComputedStyle(document.body).overflow,
      scroll: document.documentElement.scrollHeight <= innerHeight + 1,
      railLeft: rail.left,
      railBottom: rail.bottom,
      stripTop: strip.top,
      headingSize: parseFloat(getComputedStyle(heading).fontSize),
      panelCount: document.querySelectorAll('[data-flight-panel].is-active').length,
    };
  });
  assert.equal(deck.overflow, 'hidden', 'the Flight Deck must never scroll the document');
  assert.ok(deck.scroll, 'the Flight Deck must fit one viewport');
  assert.equal(deck.railLeft, 0, 'the station rail anchors the left edge');
  assert.ok(deck.stripTop < 120, 'the command strip must sit at the top of the console');
  assert.ok(deck.headingSize < 26, `the station header must be console-scale, got ${deck.headingSize}px`);
  assert.equal(deck.panelCount, 1, 'exactly one station panel is live at a time');

  for (const mode of ['academy', 'propulsion', 'vehicle', 'prediction', 'flight']) {
    await page.locator(`[data-flight-mode="${mode}"]`).click();
    assert.equal(await page.evaluate(() => odderonFlightShell.mode), mode);
    const box = await page.evaluate(() => {
      const rail = document.querySelector('.flight-mode-nav').getBoundingClientRect();
      return { left: rail.left, bottom: rail.bottom, viewport: innerHeight };
    });
    assert.equal(box.left, 0, `station rail must stay docked on ${mode}`);
    assert.ok(Math.abs(box.bottom - box.viewport) < 2, `station rail must reach the bottom on ${mode}`);
  }

  // ---- Academy: the lesson well and the live flight board are separate
  // panels that both take real space; nothing hides behind `[hidden]` and no
  // control cluster keeps a slot after it is told to go away.
  await page.goto(`${origin}/mission.html#onboarding`, { waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => window.missionDiagnostics?.nodes !== undefined, null, { timeout: 60000 });
  await page.locator('#onboarding-start').click();
  const academy = await page.evaluate(() => {
    const well = document.getElementById('onboarding');
    const board = document.getElementById('flight-workspace');
    const hiddenVisible = [...document.querySelectorAll('[hidden]')]
      .filter(node => getComputedStyle(node).display !== 'none').map(node => node.id || node.className);
    return {
      progress: document.getElementById('onboarding-progress').textContent,
      wellHeight: well.getBoundingClientRect().height,
      // The well is either fully readable or scrolls inside its own bezel; what
      // it must never do is clip content without a way to reach it.
      wellReachable: well.scrollHeight <= well.clientHeight + 1 ||
        ['auto', 'scroll'].includes(getComputedStyle(well).overflowY),
      boardHeight: board.getBoundingClientRect().height,
      hiddenVisible,
      scroll: document.documentElement.scrollHeight <= innerHeight + 1,
    };
  });
  assert.match(academy.progress, /LESSON 1 \/ 7/, 'the guided flight must open on lesson one');
  assert.ok(academy.wellHeight > 120, 'the lesson well must keep a readable window');
  assert.ok(academy.wellReachable, 'lesson content below the fold must stay reachable');
  assert.ok(academy.boardHeight > 200, 'the academy must keep the live flight board visible');
  assert.deepEqual(academy.hiddenVisible, [], 'a hidden element must never stay painted');
  assert.ok(academy.scroll, 'the academy must fit one viewport');

  // ---- Propulsion: the rack keeps a usable cross-section on a 720p laptop.
  await page.setViewportSize({ width: 1280, height: 720 });
  await page.locator('[data-flight-mode="propulsion"]').click();
  await page.waitForTimeout(300);
  const lab = await page.evaluate(() => {
    const view = document.querySelector('.engine-view').getBoundingClientRect();
    const card = document.getElementById('engine-lab');
    return { height: view.height, width: view.width, overflow: card.scrollHeight - card.clientHeight };
  });
  assert.ok(lab.height >= 180, `the Schlieren cross-section needs real height, got ${lab.height}`);
  assert.ok(lab.overflow <= 0, 'the propulsion rack must fit its own bezel');
  await page.setViewportSize({ width: 1440, height: 900 });

  // ---- Phone: the same console, one hand, no document scroll. The board and
  // the rail must both stay on screen, and the header must keep every route out.
  await page.setViewportSize({ width: 390, height: 844 });
  await page.locator('[data-flight-mode="vehicle"]').click();
  await page.waitForTimeout(400);
  const phone = await page.evaluate(() => {
    const rail = document.querySelector('.flight-mode-nav').getBoundingClientRect();
    return {
      railVisible: rail.top > 300 && rail.top < innerHeight,
      railBottom: Math.abs(rail.bottom - innerHeight),
      headerWidth: document.querySelector('.game-header, .topbar').getBoundingClientRect().width,
      width: innerWidth,
      horizontal: document.documentElement.scrollWidth - innerWidth,
      bodyScroll: getComputedStyle(document.body).overflowY,
    };
  });
  assert.ok(phone.railVisible, 'the station rail must be a reachable game bar on a phone');
  assert.ok(phone.railBottom < 2, 'the game bar must sit on the bottom edge');
  assert.ok(phone.headerWidth <= phone.width + 1, 'the command bar must not overflow the phone');
  assert.ok(phone.horizontal <= 0, `no route may scroll sideways, overflow ${phone.horizontal}px`);
  await page.setViewportSize({ width: 1440, height: 900 });

  // ---- Observatory: viewport owns the frame, telemetry docks right.
  await page.goto(`${origin}/observatory.html`, { waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => window.odderonDiagnostics?.draws > 0, null, { timeout: 30000 });
  const station = await page.evaluate(() => {
    const scene = document.getElementById('scene').getBoundingClientRect();
    const side = document.querySelector('.obs-side').getBoundingClientRect();
    return {
      scroll: document.documentElement.scrollHeight <= innerHeight + 1,
      overflow: getComputedStyle(document.body).overflow,
      width: innerWidth,
      sceneWidthRatio: scene.width / innerWidth,
      sideRight: side.right,
      footers: document.querySelectorAll('footer').length,
    };
  });
  assert.ok(station.scroll, 'the tracking station must fit one viewport');
  assert.equal(station.overflow, 'hidden');
  assert.ok(station.sceneWidthRatio > .6, 'the ephemeris viewport must own the frame');
  assert.ok(Math.abs(station.sideRight - station.width) < 40, 'the telemetry column docks to the right edge');
  assert.equal(station.footers, 0, 'no route may carry a website footer');

  // ---- Legibility floor. Instrument type is small by design, but a label
  // below 8px is a smudge on a real display; nothing that carries words may
  // drop under it, at desktop or phone width.
  for (const [width, height, route] of [[1440, 900, '/'], [1440, 900, '/mission.html'], [390, 844, '/mission.html'], [1440, 900, '/observatory.html']]) {
    await page.setViewportSize({ width, height });
    await page.goto(`${origin}${route}`, { waitUntil: 'domcontentloaded' });
    await page.waitForFunction(() => document.querySelector('body *'), null, { timeout: 30000 });
    await page.waitForTimeout(600);
    const smudges = await page.evaluate(() => [...document.querySelectorAll('body *')].filter(element => {
      const text = [...element.childNodes].filter(node => node.nodeType === 3).map(node => node.textContent.trim()).join('');
      if (!text) return false;
      const box = element.getBoundingClientRect();
      if (!box.width || !box.height) return false;
      return parseFloat(getComputedStyle(element).fontSize) < 8;
    }).map(element => `${element.tagName}.${element.className} ${getComputedStyle(element).fontSize}px`));
    assert.deepEqual(smudges, [], `${route} at ${width}px must keep every word at 8px or larger`);
  }
  await page.setViewportSize({ width: 1440, height: 900 });

  // ---- Canvas type uses the bundled instrument faces. A canvas cannot inherit
  // a CSS font stack, so a system fallback here is invisible to the DOM audit
  // and would quietly make the scene look like a web chart.
  for (const route of ['/mission.html', '/observatory.html']) {
    await page.goto(`${origin}${route}`, { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(1500);
    const fallback = await page.evaluate(() => {
      // `document.fonts.check` reports false for a face that is not resident, so
      // this fails if the drawers could paint in a system fallback.
      return {
        mono: document.fonts.check('10px "Odderon Data"'),
        display: document.fonts.check('500 11px "Odderon Display"'),
      };
    });
    assert.ok(fallback.mono, `${route} must have the Odderon Data face resident before painting labels`);
    assert.ok(fallback.display, `${route} must have the Odderon Display face resident`);
  }

  assert.deepEqual(errors, [], 'the interface must load without page errors');
  console.log('Interface standard holds: console shell, instrument controls, no page chrome.');
} finally {
  await browser?.close();
  server.close();
}
