const settings = document.getElementById('settings-dialog');
const effects = document.getElementById('reduced-effects');
const contrast = document.getElementById('high-contrast');
const state = document.getElementById('system-state');

function stored(name) {
  try { return localStorage.getItem(`odderon.setting.${name}`) === 'true'; }
  catch { return false; }
}
function apply() {
  document.body.classList.toggle('reduced-effects', effects.checked);
  document.body.classList.toggle('high-contrast', contrast.checked);
}
effects.checked = stored('reduced-effects') || matchMedia('(prefers-reduced-motion: reduce)').matches;
contrast.checked = stored('high-contrast');
apply();
for (const [control, name] of [[effects, 'reduced-effects'], [contrast, 'high-contrast']]) {
  control.addEventListener('change', () => {
    try { localStorage.setItem(`odderon.setting.${name}`, String(control.checked)); } catch {}
    apply();
  });
}
document.getElementById('open-settings').addEventListener('click', () => settings.showModal());
// A game menu must be immediately operable from a keyboard or controller-style
// Enter key, rather than requiring a pointer click before focus exists.
document.querySelector('.menu-item.primary').focus({ preventScroll: true });

// Verify the generated flight module without instantiating a second simulation.
// The menu remains usable if the optional build artifact is absent.
fetch('./odd_view.wasm', { method: 'HEAD' }).then(response => {
  if (!response.ok) throw new Error();
  state.textContent = 'FLIGHT COMPUTER READY';
  state.parentElement.classList.add('ready');
}).catch(() => {
  state.textContent = 'FLIGHT MODULE NOT BUILT';
});
