const main = document.getElementById('station-surface');
const buttons = [...document.querySelectorAll('[data-flight-mode]')];
const panels = [...document.querySelectorAll('[data-flight-panel]')];
const station = document.getElementById('active-station');
const debrief = document.getElementById('debrief');
const labels = {
  flight: 'FLIGHT DYNAMICS', academy: 'ACADEMY / GUIDED FLIGHT',
  propulsion: 'PROPULSION LAB', vehicle: 'VEHICLE ASSEMBLY',
  prediction: 'PREDICTION CONTRACT', debrief: 'FLIGHT DATA REVIEW'
};

function activate(mode, updateHash = true) {
  if (!labels[mode] || (mode === 'debrief' && debrief.hidden)) mode = 'flight';
  // Closing the lesson dock is temporary; selecting ACD must always reopen it
  // without forcing a page reload or losing the current vehicle and plan.
  if (mode === 'academy') document.getElementById('onboarding').hidden = false;
  main.dataset.mode = mode;
  station.textContent = labels[mode];
  for (const button of buttons) {
    const active = button.dataset.flightMode === mode;
    button.classList.toggle('is-active', active);
    button.setAttribute('aria-pressed', String(active));
  }
  for (const panel of panels) {
    const active = panel.dataset.flightPanel === mode ||
      (mode === 'academy' && panel.dataset.flightPanel === 'flight');
    panel.classList.toggle('is-active', active);
  }
  if (updateHash) history.replaceState(null, '', mode === 'academy' ? '#onboarding' : location.pathname + location.search);
  main.scrollTo({ top: 0, behavior: 'instant' });
}

for (const button of buttons) button.addEventListener('click', () => activate(button.dataset.flightMode));

// The console advertises station keys in its status line, so they must work:
// digits select a station, Q/E step along the rail. Typing in a field, an open
// dialog or a modifier chord is never treated as a station key.
const MODES = ['flight', 'academy', 'propulsion', 'vehicle', 'prediction', 'debrief'];
const typing = target => target instanceof Element
  && (target.closest('input,textarea,select,[contenteditable="true"]') !== null);
document.addEventListener('keydown', event => {
  if (event.metaKey || event.ctrlKey || event.altKey || event.repeat) return;
  if (typing(event.target) || document.querySelector('dialog[open]')) return;
  const available = MODES.filter(mode => mode !== 'debrief' || !debrief.hidden);
  if (/^[1-6]$/.test(event.key)) {
    const mode = MODES[Number(event.key) - 1];
    if (!available.includes(mode)) return;
    event.preventDefault(); activate(mode); return;
  }
  if (event.key === 'q' || event.key === 'Q' || event.key === 'e' || event.key === 'E') {
    const index = Math.max(0, available.indexOf(main.dataset.mode));
    const step = event.key === 'q' || event.key === 'Q' ? -1 : 1;
    const next = available[(index + step + available.length) % available.length];
    event.preventDefault(); activate(next);
  }
});
document.getElementById('onboarding-close').addEventListener('click', () => activate('flight'));

// A completed physical flight unlocks and opens the FDR station. Watching the
// existing hidden attribute avoids coupling the shell to simulation internals.
new MutationObserver(() => {
  const ready = !debrief.hidden;
  document.getElementById('debrief-mode').disabled = !ready;
  if (ready) activate('debrief');
}).observe(debrief, { attributes: true, attributeFilter: ['hidden'] });

activate(location.hash === '#onboarding' ? 'academy' : 'flight', false);

// Deep links stay live: a fragment change on an already-loaded console must move
// the player to the academy instead of leaving them on the old station.
window.addEventListener('hashchange', () => {
  activate(location.hash === '#onboarding' ? 'academy' : 'flight', false);
});
window.odderonFlightShell = Object.freeze({ activate, get mode() { return main.dataset.mode; } });
