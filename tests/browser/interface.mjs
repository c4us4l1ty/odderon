// Interface regression gate: the shipped screens must read as a heads-up
// display painted over the game, never as a document with page chrome. This
// checks structure, computed style and geometry, so a later change cannot
// quietly reintroduce cards, panels, bars or web form controls.
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
  page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });

  // ---- Route law: every screen loads one shared HUD layer and no page sheet.
  for (const route of ['/', '/mission.html', '/observatory.html']) {
    await page.goto(`${origin}${route}`, { waitUntil: 'domcontentloaded' });
    const sheets = await page.evaluate(() => [...document.styleSheets].map(sheet => sheet.href?.split('/').pop()));
    assert.ok(sheets.includes('hud.css'), `${route} must load the shared HUD layer, got ${sheets}`);
    for (const retired of ['style.css', 'ui.css', 'game.css', 'flight-shell.css', 'observatory-shell.css']) {
      assert.ok(!sheets.includes(retired), `${route} must not load the retired sheet ${retired}`);
    }
  }

  // ---- THE PLATE LAW. A cockpit is full of plates: MFD bezels, instrument
  // racks, switch panels. What a game console must never have is what a web
  // page is made of — rounded cards, flat light fills, floating content boxes.
  // So a large surface is allowed, but it must be machined: square corners, a
  // glass-dark fill, and no bright card behind type.
  const SURFACE_ROUTES = [['/', null], ['/mission.html', null], ['/mission.html', 'academy'],
    ['/mission.html', 'propulsion'], ['/mission.html', 'vehicle'], ['/mission.html', 'prediction'],
    ['/observatory.html', null]];
  const luminance = value => {
    const numbers = /rgba?\(([^)]+)\)/.exec(value)?.[1].split(',').map(Number) ?? [0, 0, 0];
    const [r, g, b] = numbers.map(channel => {
      const c = channel / 255;
      return c <= .03928 ? c / 12.92 : ((c + .055) / 1.055) ** 2.4;
    });
    return .2126 * r + .7152 * g + .0722 * b;
  };
  const surfaceReport = async () => page.evaluate(() => {
    const offences = [];
    for (const element of document.querySelectorAll('body *')) {
      if (!element.offsetParent || element.closest('dialog:not([open])')) continue;
      const box = element.getBoundingClientRect();
      if (box.width < 120 || box.height < 50) continue;
      const style = getComputedStyle(element);
      const radius = parseFloat(style.borderTopLeftRadius) || 0;
      const fill = style.backgroundColor;
      const channels = /rgba?\(([^)]+)\)/.exec(fill)?.[1].split(',').map(Number) ?? [0, 0, 0, 1];
      const [r, g, b] = channels;
      const fillAlpha = channels.length > 3 ? channels[3] : 1;
      // Paint that is 92% transparent is a grid seam, not a card. What matters
      // is the rendered weight: an opaque bright panel, not a hairline.
      const light = ((r * .299 + g * .587 + b * .114) / 255) * fillAlpha;
      if (radius > 0) {
        offences.push(`${element.tagName.toLowerCase()}.${String(element.className).split(' ')[0] || element.id} is a rounded card (${radius}px)`);
      }
      // A flat, bright plate behind type is a web card. Instrument glass is
      // either a dark fill or a gradient (which reports a transparent colour).
      if (/^rgb/.test(fill) && light > .22) {
        offences.push(`${element.tagName.toLowerCase()}.${String(element.className).split(' ')[0] || element.id} has a flat light card fill (${fill})`);
      }
    }
    return [...new Set(offences)];
  });

  // ---- THE WEBSITE LAW. The mechanical signature of a web page, checked on
  // whichever station is live: marketing prose blocks, a page title heading,
  // nav-link rows, and a picture that is decoration rather than an instrument.
  const websiteReport = async () => page.evaluate(() => {
    const offences = [];
    // No document-flow prose: a paragraph longer than a briefing line, or set
    // as a centred marketing block, is copy, not a readout.
    for (const element of document.querySelectorAll('p,li,dd')) {
      if (!element.offsetParent) continue;
      // A folded model-limits strip belongs to the manual, not the console:
      // it is reference material the pilot opens, like the aircraft's AFM.
      if (element.closest('details:not([open])')) continue;
      const text = element.textContent.trim();
      const style = getComputedStyle(element);
      const box = element.getBoundingClientRect();
      if (text.length > 240 && box.width > 520) offences.push(`prose block ${text.length}ch in .${String(element.className).split(' ')[0] || element.tagName}`);
      if (text.length > 80 && style.textAlign === 'center') offences.push(`centred marketing copy in .${String(element.className).split(' ')[0] || element.tagName}`);
    }
    // No page title heading on a game screen.
    const headings = [...document.querySelectorAll('h1')].filter(node => node.offsetParent);
    for (const heading of headings) {
      if (parseFloat(getComputedStyle(heading).fontSize) > 42 && location.pathname !== '/') {
        offences.push(`page-title heading "${heading.textContent.trim().slice(0, 24)}" on a game screen`);
      }
    }
    // A console is not a site map: the shell keeps a handful of function keys.
    const anchors = [...document.querySelectorAll('a[href]')].filter(node => node.offsetParent);
    if (anchors.length > 5) offences.push(`${anchors.length} navigation links in the shell`);
    if (document.querySelector('footer a[href]')) offences.push('site-map links in a footer');
    return offences;
  });

  for (const [route, station] of SURFACE_ROUTES) {
    await page.goto(`${origin}${route}`, { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(900);
    if (station) {
      await page.locator(`[data-flight-mode="${station}"]`).click();
      await page.waitForTimeout(500);
    }
    const offences = await surfaceReport();
    assert.deepEqual(offences, [], `${route}${station ? ` (${station})` : ''} must paint machined plates, not web cards`);
    const web = await websiteReport();
    assert.deepEqual(web, [], `${route}${station ? ` (${station})` : ''} must not read as a web page`);
  }

  // ---- THE INSTRUMENT LAW. On the flight deck the picture is a live
  // instrument that owns the frame; the station rail is bolted to an edge and
  // exactly one station is powered. A page with a decorative illustration
  // cannot satisfy this.
  await page.goto(`${origin}/mission.html`, { waitUntil: 'domcontentloaded' });
  await page.waitForTimeout(1200);
  const instrument = await page.evaluate(() => {
    const canvas = document.getElementById('plan-canvas').getBoundingClientRect();
    const rail = document.querySelector('.flight-mode-nav').getBoundingClientRect();
    return {
      canvasArea: (canvas.width * canvas.height) / (innerWidth * innerHeight),
      canvasHeight: canvas.height / innerHeight,
      railEdge: Math.min(rail.left, innerWidth - rail.right),
      railShare: (rail.width * rail.height) / (innerWidth * innerHeight),
      live: document.querySelectorAll('[data-flight-panel].is-active').length,
    };
  });
  assert.ok(instrument.canvasArea > .25, `the orbit instrument must own the frame, got ${instrument.canvasArea.toFixed(2)} of it`);
  assert.ok(instrument.canvasHeight > .4, `the orbit instrument must have real height, got ${(instrument.canvasHeight * 100).toFixed(0)}vh`);
  assert.ok(instrument.railEdge < 1, 'the station rail is bolted to an edge of the frame');
  assert.ok(instrument.railShare > .03, 'the station rail is a hardware strip, not a hidden menu');
  assert.equal(instrument.live, 1, 'exactly one station is powered at a time');

  // ---- Control law: no native appearance anywhere, square corners, and the
  // typed fields sit on a ruled entry line rather than in a web form field.
  for (const route of ['/', '/mission.html', '/observatory.html']) {
    await page.goto(`${origin}${route}`, { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(700);
    const controls = await page.evaluate(() => [...document.querySelectorAll('input,select,textarea,button')]
      .filter(node => node.offsetParent && !['checkbox', 'range', 'file', 'hidden', 'radio'].includes(node.type))
      .map(node => {
        const style = getComputedStyle(node);
        return { id: node.id || node.tagName, tag: node.tagName, appearance: style.appearance || style.webkitAppearance, radius: style.borderRadius, shadow: style.boxShadow };
      }));
    for (const control of controls) {
      assert.equal(control.appearance, 'none', `${route} ${control.id} still uses the native control appearance`);
      assert.equal(control.radius, '0px', `${route} ${control.id} must be square, not a rounded web field`);
      // A typed field must show the entry rule. A button may be bare in the
      // top bar, where it is a mark rather than a key with a well.
      if (['INPUT', 'SELECT', 'TEXTAREA'].includes(control.tag)) {
        assert.ok(control.shadow !== 'none', `${route} ${control.id} must sit on the instrument entry rule`);
      }
    }
    // A slider must be a real hit area, not a flattened 2px line: a catch-all
    // input rule once matched `[type=range]` and made every slider unusable.
    const sliders = await page.evaluate(() => [...document.querySelectorAll('input[type=range]')]
      .filter(node => node.getBoundingClientRect().width > 0)
      .map(node => ({ id: node.id, height: node.getBoundingClientRect().height })));
    for (const slider of sliders) {
      assert.ok(slider.height >= 20, `${route} ${slider.id} slider must keep a 20px hit area, got ${slider.height}px`);
    }
  }

  // ---- Frame law: the document never scrolls. Each screen owns one viewport,
  // and anything dense scrolls inside its own well.
  for (const [width, height, route] of [[1440, 900, '/'], [1440, 900, '/mission.html'],
    [1440, 900, '/observatory.html'], [1280, 720, '/mission.html'], [390, 844, '/'],
    [390, 844, '/mission.html'], [390, 844, '/observatory.html'], [1024, 600, '/mission.html']]) {
    await page.setViewportSize({ width, height });
    await page.goto(`${origin}${route}`, { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(800);
    const frame = await page.evaluate(() => ({
      overflow: getComputedStyle(document.body).overflow,
      position: getComputedStyle(document.body).position,
      vertical: document.documentElement.scrollHeight - innerHeight,
      horizontal: document.documentElement.scrollWidth - innerWidth,
    }));
    assert.equal(frame.overflow, 'hidden', `${route} at ${width}px must lock the document`);
    assert.ok(frame.vertical <= 0, `${route} at ${width}px must not scroll the document vertically, overflow ${frame.vertical}px`);
    assert.ok(frame.horizontal <= 0, `${route} at ${width}px must not scroll the document horizontally, overflow ${frame.horizontal}px`);
  }
  await page.setViewportSize({ width: 1440, height: 900 });

  // ---- Title frame: a camera plate with a soft-key stack on the lower edge.
  await page.goto(origin, { waitUntil: 'networkidle' });
  const title = await page.evaluate(() => {
    const menu = document.querySelector('.game-menu').getBoundingClientRect();
    const plate = document.querySelector('.spacecraft');
    return {
      menuBottomRatio: menu.bottom / innerHeight,
      menuTopRatio: menu.top / innerHeight,
      h1: document.querySelector('h1').textContent.trim(),
      plate: getComputedStyle(plate).backgroundImage,
      footerText: document.querySelector('.game-footer').textContent.trim(),
      focus: document.activeElement?.getAttribute('href'),
    };
  });
  assert.ok(title.menuBottomRatio > .9, 'the soft-key stack must sit on the lower edge');
  assert.ok(title.menuTopRatio > .55, `the keys must not become a landing-page block, top at ${title.menuTopRatio}`);
  assert.equal(title.h1, 'ODDERON');
  assert.match(title.plate, /title-orbit\.jpg/, 'the title frame must paint the camera plate');
  assert.doesNotMatch(title.footerText, /copyright|©|about|privacy|contact|terms/i, 'no website footer furniture');
  assert.equal(title.focus, './mission.html', 'Enter must select the focused primary game action');

  // ---- Flight Deck: rail docked left, one station live, keys work.
  await page.goto(`${origin}/mission.html`, { waitUntil: 'networkidle' });
  await page.waitForFunction(() => !document.getElementById('add-node')?.disabled, null, { timeout: 30000 });
  const deck = await page.evaluate(() => {
    const rail = document.querySelector('.flight-mode-nav').getBoundingClientRect();
    return {
      railLeft: rail.left,
      panelCount: document.querySelectorAll('[data-flight-panel].is-active').length,
      surface: document.getElementById('station-surface').dataset.mode,
    };
  });
  assert.equal(deck.railLeft, 0, 'the station rail anchors the left edge');
  assert.equal(deck.panelCount, 1, 'exactly one station is live at a time');
  assert.equal(deck.surface, 'flight');

  for (const station of ['academy', 'propulsion', 'vehicle', 'prediction', 'flight']) {
    await page.locator(`[data-flight-mode="${station}"]`).click();
    assert.equal(await page.evaluate(() => window.odderonFlightShell.mode), station);
    const rail = await page.evaluate(() => {
      const box = document.querySelector('.flight-mode-nav').getBoundingClientRect();
      const frame = document.querySelector('.console-body').getBoundingClientRect();
      return { left: box.left, bottom: box.bottom, frameBottom: frame.bottom };
    });
    assert.equal(rail.left, 0, `the station rail must stay docked on ${station}`);
    assert.ok(Math.abs(rail.bottom - rail.frameBottom) < 2, `the station rail must fill its frame column on ${station}`);
  }

  // ---- On a phone the rail becomes a game bar on the bottom edge.
  await page.setViewportSize({ width: 390, height: 844 });
  await page.waitForTimeout(400);
  const phoneRail = await page.evaluate(() => {
    const box = document.querySelector('.flight-mode-nav').getBoundingClientRect();
    return { left: box.left, bottom: Math.abs(box.bottom - innerHeight), width: box.width };
  });
  assert.ok(phoneRail.bottom < 70, `the phone game bar must sit at the foot of the frame, ${phoneRail.bottom}px away`);
  assert.ok(phoneRail.width > 300, 'the phone game bar must span the frame');
  await page.setViewportSize({ width: 1440, height: 900 });

  // ---- Every station keeps its working controls, which is what "the core
  // things are still there" means: the wipe is skin, not function.
  await page.goto(`${origin}/mission.html`, { waitUntil: 'networkidle' });
  await page.waitForFunction(() => window.missionDiagnostics?.nodes !== undefined, null, { timeout: 60000 });
  await page.locator('[data-flight-mode="propulsion"]').click();
  await page.waitForTimeout(600);
  const lab = await page.evaluate(() => ({
    cross: document.getElementById('engine-canvas').getBoundingClientRect().height,
    readouts: document.querySelectorAll('#engine-readout > div').length,
    sliders: [...document.querySelectorAll('#engine-lab input[type=range]')].length,
  }));
  assert.ok(lab.cross >= 180, `the Schlieren cross-section needs real height, got ${lab.cross}`);
  assert.ok(lab.readouts >= 10, `the engine rack must publish its numbers, got ${lab.readouts}`);
  assert.equal(lab.sliders, 3, 'the engine lab keeps its three drag tracks');

  await page.locator('[data-flight-mode="prediction"]').click();
  await page.waitForTimeout(400);
  const prediction = await page.evaluate(() => document.querySelectorAll('#prediction-workspace input[type=number]').length);
  assert.equal(prediction, 6, 'the prediction contract keeps all six committed fields');

  await page.locator('[data-flight-mode="vehicle"]').click();
  await page.waitForTimeout(400);
  const vehicle = await page.evaluate(() => ({
    delegation: document.querySelectorAll('#delegation-controls input[type=checkbox]').length,
    ledger: document.querySelectorAll('#assembly-readout > div').length,
  }));
  assert.equal(vehicle.delegation, 4, 'the assembly keeps all four delegated disciplines');
  assert.ok(vehicle.ledger >= 4, 'the assembly keeps its mass ledger');

  // ---- Academy: the lesson well is readable and the live board stays visible.
  await page.goto(`${origin}/mission.html#onboarding`, { waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => window.missionDiagnostics?.nodes !== undefined, null, { timeout: 60000 });
  await page.locator('#onboarding-start').click();
  const academy = await page.evaluate(() => ({
    progress: document.getElementById('onboarding-progress').textContent,
    well: document.getElementById('onboarding').getBoundingClientRect().height,
    board: document.getElementById('flight-workspace').getBoundingClientRect().height,
    paintedHidden: [...document.querySelectorAll('[hidden]')].filter(node => getComputedStyle(node).display !== 'none').length,
  }));
  assert.match(academy.progress, /LESSON 1 \/ 7/, 'the guided flight must open on lesson one');
  assert.ok(academy.well > 100, 'the lesson well must keep a readable window');
  assert.ok(academy.board > 200, 'the academy must keep the live flight board visible');
  assert.equal(academy.paintedHidden, 0, 'a retired element must never stay painted');

  // ---- Tracking station: the viewport owns the frame, telemetry floats on it.
  await page.goto(`${origin}/observatory.html`, { waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => window.odderonDiagnostics?.draws > 0, null, { timeout: 60000 });
  const station = await page.evaluate(() => {
    const scene = document.getElementById('scene').getBoundingClientRect();
    const side = document.querySelector('.obs-side').getBoundingClientRect();
    return {
      sceneWidthRatio: scene.width / innerWidth,
      // The telemetry column is the only reserved strip, and it must stay a
      // strip: the scene has to keep the overwhelming majority of the frame.
      sideWidthRatio: side.width / innerWidth,
      sceneTop: scene.top,
      footers: document.querySelectorAll('footer').length,
      distance: document.getElementById('distance').textContent,
      layers: document.querySelectorAll('.toggle-row input[type=checkbox]').length,
    };
  });
  assert.ok(station.sceneWidthRatio > .7, `the ephemeris viewport must own the frame, got ${station.sceneWidthRatio}`);
  assert.ok(station.sideWidthRatio < .3, `the telemetry column must stay a strip, got ${station.sideWidthRatio}`);
  assert.ok(station.sceneTop < 80, `the optics run to the top of the console, got ${station.sceneTop}px`);
  assert.equal(station.footers, 0, 'no route may carry a website footer');
  assert.equal(station.distance, '361,026', 'the station publishes the live ephemeris');
  assert.equal(station.layers, 4, 'the station keeps all four display layers');

  // ---- Legibility floor: a word below 9px is a smudge, not a readout.
  for (const [width, height, route] of [[1440, 900, '/'], [1440, 900, '/mission.html'],
    [390, 844, '/mission.html'], [1440, 900, '/observatory.html']]) {
    await page.setViewportSize({ width, height });
    await page.goto(`${origin}${route}`, { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(900);
    const smudges = await page.evaluate(() => [...document.querySelectorAll('body *')].filter(element => {
      const text = [...element.childNodes].filter(node => node.nodeType === 3).map(node => node.textContent.trim()).join('');
      if (!text) return false;
      const box = element.getBoundingClientRect();
      if (!box.width || !box.height) return false;
      return parseFloat(getComputedStyle(element).fontSize) < 9;
    }).map(element => `${element.tagName}.${element.className} ${getComputedStyle(element).fontSize}px`));
    assert.deepEqual(smudges, [], `${route} at ${width}px must keep every word at 9px or larger`);
  }
  await page.setViewportSize({ width: 1440, height: 900 });

  // ---- Canvas type uses the bundled instrument faces. A canvas cannot inherit
  // a CSS font stack, so a system fallback here would be invisible to the DOM
  // audit and would quietly make a solved scene look like a web chart.
  for (const route of ['/mission.html', '/observatory.html']) {
    await page.goto(`${origin}${route}`, { waitUntil: 'domcontentloaded' });
    await page.waitForTimeout(1500);
    // `fonts.check` answers true for a family it cannot find, so the faces are
    // also looked up in the document's own font set: a renamed or deleted
    // WOFF2 must fail here rather than silently paint a system fallback.
    const faces = await page.evaluate(async () => {
      const resolved = family => [...document.fonts].some(face => face.family.replace(/["']/g, '') === family && face.status === 'loaded');
      return {
        mono: document.fonts.check('10px "Odderon Data"') && resolved('Odderon Data'),
        display: document.fonts.check('600 11px "Odderon Gauge"') && resolved('Odderon Gauge'),
        ui: document.fonts.check('700 12px "Odderon Panel"') && resolved('Odderon Panel'),
      };
    });
    assert.ok(faces.mono, `${route} must have the Odderon Data face resident before painting labels`);
    assert.ok(faces.display, `${route} must have the Odderon Gauge face resident`);
    assert.ok(faces.ui, `${route} must have the Odderon Panel face resident`);
  }

  // The accessibility preferences are only real if they change the rendered
  // page. Both switches write a class on <body>; a class with no rule behind it
  // stores a setting and does nothing, which reads as a broken control.
  await page.goto(`${origin}/`, { waitUntil: 'domcontentloaded' });
  await page.waitForTimeout(400);
  const contrastOff = await page.evaluate(() => ({
    rule: getComputedStyle(document.body).getPropertyValue('--rule').trim(),
    grain: getComputedStyle(document.querySelector('.grain')).display,
  }));
  await page.evaluate(() => document.getElementById('high-contrast').click());
  await page.evaluate(() => document.getElementById('reduced-effects').click());
  await page.waitForTimeout(150);
  const contrastOn = await page.evaluate(() => ({
    rule: getComputedStyle(document.body).getPropertyValue('--rule').trim(),
    grain: getComputedStyle(document.querySelector('.grain')).display,
  }));
  assert.notEqual(contrastOn.rule, contrastOff.rule, 'the contrast switch must change the instrument rules');
  assert.equal(contrastOn.grain, 'none', 'the reduced-effects switch must suppress the grain plate');

  // Every class the scripts apply must have a rule behind it. The wipe removed
  // a shared sheet; a rule that markup still depends on cannot go with it, and
  // this catches the next one before it ships.
  await page.goto(`${origin}/mission.html`, { waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => !document.getElementById('add-node').disabled, null, { timeout: 30000 });
  await page.locator('#example').click();
  await page.waitForTimeout(1200);
  const orphaned = await page.evaluate(() => {
    const used = new Set();
    for (const el of document.querySelectorAll('[class]')) {
      for (const token of el.classList) used.add(token);
    }
    const sheets = [...document.styleSheets].flatMap(sheet => {
      try { return [...sheet.cssRules].map(rule => rule.cssText); } catch { return []; }
    }).join('\n');
    return [...used].filter(token => !new RegExp(`\\.${token}(?![\\w-])`).test(sheets));
  });
  assert.deepEqual(orphaned, [], `classes rendered with no rule in any loaded sheet: ${orphaned.join(', ')}`);

  // A refused plan must not report the previous valid plan's numbers beside the
  // refusal. The check reads the plan, so a plan that cannot compile has none.
  await page.locator('[data-flight-mode="prediction"]').click();
  await page.waitForTimeout(300);
  await page.locator('[data-flight-mode="flight"]').click();
  await page.waitForTimeout(400);
  const before = await page.evaluate(() => document.getElementById('check-dv').textContent);
  assert.notEqual(before, '—', 'a valid plan must report its delta-v cost');
  await page.evaluate(() => {
    const field = document.querySelector('#node-t');
    field.value = '600';
    field.dispatchEvent(new Event('input', { bubbles: true }));
  });
  await page.waitForTimeout(1200);
  const refused = await page.evaluate(() => ({
    status: document.getElementById('plan-status').classList.contains('failure'),
    code: document.getElementById('plan-check-code').textContent,
    dv: document.getElementById('check-dv').textContent,
    row: document.getElementById('check-dv').parentElement.className,
  }));
  assert.ok(refused.status, 'the unaffordable burn must be refused');
  assert.equal(refused.code, 'REFUSED', 'the plan check must say the plan was refused');
  assert.equal(refused.dv, '—', 'a refused plan must not show a stale delta-v');
  assert.ok(/is-bad/.test(refused.row), 'a refused plan must be styled as a failure');

  assert.deepEqual(errors, [], 'the interface must load without page errors');
  console.log('Console standard holds: machined plates, no web chrome, one live instrument, no document scroll.');
} finally {
  await browser?.close();
  server.close();
}
