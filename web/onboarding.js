// Event-driven first-30-minutes guide. It never simulates a result or runs a
// timer: completion is derived from the mission page's real state after input.
export const ONBOARDING_VERSION = 1;
export const ONBOARDING_KEY = `odderon:onboarding:v${ONBOARDING_VERSION}`;

export const LESSONS = Object.freeze([
  Object.freeze({
    title: 'Your satellite is falling', contract: 101, focus: '#plan-canvas',
    copy: 'This satellite is safe but in the wrong place. It needs to be higher, and rounder. Drag the gold arrow to push it. Press LOCK when you think it is right.',
    hints: ['Watch which side of the orbit moves.', 'One push raises the opposite side.', 'Move around the orbit before adding another push.', 'Use two prograde pushes, one near each low point.'],
  }),
  Object.freeze({
    title: 'Guess first', contract: 101, focus: '#prediction-title',
    copy: 'Before you push: where will the far side end up? Set your prediction, then make the same burn as last time.',
    hints: ['Prediction must happen before the modeled flight.', 'The ideal cyan path is a useful first estimate.', 'Use the ideal-prediction button, then change one value.', 'Fill every required prediction field before locking.'],
  }),
  Object.freeze({
    title: 'Tilting the orbit', contract: 102, focus: '#node-w',
    copy: 'This orbit has to cross the same ground at the same time every day. Its plane is short. Which of the three handles turns the plane?',
    hints: ['Height and plane are separate.', 'In-plane sideways is not out of plane.', 'Watch the purple Plane tilt handle.', 'Add at least 50 meters per second out of the orbital plane.'],
  }),
  Object.freeze({
    title: 'The shaking engine', contract: 103, focus: '#engine-altitude',
    copy: 'This engine was shaped for space. Drag the altitude slider up and watch the exhaust. Find where the flow stops separating from the bell.',
    hints: ['Ambient pressure falls with altitude.', 'Watch the separation warning, not only thrust.', 'The transition is below 20 kilometers.', 'Move the test stand to at least 17 kilometers.'],
  }),
  Object.freeze({
    title: 'The long-burn trap', contract: 103, focus: '#split-kick',
    copy: 'This tug is heavy and its engine is patient. One burn is the obvious plan. Watch where it ends, then divide the same push.',
    hints: ['A long burn sweeps through a large orbital angle.', 'Keep the total velocity change; change its timing.', 'The splitter is available in Practice mode.', 'Use three numerical periapsis kicks.'],
  }),
  Object.freeze({
    title: 'First real contract', contract: 101, focus: '#prediction-title',
    copy: 'Same rescue, real contract. Fill in what you think will happen, lock it, and fly it. The score is not only about the orbit.',
    hints: ['Half the score is the objective.', 'Prediction and efficiency are each one quarter.', 'A score requires a locked prediction.', 'Use your two-push rescue and compare every debrief row.'],
  }),
]);

export function lessonComplete(index, state) {
  switch (index) {
    case 0: return state.contract === 101 && state.nodes >= 2;
    case 1: return state.contract === 101 && state.predictionReady;
    case 2: return state.contract === 102 && state.maxPlaneChange >= 50;
    case 3: return state.contract === 103 && state.engineAltitudeKm >= 17 && !state.engineSeparated;
    case 4: return state.contract === 103 && state.nodes >= 3;
    case 5: return state.contract === 101 && Number.isFinite(state.score);
    default: return false;
  }
}

function safeProgress(storage) {
  try {
    const value = JSON.parse(storage.getItem(ONBOARDING_KEY));
    if (value?.version === ONBOARDING_VERSION
        && Number.isInteger(value.lesson)
        && value.lesson >= 0 && value.lesson < LESSONS.length) return value;
  } catch { /* A malformed local value is not tutorial state. */ }
  return null;
}

export function createOnboarding({ snapshot, activate, storage = localStorage }) {
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
  let active = false, lesson = 0, hintIndex = 0;

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
    const complete = lessonComplete(lesson, snapshot());
    status.textContent = complete ? 'Measured condition reached.' : 'Act in the real mission controls to continue.';
    status.classList.toggle('is-complete', complete);
    next.disabled = !complete;
  };
  const begin = (at = 0) => {
    active = true; lesson = at;
    start.hidden = true; skip.hidden = true; lessonUi.hidden = false;
    activate(LESSONS[lesson], lesson);
    render(); persist();
  };
  start.addEventListener('click', () => begin(0));
  skip.addEventListener('click', () => { panel.hidden = true; });
  save.addEventListener('change', () => {
    if (save.checked) {
      const prior = safeProgress(storage);
      if (prior && !active) begin(prior.lesson);
      else persist();
    } else {
      try { storage.removeItem(ONBOARDING_KEY); } catch { /* no storage */ }
    }
  });
  hint.addEventListener('click', () => {
    const hints = LESSONS[lesson].hints;
    hintIndex = Math.min(hintIndex + 1, hints.length);
    hint.textContent = hints[hintIndex - 1];
    hint.dataset.level = String(hintIndex);
  });
  next.addEventListener('click', () => {
    if (!lessonComplete(lesson, snapshot())) return;
    if (lesson === LESSONS.length - 1) {
      status.textContent = 'Guide complete. The contracts are now yours.';
      next.hidden = true; persist(); return;
    }
    lesson += 1; activate(LESSONS[lesson], lesson); render(); persist();
  });
  close.addEventListener('click', () => { panel.hidden = true; });
  document.addEventListener('input', update);
  document.addEventListener('change', update);
  document.addEventListener('click', update);
  return Object.freeze({ update, get lesson() { return lesson; }, get active() { return active; } });
}
