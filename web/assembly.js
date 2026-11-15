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
  array: 8,
});

/// Snapshot the assembly the page is flying, for the worker payload.
export function assemblyPayload(wasm) {
  // A storable stage has no cryogenic fit-out, and the bridge refuses cryo
  // edits there. Sending them anyway would make every reconfigure fail, so
  // the snapshot carries only the knobs this stage actually has.
  const cryogenic = wasm.mission_thermal_get(7) === 1;
  return {
    mask: wasm.mission_delegation_mask(),
    // Order matters: a cooler needs radiator area before it can be set.
    fields: cryogenic ? [
      [THERMAL_FIELDS.layers, wasm.mission_thermal_get(0)],
      [THERMAL_FIELDS.strut, wasm.mission_thermal_get(4)],
      [THERMAL_FIELDS.radiator, wasm.mission_thermal_get(3)],
      [THERMAL_FIELDS.cooler, wasm.mission_thermal_get(2)],
      [THERMAL_FIELDS.array, wasm.mission_thermal_get(8)],
    ] : [],
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
    array: el('thermal-array'),
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

  // Which department owns each control, read from Rust so the browser never
  // restates the field-ownership map.
  const OWNER = Object.fromEntries(Object.entries({ mli: 0, strut: 4, radiator: 3, cooler: 2, array: 8 })
    .map(([key, field]) => [key, wasm.mission_thermal_owner(field)]));
  const FIELD = { mli: 0, strut: 4, radiator: 3, cooler: 2, array: 8 };

  function ownerDelegated(key) {
    const owner = OWNER[key];
    return owner !== 0xFFFFFFFF && (wasm.mission_delegation_mask() & (1 << owner)) !== 0;
  }

  // A delegated control displays the CHIEF's chosen value and is read-only;
  // a manual control displays the player's own value. The player's value is
  // kept in Rust, so toggling back to MANUAL restores what they had typed.
  function syncInputs() {
    const get = field => wasm.mission_thermal_get(field);
    const flown = field => wasm.mission_thermal_effective(field);
    for (const [key, control] of Object.entries(thermalControls)) {
      const value = ownerDelegated(key) ? flown(FIELD[key]) : get(FIELD[key]);
      control.value = key === 'mli' ? String(Math.round(value)) : String(value);
      control.readOnly = ownerDelegated(key);
      control.classList.toggle('chief-owned', ownerDelegated(key));
    }
    const epsilon = ownerDelegated('mli') ? flown(1) : get(1);
    el('thermal-epsilon').textContent = `effective emittance ${fmt(epsilon, 5)}`;
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
    const choices = [];
    if (mask & (1 << 0)) {
      choices.push(`Propulsion chief flies the qualified reference engine: ${fmt(wasm.mission_engine_effective(0), 1)} N at ${fmt(wasm.mission_engine_effective(1), 2)} s specific impulse.`);
    }
    if (mask & (1 << 1)) {
      const lift = wasm.mission_thermal_effective(2);
      const cooling = lift > 0
        ? `a ${fmt(lift, 0)} W cooler rejecting through ${fmt(wasm.mission_thermal_effective(3), 0)} m² of radiator`
        : 'no cooler';
      choices.push(`Thermal chief buys ${fmt(wasm.mission_thermal_effective(0), 0)} blanket layers on a stainless shell with ${fmt(wasm.mission_thermal_effective(4), 2)} W/K mounts and ${cooling}.`);
    }
    if (mask & (1 << 2)) {
      choices.push(`Power chief sizes the array at ${fmt(wasm.mission_thermal_effective(8), 0)} W.`);
    }
    if (mask & (1 << 3)) {
      choices.push(`Guidance and control chief buys ${fmt(wasm.mission_thermal_effective(9), 0)} W of avionics.`);
    }
    for (const line of choices) {
      const row = document.createElement('p');
      row.className = 'mission-sub chief-choice';
      row.textContent = line;
      badges.append(row);
    }
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
    for (const [key, control] of Object.entries(thermalControls)) {
      // A delegated slider is locked because the chief owns it, not because
      // the mission is locked; the tooltip says which.
      control.disabled = locked || !cryogenic || ownerDelegated(key);
      control.title = ownerDelegated(key)
        ? 'AUTO-DELEGATED: the chief sized this. Switch to MANUAL to size it yourself.'
        : '';
    }
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
  const fields = { mli: 0, strut: 4, radiator: 3, cooler: 2, array: 8 };
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
      // What the stage actually carries, after any chief substitution.
      mliLayers: wasm.mission_thermal_effective(0),
      epsilon: wasm.mission_thermal_effective(1),
      coolerLift: wasm.mission_thermal_effective(2),
      radiatorArea: wasm.mission_thermal_effective(3),
      strutConductance: wasm.mission_thermal_effective(4),
      arrayWatts: wasm.mission_thermal_effective(8),
      avionicsWatts: wasm.mission_thermal_effective(9),
      // What the player typed, restored when a discipline goes back to MANUAL.
      player: {
        mliLayers: wasm.mission_thermal_get(0),
        coolerLift: wasm.mission_thermal_get(2),
        radiatorArea: wasm.mission_thermal_get(3),
        strutConductance: wasm.mission_thermal_get(4),
        arrayWatts: wasm.mission_thermal_get(8),
      },
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
