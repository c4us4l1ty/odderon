// Compare the same compiled fixture on native, Node WebAssembly, and headless
// Chrome. The local server is used only during this one-shot CI test.
import { readFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { spawn, execFileSync } from 'node:child_process';
import { resolve } from 'node:path';

const wasm = await readFile(resolve('target/wasm32-unknown-unknown/debug/odd_core_oracle.wasm'));
const expected = execFileSync(resolve('target/debug/odd-core-oracle'), { encoding: 'utf8' }).trim();
const { instance } = await WebAssembly.instantiate(wasm, {});
const nodeResult = Array.from({ length: 32 }, (_, i) => instance.exports.oracle_byte(i).toString(16).padStart(2, '0')).join('');
if (nodeResult !== expected) throw new Error(`Native/Node wasm mismatch: ${expected} != ${nodeResult}`);

const html = `<!doctype html><html><body>WAIT<script>
fetch('/oracle.wasm').then(r => r.arrayBuffer()).then(b => WebAssembly.instantiate(b, {})).then(({instance}) => {
  document.body.textContent = 'ODD_RESULT:' + Array.from({length:32}, (_, i) => instance.exports.oracle_byte(i).toString(16).padStart(2,'0')).join('');
}).catch(e => { document.body.textContent = 'ODD_ERROR:' + e; });
</script></body></html>`;
const server = createServer((req, res) => {
  if (req.url === '/oracle.wasm') { res.writeHead(200, { 'content-type': 'application/wasm' }); res.end(wasm); }
  else { res.writeHead(200, { 'content-type': 'text/html' }); res.end(html); }
});
await new Promise((resolveListen, reject) => server.listen(0, '127.0.0.1', resolveListen).once('error', reject));
const port = server.address().port;
try {
  const chrome = process.env.CHROME_BIN || 'google-chrome';
  const proc = spawn(chrome, ['--headless', '--disable-gpu', '--no-sandbox', '--disable-dev-shm-usage', '--virtual-time-budget=10000', '--dump-dom', `http://127.0.0.1:${port}/`]);
  let stdout = '', stderr = '';
  const timeout = setTimeout(() => proc.kill('SIGKILL'), 30_000);
  proc.stdout.on('data', chunk => stdout += chunk);
  proc.stderr.on('data', chunk => stderr += chunk);
  const result = await new Promise((resolveExit, reject) => {
    proc.on('error', reject);
    proc.on('close', code => resolveExit(code));
  });
  clearTimeout(timeout);
  const match = stdout.match(/ODD_RESULT:([a-f0-9]{64})/);
  if (result !== 0 || !match || match[1] !== expected) throw new Error(`Chrome wasm parity failed (${result}): ${stdout}\n${stderr}`);
  console.log(`Native, Node and Chrome wasm fingerprints agree: ${expected}`);
} finally {
  server.close();
}
