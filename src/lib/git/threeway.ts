/**
 * Comparing the three versions of a file in conflict (guidelines, 9.26): a diff of two texts, line
 * by line (the algorithm of Myers), and from it the changes that do not conflict, which git has
 * already merged into the result.
 */
import type { Side } from './conflicts';

/** Lines `fromA` to `toA` (end excluded) of the first text give way to `fromB` to `toB` of the second. */
export interface Hunk {
  fromA: number;
  toA: number;
  fromB: number;
  toB: number;
}

/** Beyond this many edits, the middle of the texts is one hunk: the diff stays quick. */
const MAX_EDITS = 2000;

export function diffLines(a: string[], b: string[]): Hunk[] {
  // The common start and end need no search.
  let start = 0;
  while (start < a.length && start < b.length && a[start] === b[start]) start++;
  let endA = a.length;
  let endB = b.length;
  while (endA > start && endB > start && a[endA - 1] === b[endB - 1]) {
    endA--;
    endB--;
  }
  const n = endA - start;
  const m = endB - start;
  if (!n && !m) return [];
  if (!n || !m) return [{ fromA: start, toA: endA, fromB: start, toB: endB }];

  // The furthest x reached on each diagonal k = x - y, after each number of edits d. Only the
  // diagonals -d to d are kept for each d, to find the path back.
  const trace: Int32Array[] = [];
  let v = new Int32Array(3);
  let found = -1;
  for (let d = 0; d <= Math.min(n + m, MAX_EDITS) && found < 0; d++) {
    const next = new Int32Array(2 * d + 3);
    const at = (k: number) => (Math.abs(k) <= d - 1 ? v[k + d - 1 + 1] : -1);
    for (let k = -d; k <= d; k += 2) {
      let x = k === -d || (k !== d && at(k - 1) < at(k + 1)) ? at(k + 1) : at(k - 1) + 1;
      if (d === 0) x = 0;
      let y = x - k;
      while (x < n && y < m && a[start + x] === b[start + y]) {
        x++;
        y++;
      }
      next[k + d + 1] = x;
      if (x >= n && y >= m) {
        found = d;
        break;
      }
    }
    trace.push(next);
    v = next;
  }
  if (found < 0) return [{ fromA: start, toA: endA, fromB: start, toB: endB }];

  // Back from the end: the lines that stay the same, in pairs.
  const pairs: [number, number][] = [];
  let x = n;
  let y = m;
  for (let d = found; d > 0; d--) {
    const prev = trace[d - 1];
    const at = (k: number) => (Math.abs(k) <= d - 1 ? prev[k + d - 1 + 1] : -1);
    const k = x - y;
    const down = k === -d || (k !== d && at(k - 1) < at(k + 1));
    const prevK = down ? k + 1 : k - 1;
    const prevX = at(prevK);
    const prevY = prevX - prevK;
    const [moveX, moveY] = down ? [prevX, prevY + 1] : [prevX + 1, prevY];
    while (x > moveX && y > moveY) pairs.push([--x, --y]);
    x = prevX;
    y = prevY;
  }
  while (x > 0 && y > 0) pairs.push([--x, --y]);
  pairs.reverse();

  // The gaps between the pairs are the hunks.
  const hunks: Hunk[] = [];
  let lastA = -1;
  let lastB = -1;
  for (const [i, j] of [...pairs, [n, m] as [number, number]]) {
    if (i > lastA + 1 || j > lastB + 1) hunks.push({ fromA: start + lastA + 1, toA: start + i, fromB: start + lastB + 1, toB: start + j });
    lastA = i;
    lastB = j;
  }
  return hunks;
}

// --- The changes that do not conflict -------------------------------------------------------
// The lines each side changed from the common ancestor outside the conflicts, and where they are in
// the result.


export interface MergedChanges {
  /** Lines of each side, from 0, changed from the ancestor outside the conflicts. */
  ours: number[];
  theirs: number[];
  /** Lines of the result, from 0, that git merged from one side; a line changed alike on both
   *  sides has no single side and is not listed. */
  result: { line: number; side: Side }[];
}

/** A span of lines, from 0, end excluded. */
interface Span {
  from: number;
  to: number;
}

const inside = (spans: Span[], line: number) => spans.some((s) => line >= s.from && line < s.to);

/** The lines of `side` changed from `base`, outside `conflicts`. */
function changed(base: string[], side: string[], conflicts: Span[] | null): number[] {
  // A conflict not found in the side: its lines would pass for changes, so the side marks nothing.
  if (!conflicts) return [];
  const out: number[] = [];
  for (const h of diffLines(base, side)) {
    for (let line = h.fromB; line < h.toB; line++) if (!inside(conflicts, line)) out.push(line);
  }
  return out;
}

/** Where each line of `from` is in `to`, when it is there unchanged. */
function follow(from: string[], to: string[]): Map<number, number> {
  const at = new Map<number, number>();
  let i = 0;
  let j = 0;
  for (const h of [...diffLines(from, to), { fromA: from.length, toA: from.length, fromB: to.length, toB: to.length }]) {
    while (i < h.fromA && j < h.fromB) at.set(i++, j++);
    i = h.toA;
    j = h.toB;
  }
  return at;
}

/**
 * `base` null: the file has no common ancestor (added on both sides), and nothing is marked
 * outside the conflicts. `oursConflicts`, `theirsConflicts`: the lines of the conflicts in each
 * side, null when one of them was not found there; `resultConflicts`: those of the result, its
 * markers included.
 */
export function mergedChanges(
  base: string[] | null,
  ours: string[],
  theirs: string[],
  result: string[],
  oursConflicts: Span[] | null,
  theirsConflicts: Span[] | null,
  resultConflicts: Span[],
): MergedChanges {
  if (!base) return { ours: [], theirs: [], result: [] };
  const lines = { ours: changed(base, ours, oursConflicts), theirs: changed(base, theirs, theirsConflicts) };
  const origin = new Map<number, Side | null>();
  for (const side of ['ours', 'theirs'] as const) {
    const at = follow(side === 'ours' ? ours : theirs, result);
    for (const line of lines[side]) {
      const target = at.get(line);
      if (target === undefined || inside(resultConflicts, target)) continue;
      origin.set(target, origin.has(target) && origin.get(target) !== side ? null : side);
    }
  }
  const merged = [...origin].filter((e): e is [number, Side] => e[1] !== null).map(([line, side]) => ({ line, side }));
  return { ...lines, result: merged.sort((a, b) => a.line - b.line) };
}
