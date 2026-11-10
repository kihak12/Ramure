// Lecture des marqueurs de conflit que git écrit dans un fichier :
//
//   <<<<<<< HEAD            ← début, avec le nom du premier côté
//   lignes du premier côté
//   ||||||| ancêtre         ← seulement avec merge.conflictStyle = diff3 ou zdiff3
//   lignes de l'ancêtre
//   =======
//   lignes du second côté
//   >>>>>>> autre-branche   ← fin, avec le nom du second côté

export interface ConflictBlock {
  /** Indices (à partir de 0) des lignes `<<<<<<<` et `>>>>>>>`. */
  start: number;
  end: number;
  ours: string[];
  base: string[] | null;
  theirs: string[];
  oursLabel: string;
  theirsLabel: string;
  /** Indices de la première ligne de chaque partie, pour la mise en couleur. */
  oursAt: number;
  baseAt: number;
  theirsAt: number;
}

/** Découpe un texte en lignes comme le fait l'éditeur, selon la fin de ligne du fichier. */
export function splitLines(text: string, separator: string): string[] {
  return separator === '\r\n' ? text.split('\r\n') : text.split(/\r\n|\r|\n/);
}

const marker = (line: string, sign: string) => line === sign || line.startsWith(`${sign} `);

/** Trouve les conflits encore présents dans `lines`. Un bloc mal formé est ignoré. */
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
      if (marker(line, '<<<<<<<')) break; // un nouveau début avant la fin : bloc mal formé
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
 * Cherche la suite de lignes `needle` dans `haystack` et renvoie l'indice de l'occurrence
 * la plus proche de `near`, ou -1.
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
