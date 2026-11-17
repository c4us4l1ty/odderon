import test from 'node:test';
import assert from 'node:assert/strict';
import { LESSONS, createPlaytestRecorder, lessonComplete, placementRoute } from '../../web/onboarding.js';

const base = {
  contract: 101,
  nodes: 0,
  predictionReady: false,
  idealApoapsisKm: NaN,
  idealPeriapsisKm: NaN,
  idealEccentricity: NaN,
  engineAltitudeKm: 0,
  engineSeparated: true,
  flown: false,
  apoapsisKm: NaN,
  periapsisKm: NaN,
  eccentricity: NaN,
  inclinationDeg: NaN,
  impacted: false,
  score: NaN,
};

test('every onboarding lesson stays under the pre-action copy budget', () => {
  assert.equal(LESSONS.length, 7);
  for (const lesson of LESSONS) {
    const words = lesson.copy.trim().split(/\s+/).length;
    assert.ok(words <= 40, `${lesson.title} has ${words} words`);
    assert.equal(lesson.hints.length, 4);
    assert.ok(lesson.success.length > 0);
  }
});

test('lesson one requires the measured orbit, not merely two controls', () => {
  assert.equal(lessonComplete(0, { ...base, nodes: 2 }), false);
  assert.equal(lessonComplete(0, {
    ...base, nodes: 2, idealPeriapsisKm: 499.9, idealEccentricity: 0.001,
  }), false);
  assert.equal(lessonComplete(0, {
    ...base, nodes: 2, idealPeriapsisKm: 500, idealEccentricity: 0.0199,
  }), true);
});

test('flight lessons require modeled outcomes at their documented thresholds', () => {
  assert.equal(lessonComplete(1, { ...base, predictionReady: true }), false);
  assert.equal(lessonComplete(1, { ...base, predictionReady: true, flown: true }), true);

  const plane = { ...base, contract: 102, flown: true, inclinationDeg: 98.50, apoapsisKm: 800, periapsisKm: 800 };
  assert.equal(lessonComplete(2, { ...plane, inclinationDeg: 98.499 }), false);
  assert.equal(lessonComplete(2, { ...plane, periapsisKm: 759.9 }), false);
  assert.equal(lessonComplete(2, plane), true);

  assert.equal(lessonComplete(3, { ...base, contract: 103, engineAltitudeKm: 17, engineSeparated: true }), false);
  assert.equal(lessonComplete(3, { ...base, contract: 103, engineAltitudeKm: 17, engineSeparated: false }), true);

  const lunarAltitudeKm = (380_000_000 - 6_378_137) / 1000;
  assert.equal(lessonComplete(4, { ...base, contract: 103, flown: true, apoapsisKm: lunarAltitudeKm, impacted: true }), false);
  assert.equal(lessonComplete(4, { ...base, contract: 103, flown: true, apoapsisKm: lunarAltitudeKm, impacted: false }), true);

  assert.equal(lessonComplete(5, { ...base, score: 0 }), false);
  assert.equal(lessonComplete(5, { ...base, score: 0.0001 }), true);
});

test('oracle_lesson7_schema5_redesign_threshold', () => {
  const delegated = { ...base, contract: 103, schemaVersion: 5, verified: true, flown: true,
    delegatedBaselineDryKg: 5500, fittedDryKg: 5500, predictionErrorKg: 12,
    efficiency: 0.65, objectivePassed: false };
  assert.equal(lessonComplete(6, delegated), false);
  const manual = { ...delegated, fittedDryKg: 5320, predictionErrorKg: 1.99,
    efficiency: 0.8, objectivePassed: true };
  assert.equal(lessonComplete(6, manual), true);
  assert.equal(lessonComplete(6, { ...manual, predictionErrorKg: 2 }), false);
  assert.equal(lessonComplete(6, { ...manual, efficiency: 0.65 }), false);
});

test('oracle_placement_diagnostic_routing', () => {
  assert.equal(placementRoute({ apsis: false, phasing: false, massPrediction: false }), 0);
  assert.equal(placementRoute({ apsis: true, phasing: false, massPrediction: false }), 1);
  assert.equal(placementRoute({ apsis: true, phasing: true, massPrediction: false }), 6);
  assert.equal(placementRoute({ apsis: true, phasing: true, massPrediction: true }), 'ranked');
});

test('oracle_playtest_consent_privacy_and_export', () => {
  const values = new Map(); let writes = 0, clock = 100;
  const storage = { setItem(k, v) { writes += 1; values.set(k, v); }, getItem: k => values.get(k) ?? null, removeItem(k) { values.delete(k); } };
  const recorder = createPlaytestRecorder({ storage, now: () => clock });
  recorder.startLesson(7, { predictionError: 15 }); recorder.hint(); recorder.causal('PropellantDeficit'); recorder.complete({ massDelta: -180, predictionError: 1.5 });
  assert.equal(writes, 0); assert.equal(recorder.count, 0); assert.equal(recorder.exportJson(), null);
  recorder.setConsent(true); recorder.startLesson(7, { predictionError: 15 }); clock = 350;
  recorder.hint(); recorder.retry(); recorder.setPhasingDirection('Retrograde'); recorder.causal('PropellantDeficit'); recorder.complete({ massDelta: -180, predictionError: 1.5 });
  const exported = JSON.parse(recorder.exportJson());
  assert.equal(exported.observations.length, 1);
  assert.deepEqual(exported.observations[0].causal_rules_triggered, ['PropellantDeficit']);
  assert.equal(exported.observations[0].time_to_threshold_ms, 250);
  assert.equal(exported.observations[0].retry_count, 1);
  assert.equal(exported.observations[0].first_burn_direction_on_phasing, 'Retrograde');
  assert.equal(exported.observations[0].prediction_error_before_redesign_kg, 15);
  assert.equal(exported.observations[0].schema5_mass_delta_kg, -180);
  assert.equal(exported.observations[0].prediction_error_final_kg, 1.5);
  assert.ok(writes > 0);
});
