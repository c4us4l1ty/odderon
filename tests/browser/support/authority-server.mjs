// Real local authority for browser gates. This seed is public test data, NOT
// a deployment credential. Each run has a disposable SQLite database.
import { spawn } from 'node:child_process';
import { request } from 'node:http';
import { mkdtemp, rm } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

export async function startAuthority() {
  const temp = await mkdtemp(join(tmpdir(), 'odderon-authority-e2e-'));
  // The v4 flight is coupled: the tanks ride the integrator, so a full replay
  // costs roughly an order of magnitude more than v3's post-hoc integration.
  // Run the optimized binary so the gate measures the shipped physics rather
  // than an unoptimized build, and give it the CPU a deployment would have.
  const binary = process.env.ODDERON_AUTHORITY_BIN
    || (existsSync(resolve('target/release/odderon-authority'))
      ? resolve('target/release/odderon-authority')
      : resolve('target/debug/odderon-authority'));
  const child = spawn(binary, [], {
    env: { ...process.env, ODDERON_SERVER_SEED: '5e'.repeat(32), ODDERON_DB: join(temp, 'authority.sqlite'), ODDERON_BIND: '127.0.0.1:0' },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  let output = '', stderr = '';
  child.stderr.on('data', bytes => { stderr += bytes; });
  const { key, port } = await new Promise((ready, reject) => {
    const timer = setTimeout(() => reject(Error(`authority did not start: ${output} ${stderr}`)), 30000);
    child.on('error', reject);
    child.on('exit', code => reject(Error(`authority exited ${code}: ${stderr}`)));
    child.stdout.on('data', bytes => {
      output += bytes;
      const key = output.match(/Authority public key: ([0-9a-f]{64})/)?.[1];
      const port = output.match(/Authority listening on 127\.0\.0\.1:(\d+)/)?.[1];
      if (key && port) { clearTimeout(timer); ready({ key, port: Number(port) }); }
    });
  });
  return {
    key,
    // Called by a same-origin test web server; browser code never calls the
    // backend's loopback address directly.
    handle(req, res) {
      const upstream = request({ hostname: '127.0.0.1', port, path: req.url, method: req.method, headers: req.headers }, response => {
        res.writeHead(response.statusCode, response.headers); response.pipe(res);
      });
      upstream.on('error', () => { if (!res.headersSent) res.writeHead(502); res.end(); });
      req.pipe(upstream);
    },
    async close() { child.kill('SIGTERM'); await rm(temp, { recursive: true, force: true }); },
  };
}
