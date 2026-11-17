const main = document.querySelector('.mission-main');
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
  document.querySelector('.mission-main').scrollTo({ top: 0, behavior: 'instant' });
}

for (const button of buttons) button.addEventListener('click', () => activate(button.dataset.flightMode));
document.getElementById('onboarding-close').addEventListener('click', () => activate('flight'));

// A completed physical flight unlocks and opens the FDR station. Watching the
// existing hidden attribute avoids coupling the shell to simulation internals.
new MutationObserver(() => {
  const ready = !debrief.hidden;
  document.getElementById('debrief-mode').disabled = !ready;
  if (ready) activate('debrief');
}).observe(debrief, { attributes: true, attributeFilter: ['hidden'] });

activate(location.hash === '#onboarding' ? 'academy' : 'flight', false);
window.odderonFlightShell = Object.freeze({ activate, get mode() { return main.dataset.mode; } });
