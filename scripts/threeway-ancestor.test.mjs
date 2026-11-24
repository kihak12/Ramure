// Tests of where a conflict was in the common ancestor (src/lib/git/threeway.ts, ancestorSpans).
// Run by `npm test`.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { ancestorSpans } from '../src/lib/git/threeway.ts';

// The ancestor: ten settings. Side 1 adds two lines at the top (no conflict); both sides change `e`.
const base = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j'];
const ours = ['top 1', 'top 2', 'a', 'b', 'c', 'd', 'E1', 'f', 'g', 'h', 'i', 'j'];
const theirs = ['a', 'b', 'c', 'd', 'E2', 'f', 'g', 'h', 'i', 'j'];

test('a conflict is found in the ancestor through the diffs of both sides', () => {
  const [span] = ancestorSpans(base, ours, theirs, [{ ours: { from: 6, to: 7 }, theirs: { from: 4, to: 5 } }]);
  assert.deepEqual(span, { from: 4, to: 5 });
});

test('one side is enough when the conflict is not found in the other', () => {
  // Side 2 deleted `e`: the conflict is empty there.
  const deleted = ['a', 'b', 'c', 'd', 'f', 'g', 'h', 'i', 'j'];
  assert.deepEqual(ancestorSpans(base, ours, deleted, [{ ours: { from: 6, to: 7 }, theirs: null }]), [{ from: 4, to: 5 }]);
});

test('the conflict covers what each side changed of the ancestor', () => {
  // Side 1 changes `e` and `f`, side 2 only `e`.
  const wider = ['a', 'b', 'c', 'd', 'E1', 'F1', 'g', 'h', 'i', 'j'];
  assert.deepEqual(ancestorSpans(base, wider, theirs, [{ ours: { from: 4, to: 6 }, theirs: { from: 4, to: 5 } }]), [{ from: 4, to: 6 }]);
});

test('lines added by both sides give an empty span, at their place in the ancestor', () => {
  const addedOurs = ['a', 'b', 'c', 'new 1', 'd', 'e', 'f', 'g', 'h', 'i', 'j'];
  const addedTheirs = ['a', 'b', 'c', 'new 2', 'd', 'e', 'f', 'g', 'h', 'i', 'j'];
  assert.deepEqual(ancestorSpans(base, addedOurs, addedTheirs, [{ ours: { from: 3, to: 4 }, theirs: { from: 3, to: 4 } }]), [{ from: 3, to: 3 }]);
});

test('a conflict found in neither side has no span', () => {
  assert.deepEqual(ancestorSpans(base, ours, theirs, [{ ours: null, theirs: null }]), [null]);
});
