/**
 * Unit tests for the pure timeline helpers.
 *
 * Run with Node's built-in runner (Node 22+ strips the types):
 *   node --test tests/timeline.test.ts
 */

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  MIN_RANGE,
  chooseTickStep,
  clamp,
  dragEnd,
  dragStart,
  formatTick,
  formatTimecode,
  fullRange,
  isFullRange,
  moveRange,
  normalizeRange,
  percentOf,
  rangeDuration,
  secondsOf,
} from '../src/app/lib/timeline.ts';

test('clamp keeps values inside the range', () => {
  assert.equal(clamp(5, 0, 10), 5);
  assert.equal(clamp(-3, 0, 10), 0);
  assert.equal(clamp(42, 0, 10), 10);
  assert.equal(clamp(Number.NaN, 2, 10), 2);
});

test('formatTimecode renders minutes and tenths', () => {
  assert.equal(formatTimecode(0), '0:00.0');
  assert.equal(formatTimecode(9.4), '0:09.4');
  assert.equal(formatTimecode(65.5), '1:05.5');
  assert.equal(formatTimecode(3725.25), '1:02:05.3');
  assert.equal(formatTimecode(-4), '0:00.0');
});

test('formatTick shortens labels for long clips', () => {
  assert.equal(formatTick(5, 20), '5s');
  assert.equal(formatTick(120, 300), '2:00');
  assert.equal(formatTick(600, 1800), '10m');
});

test('chooseTickStep keeps label count reasonable', () => {
  assert.equal(chooseTickStep(10), 2);
  assert.equal(chooseTickStep(120), 15);
  assert.equal(chooseTickStep(3600), 600);
  assert.ok(3600 / chooseTickStep(3600) <= 8);
});

test('percent <-> seconds round-trip', () => {
  assert.equal(percentOf(5, 10), 50);
  assert.equal(percentOf(20, 10), 100);
  assert.equal(percentOf(1, 0), 0);
  assert.equal(secondsOf(50, 10), 5);
  assert.equal(secondsOf(150, 10), 10);
});

test('normalizeRange clamps, sorts and enforces the minimum gap', () => {
  assert.deepEqual(normalizeRange({ start: -2, end: 12 }, 10), { start: 0, end: 10 });
  assert.deepEqual(normalizeRange({ start: 8, end: 3 }, 10), { start: 3, end: 8 });
  assert.deepEqual(normalizeRange({ start: 4, end: 4 }, 10), {
    start: 4,
    end: 4 + MIN_RANGE,
  });
  // Gap cannot overflow the clip: the range slides back instead.
  const tail = normalizeRange({ start: 9.99, end: 9.99 }, 10);
  assert.ok(Math.abs(tail.end - 10) < 1e-9);
  assert.ok(tail.end - tail.start >= MIN_RANGE - 1e-9);
  assert.deepEqual(normalizeRange({ start: 0, end: 5 }, 0), { start: 0, end: 0 });
});

test('dragStart and dragEnd respect the gap and the clip bounds', () => {
  const base = { start: 2, end: 6 };
  assert.deepEqual(dragStart(base, 1, 10), { start: 1, end: 6 });
  assert.deepEqual(dragStart(base, 5.99, 10), { start: 5.95, end: 6 });
  assert.deepEqual(dragStart(base, -4, 10), { start: 0, end: 6 });
  assert.deepEqual(dragEnd(base, 9, 10), { start: 2, end: 9 });
  assert.deepEqual(dragEnd(base, 1, 10), { start: 2, end: 2.05 });
  assert.deepEqual(dragEnd(base, 99, 10), { start: 2, end: 10 });
});

test('moveRange slides without leaving the clip', () => {
  assert.deepEqual(moveRange({ start: 2, end: 4 }, 1, 10), { start: 3, end: 5 });
  assert.deepEqual(moveRange({ start: 2, end: 4 }, -5, 10), { start: 0, end: 2 });
  assert.deepEqual(moveRange({ start: 8, end: 10 }, 5, 10), { start: 8, end: 10 });
});

test('full range helpers', () => {
  assert.deepEqual(fullRange(4), { start: 0, end: 4 });
  assert.equal(rangeDuration({ start: 1.5, end: 4 }), 2.5);
  assert.equal(isFullRange({ start: 0, end: 4 }, 4), true);
  assert.equal(isFullRange({ start: 0.5, end: 4 }, 4), false);
  assert.equal(isFullRange({ start: 0, end: 4 }, 0), false);
});
