// The log of a terminal session: its file, its name, its first line and the line that marks a
// resumed connection (guidelines, 9.15, "The log of a session"). The Rust core writes what is
// shown; here, we decide where and under what name.

import { invoke } from '@tauri-apps/api/core';
import { join } from '@tauri-apps/api/path';
import type { Terminal } from '@xterm/xterm';
import { prefs } from '../git/prefs.svelte';

/** What the core receives to open a log, or to resume one. */
export interface JournalOpen {
  path: string;
  header: string;
  note?: string | null;
  backlog?: string | null;
}

const two = (n: number) => String(n).padStart(2, '0');
/** `11:05 PM`, the American time (guidelines, section 12), built by hand: `Intl` puts a narrow
 *  no-break space before AM and PM, which the interface does not write. */
const time = (d: Date) => `${d.getHours() % 12 || 12}:${two(d.getMinutes())} ${d.getHours() < 12 ? 'AM' : 'PM'}`;

/** The log folder: the one chosen in the settings, otherwise the default of the Rust core,
 *  `Documents/Ramure/Journaux`. */
export async function journalDir(): Promise<string> {
  return prefs.journalDir || (await invoke<string | null>('journal_default_dir')) || '';
}

/** `test-server_2026-11-11_23-05-12.log`: a character that a file name cannot hold becomes
 *  "_". The seconds tell apart two tabs of the same host. */
export function fileName(name: string, d: Date): string {
  const safe = name.trim().replace(/[^\p{L}\p{N}._-]+/gu, '_').replace(/^[._]+/, '') || 'session';
  const stamp = `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())}_${two(d.getHours())}-${two(d.getMinutes())}-${two(d.getSeconds())}`;
  return `${safe}_${stamp}.log`;
}

/** "Log of test-server, started Nov 11, 2026 at 11:05 PM"; `title` says of what. */
export function header(title: string, d: Date): string {
  const day = d.toLocaleDateString('en-US', { day: 'numeric', month: 'short', year: 'numeric' });
  return `${title}, started ${day} at ${time(d)}`;
}

/** The line that marks a lost connection coming back, in the same file. */
export const resumed = (d = new Date()) => `— connection lost, resumed at ${time(d)} —`;

/** A new log: `stem` names the file, `title` starts its first line. */
export async function newJournal(stem: string, title: string): Promise<JournalOpen> {
  const now = new Date();
  return { path: await join(await journalDir(), fileName(stem, now)), header: header(title, now) };
}

/** What the terminal already shows, as text, its wrapped lines joined again. The line of the
 *  cursor stops at the cursor, space included: what is typed next continues it (a prompt). */
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
