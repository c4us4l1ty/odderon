import module from './authority.wasm';
import { handle } from './handler.js';

// A bare Rust wasm module, built from the shared AuthorityEngine. No browser
// or client can access SERVER_SEED; it stays in this server-side binding.
const wasm = new WebAssembly.Instance(module, {}).exports;
export default { fetch(request, env) { return handle(request, env, wasm); } };
