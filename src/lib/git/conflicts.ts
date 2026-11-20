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

// --- Keeping the sides of a conflict (guidelines, 9.26) -------------------------------------
// A side of a conflict is kept whole, with the arrow of its header toward the result, or set aside
// with its cross: the arrows of the merge tools of the IDEs.

export type Side = 'ours' | 'theirs';

/** The lines of each side of a conflict. */
export interface Sides {
  ours: string[];
  theirs: string[];
}

/** What is decided of a conflict. */
export interface Pick {
  /** The sides kept, in the order they were kept: the result takes their lines in that order. */
  kept: Side[];
  /** The sides set aside. Both set aside: the conflict is decided, with no line. */
  aside: Side[];
}

export type Choice = 'ours' | 'theirs' | 'both' | 'both-reversed' | 'neither';

/** Nothing kept, nothing set aside: the conflict is still to decide. */
export const emptyPick = (): Pick => ({ kept: [], aside: [] });

/** A conflict is decided once a side is kept, or once both are set aside. */
export const isDecided = (pick: Pick) => pick.kept.length > 0 || pick.aside.length === 2;

/** What a side is in a conflict: kept, set aside, or still open. */
export function sideStatus(pick: Pick, side: Side): 'kept' | 'aside' | 'open' {
  return pick.kept.includes(side) ? 'kept' : pick.aside.includes(side) ? 'aside' : 'open';
}

/** The arrow of a side: kept whole, after a side already kept. */
export function keepSide(pick: Pick, side: Side): Pick {
  if (pick.kept.includes(side)) return pick;
  return { kept: [...pick.kept, side], aside: pick.aside.filter((s) => s !== side) };
}

/** The cross of a side: its lines go, kept or not. */
export function setAside(pick: Pick, side: Side): Pick {
  if (pick.aside.includes(side)) return pick;
  return { kept: pick.kept.filter((s) => s !== side), aside: [...pick.aside, side] };
}

/** The buttons of the bar. "Keep both" puts side 1, the one on the left (`first`), first. */
export function choose(choice: Choice, first: Side = 'ours'): Pick {
  const second: Side = first === 'ours' ? 'theirs' : 'ours';
  switch (choice) {
    case 'both':
      return { kept: [first, second], aside: [] };
    case 'both-reversed':
      return { kept: [second, first], aside: [] };
    case 'neither':
      return { kept: [], aside: [first, second] };
    default:
      return { kept: [choice], aside: [] };
  }
}

/** The lines kept, in the order of the result, with the side each comes from. */
export function resolved(block: Sides, pick: Pick): { line: string; side: Side }[] {
  return pick.kept.flatMap((side) => block[side].map((line) => ({ line, side })));
}
