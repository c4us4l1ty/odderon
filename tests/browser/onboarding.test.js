import test from 'node:test';
import assert from 'node:assert/strict';
import { LESSONS, lessonComplete } from '../../web/onboarding.js';

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
  assert.equal(LESSONS.length, 6);
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
