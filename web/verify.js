// Verify / Replay Dossier. Pure DOM plus one-shot workers: no frames, no timers.
//
// Verification is two independent checks performed inside the physics module:
// the Ed25519 signature over canonical bytes rebuilt from the typed fields,
// then a Level-3 replay that must reproduce all 58 fields and the terminal
// state hash bit for bit. The banner is only shown when both agree.
import * as recorder from './recorder.js';

const $ = id => document.getElementById(id);
let wasm = null;
let worker = null;
let dialog = null;
let lastResult = null;
let lastFocused = null;
const listeners = new Set();

export function onVerification(listener) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function result() {
  return lastResult;
}

function cancel() {
  if (worker) {
    worker.terminate();
    worker = null;
  }
}

function show(result) {
  lastResult = result;
  const banner = $('verify-verdict');
  banner.textContent = result.verdict;
  banner.className = `verify-verdict ${result.code === 1 ? 'is-verified' : 'is-rejected'}`;
  const detail = $('verify-detail');
  if (result.code === 1) {
    detail.textContent = `Signature valid and Level-3 replay reproduced every recorded field and the terminal-state hash${Number.isFinite(result.computeMs) ? ` in ${result.computeMs.toFixed(0)} ms` : ''}.`;
  } else if (result.code === -1) {
    detail.textContent = 'The signature does not cover these fields. The document was edited after signing.';
  } else if (result.code === -2) {
    detail.textContent = `The signature is valid but the flight does not reproduce: ${result.mismatchName} (field ${result.mismatchIndex}) differs from the recorded value.`;
  } else if (result.code === -3) {
    detail.textContent = 'The signature is valid but the replayed terminal state does not match final_state_hash.';
  } else {
    detail.textContent = 'The document could not be interpreted as a flight recorder.';
  }
  $('verify-download').hidden = !(result.code === 1 && result.dossier);
  $('verify-print').hidden = !(result.code === 1 && result.dossier);
  $('verify-download-json').hidden = !result.json;
  $('verify-busy').hidden = true;
  for (const listener of listeners) listener(result);
}

function run(inputs) {
  cancel();
  // A verdict belongs to one document. Clearing it here stops a stale result
  // from being read as the answer for the run that just started.
  lastResult = null;
  $('verify-busy').hidden = false;
  $('verify-verdict').textContent = '';
  $('verify-verdict').className = 'verify-verdict';
  $('verify-detail').textContent = '';
  $('verify-download').hidden = true;
  $('verify-print').hidden = true;
  $('verify-download-json').hidden = true;
  worker = new Worker('./mission-worker.js', { type: 'module' });
  const current = worker;
  current.onmessage = ({ data }) => {
    if (worker !== current) return;
    current.terminate();
    worker = null;
    if (data.error) {
      show({ code: 0, verdict: String(data.error), mismatchIndex: -1, mismatchName: '', dossier: null, json: '' });
      return;
    }
    show(data.verification);
  };
  current.onerror = () => {
    if (worker !== current) return;
    cancel();
    show({ code: 0, verdict: 'The verification worker failed.', mismatchIndex: -1, mismatchName: '', dossier: null, json: '' });
  };
  current.postMessage({ action: 'verify', ...inputs });
}

/// Verify a recorder JSON document or a #verify= payload.
export function verifyText(text) {
  if (!dialog) build();
  let inputs;
  try {
    inputs = recorder.prepareVerification(wasm, text);
  } catch (error) {
    show({ code: 0, verdict: String(error.message || error), mismatchIndex: -1, mismatchName: '', dossier: null, json: '' });
    return false;
  }
  run(inputs);
  return true;
}

/// Verify already-captured fields (the flight that just ran).
export function verifyFields(fields, callsign) {
  if (!dialog) build();
  run({ fields, callsign });
}

function build() {
  dialog = document.createElement('dialog');
  dialog.id = 'verify-dialog';
  dialog.setAttribute('aria-labelledby', 'verify-title');
  dialog.innerHTML = `
    <div class="profile-head">
      <div><span class="section-label">SIGNED FLIGHT DATA RECORDER</span><h2 id="verify-title">Verify / Replay Dossier</h2></div>
      <button type="button" id="verify-close" aria-label="Close verification">✕</button>
    </div>
    <p class="profile-note">Verification checks the Ed25519 signature over the canonical bytes, then re-runs the Level-3 flight from the command timeline and requires every recorded field and the terminal-state hash to reproduce bit for bit.</p>
    <label class="field-label" for="verify-input">Recorder JSON or #verify= payload</label>
    <textarea id="verify-input" rows="4" spellcheck="false" autocomplete="off" placeholder='{"schema_version":1,…} or #verify=…'></textarea>
    <div class="profile-row">
      <button type="button" id="verify-run">Verify and replay</button>
      <label class="file-button" for="verify-import">Import .odderon-fdr.json</label>
      <input type="file" id="verify-import" accept=".json,application/json" hidden>
    </div>
    <p id="verify-busy" class="profile-note" role="status" hidden>Replaying the flight…</p>
    <p id="verify-verdict" class="verify-verdict" role="status"></p>
    <p id="verify-detail" class="profile-note"></p>
    <div class="profile-row">
      <button type="button" id="verify-download" hidden>Download dossier (HTML)</button>
      <button type="button" id="verify-print" hidden>Print dossier (PDF)</button>
      <button type="button" id="verify-download-json" hidden>Download recorder JSON</button>
    </div>`;
  document.body.append(dialog);
  $('verify-close').addEventListener('click', () => dialog.close());
  dialog.addEventListener('click', event => { if (event.target === dialog) dialog.close(); });
  dialog.addEventListener('close', () => {
    cancel();
    $('verify-busy').hidden = true;
    lastFocused?.focus();
  });
  $('verify-run').addEventListener('click', () => verifyText($('verify-input').value));
  $('verify-import').addEventListener('change', async event => {
    const file = event.target.files?.[0];
    if (!file) return;
    const text = await file.text();
    $('verify-input').value = text;
    verifyText(text);
    event.target.value = '';
  });
  $('verify-download').addEventListener('click', () => {
    if (lastResult?.dossier) {
      recorder.download('odderon-dossier.html', lastResult.dossier, 'text/html');
    }
  });
  $('verify-print').addEventListener('click', () => {
    if (!lastResult?.dossier) return;
    // Print the verified document itself, not the surrounding app page. The
    // frame is sandboxed to same-origin scripts and self-contained HTML, so
    // printing needs no network and schedules no animation frames here.
    document.getElementById('dossier-print-frame')?.remove();
    const frame = document.createElement('iframe');
    frame.id = 'dossier-print-frame';
    frame.setAttribute('sandbox', 'allow-same-origin allow-modals');
    frame.setAttribute('aria-hidden', 'true');
    frame.className = 'dossier-print-frame';
    frame.addEventListener('load', () => {
      frame.contentWindow?.focus();
      frame.contentWindow?.print();
    }, { once: true });
    frame.srcdoc = lastResult.dossier;
    document.body.append(frame);
  });
  $('verify-download-json').addEventListener('click', () => {
    if (lastResult?.json) {
      recorder.download('flight.odderon-fdr.json', `${lastResult.json}\n`, 'application/json');
    }
  });
}

export function openVerify(text) {
  if (!dialog) build();
  lastFocused = document.activeElement;
  dialog.showModal();
  if (text) {
    $('verify-input').value = text;
    verifyText(text);
  }
}

export function initVerify(module) {
  wasm = module;
  $('open-verify')?.addEventListener('click', () => openVerify());
  // A shared link carries the whole recorder in the fragment, so verification
  // needs no server and no account.
  const handleHash = () => {
    const hash = location.hash;
    if (hash.startsWith('#verify=')) openVerify(hash);
  };
  window.addEventListener('hashchange', handleHash);
  handleHash();
}
