// Tests of the line diff of the conflict editor (src/lib/git/threeway.ts). Run by `npm test`.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { diffLines } from '../src/lib/git/threeway.ts';

const L = (s) => (s ? s.split(' ') : []);

/** Applies the hunks to `a`, taking the lines of `b`: it must give `b` back. */
function apply(a, b, hunks) {
  const out = [];
  let i = 0;
  for (const h of hunks) {
    out.push(...a.slice(i, h.fromA), ...b.slice(h.fromB, h.toB));
    i = h.toA;
  }
  return [...out, ...a.slice(i)];
}

test('the same text has no hunk', () => {
  assert.deepEqual(diffLines(L('a b c'), L('a b c')), []);
  assert.deepEqual(diffLines([], []), []);
});

test('an added, a removed and a changed line', () => {
  assert.deepEqual(diffLines(L('a b c'), L('a b x c')), [{ fromA: 2, toA: 2, fromB: 2, toB: 3 }]);
  assert.deepEqual(diffLines(L('a b c'), L('a c')), [{ fromA: 1, toA: 2, fromB: 1, toB: 1 }]);
  assert.deepEqual(diffLines(L('a b c'), L('a x c')), [{ fromA: 1, toA: 2, fromB: 1, toB: 2 }]);
});

test('several hunks apart, in order', () => {
  const a = L('a b c d e f g h');
  const b = L('a B c d e F G h i');
  const hunks = diffLines(a, b);
  assert.deepEqual(hunks, [
    { fromA: 1, toA: 2, fromB: 1, toB: 2 },
    { fromA: 5, toA: 7, fromB: 5, toB: 7 },
    { fromA: 8, toA: 8, fromB: 8, toB: 9 },
  ]);
  assert.deepEqual(apply(a, b, hunks), b);
});

test('an empty side, and two texts with nothing in common', () => {
  assert.deepEqual(diffLines([], L('a b')), [{ fromA: 0, toA: 0, fromB: 0, toB: 2 }]);
  assert.deepEqual(diffLines(L('a b'), []), [{ fromA: 0, toA: 2, fromB: 0, toB: 0 }]);
  assert.deepEqual(diffLines(L('a b'), L('x y z')), [{ fromA: 0, toA: 2, fromB: 0, toB: 3 }]);
});

test('repeated lines are matched in order', () => {
  const a = L('x a x b x');
  const b = L('x b x a x');
  assert.deepEqual(apply(a, b, diffLines(a, b)), b);
});

test('a long text with a few changes, and every hunk gives the other text back', () => {
  const a = Array.from({ length: 3000 }, (_, i) => `line ${i}`);
  const b = [...a];
  b[10] = 'changed';
  b.splice(1500, 2);
  b.splice(2500, 0, 'added 1', 'added 2');
  const hunks = diffLines(a, b);
  assert.equal(hunks.length, 3);
  assert.deepEqual(apply(a, b, hunks), b);
});

test('random texts: the hunks always give the other text back', () => {
  let seed = 7;
  const rand = (n) => ((seed = (seed * 1103515245 + 12345) % 2147483648) % n);
  for (let t = 0; t < 200; t++) {
    const a = Array.from({ length: rand(30) }, () => 'abcde'[rand(5)]);
    const b = Array.from({ length: rand(30) }, () => 'abcde'[rand(5)]);
    assert.deepEqual(apply(a, b, diffLines(a, b)), b, `${a.join('')} → ${b.join('')}`);
  }
});
