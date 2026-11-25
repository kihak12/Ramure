// Which program opens a file of the Files module (guidelines, 9.24, "Opening a file"): the
// exception of its extension or of its name, first match in the order of the settings; else the
// text editor, which the core uses for a text file only; else the program of the system.

import type { OpenWith } from '../git/prefs.svelte';

/** The extension of a name, with its dot and in lower case: "" for "Dockerfile" or ".bashrc". */
export function extensionOf(name: string): string {
  const at = name.lastIndexOf('.');
  return at > 0 ? name.slice(at).toLowerCase() : '';
}

/** An exception as it is kept: "conf" and ".conf" are the same, case does not matter. */
export function normalMatch(match: string): string {
  return match.trim().toLowerCase();
}

/** Whether `match` names this file: its whole name, or its extension with or without its dot. */
export function matches(match: string, name: string): boolean {
  const m = normalMatch(match);
  if (!m) return false;
  const lower = name.toLowerCase();
  const ext = extensionOf(name);
  return lower === m || (ext !== '' && ext === (m.startsWith('.') ? m : `.${m}`));
}

/** The program of the first exception that names this file, or null. */
export function exceptionFor(name: string, rules: readonly OpenWith[]): string | null {
  return rules.find((rule) => rule.program.trim() && matches(rule.match, name))?.program.trim() ?? null;
}

/** What "Always open … with it" keeps for this file: its extension, else its whole name. */
export function matchFor(name: string): { match: string; label: string } {
  const ext = extensionOf(name);
  return ext ? { match: ext, label: `Always open ${ext} files with it` } : { match: name, label: `Always open files named ${name} with it` };
}
