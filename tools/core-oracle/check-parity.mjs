// Compare the same compiled fixture on native, Node WebAssembly, and headless
// Chrome. The local server is used only during this one-shot CI test.
import { readFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { spawn, execFileSync } from 'node:child_process';
import { resolve } from 'node:path';

const wasm = await readFile(resolve('target/wasm32-unknown-unknown/debug/odd_core_oracle.wasm'));
const expected = execFileSync(resolve('target/debug/odd-core-oracle'), { encoding: 'utf8' }).trim().split('\n');
const { instance } = await WebAssembly.instantiate(wasm, {});
const fingerprints = exp => ['oracle_byte','integrator_oracle_byte','blended_integrator_oracle_byte','analytical_integrator_oracle_byte','maneuver_oracle_byte'].map(name=>Array.from({length:32},(_,i)=>exp[name](i).toString(16).padStart(2,'0')).join(''));
const nodeResult = fingerprints(instance.exports);
if (nodeResult.join(',') !== expected.join(',')) throw new Error(`Native/Node wasm mismatch: ${expected} != ${nodeResult}`);

const html = `<!doctype html><html><body><script>
fetch('/oracle.wasm').then(r => r.arrayBuffer()).then(b => WebAssembly.instantiate(b, {})).then(({instance}) => {
  const hash = ['oracle_byte','integrator_oracle_byte','blended_integrator_oracle_byte','analytical_integrator_oracle_byte','maneuver_oracle_byte'].map(name=>Array.from({length:32},(_,i)=>instance.exports[name](i).toString(16).padStart(2,'0')).join('')).join(',');
  return fetch('/result?value=' + encodeURIComponent(hash));
}).catch(e => fetch('/result?error=' + encodeURIComponent(String(e))));
</script></body></html>`;
let report;
const browserResult = new Promise(resolveReport => { report = resolveReport; });
const server = createServer((req, res) => {
  if (req.url === '/oracle.wasm') {
    res.writeHead(200, { 'content-type': 'application/wasm' });
    res.end(wasm);
  } else if (req.url.startsWith('/result?')) {
    const query = new URL(req.url, 'http://localhost').searchParams;
    report({ value: query.get('value'), error: query.get('error') });
    res.writeHead(200, { 'content-type': 'text/plain' });
    res.end('ok');
  } else {
    res.writeHead(200, { 'content-type': 'text/html' });
    res.end(html);
  }
});
await new Promise((resolveListen, reject) => server.listen(0, '127.0.0.1', resolveListen).once('error', reject));
const port = server.address().port;
const chrome = process.env.CHROME_BIN || 'google-chrome';
let stderr = '';
const proc = spawn(chrome, ['--headless', '--disable-gpu', '--no-sandbox', '--disable-dev-shm-usage', '--disable-background-networking', `http://127.0.0.1:${port}/`], { stdio: ['ignore', 'ignore', 'pipe'] });
proc.stderr.on('data', chunk => { stderr += chunk; });
const exited = new Promise((_, reject) => {
  proc.on('error', reject);
  proc.on('close', code => reject(new Error(`Chrome exited before reporting a result (${code}): ${stderr}`)));
});
let timer;
const timeout = new Promise((_, reject) => { timer = setTimeout(() => reject(new Error(`Chrome parity timed out: ${stderr}`)), 90_000); });
try {
  const result = await Promise.race([browserResult, exited, timeout]);
  if (result.error || result.value !== expected.join(',')) throw new Error(`Chrome wasm parity failed: ${JSON.stringify(result)}, expected ${expected}`);
  console.log(`Native, Node and Chrome wasm fingerprints agree: ${expected}`);
} finally {
  clearTimeout(timer);
  proc.kill('SIGTERM');
  server.close();
}
