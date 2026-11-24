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

// --- The conflict in the common ancestor ---------------------------------------------------
// Where the lines of a conflict were in the ancestor, found from the diffs of the ancestor against
// each side: git writes the ancestor between the markers only with `merge.conflictStyle diff3`.

/** Where lines `span` of a side were in the ancestor: an unchanged line on its line, a changed one
 *  on the whole hunk it is in. A line added by the side gives an empty span, at its place. */
function toBase(hunks: Hunk[], span: Span): Span {
  const at = (line: number, end: boolean) => {
    let shift = 0;
    for (const h of hunks) {
      if (line < h.fromB) break;
      if (line < h.toB) return end ? h.toA : h.fromA;
      shift = h.toA - h.toB;
    }
    return line + shift + (end ? 1 : 0);
  };
  return { from: at(span.from, false), to: at(span.to - 1, true) };
}

/**
 * The lines of the ancestor where each conflict was, end excluded, from its lines in each side
 * (null where it was not found in that side). Both sides give a span: the conflict covers the two.
 * Null when it was found in neither side; an empty span when the ancestor had no line there, both
 * sides having added theirs.
 */
export function ancestorSpans(base: string[], ours: string[], theirs: string[], conflicts: { ours: Span | null; theirs: Span | null }[]): (Span | null)[] {
  const hunks = { ours: diffLines(base, ours), theirs: diffLines(base, theirs) };
  return conflicts.map((c) => {
    const spans = (['ours', 'theirs'] as const).flatMap((side) => {
      const span = c[side];
      return span && span.to > span.from ? [toBase(hunks[side], span)] : [];
    });
    if (!spans.length) return null;
    return { from: Math.min(...spans.map((s) => s.from)), to: Math.max(...spans.map((s) => s.to)) };
  });
}

// --- Aligning the three columns -----------------------------------------------------------------
// The two sides and the result face one another, row by row (guidelines, 9.26). A row is a line of
// text, or the header of a conflict in a side and its band in the result, which share a key. Where
// a column has fewer rows for a change, a blank of the missing height fills it: the three columns
// keep the same height and scroll as one.

/** The key of the header of conflict `i` in a side, and of its band in the result. */
const CONFLICT = '\u0000conflict ';
export const conflictKey = (i: number) => `${CONFLICT}${i}`;

/** A row of a column: its key, matched across the columns, and its height in pixels. */
export interface Row {
  key: string;
  height: number;
  /** The conflict whose lines it is: in a side, the lines under its header; in the result, the
   *  lines of a decided conflict. None for the lines outside the conflicts, and the headers. */
  part?: number;
}

/** A blank of `height` pixels before row `row` of a column (`row` = its number of rows: at the end). */
export interface Spacer {
  row: number;
  height: number;
}

/**
 * The pairs of indexes of `a` and `b` that face one another, in order: the rows that stay the
 * same; and, in what changed, inside one conflict or outside the conflicts, the rows alike, then
 * the others in the same place. A line typed over faces the line it was, and the blanks stay in
 * their place.
 */
function matches(a: Row[], b: Row[]): [number, number][] {
  const keys = (rows: Row[]) => rows.map((r) => r.key);
  const pairs: [number, number][] = [];
  let i = 0;
  let j = 0;
  for (const h of [...diffLines(keys(a), keys(b)), { fromA: a.length, toA: a.length, fromB: b.length, toB: b.length }]) {
    while (i < h.fromA && j < h.fromB) pairs.push([i++, j++]);
    const ra = runs(a, h.fromA, h.toA);
    const rb = runs(b, h.fromB, h.toB);
    let p = 0;
    let q = 0;
    while (p < ra.length && q < rb.length) {
      const x = ra[p];
      const y = rb[q];
      if (x.part === y.part) {
        pairs.push(...alike(a, x.rows, b, y.rows));
        p++;
        q++;
      } else if (y.part === undefined || (x.part !== undefined && x.part < y.part)) {
        // The lines of a conflict that the other column does not show here face nothing.
        p++;
      } else {
        q++;
      }
    }
    i = h.toA;
    j = h.toB;
  }
  return pairs;
}

/**
 * What two lines share as one typed over: the characters alike at their start and at their end,
 * spaces around left out; 0 when that is less than half of the shorter, or nothing.
 */
function shared(a: string, b: string): number {
  const x = a.trim();
  const y = b.trim();
  let start = 0;
  while (start < x.length && start < y.length && x[start] === y[start]) start++;
  let end = 0;
  while (end < x.length - start && end < y.length - start && x[x.length - 1 - end] === y[y.length - 1 - end]) end++;
  const kept = start + end;
  return kept * 2 >= Math.min(x.length, y.length) ? kept : 0;
}

/**
 * The rows `xs` of `a` and `ys` of `b` that face one another: the rows alike, in order, those
 * sharing the most; the others in between, in the same place from the first. Past 200 rows on a
 * side, in the same place only.
 */
function alike(a: Row[], xs: number[], b: Row[], ys: number[]): [number, number][] {
  const n = xs.length;
  const m = ys.length;
  const pairs: [number, number][] = [];
  /** The rows left between the last pair and `i`, `j`, in the same place. */
  let gi = 0;
  let gj = 0;
  const between = (i: number, j: number) => {
    for (let k = 0; gi + k < i && gj + k < j; k++) pairs.push([xs[gi + k], ys[gj + k]]);
  };
  if (n <= 200 && m <= 200) {
    const score = new Int32Array(n * m);
    for (let i = 0; i < n; i++) for (let j = 0; j < m; j++) score[i * m + j] = shared(a[xs[i]].key, b[ys[j]].key);
    // best[i][j]: the most characters shared by rows alike, in order, from row i of xs and j of ys.
    const best = Array.from({ length: n + 1 }, () => new Int32Array(m + 1));
    for (let i = n - 1; i >= 0; i--) {
      for (let j = m - 1; j >= 0; j--) {
        const s = score[i * m + j];
        best[i][j] = Math.max(best[i + 1][j], best[i][j + 1], s ? s + best[i + 1][j + 1] : 0);
      }
    }
    let i = 0;
    let j = 0;
    while (i < n && j < m) {
      const s = score[i * m + j];
      if (s && best[i][j] === s + best[i + 1][j + 1]) {
        between(i, j);
        pairs.push([xs[i], ys[j]]);
        gi = ++i;
        gj = ++j;
      } else if (best[i + 1][j] >= best[i][j + 1]) i++;
      else j++;
    }
  }
  between(n, m);
  return pairs;
}

/** The rows from `from` to `to` (excluded), in runs of one part. A header or a band faces only itself. */
function runs(rows: Row[], from: number, to: number): { part: number | undefined; rows: number[] }[] {
  const out: { part: number | undefined; rows: number[] }[] = [];
  let open = false;
  for (let k = from; k < to; k++) {
    const row = rows[k];
    if (row.key.startsWith(CONFLICT)) {
      open = false;
      continue;
    }
    const last = out.at(-1);
    if (open && last && last.part === row.part) last.rows.push(k);
    else out.push({ part: row.part, rows: [k] });
    open = true;
  }
  return out;
}

/**
 * The blanks of each column. Each row of the result matched in a side is an anchor: the rows it
 * matches start at the same height, the lower setting the others, which take a blank before it. A
 * row matched in one side only lines up with it, the third column going on freely; the ends line
 * up the three, which keep the same height.
 */
export function align(left: Row[], result: Row[], right: Row[]): { left: Spacer[]; result: Spacer[]; right: Spacer[] } {
  const inLeft = new Map(matches(left, result).map(([l, r]) => [r, l]));
  const inRight = new Map(matches(right, result).map(([s, r]) => [r, s]));
  const columns = [left, result, right];
  const next = [0, 0, 0]; // the next row of each column
  const top = [0, 0, 0]; // where it starts, blanks included
  const out: Spacer[][] = [[], [], []];
  /** Lines up row `row` of each column `c` given. */
  const sync = (anchors: [number, number][]) => {
    for (const [c, row] of anchors) {
      while (next[c] < row) top[c] += columns[c][next[c]++].height;
    }
    const lowest = Math.max(...anchors.map(([c]) => top[c]));
    for (const [c, row] of anchors) {
      if (top[c] >= lowest) continue;
      const last = out[c].at(-1);
      if (last && last.row === row) last.height += lowest - top[c];
      else out[c].push({ row, height: lowest - top[c] });
      top[c] = lowest;
    }
  };
  for (let r = 0; r < result.length; r++) {
    const anchors: [number, number][] = [[1, r]];
    const l = inLeft.get(r);
    const s = inRight.get(r);
    if (l !== undefined) anchors.push([0, l]);
    if (s !== undefined) anchors.push([2, s]);
    if (anchors.length > 1) sync(anchors);
  }
  sync([
    [0, left.length],
    [1, result.length],
    [2, right.length],
  ]);
  return { left: out[0], result: out[1], right: out[2] };
}
