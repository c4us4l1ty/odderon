#!/usr/bin/env node
// ensure-chromium.mjs — locate (or bootstrap) a headless Chromium for the Odderon tools.
//
// Order of preference:
//   1. $CHROMIUM_PATH
//   2. a Playwright-managed Chromium already in ~/.cache/ms-playwright
//   3. a previously extracted @sparticuz/chromium binary in tools/.cache/chromium/
//   4. `npm i @sparticuz/chromium` (pure npm, no CDN) and extract it
//
// Returns { executablePath, env } — `env` carries LD_LIBRARY_PATH pointing at the
// no-op NSS shim (see ./chromium-nss-shim/README.md) when the binary needs it.

import fs from 'node:fs';
import path from 'node:path';
import { execFileSync, spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const CACHE = path.join(HERE, '.cache');
const SHIM_DIR = path.join(CACHE, 'nss-shim');

function exists(p) { try { fs.accessSync(p); return true; } catch { return false; } }

function missingLibs(bin) {
  const r = spawnSync('ldd', [bin], { encoding: 'utf8' });
  if (r.error) return [];
  return r.stdout.split('\n').filter((l) => l.includes('not found')).map((l) => l.trim());
}

function ensureShim(bin) {
  const missing = missingLibs(bin);
  const needsNss = missing.some((l) => /libnss3|libnspr4|libnssutil3/.test(l));
  if (!missing.length) return {};
  if (!needsNss) {
    throw new Error(`Chromium is missing system libraries that the NSS shim cannot fake:\n  ${missing.join('\n  ')}`);
  }
  if (!exists(path.join(SHIM_DIR, 'libnss3.so'))) {
    const r = spawnSync('bash', [path.join(HERE, 'chromium-nss-shim', 'build-shim.sh'), SHIM_DIR], {
      encoding: 'utf8',
    });
    if (r.status !== 0) throw new Error('Failed to build the Chromium NSS shim:\n' + r.stdout + r.stderr);
  }
  const prev = process.env.LD_LIBRARY_PATH;
  return { LD_LIBRARY_PATH: prev ? `${SHIM_DIR}:${prev}` : SHIM_DIR };
}

async function sparticuzPath() {
  const extractDir = path.join(CACHE, 'chromium');
  const marker = path.join(extractDir, 'chromium');
  if (exists(marker)) return marker;
  // Resolve @sparticuz/chromium from tools/node_modules, then install it on demand.
  let mod;
  try {
    mod = await import('@sparticuz/chromium');
  } catch {
    process.stderr.write('[ensure-chromium] installing @sparticuz/chromium from the npm registry…\n');
    const r = spawnSync('npm', ['i', '--no-audit', '--no-fund', '--prefix', HERE, '@sparticuz/chromium@latest'], {
      encoding: 'utf8',
    });
    if (r.status !== 0) throw new Error(r.stderr || r.stdout);
    mod = await import('@sparticuz/chromium');
  }
  const chromium = mod.default ?? mod;
  // NOTE: in @sparticuz/chromium >= 150 `executablePath(input)` expects the
  // directory holding the packed `*.br` payloads (the package's own `bin/`),
  // *not* an output directory. Passing a cache dir makes it throw on a fresh
  // container. Call it with no argument: it inflates into `os.tmpdir()` and
  // returns the binary path. The result is cached in `tools/.cache/chromium`
  // as a convenience marker for subsequent runs.
  chromium.setGraphicsMode = false;
  const p = await chromium.executablePath();
  return p;
}

export async function ensureChromium() {
  if (process.env.CHROMIUM_PATH && exists(process.env.CHROMIUM_PATH)) {
    return { executablePath: process.env.CHROMIUM_PATH, env: ensureShim(process.env.CHROMIUM_PATH) };
  }

  const pwRoot = path.join(process.env.HOME || '', '.cache', 'ms-playwright');
  if (exists(pwRoot)) {
    for (const d of fs.readdirSync(pwRoot).filter((n) => n.startsWith('chromium'))) {
      for (const rel of ['chrome-linux/chrome', 'chrome-linux64/chrome', 'chrome-headless-shell-linux64/chrome-headless-shell']) {
        const p = path.join(pwRoot, d, rel);
        if (exists(p)) return { executablePath: p, env: ensureShim(p) };
      }
    }
  }

  const bin = await sparticuzPath();
  fs.chmodSync(bin, 0o755);
  return { executablePath: bin, env: ensureShim(bin) };
}

export const CHROMIUM_ARGS = [
  '--no-sandbox',
  '--headless=new',
  '--use-angle=swiftshader',       // deterministic software rasteriser: CI has no GPU
  '--use-gl=angle',
  '--enable-unsafe-swiftshader',
  '--disable-dev-shm-usage',
  '--hide-scrollbars',
  '--mute-audio',
  '--disable-lcd-text',
  '--force-color-profile=srgb',
  '--disable-features=DefaultPassthroughCommandDecoder',
];

if (import.meta.url === `file://${process.argv[1]}`) {
  ensureChromium().then((r) => {
    console.log(JSON.stringify(r, null, 2));
    try { execFileSync(r.executablePath, ['--version'], { env: { ...process.env, ...r.env } }); } catch {}
  }).catch((e) => { console.error(e); process.exit(1); });
}
