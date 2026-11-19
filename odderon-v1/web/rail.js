// The console rail: reads the planner's published state and stamps it onto the
// machined hardware — the station keys' lamps, the header's authority block and
// the status rail. Presentation only; the station switcher stays in
// `flight-shell.js` and the physics stays in Rust.
const $ = id => document.getElementById(id);

const update = () => {
  const mode = $('station-surface')?.dataset.mode;
  const authority = $('attempt-mode')?.value;
  const block = document.querySelector('.cmd-authority strong');
  if (block) block.textContent = authority === 'ranked' ? 'RANKED / SERVER' : 'LOCAL / PRACTICE';
  const lamp = $('debrief-mode');
  if (lamp) lamp.classList.toggle('is-complete', !lamp.disabled);
  const surface = $('station-surface');
  if (surface) {
    const panel = surface.querySelector('[data-flight-panel].is-active');
    const busy = $('plan-status');
    if (busy) busy.parentElement?.classList.toggle('is-busy', /loading|simulat|commit|targeting/i.test(busy.textContent ?? ''));
  }
  document.body.dataset.station = mode ?? 'flight';
};

function attach() {
  const surface = $('station-surface');
  if (surface) new MutationObserver(update).observe(surface, { attributes: true, attributeFilter: ['data-mode'] });
  const attempt = $('attempt-mode');
  if (attempt) attempt.addEventListener('change', update);
  for (const id of ['plan-status', 'execute-status', 'debrief-mode']) {
    const node = $(id);
    if (node) new MutationObserver(update).observe(node, { childList: true, characterData: true, subtree: true, attributes: true });
  }
  update();
}

if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', attach, { once: true });
else attach();
