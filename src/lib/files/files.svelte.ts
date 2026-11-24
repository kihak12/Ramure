// State of the Files module: the panes, their connections, and the transfers between them
// (guidelines, 9.24).

import { Channel, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { tick } from 'svelte';
import { dialogs } from '../ui/dialogs.svelte';
import { toasts } from '../ui/toasts.svelte';
import { explain } from '../ssh/reasons';
import { ssh } from '../ssh/ssh.svelte';
import { addressOf, count, type Host, type SshError } from '../ssh/types';
import {
  fsMessage,
  isLost,
  joinPath,
  nameOf,
  numbered,
  parentPath,
  type Entry,
  type Listing,
  type Place,
  type Progress,
  type SftpOpened,
  type Summary,
  type Taken,
} from './types';

const STORAGE_KEY = 'ramure.files';
/** Four panes at the most (guidelines, 9.24). */
export const MAX_PANES = 4;

/** What a pane shows: this machine, or a host of the book. */
export type Location = { kind: 'local' } | { kind: 'host'; hostId: string };

/** Where the connection of a host pane stands; a pane of this machine is always `ready`. */
export type Phase =
  | { step: 'idle' }
  | { step: 'connecting' }
  | { step: 'hostKey'; fingerprint: string; algorithm: string }
  | { step: 'hostKeyChanged'; fingerprint: string; line: number }
  | { step: 'passphrase'; path: string; error: string }
  | { step: 'password'; error: string }
  | { step: 'failed'; message: string }
  | { step: 'lost' }
  | { step: 'ready' };

/** What the next connection carries: an accepted fingerprint, a typed secret. */
interface Attempt {
  accept?: string;
  remember?: boolean;
  password?: string;
  passphrase?: string;
  keyPath?: string;
  keep?: boolean;
}

export type SortBy = 'name' | 'modified' | 'size';

export interface Pane {
  key: number;
  /** Null: the pane chooses its location. */
  location: Location | null;
  phase: Phase;
  /** The SFTP connection of a host pane, while it is open. */
  remoteId: number | null;
  /** The folder shown, or asked for; empty before the first one. */
  path: string;
  entries: Entry[];
  loading: boolean;
  /** Why the folder could not be listed. */
  error: string;
  hidden: boolean;
  sort: { by: SortBy; desc: boolean };
  /** Names of the selected rows. */
  selected: string[];
  /** The row of the keyboard, and the start of a selection with Shift. */
  current: string | null;
  anchor: string | null;
  attempt: Attempt;
  /** Counts the listings asked for: an answer that comes after a newer one is dropped. */
  seq: number;
}

export type TransferState = 'counting' | 'copying' | 'done' | 'incomplete' | 'failed' | 'canceled';

export interface Transfer {
  id: number;
  /** "dist", or "3 items". */
  title: string;
  /** "This machine → staging". */
  route: string;
  /** Between two hosts: the files go through this machine. */
  relayed: boolean;
  state: TransferState;
  progress: Progress;
  summary: Summary | null;
  /** Why it failed. */
  error: string;
  /** What to run again for "Retry". */
  /** `color`: the color of a host, "0" to "7", or "" (guidelines, 9.24). */
  from: { paneKey: number; place: Place; label: string; color: string };
  to: { paneKey: number; place: Place; label: string; color: string };
  sources: string[];
  dest: string;
}

/** What is being dragged: rows of a pane. */
export interface Dragged {
  paneKey: number;
  names: string[];
}

/** What is kept of a pane between two launches: no connection, no listing. */
interface Saved {
  location: Location | null;
  path: string;
  hidden: boolean;
  sort: Pane['sort'];
}

let nextKey = 1;
let nextTransfer = 1;

function blank(location: Location | null, path = '', hidden = false, sort: Pane['sort'] = { by: 'name', desc: false }): Pane {
  return {
    key: nextKey++,
    location,
    phase: location?.kind === 'local' ? { step: 'ready' } : { step: 'idle' },
    remoteId: null,
    path,
    entries: [],
    loading: false,
    error: '',
    hidden,
    sort,
    selected: [],
    current: null,
    anchor: null,
    attempt: {},
    seq: 0,
  };
}

function normalize(e: unknown): SshError {
  if (e && typeof e === 'object' && 'kind' in e) return e as SshError;
  return { kind: 'other', message: typeof e === 'string' ? e : String(e) };
}

/** The rows as the list shows them: hidden files left out unless asked for, folders first, then
 *  the chosen column. */
export function visible(pane: Pane): Entry[] {
  const rows = pane.hidden ? pane.entries : pane.entries.filter((e) => !e.hidden);
  const by = pane.sort.by;
  const sign = pane.sort.desc ? -1 : 1;
  const names = new Intl.Collator('en-US', { numeric: true, sensitivity: 'base' });
  return [...rows].sort((a, b) => {
    if (a.dir !== b.dir) return a.dir ? -1 : 1;
    const order = by === 'size' ? a.size - b.size : by === 'modified' ? (a.modified ?? 0) - (b.modified ?? 0) : 0;
    return (order || names.compare(a.name, b.name)) * sign;
  });
}

class Files {
  panes = $state<Pane[]>([]);
  transfers = $state<Transfer[]>([]);
  /** The pane that last had the keyboard or the pointer: its name is in weight 650. */
  focused = $state(0);
  drag = $state.raw<Dragged | null>(null);
  #started = false;

  /** The panes as they were left, or "This machine" and an empty pane the first time. */
  start() {
    if (this.#started) return;
    this.#started = true;
    let saved: Saved[] = [];
    try {
      saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '[]');
    } catch {
      saved = [];
    }
    const panes = Array.isArray(saved) && saved.length
      ? saved.slice(0, MAX_PANES).map((s) => blank(s.location ?? null, s.path ?? '', !!s.hidden, s.sort ?? { by: 'name', desc: false }))
      : [blank({ kind: 'local' }), blank(null)];
    // Two panes at least: the module is made to copy from one to another (guidelines, 9.24).
    if (panes.length < 2) panes.push(blank(null));
    this.panes = panes;
    this.focused = panes[0].key;
    for (const pane of this.panes) if (pane.location?.kind === 'local') void this.list(pane, pane.path);
    void listen<{ id: number }>('sftp-closed', (event) => {
      const pane = this.panes.find((p) => p.remoteId === event.payload.id);
      if (!pane) return;
      pane.remoteId = null;
      if (pane.phase.step === 'ready') pane.phase = { step: 'lost' };
    });
  }

  /** Kept for the next launch: where each pane is, never a connection. */
  save() {
    const saved: Saved[] = this.panes.map((p) => ({ location: p.location, path: p.path, hidden: p.hidden, sort: p.sort }));
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(saved));
    } catch {
      /* not kept: the module opens as the first time */
    }
  }

  pane(key: number) {
    return this.panes.find((p) => p.key === key) ?? null;
  }

  host(pane: Pane): Host | null {
    return pane.location?.kind === 'host' ? (ssh.hosts.find((h) => h.id === (pane.location as { hostId: string }).hostId) ?? null) : null;
  }

  /** The name of a pane: "This machine", the name of its host, or "Empty pane". */
  label(pane: Pane): string {
    if (!pane.location) return 'Empty pane';
    if (pane.location.kind === 'local') return 'This machine';
    return this.host(pane)?.alias || 'Host';
  }

  /** The address of the host of a pane (guidelines, 9.21). */
  address(pane: Pane): string {
    const host = this.host(pane);
    return host ? addressOf(host.user || ssh.defaultUser, host.hostname, host.port) : '';
  }

  /** Where commands of the pane act; null while a host pane is not connected. */
  place(pane: Pane): Place | null {
    if (pane.location?.kind === 'local') return { kind: 'local' };
    if (pane.location?.kind === 'host' && pane.remoteId !== null && pane.phase.step === 'ready') return { kind: 'remote', id: pane.remoteId };
    return null;
  }

  /** Two panes on one place: this machine twice, or the same host. A drop between them moves. */
  samePlace(a: Pane, b: Pane): boolean {
    if (!a.location || !b.location) return false;
    if (a.location.kind === 'local' || b.location.kind === 'local') return a.location.kind === b.location.kind;
    return a.location.hostId === b.location.hostId;
  }

  // --- Panes ----------------------------------------------------------------------------------

  /** "Add pane": an empty pane to the right of `after`. */
  add(after?: Pane): Pane | null {
    if (this.panes.length >= MAX_PANES) return null;
    const pane = blank(null);
    const at = after ? this.panes.findIndex((p) => p.key === after.key) + 1 : this.panes.length;
    this.panes.splice(at, 0, pane);
    this.focused = pane.key;
    this.save();
    return pane;
  }

  /** The transfers under way that use the connection of a pane. */
  runningWith(pane: Pane): Transfer[] {
    return this.transfers.filter(
      (t) => (t.state === 'counting' || t.state === 'copying') && (t.from.paneKey === pane.key || t.to.paneKey === pane.key),
    );
  }

  get running(): number {
    return this.transfers.filter((t) => t.state === 'counting' || t.state === 'copying').length;
  }

  /** "Close pane": asks first when a transfer uses it. Of two panes, the one closed becomes empty. */
  async close(pane: Pane) {
    const using = this.runningWith(pane);
    if (using.length) {
      const way = using[0].to.paneKey === pane.key ? 'to' : 'from';
      const what = using.length === 1 ? `A transfer ${way} ${this.label(pane)} is running. It will be stopped` : `${using.length} transfers of ${this.label(pane)} are running. They will be stopped`;
      const confirmed = await dialogs.confirm({
        title: 'Close this pane?',
        body: `${what}; the files already copied stay.`,
        confirmLabel: 'Close pane',
        danger: true,
      });
      if (!confirmed) return;
      for (const t of using) void invoke('transfer_cancel', { id: t.id });
    }
    this.#disconnect(pane);
    // Of two panes, the one closed becomes empty and offers the locations again (9.24).
    if (this.panes.length <= 2) {
      Object.assign(pane, blank(null), { key: pane.key });
    } else {
      const at = this.panes.indexOf(pane);
      this.panes.splice(at, 1);
      if (this.focused === pane.key) this.focused = this.panes[Math.min(at, this.panes.length - 1)].key;
    }
    this.save();
  }

  #disconnect(pane: Pane) {
    if (pane.remoteId !== null) void invoke('sftp_close', { id: pane.remoteId }).catch(() => {});
    pane.remoteId = null;
  }

  /** Opens `location` in the pane, in place of what it showed. */
  choose(pane: Pane, location: Location, path = '') {
    this.#disconnect(pane);
    pane.location = location;
    pane.path = path;
    pane.entries = [];
    pane.error = '';
    pane.selected = [];
    pane.current = null;
    pane.attempt = {};
    pane.seq++;
    this.focused = pane.key;
    this.save();
    if (location.kind === 'local') {
      pane.phase = { step: 'ready' };
      void this.list(pane, path);
    } else {
      pane.phase = { step: 'idle' };
      void this.connect(pane);
    }
  }

  /** "Cancel" in a panel of the connection: the pane chooses a location again. */
  unchoose(pane: Pane) {
    this.#disconnect(pane);
    Object.assign(pane, { location: null, phase: { step: 'idle' }, entries: [], path: '', error: '', selected: [], attempt: {} });
    this.save();
  }

  /** "Browse files" on a host of the book: in an empty pane if there is one, else in a new pane,
   *  else in place of the last one (guidelines, 9.24). */
  browse(host: Host) {
    this.start();
    const pane = this.panes.find((p) => !p.location) ?? this.add(this.panes.at(-1)) ?? this.panes.at(-1);
    if (pane) this.choose(pane, { kind: 'host', hostId: host.id });
  }

  // --- Connection -----------------------------------------------------------------------------

  async connect(pane: Pane, next: Attempt = {}) {
    const host = this.host(pane);
    if (!host) {
      pane.phase = { step: 'failed', message: 'This host is no longer in the book. Choose another location.' };
      return;
    }
    const user = host.user || ssh.defaultUser;
    if (host.proxyJump) {
      pane.phase = { step: 'failed', message: 'This host goes through a bastion (ProxyJump). Connecting through a bastion is not supported yet.' };
      return;
    }
    if (!user) {
      pane.phase = { step: 'failed', message: 'This host has no user: edit the host to add one.' };
      return;
    }
    pane.attempt = { ...pane.attempt, ...next };
    const attempt = pane.attempt;
    pane.phase = { step: 'connecting' };
    this.#disconnect(pane);
    const keyPath = attempt.keyPath ?? (host.auth === 'keyFile' ? host.keyPath : '');
    const auth =
      host.auth === 'password'
        ? { kind: 'password', password: attempt.password ?? null }
        : keyPath
          ? { kind: 'keyFile', path: keyPath, passphrase: attempt.passphrase ?? null }
          : { kind: 'auto' };
    const asked = pane.seq;
    try {
      const opened = await invoke<SftpOpened>('sftp_connect', {
        params: {
          host: host.hostname,
          port: host.port || 22,
          user,
          auth,
          acceptFingerprint: attempt.accept ?? null,
          rememberHost: attempt.remember ?? false,
          rememberSecret: attempt.keep ?? false,
        },
      });
      // The pane was given another location meanwhile: this connection has no place.
      if (pane.seq !== asked || pane.location?.kind !== 'host') {
        void invoke('sftp_close', { id: opened.id }).catch(() => {});
        return;
      }
      pane.remoteId = opened.id;
      pane.phase = { step: 'ready' };
      // Secrets do not outlive the opening of the connection.
      pane.attempt = {};
      if (opened.learnFailed) toasts.warn('The fingerprint could not be remembered', opened.learnFailed);
      if (opened.keepFailed) toasts.warn('The secret could not be kept', `The system keyring refused it: ${opened.keepFailed}`);
      await this.list(pane, pane.path || opened.home, undefined, pane.path ? opened.home : undefined);
    } catch (e) {
      if (pane.seq !== asked) return;
      const problem = normalize(e);
      if (problem.kind === 'hostKeyUnknown') {
        pane.phase = { step: 'hostKey', fingerprint: problem.fingerprint, algorithm: problem.algorithm };
      } else if (problem.kind === 'hostKeyChanged') {
        pane.phase = { step: 'hostKeyChanged', fingerprint: problem.fingerprint, line: problem.line };
      } else if (problem.kind === 'needPassphrase') {
        pane.attempt.passphrase = undefined;
        pane.phase = {
          step: 'passphrase',
          path: problem.path,
          error:
            problem.refused === 'typed'
              ? 'This passphrase does not unlock the key.'
              : problem.refused === 'retained'
                ? 'The remembered passphrase no longer unlocks the key. It has been forgotten.'
                : '',
        };
      } else if (problem.kind === 'needPassword') {
        pane.attempt.password = undefined;
        pane.phase = { step: 'password', error: problem.refused ? 'The server refused the remembered password. It has been forgotten.' : '' };
      } else if (problem.kind === 'auth' && host.auth === 'password') {
        pane.attempt.password = undefined;
        pane.phase = { step: 'password', error: problem.message };
      } else {
        pane.phase = { step: 'failed', message: problem.kind === 'auth' ? problem.message : explain(problem.message) };
      }
    }
  }

  // --- Folders --------------------------------------------------------------------------------

  /**
   * Lists the folder `path` in the pane; its home folder when `path` is empty. `select`: the
   * names to select once listed. `fallback`: where to go when `path` cannot be opened (a folder
   * kept from the last launch that no longer exists). `keep`: a folder that cannot be opened
   * leaves the pane as it was, and the reason is returned (a path typed in the path bar).
   */
  async list(pane: Pane, path: string, select?: string[], fallback?: string, keep = false): Promise<string | null> {
    const place = this.place(pane);
    if (!place) return null;
    const asked = ++pane.seq;
    pane.loading = true;
    try {
      const listing = await invoke<Listing>('files_list', { place, path });
      if (pane.seq !== asked) return null;
      const changed = listing.path !== pane.path;
      pane.path = listing.path;
      pane.entries = listing.entries;
      pane.error = '';
      const names = new Set(listing.entries.map((e) => e.name));
      if (select) {
        pane.selected = select.filter((n) => names.has(n));
        pane.current = pane.selected[0] ?? null;
        pane.anchor = pane.current;
      } else if (changed) {
        pane.selected = [];
        pane.current = null;
        pane.anchor = null;
      } else {
        pane.selected = pane.selected.filter((n) => names.has(n));
        if (pane.current && !names.has(pane.current)) pane.current = null;
      }
      this.save();
      return null;
    } catch (e) {
      if (pane.seq !== asked) return null;
      if (isLost(e)) {
        pane.remoteId = null;
        pane.phase = { step: 'lost' };
      } else if (keep) {
        return fsMessage(e);
      } else if (fallback !== undefined && fallback !== path) {
        pane.loading = false;
        return this.list(pane, fallback, select);
      } else {
        pane.path = path || pane.path;
        pane.entries = [];
        pane.selected = [];
        pane.current = null;
        pane.error = fsMessage(e);
      }
      return null;
    } finally {
      if (pane.seq === asked) pane.loading = false;
    }
  }

  refresh(pane: Pane, select?: string[]) {
    return this.list(pane, pane.path, select);
  }

  up(pane: Pane) {
    const parent = parentPath(pane.path);
    if (parent !== null) void this.list(pane, parent, [nameOf(pane.path)]);
  }

  open(pane: Pane, entry: Entry) {
    if (entry.dir) void this.list(pane, joinPath(pane.path, entry.name));
  }

  /** The pane next to `pane`: on its right, or on its left for the last one (guidelines, 9.24). */
  neighbor(pane: Pane): Pane | null {
    const at = this.panes.findIndex((p) => p.key === pane.key);
    if (at < 0 || this.panes.length < 2) return null;
    return this.panes[at + 1] ?? this.panes[at - 1];
  }

  /**
   * A double-click on a file, or Enter: a copy into the folder shown by the pane next to it, as a
   * drag does, after asking about a name already there. Never a move, even between two folders of
   * the same place.
   */
  send(pane: Pane, names: string[]) {
    const target = this.neighbor(pane);
    if (!target || !names.length) return;
    const dragged = { paneKey: pane.key, names };
    // The pane next to it says what to do first: choose a place, or wait for its connection.
    const refusal = !target.location
      ? 'Choose a place in the pane next to this one first.'
      : !this.place(target)
        ? 'The pane next to this one is not connected yet.'
        : this.refusal(dragged, target, target.path);
    if (refusal) {
      toasts.warn(names.length === 1 ? `${names[0]} was not copied` : `${count(names.length, 'item')} were not copied`, refusal);
      return;
    }
    void this.drop(dragged, target, target.path, true);
  }

  /** "New folder…": its name in a dialog; a name that is taken is said there. */
  mkdir(pane: Pane) {
    return this.#create(pane, 'folder');
  }

  /** "New file…": an empty file; it never replaces one that is there (guidelines, 9.24). */
  touch(pane: Pane) {
    return this.#create(pane, 'file');
  }

  async #create(pane: Pane, what: 'folder' | 'file') {
    const place = this.place(pane);
    if (!place) return;
    let made = '';
    const answers = await dialogs.ask({
      title: what === 'folder' ? 'New folder' : 'New file',
      body: `In ${pane.path}`,
      fields: [{ name: 'name', label: 'Name', type: 'text', required: true }],
      confirmLabel: what === 'folder' ? 'Create folder' : 'Create file',
      busyLabel: 'Creating…',
      check: async (a) => {
        try {
          const command = what === 'folder' ? 'files_mkdir' : 'files_create';
          made = nameOf(await invoke<string>(command, { place, dir: pane.path, name: String(a.name) }));
          return null;
        } catch (e) {
          return fsMessage(e);
        }
      },
    });
    if (answers) await this.refresh(pane, [made]);
  }

  /** "Rename…": the name selected up to its extension, so that typing keeps the extension. */
  async rename(pane: Pane, entry: Entry) {
    const place = this.place(pane);
    if (!place) return;
    const dot = entry.dir ? -1 : entry.name.lastIndexOf('.');
    let renamed = '';
    const answers = await dialogs.ask({
      title: `Rename ${entry.name}`,
      fields: [{ name: 'name', label: 'New name', type: 'text', value: entry.name, required: true }],
      select: [0, dot > 0 ? dot : entry.name.length],
      confirmLabel: 'Rename',
      busyLabel: 'Renaming…',
      check: async (a) => {
        try {
          renamed = nameOf(await invoke<string>('files_rename', { place, path: joinPath(pane.path, entry.name), name: String(a.name) }));
          return null;
        } catch (e) {
          return fsMessage(e);
        }
      },
    });
    if (answers) await this.refresh(pane, [renamed]);
  }

  /** "Delete…": always asks, and says that nothing can be recovered (guidelines, 9.24). */
  async remove(pane: Pane, names: string[]) {
    const place = this.place(pane);
    if (!place || !names.length) return;
    const what = names.length === 1 ? names[0] : count(names.length, 'item');
    const local = place.kind === 'local';
    const confirmed = await dialogs.ask({
      title: `Delete ${what} from ${local ? 'this machine' : this.label(pane)}?`,
      body: local
        ? `${names.length === 1 ? 'It is' : 'They are'} deleted for good, with what the folders hold: ${names.length === 1 ? 'it does' : 'they do'} not go to the trash and cannot be recovered.`
        : `${names.length === 1 ? 'It is' : 'They are'} deleted from the server for good, with what the folders hold: ${names.length === 1 ? 'it' : 'they'} cannot be recovered.`,
      confirmLabel: 'Delete',
      busyLabel: 'Deleting…',
      danger: true,
      check: async () => {
        try {
          await invoke('files_remove', { place, paths: names.map((n) => joinPath(pane.path, n)) });
          return null;
        } catch (e) {
          await this.refresh(pane);
          return fsMessage(e);
        }
      },
    });
    if (confirmed) await this.refresh(pane);
  }

  async copyPath(pane: Pane, entry: Entry) {
    const path = joinPath(pane.path, entry.name);
    try {
      await navigator.clipboard.writeText(path);
      toasts.ok('Path copied', path);
    } catch (e) {
      toasts.err('The path could not be copied', fsMessage(e));
    }
  }

  // --- Transfers ------------------------------------------------------------------------------

  /** Why a drop of `dragged` into `dest` of `target` cannot be made; empty when it can. */
  refusal(dragged: Dragged, target: Pane, dest: string): string {
    const source = this.pane(dragged.paneKey);
    if (!source || !this.place(source)) return 'Not connected.';
    if (!this.place(target)) return 'Not connected.';
    if (this.samePlace(source, target)) {
      if (dest === source.path) return 'Already in this folder.';
      for (const name of dragged.names) {
        const path = joinPath(source.path, name);
        if (dest === path || dest.startsWith(`${path}/`) || dest.startsWith(`${path}\\`)) return 'A folder cannot go into itself.';
      }
    }
    return '';
  }

  /** What a drop would do: "Copy 3 items here", "Move 2 items into releases". */
  verb(dragged: Dragged, target: Pane, into: string | null, copy: boolean): string {
    const source = this.pane(dragged.paneKey);
    const move = !!source && this.samePlace(source, target) && !copy;
    const what = dragged.names.length === 1 ? dragged.names[0] : count(dragged.names.length, 'item');
    return `${move ? 'Move' : 'Copy'} ${what} ${into ? `into ${into}` : 'here'}`;
  }

  /** A drop, or "Copy to…" and "Move to…" of the menu of a row. */
  async drop(dragged: Dragged, target: Pane, dest: string, copy = false) {
    const source = this.pane(dragged.paneKey);
    const from = source && this.place(source);
    const to = this.place(target);
    if (!source || !from || !to || this.refusal(dragged, target, dest)) return;
    if (this.samePlace(source, target) && !copy) return this.#move(source, target, from, dragged.names, dest);
    return this.copy(source, target, dragged.names, dest);
  }

  /** On one place, a drop moves: a rename. What the folder has already stays where it is. */
  async #move(source: Pane, target: Pane, place: Place, names: string[], dest: string) {
    try {
      const moved = await invoke<{ moved: number; taken: string[] }>('files_move', {
        place,
        sources: names.map((n) => joinPath(source.path, n)),
        dest,
      });
      if (moved.taken.length) {
        toasts.warn(
          `${count(moved.taken.length, 'item')} ${moved.taken.length === 1 ? 'was' : 'were'} not moved`,
          `${moved.taken.join(', ')} already ${moved.taken.length === 1 ? 'exists' : 'exist'} in ${dest}.`,
        );
      }
    } catch (e) {
      toasts.err('The items could not be moved', fsMessage(e));
    }
    await this.refresh(source);
    for (const pane of this.panes) {
      if (pane !== source && this.samePlace(pane, source) && (pane.path === dest || pane.path === source.path)) {
        await this.refresh(pane, pane.path === dest ? names : undefined);
      }
    }
  }

  /** Copies between two places, after asking about the names already there. */
  async copy(source: Pane, target: Pane, names: string[], dest: string) {
    const from = this.place(source);
    const to = this.place(target);
    if (!from || !to) return;
    let mode: Taken = 'skip';
    try {
      const taken = await invoke<string[]>('files_existing', { place: to, dir: dest, names });
      if (taken.length) {
        // What the buttons answered; set from their callbacks, which TypeScript does not follow.
        const chosen: { answer: Taken | null } = { answer: null };
        const shown = taken.length > 6 ? `${taken.slice(0, 6).join(', ')} and ${taken.length - 6} more` : taken.join(', ');
        // The free name of the first one, as an example, among the names the destination pane
        // shows when it shows that folder: the core finds the real ones as it copies.
        const isDir = source.entries.find((e) => e.name === taken[0])?.dir ?? false;
        const visible = target.path === dest;
        const there = new Set(visible ? target.entries.map((e) => e.name) : [...taken]);
        for (const name of names) there.add(name);
        let n = 1;
        while (there.has(numbered(taken[0], isDir, n))) n++;
        const example = numbered(taken[0], isDir, n);
        // The name is promised only when the pane shows the folder it goes into.
        const both =
          taken.length === 1 && visible
            ? `“Keep both” copies it as ${example}.`
            : `“Keep both” copies ${taken.length === 1 ? 'it' : 'them'} under a free name, such as ${example}.`;
        const confirmed = await dialogs.confirm({
          title: `Replace ${taken.length === 1 ? taken[0] : count(taken.length, 'item')} in ${nameOf(dest)}?`,
          body: `${taken.length === 1 ? 'This name already exists' : `These names already exist`} there: ${shown}. A replaced file cannot be recovered. A folder is merged: only the files of the same name inside it are replaced. ${both}`,
          confirmLabel: 'Replace',
          danger: true,
          extra: [
            { label: taken.length === 1 ? 'Skip it' : 'Skip them', run: () => (chosen.answer = 'skip') },
            { label: 'Keep both', run: () => (chosen.answer = 'keepBoth') },
          ],
        });
        if (confirmed) chosen.answer = 'replace';
        if (!chosen.answer) return;
        mode = chosen.answer;
        // Everything is already there and skipped: nothing to do.
        if (mode === 'skip' && taken.length === names.length) return;
      }
    } catch (e) {
      toasts.err('The copy could not start', fsMessage(e));
      return;
    }
    const sourceLabel = this.label(source);
    const targetLabel = this.label(target);
    const transfer: Transfer = {
      id: nextTransfer++,
      title: names.length === 1 ? names[0] : count(names.length, 'item'),
      route: `${sourceLabel} → ${targetLabel}`,
      relayed: from.kind === 'remote' && to.kind === 'remote',
      state: 'counting',
      progress: { counting: true, filesTotal: 0, filesDone: 0, bytesTotal: 0, bytesDone: 0, current: '' },
      summary: null,
      error: '',
      from: { paneKey: source.key, place: from, label: sourceLabel, color: this.host(source)?.color ?? '' },
      to: { paneKey: target.key, place: to, label: targetLabel, color: this.host(target)?.color ?? '' },
      sources: names.map((n) => joinPath(source.path, n)),
      dest,
    };
    this.transfers.push(transfer);
    await this.#run(this.transfers.at(-1)!, mode);
  }

  async #run(t: Transfer, taken: Taken) {
    const progress = new Channel<Progress>();
    progress.onmessage = (p) => {
      t.progress = p;
      if (t.state === 'counting' && !p.counting) t.state = 'copying';
    };
    try {
      const summary = await invoke<Summary>('transfer_start', {
        params: { id: t.id, from: t.from.place, to: t.to.place, sources: t.sources, dest: t.dest, taken },
        onProgress: progress,
      });
      t.summary = summary;
      t.state = summary.failed.length ? 'incomplete' : 'done';
    } catch (e) {
      const error = e as { kind?: string };
      if (error?.kind === 'canceled') {
        t.state = 'canceled';
      } else {
        t.state = 'failed';
        t.error = isLost(e) ? `The connection to ${this.#lostOf(t)} was lost. Connect again, then retry.` : fsMessage(e);
      }
    }
    // The folder it went to is listed again, and what was copied becomes its selection, under the
    // new names of what was kept beside a name already there.
    const target = this.pane(t.to.paneKey);
    if (target && target.path === t.dest && this.place(target)) {
      const renamed = new Map(t.summary?.renamed ?? []);
      await this.refresh(target, t.sources.map((s) => renamed.get(nameOf(s)) ?? nameOf(s)));
    }
  }

  /** The host whose connection a transfer lost: the remote side, or between two hosts the one
   *  whose pane says so. */
  #lostOf(t: Transfer): string {
    const sides = [t.from, t.to].filter((side) => side.place.kind === 'remote');
    if (sides.length === 1) return sides[0].label;
    const lost = sides.find((side) => this.pane(side.paneKey)?.phase.step !== 'ready');
    return lost?.label ?? 'a host';
  }

  cancel(t: Transfer) {
    void invoke('transfer_cancel', { id: t.id });
  }

  /** "Retry": the same copy, from the panes as they are now; what is already there is asked about. */
  async retry(t: Transfer) {
    const source = this.pane(t.from.paneKey);
    const target = this.pane(t.to.paneKey);
    this.dismiss(t);
    if (!source || !target || !this.place(source) || !this.place(target)) {
      toasts.err('The copy could not start', 'Connect both panes first.');
      return;
    }
    const dir = parentPath(t.sources[0]) ?? source.path;
    const names = t.sources.map((s) => nameOf(s));
    if (dir !== source.path) await this.list(source, dir, names);
    await this.copy(source, target, names, t.dest);
  }

  dismiss(t: Transfer) {
    this.transfers = this.transfers.filter((other) => other.id !== t.id);
  }

  clearFinished() {
    this.transfers = this.transfers.filter((t) => t.state === 'counting' || t.state === 'copying');
  }

  /** Gives the keyboard to the list of a pane. */
  async focus(pane: Pane) {
    this.focused = pane.key;
    await tick();
    document.querySelector<HTMLElement>(`[data-pane="${pane.key}"] .rows`)?.focus();
  }
}

export const files = new Files();
