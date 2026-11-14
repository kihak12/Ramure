// State of the SSH module: host book, keys, and open terminals, remote or local.

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { tick } from 'svelte';
import { dialogs } from '../ui/dialogs.svelte';
import { toasts } from '../ui/toasts.svelte';
import { secretsHeld } from './secrets';
import { addressOf, count, listenOf, type Host, type KeyInfo, type Tunnel, type TunnelState } from './types';

/** The error notification of each tunnel that could not start, by group and tunnel: it goes away
 *  once the tunnel starts or stops (guidelines, 9.15). */
const failures = new Map<string, number>();
function dismissFailure(group: string, id: string) {
  const toast = failures.get(`${group}:${id}`);
  if (toast === undefined) return;
  failures.delete(`${group}:${id}`);
  toasts.dismiss(toast);
}

/** What the core last said of a tunnel of the session. */
export type TunnelRuntime = Omit<TunnelState, 'group' | 'id'>;

/** The tunnels of a session (guidelines, 9.15, "Tunnels"). They belong to a group of the core,
 *  which outlives a connection: a lost connection keeps their ports, and they start again when it
 *  comes back. */
export interface SessionTunnels {
  /** The key of the group in the core: the same across the reconnections of a lost connection. */
  group: string;
  /** The group holds tunnels in the core: from the first connection until the session is over. */
  live: boolean;
  /** Tunnels added during the session and not kept on the host: they live as long as it does. */
  own: Tunnel[];
  /** Ids of the tunnels the user stopped: when the session comes back, they stay stopped. */
  stopped: string[];
  /** What the core says of each tunnel, by id (event `tunnel-state`). */
  states: Record<string, TunnelRuntime>;
  /** The definition each tunnel was started with, by id: a change of the host restarts it. */
  running: Record<string, Tunnel>;
  /** The strip of the tunnels is shown. */
  shown: boolean;
}

/** A tunnel of a session, as the strip lists it: `own` for one added during the session. */
export interface SessionTunnel {
  tunnel: Tunnel;
  own: boolean;
  state: TunnelRuntime;
}

const IDLE: TunnelRuntime = { state: 'stopped', connections: 0, error: null, lastFailure: null };

/** What a change of the host must touch to start a tunnel again: not its name, nor its start with the session. */
const shape = (t: Tunnel) => JSON.stringify([t.kind, t.listenPort, t.targetHost, t.targetPort, t.bindAny]);

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
  /** The tunnels of an SSH session; a local terminal has none. */
  tunnels?: SessionTunnels;
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
  tunnels: [],
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
    this.#listenTunnels();
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
      this.#followHost(saved);
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
      tunnels: { group: crypto.randomUUID(), live: false, own: [], stopped: [], states: {}, running: {}, shown: false },
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

  // --- Tunnels (guidelines, 9.15, "Tunnels") ---------------------------------------------------

  /** The host of a session as the book has it now: its tunnels follow what the form saves. */
  hostOf(session: Session): Host | null {
    return this.hosts.find((h) => h.id === session.host.id) ?? null;
  }

  /** The tunnels of a session: those of its host, then its own, each with its state. */
  tunnelsOf(session: Session): SessionTunnel[] {
    const t = session.tunnels;
    if (!t) return [];
    const kept = (this.hostOf(session) ?? session.host).tunnels ?? [];
    const ids = new Set(kept.map((tunnel) => tunnel.id));
    return [
      ...kept.map((tunnel) => ({ tunnel, own: false })),
      ...t.own.filter((tunnel) => !ids.has(tunnel.id)).map((tunnel) => ({ tunnel, own: true })),
    ].map((row) => ({ ...row, state: t.states[row.tunnel.id] ?? IDLE }));
  }

  /** "2 tunnels are paused. Connections in progress are cut.", for the panel of a lost connection;
   *  empty when no tunnel was running. */
  tunnelsPaused(session: Session): string {
    const paused = this.tunnelsOf(session).filter((row) => ['starting', 'listening', 'waiting'].includes(row.state.state)).length;
    if (!paused) return '';
    return `${count(paused, 'tunnel')} ${paused === 1 ? 'is' : 'are'} paused. Connections in progress are cut.`;
  }

  /** The first connection of a session has opened: its tunnels start, those of the host that start
   *  with the session and its own, except those the user stopped. */
  tunnelsStart(session: Session) {
    const t = session.tunnels;
    if (!t || t.live) return;
    t.live = true;
    for (const row of this.tunnelsOf(session)) {
      if ((row.own || row.tunnel.autostart) && !t.stopped.includes(row.tunnel.id)) void this.tunnelStart(session, row.tunnel);
    }
  }

  /** The session is over for good: its tunnels stop and their ports are freed. `resumable`: the
   *  connection was lost, and "Reconnect" brings the same session back, with its own tunnels and
   *  those the user stopped; otherwise the next one starts afresh. */
  tunnelsClose(session: Session, resumable = false) {
    const t = session.tunnels;
    if (!t) return;
    if (t.live) void invoke('tunnels_close', { group: t.group }).catch(() => {});
    // A new key: what the core still says of the old group no longer reaches this session.
    t.group = crypto.randomUUID();
    t.live = false;
    t.states = {};
    t.running = {};
    if (!resumable) {
      t.own = [];
      t.stopped = [];
    }
  }

  /** Starts a tunnel, or starts it again ("Start", "Retry", a change of the host). */
  async tunnelStart(session: Session, tunnel: Tunnel) {
    const t = session.tunnels;
    if (!t) return;
    dismissFailure(t.group, tunnel.id);
    t.stopped = t.stopped.filter((id) => id !== tunnel.id);
    t.running[tunnel.id] = { ...tunnel };
    // "Starting…" at once; a failure that follows is then a new one, and is said again.
    t.states[tunnel.id] = { ...IDLE, state: 'starting', lastFailure: t.states[tunnel.id]?.lastFailure ?? null };
    const group = t.group;
    try {
      await invoke('tunnel_start', { group, owner: session.host.alias || session.host.hostname, tunnel: { ...tunnel } });
    } catch (e) {
      if (t.group !== group) return;
      this.#tunnelState(session, { group, id: tunnel.id, state: 'failed', connections: 0, error: message(e), lastFailure: null });
    }
  }

  /** "Stop": the tunnel frees its port, and stays stopped when the session comes back. */
  async tunnelStop(session: Session, id: string) {
    const t = session.tunnels;
    if (!t) return;
    if (!t.stopped.includes(id)) t.stopped = [...t.stopped, id];
    dismissFailure(t.group, id);
    delete t.running[id];
    if (t.states[id]) t.states[id] = { ...t.states[id], state: 'stopped', connections: 0, error: null };
    if (!t.live) return;
    try {
      await invoke('tunnel_stop', { group: t.group, id });
    } catch (e) {
      toasts.err('The tunnel could not be stopped', message(e));
    }
  }

  /** "Remove": a tunnel of the session stops and leaves it. */
  async tunnelRemove(session: Session, id: string) {
    const t = session.tunnels;
    if (!t) return;
    // The row goes at once; the core frees the port meanwhile.
    const stopping = this.tunnelStop(session, id);
    t.own = t.own.filter((tunnel) => tunnel.id !== id);
    t.stopped = t.stopped.filter((other) => other !== id);
    delete t.states[id];
    await stopping;
  }

  /** "Add tunnel" in the strip: the tunnel starts at once. `keep`: it is saved on the host too,
   *  through the save of the host form. What kept it from being saved, or null. */
  async tunnelAdd(session: Session, tunnel: Tunnel, keep: boolean): Promise<string | null> {
    const t = session.tunnels;
    if (!t) return null;
    const host = keep ? this.hostOf(session) : null;
    if (host) {
      const saved = await this.save({ ...host, tunnels: [...(host.tunnels ?? []), tunnel] });
      if (typeof saved === 'string') return saved;
    } else {
      t.own = [...t.own, tunnel];
    }
    // Saving may have started it already, if the session is connected.
    if (t.live && !t.running[tunnel.id]) void this.tunnelStart(session, tunnel);
    return null;
  }

  /** A host was saved: its open sessions follow it. A tunnel removed from it stops, a changed one
   *  starts again, a new one that starts with the session starts. */
  #followHost(host: Host) {
    for (const session of this.sessions) {
      const t = session.tunnels;
      if (!t?.live || session.host.id !== host.id) continue;
      const kept = host.tunnels ?? [];
      const ids = new Set([...kept, ...t.own].map((tunnel) => tunnel.id));
      for (const id of Object.keys(t.running)) {
        if (ids.has(id)) continue;
        void this.tunnelStop(session, id);
        t.stopped = t.stopped.filter((other) => other !== id);
        delete t.states[id];
      }
      // A tunnel kept on the host leaves the session's own.
      t.own = t.own.filter((tunnel) => !kept.some((k) => k.id === tunnel.id));
      for (const tunnel of kept) {
        const before = t.running[tunnel.id];
        if (before ? shape(before) !== shape(tunnel) : tunnel.autostart && !t.stopped.includes(tunnel.id) && !t.states[tunnel.id]) {
          void this.tunnelStart(session, tunnel);
        }
      }
    }
  }

  #listening = false;
  #listenTunnels() {
    if (this.#listening) return;
    this.#listening = true;
    void listen<TunnelState>('tunnel-state', (event) => {
      const session = this.sessions.find((s) => s.tunnels?.group === event.payload.group);
      if (session) this.#tunnelState(session, event.payload);
    });
  }

  /** What the core says of a tunnel. A start that fails is said in a notification too, since the
   *  user may be looking at the terminal (guidelines, 9.15). */
  #tunnelState(session: Session, said: TunnelState) {
    const t = session.tunnels;
    if (!t || said.group !== t.group) return;
    const before = t.states[said.id];
    // A tunnel that was stopped or removed meanwhile: the core is late, the interface is right.
    if (!t.running[said.id] && (said.state !== 'stopped' || !before)) return;
    t.states[said.id] = { state: said.state, connections: said.connections, error: said.error, lastFailure: said.lastFailure };
    if (said.state === 'failed' && before?.state !== 'failed') {
      const tunnel = t.running[said.id];
      failures.set(`${said.group}:${said.id}`, toasts.err(`Tunnel ${listenOf(tunnel)} could not start`, said.error ?? ''));
    } else if (said.state !== 'failed') {
      // The tunnel has started since, or stopped: its notification no longer says what is true.
      dismissFailure(said.group, said.id);
    }
  }
}

export const ssh = new Ssh();
