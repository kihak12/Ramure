// Le journal d'une session de terminal : son fichier, son nom, ses lignes de tête et de reprise
// (charte, 9.15, « The journal of a session »). Le cœur Rust écrit ce qui s'affiche ; ici, on
// décide où et sous quel nom.

import { invoke } from '@tauri-apps/api/core';
import { join } from '@tauri-apps/api/path';
import type { Terminal } from '@xterm/xterm';
import { prefs } from '../git/prefs.svelte';

/** Ce que le cœur reçoit pour ouvrir un journal, ou le reprendre. */
export interface JournalOpen {
  path: string;
  header: string;
  note?: string | null;
  backlog?: string | null;
}

const two = (n: number) => String(n).padStart(2, '0');
const time = (d: Date) => `${two(d.getHours())}:${two(d.getMinutes())}`;

/** Le dossier des journaux : celui des réglages, sinon `Documents/Ramure/Journaux`. */
export async function journalDir(): Promise<string> {
  return prefs.journalDir || (await invoke<string | null>('journal_default_dir')) || '';
}

/** `serveur-test_2026-11-11_23-05-12.log` : un caractère qu'un nom de fichier ne peut pas porter
 *  devient « _ ». Les secondes séparent deux onglets du même hôte. */
export function fileName(name: string, d: Date): string {
  const safe = name.trim().replace(/[^\p{L}\p{N}._-]+/gu, '_').replace(/^[._]+/, '') || 'session';
  const stamp = `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())}_${two(d.getHours())}-${two(d.getMinutes())}-${two(d.getSeconds())}`;
  return `${safe}_${stamp}.log`;
}

/** « Journal de serveur-test, commencé le 11 nov. 2026 à 23:05 » ; `title` dit de quoi. */
export function header(title: string, d: Date): string {
  const day = d.toLocaleDateString('fr-FR', { day: 'numeric', month: 'short', year: 'numeric' });
  return `${title}, commencé le ${day} à ${time(d)}`;
}

/** La ligne qui marque la reprise d'une connexion perdue, dans le même fichier. */
export const resumed = (d = new Date()) => `— connexion perdue, reprise à ${time(d)} —`;

/** Un journal neuf : `stem` nomme le fichier, `title` commence sa première ligne. */
export async function newJournal(stem: string, title: string): Promise<JournalOpen> {
  const now = new Date();
  return { path: await join(await journalDir(), fileName(stem, now)), header: header(title, now) };
}

/** Ce que le terminal montre déjà, en texte, ses lignes repliées recollées. La ligne du curseur
 *  s'arrête au curseur, espace compris : ce qui sera tapé ensuite la continue (une invite). */
export function shown(term: Terminal): string {
  const buffer = term.buffer.active;
  const cursor = buffer.baseY + buffer.cursorY;
  const lines: string[] = [];
  for (let i = 0; i <= cursor && i < buffer.length; i++) {
    const line = buffer.getLine(i);
    if (!line) continue;
    const text = i === cursor ? line.translateToString(false, 0, buffer.cursorX) : line.translateToString(true);
    if (line.isWrapped && lines.length) lines[lines.length - 1] += text;
    else lines.push(text);
  }
  return lines.map((l, i) => (i === lines.length - 1 ? l : l.trimEnd())).join('\n');
}
