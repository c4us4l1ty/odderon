/* Test entry point: node test/run.mjs
 * No dependencies, no browser, no build. Kept deliberately boring so it runs anywhere.
 */
import { report } from './harness.mjs';
await import('./dmath.test.mjs');
await import('./physics.test.mjs');
await import('./provenance.test.mjs');
await import('./schedule.test.mjs');
await import('./golden.test.mjs');
await import('./glsl.test.mjs');
await import('./scene.test.mjs');
await import('./dom.test.mjs');
process.exit(report());
