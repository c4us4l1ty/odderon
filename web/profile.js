// Flight License & Profile dialog. Pure DOM: no animation frames, no timers.
import * as identity from './identity.js';
import { download } from './recorder.js';
import { openVerify } from './verify.js';
import * as authority from './authority.js';

const $ = id => document.getElementById(id);
let dialog = null;
let lastFocused = null;
let syncKey = null;
let syncRequest = 0;

/// A player GET is a snapshot, not a live subscription. Ignore it if a newer
/// sync or any profile mutation (notably a receipt or license switch) occurred
/// while it was in flight. `null` means superseded, not authority unavailable.
export async function syncProfile() {
  const profile = identity.profile();
  if (!profile || !authority.configured()) return false;
  const key = profile.publicKey.map(b => b.toString(16).padStart(2, '0')).join('');
  const version = identity.profileRevision(), current = ++syncRequest;
  const superseded = () => current !== syncRequest || version !== identity.profileRevision();
  try {
    const player = await authority.player(key);
    return superseded() ? null : identity.syncPlayer(player);
  } catch { return superseded() ? null : false; }
}

function text(tag, value, className) {
  const node = document.createElement(tag);
  node.textContent = value;
  if (className) node.className = className;
  return node;
}

async function copy(value, button) {
  try {
    await navigator.clipboard.writeText(value);
    button.textContent = 'Copied';
  } catch {
    button.textContent = 'Select and copy manually';
  }
}

function secretBlock(label, value, id) {
  const wrapper = document.createElement('div');
  wrapper.className = 'secret-block';
  const head = document.createElement('div');
  head.className = 'secret-head';
  head.append(text('span', label, 'field-label'));
  const button = document.createElement('button');
  button.type = 'button';
  button.id = id;
  button.textContent = 'Copy';
  button.addEventListener('click', () => copy(value, button));
  head.append(button);
  const code = text('code', value, 'secret-value');
  code.id = `${id}-value`;
  wrapper.append(head, code);
  return wrapper;
}

function render() {
  const profile = identity.profile();
  if (!profile || !dialog) return;
  $('profile-callsign-value').textContent = profile.callsign;
  $('profile-public-id').textContent = profile.publicId;
  $('profile-elo').textContent = profile.ratingSource === 'unverified' ? '—' : profile.elo.toFixed(1);
  $('profile-rating-source').textContent = profile.ratingSource === 'server' ? 'Synced from the authority.' : profile.ratingSource === 'receipt' ? 'Cached verified receipt; sync for current rating.' : 'Unverified local state; no server rating.';
  $('profile-flights').textContent = String(profile.flights);
  $('profile-callsign').value = profile.callsign;
  const body = $('profile-secrets');
  body.replaceChildren(
    secretBlock('Twelve recovery words', profile.mnemonic, 'copy-mnemonic'),
    secretBlock('Flight key', profile.flightKey, 'copy-flight-key'),
  );
  const ranked = $('profile-ranked');
  ranked.replaceChildren();
  const entries = Object.entries(profile.ranked);
  if (!entries.length) {
    ranked.append(text('p', 'No verified server-ranked receipt is stored for this license.', 'profile-note'));
  } else {
    const table = document.createElement('table');
    table.className = 'prediction-table';
    table.innerHTML = '<thead><tr><th>Contract</th><th>Score</th><th>Elo after</th><th>Change</th><th>Official certificate</th></tr></thead>';
    const tbody = document.createElement('tbody');
    for (const [contract, record] of entries.sort((a, b) => Number(a[0]) - Number(b[0]))) {
      const row = document.createElement('tr');
      row.innerHTML = `<th scope="row">${contract}</th><td>${(record.score * 100).toFixed(1)}%</td><td>${record.elo.toFixed(1)}</td><td>${record.delta >= 0 ? '+' : ''}${record.delta.toFixed(1)}</td>`;
      const cell = document.createElement('td');
      const stored = identity.certificate(contract);
      if (stored) {
        // The certificate is the signed recorder, so it can be exported and
        // independently verified long after the flight.
        const save = document.createElement('button');
        save.type = 'button';
        save.className = 'certificate-button';
        save.dataset.contract = contract;
        save.textContent = 'Download';
        save.addEventListener('click', () => download(`contract-${contract}.odderon-fdr.json`, `${stored}\n`, 'application/json'));
        const check = document.createElement('button');
        check.type = 'button';
        check.className = 'certificate-button certificate-verify';
        check.dataset.contract = contract;
        check.textContent = 'Verify';
        check.addEventListener('click', () => { dialog.close(); openVerify(stored); });
        cell.append(save, check);
      } else {
        cell.textContent = 'Not stored in this browser';
      }
      row.append(cell);
      tbody.append(row);
    }
    table.append(tbody);
    ranked.append(table);
  }
  $('topbar-callsign').textContent = profile.callsign;
  $('topbar-elo').textContent = profile.ratingSource === 'unverified' ? '—' : profile.elo.toFixed(0);
}

function build() {
  dialog = document.createElement('dialog');
  dialog.id = 'profile-dialog';
  dialog.setAttribute('aria-labelledby', 'profile-title');
  dialog.innerHTML = `
    <div class="profile-head">
      <div data-unit="UNIT LIC · PERSONNEL FILE · LOCAL STORE"><span class="section-label">AEROSPACE FLIGHT LICENSE</span><h2 id="profile-title">Flight License &amp; Profile</h2></div>
      <button type="button" id="profile-close" aria-label="Close profile">✕</button>
    </div>
    <div class="profile-identity">
      <div><span class="field-label">Callsign</span><strong id="profile-callsign-value">—</strong></div>
      <div><span class="field-label">Public identifier</span><code id="profile-public-id">—</code></div>
      <div><span class="field-label">Astro-Elo</span><strong id="profile-elo">—</strong></div>
      <div><span class="field-label">Flights</span><strong id="profile-flights">—</strong></div>
    </div>
    <p id="profile-rating-source" class="profile-note"></p><button type="button" id="profile-sync">Sync rating from authority</button>
    <form id="profile-callsign-form">
      <label class="field-label" for="profile-callsign">Change callsign</label>
      <div class="profile-row"><input id="profile-callsign" maxlength="24" autocomplete="off"><button type="submit">Save callsign</button></div>
      <p id="profile-callsign-status" class="profile-note" role="status"></p>
    </form>
    <details id="profile-reveal">
      <summary>Reveal recovery secrets</summary>
      <p class="profile-warning">These twelve words and this flight key <strong>are</strong> your identity. Anyone who reads them can sign as you. They are stored in this browser’s localStorage, which any script on this origin can read — treat this as a local practice identity, not key custody.</p>
      <div id="profile-secrets"></div>
      <button type="button" id="profile-download">Download license backup (.odderon-license.json)</button>
    </details>
    <section class="profile-restore">
      <h3>Restore or switch identity</h3>
      <label class="field-label" for="profile-restore-input">Paste twelve words or an ODD- flight key</label>
      <textarea id="profile-restore-input" rows="3" spellcheck="false" autocomplete="off" placeholder="ablation apoapsis … or ODD-0011-2233-…"></textarea>
      <p id="profile-restore-status" class="profile-note" role="status"></p>
      <div class="profile-row">
        <button type="button" id="profile-restore">Restore identity</button>
        <label class="file-button" for="profile-import">Import license file</label>
        <input type="file" id="profile-import" accept=".json,application/json" hidden>
      </div>
      <p id="profile-import-status" class="profile-note" role="status"></p>
    </section>
    <section class="profile-ranked"><h3>Certified ranked certificates</h3><div id="profile-ranked"></div></section>`;
  document.body.append(dialog);

  $('profile-close').addEventListener('click', () => dialog.close());
  dialog.addEventListener('close', () => {
    $('profile-restore-input').value = '';
    $('profile-restore-status').textContent = '';
    $('profile-import-status').textContent = '';
    $('profile-reveal').open = false;
    lastFocused?.focus();
  });
  // Click outside the panel closes it.
  dialog.addEventListener('click', event => { if (event.target === dialog) dialog.close(); });
  $('profile-callsign-form').addEventListener('submit', async event => {
    event.preventDefault();
    const name = $('profile-callsign').value.trim();
    if (!name || name.length > 24 || !/^[\w \-]+$/.test(name)) {
      $('profile-callsign-status').textContent = 'Use 1–24 letters, digits, spaces, dash or underscore.';
      return;
    }
    const key = identity.profile().publicId;
    try {
      const registered = authority.configured() ? await authority.registerCallsign(name) : null;
      if (identity.profile().publicId !== key) throw Error('The active license changed; try again.');
      const result = identity.setCallsign(registered?.name ?? name);
      $('profile-callsign-status').textContent = result.ok
        ? (registered ? 'Callsign registered to this license.' : 'Saved locally only; no authority is configured.') : result.error;
    } catch (error) { $('profile-callsign-status').textContent = String(error.message || error); }
  });
  $('profile-sync').addEventListener('click', async () => {
    $('profile-rating-source').textContent = 'Syncing…';
    if (await syncProfile() === false) $('profile-rating-source').textContent = 'Authority unavailable; cached values are not authoritative.';
  });
  $('profile-restore-input').addEventListener('input', () => {
    const result = identity.checkRestore($('profile-restore-input').value);
    const status = $('profile-restore-status');
    status.textContent = result.ok ? `Valid ${result.kind === 'key' ? 'flight key' : 'twelve-word license'}.` : result.error;
    status.className = `profile-note ${result.ok ? 'is-valid' : ''}`;
    $('profile-restore').disabled = !result.ok;
  });
  $('profile-restore').addEventListener('click', () => {
    const result = identity.restore($('profile-restore-input').value);
    $('profile-restore-status').textContent = result.ok ? `Restored ${result.publicId}.` : result.error;
    $('profile-restore-status').className = `profile-note ${result.ok ? 'is-valid' : ''}`;
    if (result.ok) $('profile-restore-input').value = '';
  });
  $('profile-download').addEventListener('click', () => {
    const blob = new Blob([`${JSON.stringify(identity.licenseBackup(), null, 2)}\n`], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.href = url;
    link.download = `${identity.profile().callsign.replace(/[^\w\-]+/g, '-')}.odderon-license.json`;
    link.click();
    URL.revokeObjectURL(url);
  });
  $('profile-import').addEventListener('change', async event => {
    const file = event.target.files?.[0];
    if (!file) return;
    const result = identity.importBackup(await file.text());
    $('profile-import-status').textContent = result.ok ? `Imported ${result.publicId}.` : result.error;
    $('profile-import-status').className = `profile-note ${result.ok ? 'is-valid' : ''}`;
    event.target.value = '';
  });
  identity.onChange(profile => {
    render();
    const key = profile?.publicId;
    if (key && key !== syncKey) { syncKey = key; void syncProfile(); }
  });
  render();
  syncKey = identity.profile()?.publicId;
  void syncProfile();
}

export function openProfile() {
  if (!dialog) build();
  render();
  lastFocused = document.activeElement;
  $('profile-restore').disabled = true;
  dialog.showModal();
}

export function initProfile() {
  const button = $('open-profile');
  button?.addEventListener('click', openProfile);
  // Build eagerly so the top bar shows the restored callsign on first paint.
  build();
}
