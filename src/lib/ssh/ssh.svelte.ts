// State of the SSH module: host book, keys, and open terminals, remote or local.

import { invoke } from '@tauri-apps/api/core';
import { tick } from 'svelte';
import { dialogs } from '../ui/dialogs.svelte';
import { toasts } from '../ui/toasts.svelte';
import { secretsHeld } from './secrets';
import { addressOf, type Host, type KeyInfo } from './types';

export interface Session {
  /** Id of the tab (not the same as that of the connection on the Rust side). */
  key: number;
  /** `ssh`: a session on a host of the book. `local`: the shell of this machine. */
  kind: 'ssh' | 'local';
  /** The host of an SSH session; for a local terminal, an empty host that is not used. */
  host: Host;
  /** Starting folder of a local terminal; the home folder if it is empty. */
  cwd: string;
  title: string;
  status: 'connecting' | 'reconnecting' | 'connected' | 'closed';
  /** The search bar of the terminal is open. */
  searching: boolean;
  /** Counts the requests to open it: each one gives the focus back to the field, even with the bar open. */
  searchAsk: number;
  /** The log of the tab: its file, whether it records, whether it was opened already, and whether
   *  it can resume (after a lost connection, not after a normal end: one session, one file).
   *  It stays after it stops: the header says "log saved" and where it is. */
  journal?: { path: string; header: string; live: boolean; opened: boolean; resumable: boolean } | null;
  /** The log was stopped by hand: a reconnection does not resume it, even for a host that
   *  records every session. */
  journalStopped?: boolean;
  /** Counts the requests to toggle the log that come from the palette or a shortcut. */
  journalAsk?: number;
  /** What the core says of an open local terminal: its shell and its starting folder. */
  opened?: { shell: string; cwd: string };
  /** The id of the local terminal on the core side, while its shell runs. */
  localId?: number;
}

/** The state of a session in words: the dot of a tab or of a header never says it by its color alone. */
export const STATUS: Record<Session['status'], string> = {
  connecting: 'Connecting',
  reconnecting: 'Reconnecting',
  connected: 'Connected',
  closed: 'Session ended',
};

/** The host of a local terminal: it has none, this one only fills the field. */
const LOCAL_HOST: Host = {
  id: '',
  alias: '',
  hostname: '',
  port: 0,
  user: '',
  auth: 'auto',
  keyPath: '',
  group: '',
  tags: [],
  color: '',
  startupCommand: '',
  proxyJump: false,
  logSessions: false,
  updatedAt: 0,
};

/** Ids of the terminal tabs, shared by the SSH module and the terminals of the repositories. */
let nextKey = 1;

/**
 * A local terminal that starts in `cwd` (the home folder if it is empty), titled "Terminal",
 * then "Terminal 2"… the smallest free number among `siblings`.
 */
export function localSession(cwd: string, siblings: Session[]): Session {
  const taken = new Set(siblings.filter((s) => s.kind === 'local').map((s) => s.title));
  let n = 1;
  while (taken.has(n === 1 ? 'Terminal' : `Terminal ${n}`)) n++;
  return {
    key: nextKey++,
    kind: 'local',
    cwd,
    host: LOCAL_HOST,
    title: n === 1 ? 'Terminal' : `Terminal ${n}`,
    status: 'connecting',
    searching: false,
    searchAsk: 0,
  };
}

const message = (e: unknown) => (typeof e === 'string' ? e : e instanceof Error ? e.message : String(e));

class Ssh {
  hosts = $state.raw<Host[]>([]);
  keys = $state.raw<KeyInfo[]>([]);
  /** User of the machine, offered when a host does not give one. */
  defaultUser = $state('');
  loaded = $state(false);

  sessions = $state<Session[]>([]);
  activeKey = $state(0);

  /** Host being edited; the form opens when there is one. */
  editing = $state.raw<Host | null>(null);
  showKeys = $state(false);
  /** The dialog that exports to ~/.ssh/config: for the whole book, for one host (its id), or closed. */
  exporting = $state<'all' | string | null>(null);

  active = $derived(this.sessions.find((s) => s.key === this.activeKey) ?? null);
  groups = $derived([...new Set(this.hosts.map((h) => h.group).filter(Boolean))]);

  async load() {
    try {
      this.hosts = await invoke<Host[]>('hosts_list');
    } catch (e) {
      toasts.err('The hosts could not be loaded', message(e));
    }
    try {
      this.defaultUser = (await invoke<{ user: string }>('ssh_defaults')).user;
    } catch {
      /* the user will be typed in the form */
    }
    await this.loadKeys();
    this.loaded = true;
  }

  async loadKeys() {
    try {
      this.keys = await invoke<KeyInfo[]>('ssh_keys_list');
    } catch {
      this.keys = [];
    }
  }

  /** The saved host, or what kept it from being saved: the form says it itself, since a
   *  notification would stay under its veil (guidelines, 9.11). */
  async save(host: Host): Promise<Host | string> {
    try {
      const saved = await invoke<Host>('host_save', { host });
      this.hosts = await invoke<Host[]>('hosts_list');
      return saved;
    } catch (e) {
      return `The host could not be saved: ${message(e)}`;
    }
  }

  /** True if the host was deleted. `failed`: who to tell the failure to, instead of a notification. */
  async remove(host: Host, failed?: (text: string) => void): Promise<boolean> {
    try {
      await invoke('host_delete', { id: host.id });
      this.hosts = this.hosts.filter((h) => h.id !== host.id);
      toasts.ok('Host deleted', host.alias);
      return true;
    } catch (e) {
      if (failed) failed(`The host could not be deleted: ${message(e)}`);
      else toasts.err('The host could not be deleted', message(e));
      return false;
    }
  }

  /** The account of a host on its server: a kept password belongs to it. */
  #account(host: Host) {
    return `${host.user || this.defaultUser}@${host.hostname}:${host.port || 22}`;
  }

  /**
   * Asks for confirmation, then deletes the host: from the right-click on its row, or from the
   * form itself. True if it was deleted.
   */
  async removeAsked(host: Host, failed?: (text: string) => void): Promise<boolean> {
    // The password kept for this account goes with its last host: the confirmation says so.
    const last = !this.hosts.some((other) => other.id !== host.id && this.#account(other) === this.#account(host));
    const [kept] =
      host.auth === 'password' && last
        ? await secretsHeld([{ kind: 'password', user: host.user || this.defaultUser, host: host.hostname, port: host.port || 22 }])
        : [false];
    const confirmed = await dialogs.confirm({
      title: `Delete ${host.alias}?`,
      body: `${
        kept
          ? 'The host is deleted, and the password the system keyring keeps for this account is forgotten: neither can be recovered.'
          : 'The host is deleted and cannot be recovered.'
      } The server, its keys and ~/.ssh/config are left untouched.`,
      confirmLabel: 'Delete',
      danger: true,
    });
    return confirmed && (await this.remove(host, failed));
  }

  /**
   * After a deletion, the row of the host is gone, and with it what had the focus: it goes to the
   * filter of the book, which is always there, even with no hosts (guidelines, section 9.21). To be
   * called once the dialogs are closed: as it closes, a dialog gives the focus back to what had it
   * before.
   */
  async focusBook() {
    await tick();
    document.querySelector<HTMLElement>('.book input[type="search"]')?.focus();
  }

  async importConfig() {
    try {
      const report = await invoke<{ added: number; skipped: number }>('hosts_import');
      this.hosts = await invoke<Host[]>('hosts_list');
      const added = report.added ? `${report.added} host${report.added === 1 ? '' : 's'} imported` : 'No new hosts';
      toasts.ok(added, report.skipped ? `${report.skipped} already listed.` : '');
    } catch (e) {
      toasts.err('~/.ssh/config could not be imported', message(e));
    }
  }

  /** Opens a terminal on `host` in a new tab. */
  open(host: Host) {
    const user = host.user || this.defaultUser;
    const session: Session = {
      key: nextKey++,
      kind: 'ssh',
      cwd: '',
      host: { ...host, user },
      title: host.alias || addressOf(user, host.hostname),
      status: 'connecting',
      searching: false,
      searchAsk: 0,
    };
    this.sessions.push(session);
    this.activeKey = session.key;
  }

  /** Opens a local terminal in a new tab: "Terminal", then "Terminal 2"… the smallest free number. */
  openLocal(cwd = '') {
    const session = localSession(cwd, this.sessions);
    this.sessions.push(session);
    this.activeKey = session.key;
  }

  close(key: number) {
    const at = this.sessions.findIndex((s) => s.key === key);
    if (at < 0) return;
    this.sessions.splice(at, 1);
    if (this.activeKey === key) this.activeKey = this.sessions[Math.min(at, this.sessions.length - 1)]?.key ?? 0;
  }
}

export const ssh = new Ssh();
