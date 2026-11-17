import test from 'node:test';
import assert from 'node:assert/strict';
import { LESSONS, lessonComplete } from '../../web/onboarding.js';

const base = {
  contract: 101, nodes: 0, predictionReady: false, maxPlaneChange: 0,
  engineAltitudeKm: 0, engineSeparated: true, score: NaN,
};

test('every onboarding lesson stays under the pre-action copy budget', () => {
  assert.equal(LESSONS.length, 6);
  for (const lesson of LESSONS) {
    const words = lesson.copy.trim().split(/\s+/).length;
    assert.ok(words <= 40, `${lesson.title} has ${words} words`);
    assert.equal(lesson.hints.length, 4);
  }
});

test('onboarding completion is measured from real mission state', () => {
  assert.equal(lessonComplete(0, { ...base, nodes: 1 }), false);
  assert.equal(lessonComplete(0, { ...base, nodes: 2 }), true);
  assert.equal(lessonComplete(1, { ...base, predictionReady: true }), true);
  assert.equal(lessonComplete(2, { ...base, contract: 102, maxPlaneChange: 49.9 }), false);
  assert.equal(lessonComplete(2, { ...base, contract: 102, maxPlaneChange: 50 }), true);
  assert.equal(lessonComplete(3, { ...base, contract: 103, engineAltitudeKm: 17, engineSeparated: true }), false);
  assert.equal(lessonComplete(3, { ...base, contract: 103, engineAltitudeKm: 17, engineSeparated: false }), true);
  assert.equal(lessonComplete(4, { ...base, contract: 103, nodes: 3 }), true);
  assert.equal(lessonComplete(5, { ...base, score: 0 }), true, 'a measured zero is still a score');
});
