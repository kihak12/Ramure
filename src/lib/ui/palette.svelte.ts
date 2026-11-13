// Command palette: one search over the actions available (Ctrl + K).
import type { IconName } from './Icon.svelte';

export interface Command {
  id: string;
  title: string;
  /** Group the command is listed under: “Actions”, “Branches”… */
  group: string;
  icon?: IconName;
  /** Detail shown on the right: keyboard shortcut, path, state. */
  hint?: string;
  /** Keyboard shortcut bound to the command, already formatted (`Ctrl + Shift + F`). */
  shortcut?: string;
  /** Extra words for the search (other words for an action, for instance). */
  keywords?: string;
  disabled?: boolean;
  run: () => void;
  /** What the shortcut does, when it does more than the palette (the terminal of a repository). */
  key?: () => void;
}

class Palette {
  commands = $state.raw<Command[] | null>(null);

  /** Opens the palette on the list of commands of the moment. */
  show(commands: Command[]) {
    this.commands = commands;
  }

  hide() {
    this.commands = null;
  }
}

export const palette = new Palette();

/** Lower case without diacritics: a name with accents is found when typed without them. */
export const fold = (text: string) =>
  text
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase();

/**
 * Scores a command for a search; 0 if it does not match. Each word of the search must appear: at
 * the start of the title (the best), at the start of a word, elsewhere, or letter by letter in
 * order (the worst).
 */
export function score(command: Command, words: string[]): number {
  const title = fold(command.title);
  const extra = fold(`${command.keywords ?? ''} ${command.hint ?? ''}`);
  let total = 1;
  for (const word of words) {
    const at = title.indexOf(word);
    if (at === 0) total += 100;
    else if (at > 0 && /[\s/\-_.:]/.test(title[at - 1])) total += 60;
    else if (at > 0) total += 40;
    else if (extra.includes(word)) total += 25;
    else if (subsequence(title, word)) total += 8;
    else return 0;
  }
  // For an equal match, the shortest title is the most precise.
  return total - Math.min(title.length, 60) / 100;
}

function subsequence(text: string, word: string) {
  let i = 0;
  for (const ch of text) if (ch === word[i]) i++;
  return i === word.length;
}
