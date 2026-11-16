// Verify / Replay Dossier. Pure DOM plus one-shot workers: no frames, no timers.
//
// Verification is two independent checks performed inside the physics module:
// the Ed25519 signature over canonical bytes rebuilt from the typed fields,
// then a Level-3 replay that must reproduce all 58 fields and the terminal
// state hash bit for bit. The banner is only shown when both agree.
import * as recorder from './recorder.js';
import { publicFlight } from './authority.js';

const $ = id => document.getElementById(id);
let wasm = null;
let worker = null;
let dialog = null;
let lastResult = null;
let lastFocused = null;
// Identifies the document the user most recently chose, including an input
// that fails to parse before a worker can start.
let generation = 0;
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
  const certification = result.code === 1 ? (result.certification ?? 'UNVERIFIED LOCAL / PRACTICE') : 'UNVERIFIED LOCAL / PRACTICE';
  $('verify-certification').textContent = certification;
  $('verify-certification').className = `verify-verdict ${certification.startsWith('★') ? 'is-verified' : ''}`;
  const detail = $('verify-detail');
  if (result.code === 1) {
    detail.textContent = `Signature valid and Level-3 replay reproduced every recorded field and the terminal-state hash${Number.isFinite(result.computeMs) ? ` in ${result.computeMs.toFixed(0)} ms` : ''}.${result.dossierError ? ` ${result.dossierError}` : ''}`;
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

function invalidate() {
  ++generation;
  cancel();
  // A verdict must never survive a different document, even if its input
  // is only being edited or fails before a worker can start.
  lastResult = null;
  $('verify-busy').hidden = true;
  $('verify-verdict').textContent = '';
  $('verify-verdict').className = 'verify-verdict';
  $('verify-detail').textContent = '';
  $('verify-certification').textContent = '';
  $('verify-download').hidden = true;
  $('verify-print').hidden = true;
  $('verify-download-json').hidden = true;
}

function begin(message = 'Replaying the flight…') {
  invalidate();
  $('verify-busy').hidden = false;
  $('verify-busy').textContent = message;
  return generation;
}

function run(inputs, operation) {
  worker = new Worker('./mission-worker.js', { type: 'module' });
  const current = worker;
  current.onmessage = ({ data }) => {
    if (worker !== current || operation !== generation) return;
    current.terminate();
    worker = null;
    if (data.error) {
      show({ code: 0, verdict: String(data.error), mismatchIndex: -1, mismatchName: '', dossier: null, json: '' });
      return;
    }
    show(data.verification);
  };
  current.onerror = () => {
    if (worker !== current || operation !== generation) return;
    cancel();
    show({ code: 0, verdict: 'The verification worker failed.', mismatchIndex: -1, mismatchName: '', dossier: null, json: '' });
  };
  // A scannable QR needs an absolute URL. Strip any fragment or query so the
  // recorder's own #verify= fragment is the only one appended.
  const origin = `${location.origin}${location.pathname}`;
  current.postMessage({ action: 'verify', origin, ...inputs });
}

/// Verify a recorder JSON document or a #verify= payload.
export function verifyText(text) {
  if (!dialog) build();
  $('verify-input').value = String(text);
  const operation = begin();
  let inputs;
  try {
    inputs = recorder.prepareVerification(wasm, text);
  } catch (error) {
    show({ code: 0, verdict: String(error.message || error), mismatchIndex: -1, mismatchName: '', dossier: null, json: '' });
    return false;
  }
  run(inputs, operation);
  return true;
}

/// Verify already-captured fields (the flight that just ran).
export function verifyFields(fields, callsign, receipt = null) {
  if (!dialog) build();
  // The flat fields do not have an associated input string. Do not leave a
  // previous document visible beside this new flight's verdict.
  $('verify-input').value = '';
  run({ fields, callsign, receipt }, begin());
}

/// Public, content-addressed replay. The response is not trusted: its FDR,
/// receipt, signature, hash and physics all go through the same worker.
export async function openPublicFlight(hash) {
  openVerify();
  const operation = begin('Fetching the public flight…');
  try {
    const flight = await publicFlight(hash);
    if (operation !== generation || !dialog.open) return;
    if (flight?.receipt?.fdr_hash !== hash.toLowerCase()) throw Error('The public response does not match the requested flight hash.');
    const inputs = recorder.prepareVerification(wasm, flight.fdr_payload);
    inputs.receipt = flight.receipt;
    $('verify-input').value = flight.fdr_payload;
    $('verify-busy').textContent = 'Replaying the flight…';
    run(inputs, operation);
  } catch (error) {
    if (operation === generation && dialog.open) show({ code: 0, verdict: `Public flight unavailable: ${String(error.message || error)}`, mismatchIndex: -1, mismatchName: '', dossier: null, json: '' });
  }
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
    <p id="verify-certification" class="verify-verdict" role="status"></p>
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
    // The close event can be delivered after an immediate reopen. It must
    // not cancel the new replay (e.g. leaderboard → verify → another file).
    if (dialog.open) return;
    invalidate();
    lastFocused?.focus();
  });
  $('verify-run').addEventListener('click', () => verifyText($('verify-input').value));
  $('verify-input').addEventListener('input', invalidate);
  $('verify-import').addEventListener('change', async event => {
    const file = event.target.files?.[0];
    if (!file) return;
    const operation = begin('Reading the recorder…');
    try {
      const text = await file.text();
      if (operation !== generation || !dialog.open) return;
      $('verify-input').value = text;
      verifyText(text);
    } catch (error) {
      if (operation === generation && dialog.open) show({ code: 0, verdict: `Could not read the recorder: ${String(error.message || error)}`, mismatchIndex: -1, mismatchName: '', dossier: null, json: '' });
    } finally { event.target.value = ''; }
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
  if (!dialog.open) {
    // `close` events are queued. Invalidate an old fetch even if the dialog
    // was closed and reopened before its close listener had a chance to run.
    invalidate();
    dialog.showModal();
  }
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
    else if (hash.startsWith('#flight=')) openPublicFlight(hash.slice(8));
  };
  window.addEventListener('hashchange', handleHash);
  handleHash();
}
