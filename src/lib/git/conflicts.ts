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
