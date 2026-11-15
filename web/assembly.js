// Vehicle assembly: who sizes each discipline, and the cryogenic fit-out.
//
// Every number here is read back from the Rust bridge after it rebuilds the
// stage. This module never recomputes a margin, a badge or a mass: those come
// from odd_contracts::chief_ai and odd_contracts::thermal so the assembly
// panel, the flown stage and the signed recorder cannot disagree.

const DEPARTMENTS = [
  { bit: 0, name: 'Propulsion' },
  { bit: 1, name: 'Thermal / cryo' },
  { bit: 2, name: 'Power' },
  { bit: 3, name: 'Guidance and control' },
];

// Field ids of `mission_thermal_set`. Declared once here so the main page,
// the flight worker and the capture path cannot disagree about which knob is
// which; the same ids are asserted against Rust in the browser gate.
export const THERMAL_FIELDS = Object.freeze({
  layers: 0,
  epsilon: 1,
  cooler: 2,
  radiator: 3,
  strut: 4,
  relief: 5,
  shell: 6,
});

/// Snapshot the assembly the page is flying, for the worker payload.
export function assemblyPayload(wasm) {
  return {
    mask: wasm.mission_delegation_mask(),
    // Order matters: a cooler needs radiator area before it can be set.
    fields: [
      [THERMAL_FIELDS.layers, wasm.mission_thermal_get(0)],
      [THERMAL_FIELDS.strut, wasm.mission_thermal_get(4)],
      [THERMAL_FIELDS.radiator, wasm.mission_thermal_get(3)],
      [THERMAL_FIELDS.cooler, wasm.mission_thermal_get(2)],
    ],
  };
}

/// Replay that snapshot onto a fresh module. Returns false rather than
/// throwing so the caller can report which stage could not be rebuilt.
export function applyAssembly(wasm, data) {
  if (!data) return true;
  for (const [field, value] of data.fields ?? []) {
    if (wasm.mission_thermal_set(field, value) !== 1) return false;
  }
  return wasm.mission_delegation_set(data.mask ?? 0) === 1;
}

function readString(wasm, lenFn, byteFn, ...args) {
  const len = lenFn(...args);
  if (!(len > 0)) return '';
  const bytes = new Uint8Array(len);
  for (let i = 0; i < len; i++) bytes[i] = byteFn(...args, i);
  return new TextDecoder().decode(bytes);
}

export function createAssembly({ wasm, isLocked, annotate, onChange }) {
  const el = id => document.getElementById(id);
  const toggles = DEPARTMENTS.map(d => el(`delegate-${d.bit}`));
  const thermalControls = {
    mli: el('thermal-mli'),
    strut: el('thermal-strut'),
    radiator: el('thermal-radiator'),
    cooler: el('thermal-cooler'),
  };
  const status = el('delegation-status');
  const thermalStatus = el('thermal-status');
  const readout = el('assembly-readout');
  const badges = el('delegation-badges');

  function fmt(value, decimals = 2) {
    return Number.isFinite(value)
      ? value.toLocaleString(undefined, { minimumFractionDigits: decimals, maximumFractionDigits: decimals })
      : '—';
  }

  // Pull the canonical design back out of Rust; the inputs display what the
  // stage actually carries, never what the user last typed.
  function syncInputs() {
    const get = field => wasm.mission_thermal_get(field);
    thermalControls.mli.value = String(Math.round(get(0)));
    thermalControls.strut.value = String(get(4));
    thermalControls.radiator.value = String(get(3));
    thermalControls.cooler.value = String(get(2));
    el('thermal-epsilon').textContent = `effective emittance ${fmt(get(1), 5)}`;
    const mask = wasm.mission_delegation_mask();
    toggles.forEach((toggle, i) => { toggle.checked = (mask & (1 << DEPARTMENTS[i].bit)) !== 0; });
    // Storable stages have no cryogenic fit-out; say so rather than offering
    // a knob that does nothing.
    const cryogenic = wasm.mission_thermal_get(7) === 1;
    for (const control of Object.values(thermalControls)) control.dataset.cryogenic = String(cryogenic);
  }

  function render() {
    const margin = wasm.mission_delegation_readout(0);
    const cost = wasm.mission_delegation_readout(1);
    const dry = wasm.mission_delegation_readout(2);
    const wet = wasm.mission_delegation_readout(3);
    const cap = wasm.mission_delegation_readout(4);
    const mask = wasm.mission_delegation_mask();
    const delegated = DEPARTMENTS.filter(d => (mask & (1 << d.bit)) !== 0);

    readout.replaceChildren();
    for (const [label, value] of [
      ['Chief margin added to dry mass', `${fmt(margin)} kg`],
      ['Relative stack cost', `×${fmt(cost)}`],
      ['Dry mass', `${fmt(dry)} kg`],
      ['Wet mass', `${fmt(wet)} kg`],
      ['Efficiency ceiling', fmt(cap, 2)],
      ['Cryogenic fit-out beyond baseline', `${fmt(wasm.mission_delegation_readout(5))} kg`],
    ]) {
      const cell = document.createElement('div');
      const strong = document.createElement('strong');
      const span = document.createElement('span');
      strong.textContent = value;
      span.textContent = label;
      cell.append(strong, span);
      readout.append(cell);
    }

    badges.replaceChildren();
    for (const d of DEPARTMENTS) {
      const text = readString(
        wasm,
        wasm.mission_delegation_badge_len,
        wasm.mission_delegation_badge_byte,
        d.bit,
      );
      if (!text) continue;
      const row = document.createElement('p');
      row.className = 'mission-sub';
      row.textContent = text;
      badges.append(row);
    }

    status.textContent = delegated.length === 0
      ? 'All disciplines are hand-sized. Efficiency is uncapped.'
      : `${delegated.length} of 4 disciplines delegated. Efficiency is capped at ${fmt(cap, 2)}; the margin above is real dry mass.`;

    const lift = wasm.mission_thermal_get(2);
    const radiator = wasm.mission_thermal_get(3);
    const cryogenic = wasm.mission_thermal_get(7) === 1;
    thermalStatus.textContent = !cryogenic
      ? 'This stage flies storable propellants, so there is no cryogenic fit-out to size and nothing boils off.'
      : lift > 0
      ? `Active cooling: ${fmt(lift, 0)} W lifted at the tank through ${fmt(radiator, 1)} m² of radiator. The compressor draws far more from the bus.`
      : 'Passive tanks. Heat that leaks in either pressurizes the tank or, once the relief valve opens, vents propellant overboard.';
    annotate(readout);
    annotate(badges);
    annotate(status);
    annotate(thermalStatus);
  }

  function setLocked() {
    const locked = isLocked();
    const cryogenic = wasm.mission_thermal_get(7) === 1;
    for (const control of toggles) control.disabled = locked;
    for (const control of Object.values(thermalControls)) control.disabled = locked || !cryogenic;
  }

  function rebuild(ok, message) {
    if (ok !== 1) {
      status.textContent = message;
      annotate(status);
      syncInputs();
      render();
      return false;
    }
    syncInputs();
    render();
    setLocked();
    onChange();
    return true;
  }

  toggles.forEach((toggle, i) => {
    toggle.addEventListener('change', () => {
      let mask = wasm.mission_delegation_mask();
      if (toggle.checked) mask |= (1 << DEPARTMENTS[i].bit);
      else mask &= ~(1 << DEPARTMENTS[i].bit);
      rebuild(wasm.mission_delegation_set(mask), 'The stage could not be rebuilt with that delegation.');
      setLocked();
    });
  });

  // field ids mirror mission_thermal_set: 0 layers, 4 strut, 3 radiator, 2 lift.
  const fields = { mli: 0, strut: 4, radiator: 3, cooler: 2 };
  for (const [key, field] of Object.entries(fields)) {
    const control = thermalControls[key];
    control.addEventListener('change', () => {
      const value = Number(control.value);
      const ok = wasm.mission_thermal_set(field, value);
      rebuild(ok, wasm.mission_thermal_get(7) !== 1
        ? 'This stage flies storable propellants; there is no cryogenic fit-out to size.'
        : key === 'cooler'
          ? 'A cooler needs radiator area to reject its heat. Add radiator first.'
          : 'That cryogenic design is outside the physical range.');
      setLocked();
    });
  }

  syncInputs();
  render();
  setLocked();

  return {
    sync: () => { syncInputs(); render(); },
    setLocked,
    diagnostics: () => ({
      mask: wasm.mission_delegation_mask(),
      margin: wasm.mission_delegation_readout(0),
      cost: wasm.mission_delegation_readout(1),
      dryMass: wasm.mission_delegation_readout(2),
      wetMass: wasm.mission_delegation_readout(3),
      cap: wasm.mission_delegation_readout(4),
      mliLayers: wasm.mission_thermal_get(0),
      epsilon: wasm.mission_thermal_get(1),
      coolerLift: wasm.mission_thermal_get(2),
      radiatorArea: wasm.mission_thermal_get(3),
      strutConductance: wasm.mission_thermal_get(4),
      cryogenic: wasm.mission_thermal_get(7) === 1,
      fitoutMass: wasm.mission_delegation_readout(5),
    }),
  };
}

/// Post-flight cryogenic readout, read from the executed thermal balance.
export function boilOffSummary(wasm) {
  if (!wasm || typeof wasm.mission_boil_off !== 'function') return null;
  const lost = wasm.mission_boil_off(0);
  if (!Number.isFinite(lost)) return null;
  return {
    lost,
    oxidizer: wasm.mission_boil_off(1),
    fuel: wasm.mission_boil_off(2),
    coolerJoules: wasm.mission_boil_off(3),
    vents: wasm.mission_boil_off(4),
    inert: wasm.mission_boil_off(5) === 1,
  };
}
