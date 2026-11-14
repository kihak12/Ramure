// Mirror of the structures of the SSH module (src-tauri/src/ssh).

export type AuthKind = 'auto' | 'keyFile' | 'password';

/** A host of the book (src-tauri/src/ssh/store.rs). */
export interface Host {
  /** Empty for a host that is not saved yet. */
  id: string;
  alias: string;
  hostname: string;
  port: number;
  user: string;
  auth: AuthKind;
  keyPath: string;
  group: string;
  tags: string[];
  /** Index in the palette of the lanes (`0` to `7`), or empty. */
  color: string;
  startupCommand: string;
  proxyJump: boolean;
  /** Every session of this host records its log (guidelines, 9.15, "The log of a session"). */
  logSessions: boolean;
  /** The tunnels that start with each session of this host (guidelines, 9.15, "Tunnels"). */
  tunnels: Tunnel[];
  updatedAt: number;
}

export type TunnelKind = 'local' | 'remote' | 'socks';

/** A tunnel, kept on a host or added during a session (src-tauri/src/ssh/store.rs). */
export interface Tunnel {
  /** Random, made by the interface. */
  id: string;
  /** Optional: empty for none. */
  name: string;
  kind: TunnelKind;
  /** Where it listens: this machine for `local` and `socks`, the server for `remote`. */
  listenPort: number;
  /** As the other side sees it: the server for `local`, this machine for `remote`; empty for `socks`. */
  targetHost: string;
  /** 0 for `socks`. */
  targetPort: number;
  /** "Let other machines connect": every interface, not localhost only. */
  bindAny: boolean;
  /** "Start with the session". */
  autostart: boolean;
}

/** What the core says of a tunnel of a session, at every change (event `tunnel-state`). */
export interface TunnelState {
  group: string;
  id: string;
  state: 'starting' | 'listening' | 'waiting' | 'stopped' | 'failed';
  /** Open connections through it. */
  connections: number;
  /** When `failed`: why and what to do, a full sentence. */
  error: string | null;
  /** The last connection the other side refused; `at` in ms since the epoch. */
  lastFailure: { message: string; at: number } | null;
}

/** The kinds of tunnel, named with the words and the flag of `ssh` (guidelines, 9.15). */
export const TUNNEL_KINDS: Record<TunnelKind, string> = {
  local: 'Local forward (-L)',
  remote: 'Remote forward (-R)',
  socks: 'SOCKS proxy (-D)',
};

/** The kind without its flag, before the summary in the host form (guidelines, 9.21). */
export const TUNNEL_KIND_SHORT: Record<TunnelKind, string> = {
  local: 'Local forward',
  remote: 'Remote forward',
  socks: 'SOCKS proxy',
};

/** Where a tunnel listens, as its summary says it: `localhost:5433`, `server:8080`. */
export const listenOf = (t: Tunnel) => `${t.kind === 'remote' ? 'server' : 'localhost'}:${t.listenPort}`;

/** The summary of a tunnel: `localhost:5433 → localhost:5432`, `server:8080 → localhost:3000`,
 *  `localhost:1080 (SOCKS)` (guidelines, 9.15). */
export function tunnelSummary(t: Tunnel): string {
  return t.kind === 'socks' ? `${listenOf(t)} (SOCKS)` : `${listenOf(t)} → ${t.targetHost}:${t.targetPort}`;
}

/** What a row shows of a tunnel: its name, or its summary when it has none. */
export const tunnelLabel = (t: Tunnel) => t.name.trim() || tunnelSummary(t);

/** The address to give a program, on the side where the tunnel listens: `localhost:5433`. */
export const tunnelAddress = (t: Tunnel) => `localhost:${t.listenPort}`;

/** A piece of the sentence of a tunnel; `code`: an address, in the code font (guidelines, 9.21). */
export interface Words {
  text: string;
  code?: boolean;
}

/** The tunnel in words, since `localhost` is the server's in a local forward and this machine's in
 *  a remote one (guidelines, 9.21). */
export function tunnelSentence(t: Tunnel): Words[] {
  const target = `${t.targetHost}:${t.targetPort}`;
  if (t.kind === 'local') {
    return [{ text: 'Connections to ' }, { text: `localhost:${t.listenPort}`, code: true }, { text: ' on this machine reach ' }, { text: target, code: true }, { text: ' as the server sees it.' }];
  }
  if (t.kind === 'remote') {
    return [{ text: `Connections to port ${t.listenPort} on the server reach ` }, { text: target, code: true }, { text: ' on this machine.' }];
  }
  return [{ text: 'Programs set to use the SOCKS5 proxy ' }, { text: `localhost:${t.listenPort}`, code: true }, { text: ' go out through this server.' }];
}

/** The same sentence as plain text, for a tooltip. */
export const tunnelWords = (t: Tunnel) => tunnelSentence(t).map((w) => w.text).join('');

/** The machine a tunnel listens on: two tunnels on the same machine cannot share a port. */
export const listensHere = (t: Pick<Tunnel, 'kind'>) => t.kind !== 'remote';

/** Two tunnels that would listen on the same port of the same machine. */
export const samePort = (a: Pick<Tunnel, 'kind' | 'listenPort'>, b: Pick<Tunnel, 'kind' | 'listenPort'>) =>
  listensHere(a) === listensHere(b) && a.listenPort === b.listenPort;

/** Plural that agrees with its number: "1 tunnel", "2 tunnels" (guidelines, 12). */
export const count = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

export interface KeyInfo {
  name: string;
  path: string;
  public: string;
  algorithm: string;
  fingerprint: string;
  comment: string;
  encrypted: boolean;
}

/** The address of a host, as it is shown: `deploy@vps.example.net:2222`. Without a user, the
 *  address alone, never an "@" on its own; the port, if it is given and is not 22 (guidelines, 9.21). */
export function addressOf(user: string, hostname: string, port = 22): string {
  return `${user ? `${user}@` : ''}${hostname}${port && port !== 22 ? `:${port}` : ''}`;
}

export interface Connected {
  id: number;
  fingerprint: string;
  knownHost: boolean;
  /** The fingerprint was to be remembered and `known_hosts` could not be written: why. */
  learnFailed?: string | null;
  authMethod: string;
  connectMs: number;
  /** The secret was to be kept and the keyring refused it: what it says. */
  keepFailed?: string | null;
  /** The log that was asked for could not be opened: why. */
  journalFailed?: string | null;
}

export interface Closed {
  id: number;
  exitStatus: number | null;
  reason: string | null;
  /** `user`: closed by the user; `exit`: ended by the server; `lost`: connection lost. */
  ending: 'user' | 'exit' | 'lost';
  /** Local terminal: the signal that stopped the shell, if it did not end by itself. */
  signal?: string | null;
}

export type SshError =
  | { kind: 'hostKeyUnknown'; fingerprint: string; algorithm: string }
  | { kind: 'hostKeyChanged'; fingerprint: string; line: number }
  /** `refused`: the passphrase tried does not unlock the key, typed or kept (and then forgotten). */
  | { kind: 'needPassphrase'; path: string; refused?: 'typed' | 'retained' | null }
  /** `refused`: the password the keyring kept was refused, and forgotten. */
  | { kind: 'needPassword'; refused: boolean }
  | { kind: 'auth'; message: string }
  | { kind: 'other'; message: string };

export const emptyHost = (): Host => ({
  id: '',
  alias: '',
  hostname: '',
  port: 22,
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
});
