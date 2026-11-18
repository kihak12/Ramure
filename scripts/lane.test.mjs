// Tests of the branch that a lane of the graph stands for (src/lib/git/lane.ts). The rows are
// those that the core gives (src-tauri/src/git/graph.rs): for each row, the column of its commit
// and its segments toward the next row, [start column, end column, lane]. Run by `npm test`.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { laneRun, onRun, passes } from '../src/lib/git/lane.ts';

const row = (lane, edges, wip = false) => ({ lane, edges, wip });
const commitsOn = (rows, run) => rows.map((_, i) => i).filter((i) => onRun(run, rows, i));

// M merges B into A; both come from C.
//   0 M   lane 0, parents A, B: opens lane 1 toward B
//   1 B   lane 1
//   2 A   lane 0; the line of B converges on C
//   3 C   lane 0
//   4 D   lane 0, the first commit
const merged = [
  row(0, [[0, 0, 0], [0, 1, 1]]),
  row(1, [[0, 0, 0], [1, 1, 1]]),
  row(0, [[0, 0, 0], [1, 0, 1]]),
  row(0, [[0, 0, 0]]),
  row(0, []),
];

test('a branch goes from the merge that takes it in to the commit it starts from', () => {
  const run = laneRun(merged, 1, 1);
  assert.deepEqual(run, { lane: 1, top: 0, bottom: 2 });
  assert.deepEqual(commitsOn(merged, run), [1], 'only B is on it: M and C belong to the other branch');
});

test('the main line goes through its merge commit down to the first commit', () => {
  const run = laneRun(merged, 2, 0);
  assert.deepEqual(run, { lane: 0, top: 0, bottom: 3 });
  assert.deepEqual(commitsOn(merged, run), [0, 2, 3, 4]);
});

// A column freed and taken again on the same row.
//   0 A   lane 0
//   1 B   lane 1, a tip
//   2 C   lane 0: the line of B converges here, and C, a merge, opens lane 1 again toward F
//   3 F   lane 1
//   4 E   lane 0, the first commit
const reused = [
  row(0, [[0, 0, 0]]),
  row(1, [[0, 0, 0], [1, 0, 1]]),
  row(0, [[0, 0, 0], [0, 1, 1]]),
  row(1, [[0, 0, 0], [1, 0, 1]]),
  row(0, []),
];

test('a column taken again by another branch does not stand out with the first one', () => {
  const b = laneRun(reused, 1, 1);
  assert.deepEqual(b, { lane: 1, top: 1, bottom: 1 });
  assert.deepEqual(commitsOn(reused, b), [1]);
  const f = laneRun(reused, 3, 1);
  assert.deepEqual(f, { lane: 1, top: 2, bottom: 3 });
  assert.deepEqual(commitsOn(reused, f), [3]);
});

// A merge whose second parent is already awaited by a lane: one more line from its node.
//   0 D   lane 0
//   1 M   lane 1, a tip, parents A, C: C is awaited by lane 0
//   2 A   lane 1
//   3 C   lane 0, the first commit
const joined = [
  row(0, [[0, 0, 0]]),
  row(1, [[0, 0, 0], [1, 1, 1], [1, 0, 0]]),
  row(1, [[0, 0, 0], [1, 0, 1]]),
  row(0, []),
];

test('a merge that joins a lane does not cut it', () => {
  assert.deepEqual(laneRun(joined, 3, 0), { lane: 0, top: 0, bottom: 2 }, 'from C, up through the row of M');
  assert.deepEqual(laneRun(joined, 0, 0), { lane: 0, top: 0, bottom: 2 }, 'from D, the same branch');
  assert.deepEqual(commitsOn(joined, laneRun(joined, 1, 1)), [1, 2]);
});

test('the changes in progress are not part of a branch', () => {
  const rows = [row(0, [[0, 0, 0]], true), row(0, [[0, 0, 0]]), row(0, [])];
  const run = laneRun(rows, 1, 0);
  assert.equal(run.top, 1);
  assert.deepEqual(commitsOn(rows, run), [1, 2]);
});

test('a line passes through a row at its commit or along a straight segment', () => {
  assert.equal(passes(merged, 2, 0), true, 'the commit of the row');
  assert.equal(passes(merged, 1, 0), true, 'the main line passes by B');
  assert.equal(passes(merged, 3, 1), false, 'lane 1 has merged into C');
  assert.equal(passes(reused, 2, 1), false, 'the curve out of C is not a straight segment');
});
