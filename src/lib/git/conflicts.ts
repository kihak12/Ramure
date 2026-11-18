// Reading the conflict markers that git writes in a file:
//
//   <<<<<<< HEAD            ← start, with the name of the first side
//   lines of the first side
//   ||||||| ancestor        ← only with merge.conflictStyle = diff3 or zdiff3
//   lines of the ancestor
//   =======
//   lines of the second side
//   >>>>>>> other-branch    ← end, with the name of the second side

export interface ConflictBlock {
  /** Indexes (from 0) of the `<<<<<<<` and `>>>>>>>` lines. */
  start: number;
  end: number;
  ours: string[];
  base: string[] | null;
  theirs: string[];
  oursLabel: string;
  theirsLabel: string;
  /** Indexes of the first line of each part, for the coloring. */
  oursAt: number;
  baseAt: number;
  theirsAt: number;
}

/** Splits a text into lines as the editor does, according to the line ending of the file. */
export function splitLines(text: string, separator: string): string[] {
  return separator === '\r\n' ? text.split('\r\n') : text.split(/\r\n|\r|\n/);
}

const marker = (line: string, sign: string) => line === sign || line.startsWith(`${sign} `);

/** Finds the conflicts still present in `lines`. A malformed block is ignored. */
export function parseConflicts(lines: string[]): ConflictBlock[] {
  const blocks: ConflictBlock[] = [];
  let i = 0;
  while (i < lines.length) {
    if (!marker(lines[i], '<<<<<<<')) {
      i++;
      continue;
    }
    const start = i;
    let baseAt = -1;
    let middle = -1;
    let end = -1;
    for (let j = start + 1; j < lines.length; j++) {
      const line = lines[j];
      if (marker(line, '<<<<<<<')) break; // a new start before the end: malformed block
      if (middle < 0 && baseAt < 0 && marker(line, '|||||||')) baseAt = j;
      else if (middle < 0 && line === '=======') middle = j;
      else if (middle >= 0 && marker(line, '>>>>>>>')) {
        end = j;
        break;
      }
    }
    if (middle < 0 || end < 0) {
      i = start + 1;
      continue;
    }
    const oursEnd = baseAt >= 0 ? baseAt : middle;
    blocks.push({
      start,
      end,
      ours: lines.slice(start + 1, oursEnd),
      base: baseAt >= 0 ? lines.slice(baseAt + 1, middle) : null,
      theirs: lines.slice(middle + 1, end),
      oursLabel: lines[start].slice(8),
      theirsLabel: lines[end].slice(8),
      oursAt: start + 1,
      baseAt,
      theirsAt: middle + 1,
    });
    i = end + 1;
  }
  return blocks;
}

/**
 * Looks for the sequence of lines `needle` in `haystack` and returns the index of the occurrence
 * closest to `near`, or -1.
 */
export function locate(haystack: string[], needle: string[], near: number): number {
  if (!needle.length || needle.length > haystack.length) return -1;
  let best = -1;
  for (let i = 0; i + needle.length <= haystack.length; i++) {
    if (haystack[i] !== needle[0]) continue;
    let same = true;
    for (let j = 1; j < needle.length && same; j++) same = haystack[i + j] === needle[j];
    if (same && (best < 0 || Math.abs(i - near) < Math.abs(best - near))) best = i;
  }
  return best;
}

// --- Picking lines (guidelines, 9.26) -----------------------------------------------------

export type Side = 'ours' | 'theirs';

/** The lines of each side of a conflict: what a pick is about. */
export interface Sides {
  ours: string[];
  theirs: string[];
}

/** Which lines of each side of a conflict are kept. */
export interface Pick {
  ours: boolean[];
  theirs: boolean[];
  /** "Keep both, reversed": the lines of the second side come first. */
  reversed: boolean;
  /** "Keep neither": the conflict is decided, with no line kept. */
  neither: boolean;
}

export type Choice = 'ours' | 'theirs' | 'both' | 'both-reversed' | 'neither';

/** Nothing kept yet: the conflict is still to decide. */
export const emptyPick = (block: Sides): Pick => ({
  ours: block.ours.map(() => false),
  theirs: block.theirs.map(() => false),
  reversed: false,
  neither: false,
});

/** A conflict is decided once a line is kept, or once "Keep neither" was chosen. */
export const isDecided = (pick: Pick) => pick.neither || pick.ours.some(Boolean) || pick.theirs.some(Boolean);

/** The buttons of the bar: they set the lines, which can then be picked one by one. */
export function choose(block: Sides, choice: Choice): Pick {
  const all = (lines: string[], on: boolean) => lines.map(() => on);
  return {
    ours: all(block.ours, choice === 'ours' || choice === 'both' || choice === 'both-reversed'),
    theirs: all(block.theirs, choice === 'theirs' || choice === 'both' || choice === 'both-reversed'),
    reversed: choice === 'both-reversed',
    neither: choice === 'neither',
  };
}

/** Lines `from` to `to` of one side, both included, kept or let go: a click, or Shift + click. */
export function pickLines(pick: Pick, side: Side, from: number, to: number, on: boolean): Pick {
  const [low, high] = from <= to ? [from, to] : [to, from];
  const lines = pick[side].map((kept, i) => (i >= low && i <= high ? on : kept));
  // Letting go of the last line leaves the conflict to decide again, not decided with nothing.
  return { ...pick, [side]: lines, neither: false };
}

/** Every line of one side kept, or let go: the checkbox of the header of a conflict. */
export const pickSide = (pick: Pick, side: Side, on: boolean): Pick => pickLines(pick, side, 0, pick[side].length - 1, on);

/** The state of the checkbox of a side in the header of a conflict. */
export function sideState(pick: Pick, side: Side): 'on' | 'off' | 'mixed' {
  const kept = pick[side].filter(Boolean).length;
  return kept === 0 ? 'off' : kept === pick[side].length ? 'on' : 'mixed';
}

/**
 * The lines kept, in the order of the result, with the side each comes from. The order is fixed,
 * never that of the clicks. When both sides have as many lines and no row keeps both, it goes row
 * by row: the value of each line from the side it was taken from. Otherwise, the lines of the first
 * side, then those of the second ("reversed": the second first).
 */
export function resolved(block: Sides, pick: Pick): { line: string; side: Side }[] {
  if (pick.neither) return [];
  const rows = block.ours.length === block.theirs.length && !block.ours.some((_, i) => pick.ours[i] && pick.theirs[i]);
  if (rows) {
    return block.ours.flatMap((line, i) =>
      pick.ours[i] ? [{ line, side: 'ours' as Side }] : pick.theirs[i] ? [{ line: block.theirs[i], side: 'theirs' as Side }] : [],
    );
  }
  const of = (side: Side) => block[side].flatMap((line, i) => (pick[side][i] ? [{ line, side }] : []));
  return pick.reversed ? [...of('theirs'), ...of('ours')] : [...of('ours'), ...of('theirs')];
}
