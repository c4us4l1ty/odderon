// Actual Cloudflare workerd + D1/R2 emulator. The test entry only adds schema
// setup/inspection; requests under /api use the production Worker entry.
// The deterministic seed is PUBLIC TEST DATA, never a deployment secret.
import { Miniflare } from 'miniflare';
import { createPrivateKey, createPublicKey } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

export async function startCloudAuthority() {
  const dir = resolve('crates/odd-worker-cloudflare/src');
  const migrations = [];
  for (const name of ['0001_init.sql', '0002_transaction_guard.sql']) {
    const sql = (await readFile(resolve('crates/odd-worker-cloudflare/migrations', name), 'utf8')).replace(/^\s*--.*$/gm, '');
    migrations.push(...sql.split(';').map(s => s.trim()).filter(Boolean));
  }
  const entry = `
    import app from './index.js';
    const migrations=${JSON.stringify(migrations)};
    export default {async fetch(req,env,ctx){
      const url=new URL(req.url);
      if(url.pathname==='/__test/migrate'){
        await env.DB.batch(migrations.map(s=>env.DB.prepare(s)));return new Response('ok');
      }
      if(url.pathname==='/__test/query'){
        const {sql,values=[]}=await req.json();
        return Response.json(await env.DB.prepare(sql).bind(...values).all());
      }
      if(url.pathname==='/__test/object'){
        const {key,value}=await req.json();
        if(value!==undefined){await env.FDRS.put(key,value);return Response.json({ok:true});}
        const object=await env.FDRS.get(key);return Response.json({value:object?await object.text():null});
      }
      return app.fetch(req,env,ctx);
    }};`;
  const seed = '5e'.repeat(32);
  const privateKey = createPrivateKey({ key: Buffer.concat([Buffer.from('302e020100300506032b657004220420', 'hex'), Buffer.from(seed, 'hex')]), format: 'der', type: 'pkcs8' });
  const key = createPublicKey(privateKey).export({ format: 'der', type: 'spki' }).subarray(-32).toString('hex');
  const mf = new Miniflare({
    cf: false, compatibilityDate: '2026-01-01', modulesRoot: dir,
    modules: [
      { type: 'ESModule', path: `${dir}/test-entry.js`, contents: entry },
      { type: 'ESModule', path: `${dir}/index.js` },
      { type: 'ESModule', path: `${dir}/handler.js` },
      { type: 'CompiledWasm', path: `${dir}/authority.wasm` },
    ],
    d1Databases: ['DB'], r2Buckets: ['FDRS'], bindings: { SERVER_SEED: seed },
  });
  const migration = await mf.dispatchFetch('http://authority/__test/migrate', { method: 'POST' });
  if (!migration.ok) throw Error(`D1 migration failed: ${await migration.text()}`);
  async function test(path, payload) {
    const r = await mf.dispatchFetch(`http://authority/__test/${path}`, {
      method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(payload),
    });
    if (!r.ok) throw Error(`workerd test ${path}: ${r.status} ${await r.text()}`);
    return r.json();
  }
  return {
    key,
    async handle(req, res) {
      try {
        const chunks = [];
        for await (const part of req) chunks.push(part);
        const body = chunks.length ? Buffer.concat(chunks) : undefined;
        const upstream = await mf.dispatchFetch(`http://authority${req.url}`, {
          method: req.method, headers: req.headers, ...(body ? { body } : {}),
        });
        res.writeHead(upstream.status, Object.fromEntries(upstream.headers));
        res.end(Buffer.from(await upstream.arrayBuffer()));
      } catch (error) { if (!res.headersSent) res.writeHead(502); res.end(String(error)); }
    },
    query: (sql, ...values) => test('query', { sql, values }),
    object: (key, value) => test('object', { key, value }),
    async close() { await mf.dispose(); },
  };
}
