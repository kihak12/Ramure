// Tests of the alignment of the three columns of the conflict editor (src/lib/git/threeway.ts).
// Run by `npm test`.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { align } from '../src/lib/git/threeway.ts';

const lines = (...keys) => keys.map((key) => ({ key, height: 18 }));
const head = (n) => ({ key: `\u0000conflict ${n}`, height: 24 });

/** The height of a column with its blanks. */
const total = (rows, spacers) => rows.reduce((s, r) => s + r.height, 0) + spacers.reduce((s, b) => s + b.height, 0);

test('three same columns need no blank', () => {
  const rows = lines('a', 'b', 'c');
  assert.deepEqual(align(rows, rows, rows), { left: [], result: [], right: [] });
});

test('lines added by one side: the others get a blank of their height, right after', () => {
  const left = lines('a', 'b', 'new 1', 'new 2', 'c');
  const result = lines('a', 'b', 'new 1', 'new 2', 'c');
  const right = lines('a', 'b', 'c');
  const out = align(left, result, right);
  assert.deepEqual(out, { left: [], result: [], right: [{ row: 2, height: 36 }] });
});

test('an open conflict: the band of the result faces the headers, and takes the height of the tallest side', () => {
  // Side 1: header and 3 lines; side 2: header and 1 line; the result: one band.
  const left = [...lines('a'), head(0), ...lines('x1', 'x2', 'x3'), ...lines('z')];
  const right = [...lines('a'), head(0), ...lines('y1'), ...lines('z')];
  const result = [...lines('a'), head(0), ...lines('z')];
  const out = align(left, result, right);
  assert.deepEqual(out.left, []);
  assert.deepEqual(out.right, [{ row: 3, height: 36 }], 'two lines short, before z');
  assert.deepEqual(out.result, [{ row: 2, height: 54 }], 'three lines short, under the band');
});

test('the three columns always end with the same height', () => {
  const left = [...lines('a', 'b'), head(0), ...lines('p', 'q'), ...lines('c', 'd', 'e', 'f')];
  const right = [...lines('a'), head(0), ...lines('r'), ...lines('c', 'x', 'y', 'f', 'g')];
  const result = [...lines('a', 'b', 'p', 'r', 'c', 'x', 'y', 'e', 'f', 'g')];
  const out = align(left, result, right);
  const heights = [total(left, out.left), total(result, out.result), total(right, out.right)];
  assert.equal(new Set(heights).size, 1, heights.join(' / '));
});

test('random columns: the heights are always equal, and every blank is positive', () => {
  let seed = 11;
  const rand = (n) => ((seed = (seed * 1103515245 + 12345) % 2147483648) % n);
  const column = () => Array.from({ length: rand(25) }, () => (rand(6) === 0 ? head(rand(3)) : { key: 'abcd'[rand(4)], height: 18 }));
  for (let t = 0; t < 300; t++) {
    const [left, result, right] = [column(), column(), column()];
    const out = align(left, result, right);
    const heights = [total(left, out.left), total(result, out.result), total(right, out.right)];
    assert.equal(new Set(heights).size, 1, `heights ${heights.join(' / ')}`);
    for (const s of [...out.left, ...out.result, ...out.right]) assert.ok(s.height > 0);
  }
});
