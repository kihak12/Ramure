// Le trousseau du système, vu de l'interface : s'il répond, ce qu'il retient, et l'oubli.
// La valeur d'un secret ne passe jamais par ici : c'est le cœur qui la lit au moment de
// s'authentifier (charte, section 9.15).

import { invoke } from '@tauri-apps/api/core';

/** Un secret, désigné par ce à quoi il appartient : un compte sur un serveur, ou une clé. */
export type SecretRef =
  | { kind: 'password'; user: string; host: string; port: number }
  | { kind: 'passphrase'; path: string };

/** Le trousseau répond-il ? Sans lui, on ne propose pas de retenir un secret, et on le dit. */
export const keyringAvailable = () => invoke<boolean>('secrets_available').catch(() => false);

/** Pour chaque secret, dans l'ordre : vrai s'il est retenu. */
export const secretsHeld = (secrets: SecretRef[]) =>
  secrets.length ? invoke<boolean[]>('secrets_held', { secrets }).catch(() => secrets.map(() => false)) : Promise.resolve([]);

export const forgetSecret = (secret: SecretRef) => invoke<void>('secret_forget', { secret });

/** Ce qu'on dit quand le trousseau ne répond pas, pour un mot de passe ou une phrase secrète. */
export const NO_KEYRING = {
  password: 'Le trousseau du système ne répond pas : ce mot de passe sera redemandé à chaque connexion.',
  passphrase: 'Le trousseau du système ne répond pas : cette phrase secrète sera redemandée à chaque connexion.',
} as const;
