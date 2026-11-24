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

test('a line kept from side 1 faces its line there, under the header of the conflict', () => {
  // The conflict is decided: the result holds the line kept from side 1, `x1`.
  const left = [...lines('a'), head(0), ...lines('x1'), ...lines('z')];
  const right = [...lines('a'), head(0), ...lines('y1'), ...lines('z')];
  const result = lines('a', 'x1', 'z');
  const out = align(left, result, right);
  assert.deepEqual(out.result, [{ row: 1, height: 24 }], 'a blank of the height of the header, before x1');
  assert.deepEqual(out.left, []);
  assert.deepEqual(out.right, []);
});

/** Lines of conflict `n`: under its header in a side, or kept in the result. */
const inside = (n, ...keys) => keys.map((key) => ({ key, height: 18, part: n }));

test('a line typed under the band of a conflict to decide: the blank stays above it', () => {
  // 'z' was typed over into 'zy': it still faces 'z', and the blank stays under the band (#269).
  const left = [...lines('a'), head(0), ...inside(0, 'x1', 'x2', 'x3'), ...lines('z', 'w')];
  const right = [...lines('a'), head(0), ...inside(0, 'y1'), ...lines('z', 'w')];
  const result = [...lines('a'), head(0), ...lines('zy', 'w')];
  const out = align(left, result, right);
  assert.deepEqual(out.result, [{ row: 2, height: 54 }], 'under the band, above the typed line');
  assert.deepEqual(out.right, [{ row: 3, height: 36 }], 'before z, which faces the typed line');
  assert.deepEqual(out.left, []);
});

test('a line typed over an empty line under the band: it faces the empty line', () => {
  const left = [...lines('a'), head(0), ...inside(0, 'x1', 'x2'), ...lines('', 'w')];
  const right = [...lines('a'), head(0), ...inside(0, 'y1'), ...lines('', 'w')];
  const result = [...lines('a'), head(0), ...lines('typed', 'w')];
  const out = align(left, result, right);
  assert.deepEqual(out.result, [{ row: 2, height: 36 }]);
  assert.deepEqual(out.right, [{ row: 3, height: 18 }]);
});

test('a line typed in the lines kept from both sides: it faces its line in its side', () => {
  // Kept both, side 1 then side 2: 'y1', typed over into 'y1 typed', still faces 'y1' in side 2.
  const left = [head(0), ...inside(0, 'x1'), ...lines('z')];
  const right = [head(0), ...inside(0, 'y1'), ...lines('z')];
  const result = [head(0), ...inside(0, 'x1', 'y1 typed'), ...lines('z')];
  const out = align(left, result, right);
  assert.deepEqual(out.right, [{ row: 1, height: 18 }], 'a blank above y1, under the header');
  assert.deepEqual(out.left, [{ row: 2, height: 18 }], 'a blank under x1');
  assert.deepEqual(out.result, []);
});

test('a line typed under a decided conflict: the lines of the conflict face the lines kept', () => {
  const left = [...lines('a'), head(0), ...inside(0, 'x1', 'x2', 'x3'), ...lines('z')];
  const right = [...lines('a'), head(0), ...inside(0, 'y1'), ...lines('z')];
  const result = [...lines('a'), head(0), ...inside(0, 'x1', 'x2', 'x3'), ...lines('zy')];
  const out = align(left, result, right);
  assert.deepEqual(out.right, [{ row: 3, height: 36 }], 'y1 faces x1, the blank before z');
  assert.deepEqual(out.left, []);
  assert.deepEqual(out.result, []);
});

test('lines changed by one side, more of them: the shorter side takes its blank after', () => {
  const left = lines('a', 'p1', 'p2', 'c');
  const result = lines('a', 'q1', 'q2', 'q3', 'c');
  const out = align(left, result, result);
  assert.deepEqual(out, { left: [{ row: 3, height: 18 }], result: [], right: [] });
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
  const column = () =>
    Array.from({ length: rand(25) }, () => {
      if (rand(6) === 0) return head(rand(3));
      const part = rand(3) === 0 ? rand(3) : undefined;
      return { key: ['a', 'b', 'ab', 'abc', ''][rand(5)], height: 18, part };
    });
  for (let t = 0; t < 300; t++) {
    const [left, result, right] = [column(), column(), column()];
    const out = align(left, result, right);
    const heights = [total(left, out.left), total(result, out.result), total(right, out.right)];
    assert.equal(new Set(heights).size, 1, `heights ${heights.join(' / ')}`);
    for (const s of [...out.left, ...out.result, ...out.right]) assert.ok(s.height > 0);
  }
});
