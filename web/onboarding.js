// Event-driven first-30-minutes guide. It never simulates a result or runs a
// timer: completion is derived from the mission page's real state after input.
export const ONBOARDING_VERSION = 1;
export const ONBOARDING_KEY = `odderon:onboarding:v${ONBOARDING_VERSION}`;
export const PLAYTEST_KEY = 'odderon:playtest:v1';

export function placementRoute(checks) {
  if (!checks.apsis) return 0;
  if (!checks.phasing) return 1;
  if (!checks.massPrediction) return 6;
  return 'ranked';
}

// Event-driven and local-only. With consent false no event is retained in
// memory and no storage operation is attempted.
export function createPlaytestRecorder({ storage = localStorage, now = () => Date.now() } = {}) {
  let consent = false;
  let session = null;
  let current = null;
  const freshSession = () => ({ schema: 1, session_id: `local-${now().toString(36)}`, started_at_ms: now(), observations: [] });
  const persist = () => {
    if (!consent || !session) return;
    try { storage.setItem(PLAYTEST_KEY, JSON.stringify(session)); } catch { /* storage can be unavailable */ }
  };
  const ensure = () => {
    if (!consent) return null;
    session ??= freshSession();
    return session;
  };
  return Object.freeze({
    setConsent(enabled) {
      consent = enabled === true;
      if (consent) {
        try { const prior = JSON.parse(storage.getItem(PLAYTEST_KEY)); if (prior?.schema === 1 && Array.isArray(prior.observations)) session = prior; } catch {}
        ensure(); persist();
      } else { session = null; current = null; }
    },
    startLesson(lessonId, initial = {}) {
      if (!ensure()) return;
      current = { lesson_id: lessonId, completed: false, time_to_threshold_ms: 0, retry_count: 0, hint_open_count: 0,
        causal_rules_triggered: [], first_burn_direction_on_phasing: initial.firstBurnDirection ?? null,
        schema5_mass_delta_kg: null, prediction_error_before_redesign_kg: initial.predictionError ?? null,
        prediction_error_final_kg: null, drop_off_step: null, _started: now() };
    },
    hint() { if (current) current.hint_open_count += 1; },
    retry() { if (current) current.retry_count += 1; },
    setPhasingDirection(direction) { if (current && current.first_burn_direction_on_phasing === null) current.first_burn_direction_on_phasing = direction; },
    causal(rule) { if (current && rule && !current.causal_rules_triggered.includes(rule)) current.causal_rules_triggered.push(rule); },
    complete(values = {}) {
      if (!current || !ensure()) return;
      current.completed = true;
      current.time_to_threshold_ms = Math.max(0, now() - current._started);
      current.schema5_mass_delta_kg = values.massDelta ?? null;
      current.prediction_error_final_kg = values.predictionError ?? null;
      delete current._started;
      session.observations.push(current); current = null; persist();
    },
    dropOff(step) {
      if (!current || !ensure()) return;
      current.time_to_threshold_ms = Math.max(0, now() - current._started);
      current.drop_off_step = step; delete current._started;
      session.observations.push(current); current = null; persist();
    },
    exportJson() { return consent && session ? JSON.stringify(session, null, 2) : null; },
    clear() { session = consent ? freshSession() : null; current = null; try { storage.removeItem(PLAYTEST_KEY); } catch {} },
    get count() { return session?.observations.length ?? 0; },
    get enabled() { return consent; },
  });
}

export const LESSONS = Object.freeze([
  Object.freeze({
    title: 'Your satellite is falling', contract: 101, focus: '#plan-canvas',
    copy: 'This satellite is safe but in the wrong place. It needs to be higher, and rounder. Drag the gold arrow to push it. Press LOCK when you think it is right.',
    success: 'The ideal path has its low side above 500 km and eccentricity below 0.02.',
    hints: ['Watch which side of the orbit moves.', 'One push raises the opposite side.', 'Move around the orbit before adding another push.', 'Use two prograde pushes, one near each low point.'],
  }),
  Object.freeze({
    title: 'Catch up by falling behind', contract: 101, focus: '#node-t', reset: true,
    copy: 'A target is ahead in the same orbit. Enter a lower, faster phasing orbit first. Which burn direction drops the opposite side without entering the atmosphere?',
    success: 'The first burn is retrograde and the ideal low side is between 120 and 240 km.',
    hints: ['A lower orbit moves around Earth faster.', 'The first burn must lower the opposite side.', 'Pull the gold tangential handle backward.', 'Use a retrograde burn until periapsis is below 240 km but above 120 km.'],
  }),
  Object.freeze({
    title: 'Tilting the orbit', contract: 102, focus: '#node-w',
    copy: 'This orbit has to cross the same ground at the same time every day. Its plane is short. Which of the three handles turns the plane?',
    success: 'The flown orbit reaches at least 98.50 degrees and both apsides are within 40 km of 800 km.',
    hints: ['Height and plane are separate.', 'In-plane sideways is not out of plane.', 'Watch the purple Plane tilt handle.', 'The measured reference uses 366.8 meters per second across height and plane.'],
  }),
  Object.freeze({
    title: 'The shaking engine', contract: 103, focus: '#engine-altitude',
    copy: 'This engine was shaped for space. Drag the altitude slider up and watch the exhaust. Find where the flow stops separating from the bell.',
    success: 'The solved plume is attached at or above the measured 17 km transition.',
    hints: ['Ambient pressure falls with altitude.', 'Watch the separation warning, not only thrust.', 'The transition is below 20 kilometers.', 'Move the test stand to at least 17 kilometers.'],
  }),
  Object.freeze({
    title: 'The long-burn trap', contract: 103, focus: '#split-kick',
    copy: 'This tug is heavy and its engine is patient. One burn is the obvious plan. Watch where it ends, then divide the same push.',
    success: 'The modeled flight survives and reaches at least 380,000 km from Earth’s center.',
    hints: ['A long burn sweeps through a large orbital angle.', 'Keep the total velocity change; change its timing.', 'The splitter is available in Practice mode.', 'Use three numerical periapsis kicks, then lock and fly.'],
  }),
  Object.freeze({
    title: 'First real contract', contract: 101, focus: '#prediction-title',
    copy: 'Same rescue, real contract. Fill in what you think will happen, lock it, and fly it. The score is not only about the orbit.',
    success: 'The baseline rescue completed with a finite score above zero.',
    hints: ['Half the score is the objective.', 'Prediction and efficiency are each one quarter.', 'A score requires a locked prediction.', 'Use your two-push rescue and compare every debrief row.'],
  }),
  Object.freeze({
    title: 'The mass budget', contract: 103, focus: '#assembly', schema5: true,
    copy: 'Every chief starts in control. Make the fitted stage lighter, update remaining propellant for that real mass, then fly an objective-reaching Schema 5 record.',
    success: 'Fitted dry mass beat the delegated baseline, remaining-propellant error stayed below 2 kilograms, and objective efficiency exceeded 0.65.',
    hints: ['Delegated margin is real dry mass.', 'Take Thermal and Propulsion manual, then remove every efficiency cap.', 'Right-size blanket, cooler, radiator, array, and nozzle before locking.', 'Use the fitted ledger—not catalog mass—when predicting remaining propellant.'],
  }),
]);

export function lessonComplete(index, state) {
  switch (index) {
    case 0:
      return state.contract === 101
        && state.nodes >= 2
        && state.idealPeriapsisKm >= 500
        && state.idealEccentricity < 0.02;
    case 1:
      return state.contract === 101
        && state.nodes >= 1
        && state.firstTangentialDv < 0
        && state.idealPeriapsisKm >= 120
        && state.idealPeriapsisKm < 240;
    case 2:
      return state.contract === 102
        && state.flown
        && state.inclinationDeg >= 98.50
        && Math.abs(state.apoapsisKm - 800) <= 40
        && Math.abs(state.periapsisKm - 800) <= 40;
    case 3:
      return state.contract === 103
        && state.engineAltitudeKm >= 17
        && !state.engineSeparated;
    case 4:
      return state.contract === 103
        && state.flown
        && !state.impacted
        && state.apoapsisKm * 1000 + 6_378_137 >= 380_000_000;
    case 5:
      return state.contract === 101 && Number.isFinite(state.score) && state.score > 0;
    case 6:
      return state.contract === 103
        && state.schemaVersion === 5
        && state.verified
        && state.flown
        && Number.isFinite(state.delegatedBaselineDryKg)
        && state.fittedDryKg < state.delegatedBaselineDryKg
        && state.predictionErrorKg < 2.0
        && state.efficiency > 0.65
        && state.objectivePassed;
    default: return false;
  }
}

function safeProgress(storage) {
  try {
    const value = JSON.parse(storage.getItem(ONBOARDING_KEY));
    if (value?.version === ONBOARDING_VERSION
        && Number.isInteger(value.lesson)
        && value.lesson >= 0 && value.lesson < LESSONS.length
        && (value.complete === undefined || typeof value.complete === 'boolean')) return value;
  } catch { /* A malformed local value is not tutorial state. */ }
  return null;
}

export function createOnboarding({ snapshot, activate, storage = localStorage, recorder = null }) {
  const panel = document.getElementById('onboarding');
  if (!panel) return null;
  const start = document.getElementById('onboarding-start');
  const skip = document.getElementById('onboarding-skip');
  const save = document.getElementById('onboarding-save');
  const lessonUi = document.getElementById('onboarding-lesson');
  const title = document.getElementById('onboarding-title');
  const copy = document.getElementById('onboarding-copy');
  const progress = document.getElementById('onboarding-progress');
  const status = document.getElementById('onboarding-status');
  const hint = document.getElementById('onboarding-hint');
  const next = document.getElementById('onboarding-next');
  const close = document.getElementById('onboarding-close');
  const ledger = document.getElementById('onboarding-mass-ledger');
  let active = false, lesson = 0, hintIndex = 0, recordedComplete = false;

  const persist = () => {
    if (!save.checked) return;
    try { storage.setItem(ONBOARDING_KEY, JSON.stringify({ version: ONBOARDING_VERSION, lesson })); } catch { /* local storage can be unavailable */ }
  };
  const render = () => {
    const item = LESSONS[lesson];
    title.textContent = item.title;
    copy.textContent = item.copy;
    progress.textContent = `LESSON ${lesson + 1} / ${LESSONS.length}`;
    hint.textContent = 'Show a hint';
    hint.dataset.level = '0';
    hintIndex = 0;
    next.textContent = lesson === LESSONS.length - 1 ? 'Finish guide' : 'Next lesson';
    update();
  };
  const update = () => {
    if (!active) return;
    const state = snapshot();
    const complete = lessonComplete(lesson, state);
    status.textContent = complete ? LESSONS[lesson].success : 'Act in the real mission controls to continue.';
    status.classList.toggle('is-complete', complete);
    next.disabled = !complete;
    if (ledger) {
      ledger.hidden = lesson !== 6;
      if (lesson === 6) ledger.textContent = `SCHEMA 5 MASS LEDGER · wet ${Number(state.wetMassKg).toFixed(1)} kg = fitted dry ${Number(state.fittedDryKg).toFixed(1)} kg + propellant ${Number(state.initialPropellantKg).toFixed(1)} kg · delegated baseline ${Number(state.delegatedBaselineDryKg).toFixed(1)} kg · final prediction error ${Number(state.predictionErrorKg).toFixed(2)} kg`;
    }
    if (complete && !recordedComplete) {
      recorder?.complete({ massDelta: state.fittedDryKg - state.delegatedBaselineDryKg, predictionError: state.predictionErrorKg });
      recordedComplete = true;
    }
  };
  const begin = (at = 0) => {
    active = true; lesson = at; recordedComplete = false;
    start.hidden = true; skip.hidden = true; lessonUi.hidden = false;
    activate(LESSONS[lesson], lesson);
    recorder?.startLesson(lesson + 1, { predictionError: snapshot().predictionErrorBeforeKg });
    render(); persist();
  };
  start.addEventListener('click', () => begin(0));
  skip.addEventListener('click', () => { panel.hidden = true; });
  save.addEventListener('change', () => {
    if (save.checked) {
      const prior = safeProgress(storage);
      if (prior?.complete) panel.hidden = true;
      else if (prior && !active) begin(prior.lesson);
      else persist();
    } else {
      try { storage.removeItem(ONBOARDING_KEY); } catch { /* no storage */ }
    }
  });
  hint.addEventListener('click', () => {
    recorder?.hint();
    const hints = LESSONS[lesson].hints;
    hintIndex = Math.min(hintIndex + 1, hints.length);
    hint.textContent = hints[hintIndex - 1];
    hint.dataset.level = String(hintIndex);
  });
  next.addEventListener('click', () => {
    if (!lessonComplete(lesson, snapshot())) return;
    if (lesson === LESSONS.length - 1) {
      status.textContent = 'Guide complete. The contracts are now yours.';
      next.hidden = true;
      if (save.checked) { try { storage.setItem(ONBOARDING_KEY, JSON.stringify({ version: ONBOARDING_VERSION, lesson, complete: true })); } catch {} }
      return;
    }
    lesson += 1; recordedComplete = false; activate(LESSONS[lesson], lesson);
    recorder?.startLesson(lesson + 1, { predictionError: snapshot().predictionErrorBeforeKg });
    render(); persist();
  });
  close.addEventListener('click', () => { recorder?.dropOff(`lesson-${lesson + 1}`); panel.hidden = true; });
  document.addEventListener('input', update);
  document.addEventListener('change', update);
  document.addEventListener('click', update);
  return Object.freeze({ update, begin, get lesson() { return lesson; }, get active() { return active; } });
}
