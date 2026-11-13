// The system keyring, as the interface sees it: whether it responds, what it keeps, and
// forgetting. The value of a secret never goes through here: the core reads it when it
// authenticates (guidelines, section 9.15).

import { invoke } from '@tauri-apps/api/core';

/** A secret, named by what it belongs to: an account on a server, or a key. */
export type SecretRef =
  | { kind: 'password'; user: string; host: string; port: number }
  | { kind: 'passphrase'; path: string };

/** Does the keyring respond? Without it, keeping a secret is not offered, and the interface says so. */
export const keyringAvailable = () => invoke<boolean>('secrets_available').catch(() => false);

/** For each secret, in order: true if it is kept. */
export const secretsHeld = (secrets: SecretRef[]) =>
  secrets.length ? invoke<boolean[]>('secrets_held', { secrets }).catch(() => secrets.map(() => false)) : Promise.resolve([]);

export const forgetSecret = (secret: SecretRef) => invoke<void>('secret_forget', { secret });

/** What is said when the keyring does not respond, for a password or a passphrase. */
export const NO_KEYRING = {
  password: 'The system keyring does not respond: this password will be asked for every time you connect.',
  passphrase: 'The system keyring does not respond: this passphrase will be asked for every time you connect.',
} as const;
