// Before stopping local terminals: is a program running in them? Closing a tab or the window
// stops the shells, and with them a development server or a long build (guidelines, sections
// 9.15 and 9.20).

import { invoke } from '@tauri-apps/api/core';
import { dialogs } from '../ui/dialogs.svelte';
import type { Session } from './ssh.svelte';

/**
 * What runs in `sessions`. `programs`: the terminals where a program runs, other than the shell.
 * `unknown`: those whose shell runs with no way to know more (on Windows).
 */
async function running(sessions: Session[]) {
  const open = sessions.filter((s) => s.kind === 'local' && s.localId !== undefined);
  if (!open.length) return { programs: 0, unknown: 0 };
  try {
    const busy = await invoke<(boolean | null)[]>('local_busy', { ids: open.map((s) => s.localId) });
    return { programs: busy.filter((b) => b === true).length, unknown: busy.filter((b) => b === null).length };
  } catch {
    // The core does not answer: there is no way to know. As on Windows, the repository and the
    // window then ask, and the tab of a single terminal, which the user is looking at, closes
    // without a question.
    return { programs: 0, unknown: open.length };
  }
}

const LOST = 'and its output lost';

/** Closing the tab of a local terminal: ask only when a program is known to run in it. */
export async function mayCloseTerminal(session: Session): Promise<boolean> {
  const { programs } = await running([session]);
  if (!programs) return true;
  return dialogs.confirm({
    title: 'Close this terminal?',
    body: `A program is running in this terminal. It will be stopped ${LOST}.`,
    confirmLabel: 'Close terminal',
    danger: true,
  });
}

/**
 * Closing the tab of a repository stops its terminals, which are not always in view. Ask when a
 * program runs in them, and also when there is no way to know and a shell is open.
 */
export async function mayCloseRepo(name: string, sessions: Session[]): Promise<boolean> {
  const { programs, unknown } = await running(sessions);
  if (!programs && !unknown) return true;
  return dialogs.confirm({
    title: `Close repository ${name}?`,
    body: programs
      ? programs === 1
        ? `A program is running in the terminal of this repository. It will be stopped ${LOST}.`
        : `Programs are running in ${programs} terminals of this repository. They will be stopped and their output lost.`
      : unknown === 1
        ? `A terminal is open in this repository. Whatever runs in it will be stopped ${LOST}.`
        : `${unknown} terminals are open in this repository. Whatever runs in them will be stopped and its output lost.`,
    confirmLabel: 'Close repository',
    danger: true,
  });
}

/** Quitting the application stops every local terminal, those of the repositories and those of the SSH module. */
export async function mayQuit(sessions: Session[]): Promise<boolean> {
  const { programs, unknown } = await running(sessions);
  if (!programs && !unknown) return true;
  const body = programs
    ? programs === 1
      ? `A program is running in a terminal. It will be stopped ${LOST}.`
      : `Programs are running in ${programs} terminals. They will be stopped and their output lost.`
    : unknown === 1
      ? `A terminal is open. Whatever runs in it will be stopped ${LOST}.`
      : `${unknown} terminals are open. Whatever runs in them will be stopped and its output lost.`;
  return dialogs.confirm({ title: 'Quit Ramure?', body, confirmLabel: 'Quit', danger: true });
}

/** What a restart would close in one repository tab. */
export interface OpenRepo {
  name: string;
  /** A commit message is typed and not committed. */
  message: boolean;
  /** Git is working: a fetch, a commit, a rebase. */
  busy: boolean;
}

const plural = (n: number, one: string) => `${n} ${one}${n === 1 ? '' : 's'}`;

/**
 * What installing an update would lose, one line each (guidelines, 9.23): the restart closes SSH
 * sessions and their tunnels, local terminals, and what a repository tab does not keep.
 */
export async function restartLosses(sessions: Session[], tunnels: number, repos: OpenRepo[]): Promise<string[]> {
  const lines: string[] = [];
  const remote = sessions.filter((s) => s.kind === 'ssh' && s.status !== 'closed').length;
  if (remote) {
    const closed = remote === 1 ? '1 SSH session will be closed' : `${remote} SSH sessions will be closed`;
    lines.push(tunnels ? `${closed}, and ${plural(tunnels, 'tunnel')} with ${remote === 1 ? 'it' : 'them'}.` : `${closed}.`);
  }
  const { programs, unknown } = await running(sessions);
  if (programs) {
    lines.push(
      programs === 1
        ? `A program is running in a terminal. It will be stopped ${LOST}.`
        : `Programs are running in ${programs} terminals. They will be stopped and their output lost.`,
    );
  } else if (unknown) {
    lines.push(
      unknown === 1
        ? `A terminal is open. Whatever runs in it will be stopped ${LOST}.`
        : `${unknown} terminals are open. Whatever runs in them will be stopped and its output lost.`,
    );
  }
  for (const repo of repos) {
    if (repo.busy) lines.push(`Git is working in ${repo.name}. Restarting now would stop it.`);
    if (repo.message) lines.push(`The commit message in ${repo.name} will be lost.`);
  }
  return lines;
}

/** "Restart and install…": always asks, and says what the restart would lose. */
export async function mayRestart(version: string, installer: string | null, losses: string[]): Promise<boolean> {
  const system =
    installer === 'deb' ? 'Ubuntu will ask for your administrator password.' : installer === 'nsis' ? 'The installer opens, then Ramure starts again.' : '';
  return dialogs.confirm({
    title: `Restart and install ${version}?`,
    body: 'Ramure closes, installs the update and opens again. Your repository tabs come back.',
    warning: losses.join('\n') || undefined,
    note: system || undefined,
    confirmLabel: 'Restart and install',
    danger: losses.length > 0,
  });
}
