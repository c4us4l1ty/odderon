// On-demand leaderboard: no polling or background animation. Rows are public
// server data, and each replay independently checks both signatures/physics.
import * as authority from './authority.js';
import { openPublicFlight } from './verify.js';

const $ = id => document.getElementById(id);
let dialog, page = 1, token = 0, lastFocused;

async function refresh() {
  const current = ++token;
  const status = $('leaderboard-status'), body = $('leaderboard-rows');
  body.replaceChildren();
  status.textContent = 'Loading the authority leaderboard…';
  try {
    if (!authority.configured()) throw Error('No trusted authority is configured.');
    const rows = await authority.leaderboard($('leaderboard-contract').value, page);
    if (current !== token || !dialog.open) return;
    if (!Array.isArray(rows)) throw Error('Invalid leaderboard response.');
    for (const row of rows) {
      if (typeof row.fdr_hash !== 'string' || !/^[0-9a-f]{64}$/i.test(row.fdr_hash)
          || !Number.isFinite(row.elo) || !Number.isFinite(row.score_s)) continue;
      const tr = document.createElement('tr');
      for (const value of [row.callsign || String(row.pubkey || '').slice(0, 12), row.elo.toFixed(1), `${(row.score_s * 100).toFixed(1)}%`, String(row.delegation_mask ?? 0)]) {
        const td = document.createElement('td'); td.textContent = value; tr.append(td);
      }
      const td = document.createElement('td'), button = document.createElement('button');
      button.type = 'button'; button.className = 'leaderboard-replay'; button.textContent = 'Verify / replay';
      button.dataset.hash = row.fdr_hash;
      button.addEventListener('click', () => { dialog.close(); void openPublicFlight(row.fdr_hash); });
      td.append(button); tr.append(td); body.append(tr);
    }
    status.textContent = rows.length ? `Page ${page}. Open a flight to verify its receipt and replay.` : `No ranked flights on page ${page}.`;
    $('leaderboard-prev').disabled = page <= 1;
    $('leaderboard-next').disabled = rows.length < 25;
  } catch (error) {
    if (current === token && dialog.open) status.textContent = `Leaderboard unavailable: ${String(error.message || error)}`;
  }
}
function build() {
  dialog = document.createElement('dialog');
  dialog.id = 'leaderboard-dialog'; dialog.setAttribute('aria-labelledby', 'leaderboard-title');
  dialog.innerHTML = `
    <div class="profile-head"><div><span class="section-label">SERVER-RANKED FLIGHTS</span><h2 id="leaderboard-title">Leaderboard</h2></div><button type="button" id="leaderboard-close" aria-label="Close leaderboard">✕</button></div>
    <p class="profile-note">Only ranked receipts appear here. Rankings are server data; open a flight to verify it independently.</p>
    <label class="field-label" for="leaderboard-contract">Contract</label>
    <select id="leaderboard-contract"><option value="">All contracts · by Elo</option><option value="101">101 · Orbit rescue</option><option value="102">102 · Sun-synchronous</option><option value="103">103 · Perigee-kick trap</option></select>
    <p id="leaderboard-status" class="profile-note" role="status"></p>
    <table class="prediction-table"><thead><tr><th>Callsign</th><th>Elo</th><th>Score</th><th>Delegation mask</th><th>Flight</th></tr></thead><tbody id="leaderboard-rows"></tbody></table>
    <div class="profile-row"><button type="button" id="leaderboard-prev">Previous</button><button type="button" id="leaderboard-next">Next</button><button type="button" id="leaderboard-refresh">Refresh</button></div>`;
  document.body.append(dialog);
  $('leaderboard-close').addEventListener('click', () => dialog.close());
  dialog.addEventListener('click', event => { if (event.target === dialog) dialog.close(); });
  dialog.addEventListener('close', () => { token++; lastFocused?.focus(); });
  $('leaderboard-contract').addEventListener('change', () => { page = 1; void refresh(); });
  $('leaderboard-prev').addEventListener('click', () => { page = Math.max(1, page - 1); void refresh(); });
  $('leaderboard-next').addEventListener('click', () => { page++; void refresh(); });
  $('leaderboard-refresh').addEventListener('click', () => void refresh());
}
export function openLeaderboard() {
  if (!dialog) build();
  lastFocused = document.activeElement; page = 1; dialog.showModal(); void refresh();
}
export function initLeaderboard() { $('open-leaderboard')?.addEventListener('click', openLeaderboard); }
