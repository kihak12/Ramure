// Miroir des structures du module SSH (src-tauri/src/ssh).

export type AuthKind = 'auto' | 'keyFile' | 'password';

/** Hôte du carnet (src-tauri/src/ssh/store.rs). */
export interface Host {
  /** Vide pour un hôte qui n'est pas encore enregistré. */
  id: string;
  alias: string;
  hostname: string;
  port: number;
  user: string;
  auth: AuthKind;
  keyPath: string;
  group: string;
  tags: string[];
  /** Indice dans la palette des lanes (`0` à `7`), ou vide. */
  color: string;
  startupCommand: string;
  proxyJump: boolean;
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

export interface Connected {
  id: number;
  fingerprint: string;
  knownHost: boolean;
  authMethod: string;
  connectMs: number;
}

export interface Closed {
  id: number;
  exitStatus: number | null;
  reason: string | null;
  /** `user` : fermée par l'utilisateur ; `exit` : terminée par le serveur ; `lost` : connexion perdue. */
  ending: 'user' | 'exit' | 'lost';
  /** Terminal local : le signal qui a arrêté le shell, s'il n'a pas fini de lui-même. */
  signal?: string | null;
}

export type SshError =
  | { kind: 'hostKeyUnknown'; fingerprint: string; algorithm: string }
  | { kind: 'hostKeyChanged'; fingerprint: string; line: number }
  | { kind: 'needPassphrase'; path: string }
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
  updatedAt: 0,
});
