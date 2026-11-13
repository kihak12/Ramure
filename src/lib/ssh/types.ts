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
  updatedAt: number;
}

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
  updatedAt: 0,
});
