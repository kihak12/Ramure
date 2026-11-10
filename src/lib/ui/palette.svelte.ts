// Palette de commandes : une recherche unique sur les actions disponibles (Ctrl + K).
import type { IconName } from './Icon.svelte';

export interface Command {
  id: string;
  title: string;
  /** Rubrique sous laquelle la commande est rangée : « Actions », « Branches »… */
  group: string;
  icon?: IconName;
  /** Précision affichée à droite : raccourci clavier, chemin, état. */
  hint?: string;
  /** Raccourci clavier attaché à la commande, déjà mis en forme (`Ctrl + Maj + F`). */
  shortcut?: string;
  /** Mots supplémentaires pour la recherche (le nom git d'une action, par exemple). */
  keywords?: string;
  disabled?: boolean;
  run: () => void;
}

class Palette {
  commands = $state.raw<Command[] | null>(null);

  /** Ouvre la palette sur la liste de commandes du moment. */
  show(commands: Command[]) {
    this.commands = commands;
  }

  hide() {
    this.commands = null;
  }
}

export const palette = new Palette();

/** Minuscules sans accents : « Récupérer » et « recuperer » se trouvent l'un l'autre. */
export const fold = (text: string) =>
  text
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase();

/**
 * Note une commande pour une recherche ; 0 si elle ne correspond pas. Chaque mot de la
 * recherche doit apparaître : au début du titre (le mieux), au début d'un mot, ailleurs,
 * ou lettre par lettre dans l'ordre (le moins bien).
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
  // À correspondance égale, le titre le plus court est le plus précis.
  return total - Math.min(title.length, 60) / 100;
}

function subsequence(text: string, word: string) {
  let i = 0;
  for (const ch of text) if (ch === word[i]) i++;
  return i === word.length;
}
