// Réglages de Ramure, communs à tous les dépôts ouverts. Ils se changent dans la page de réglages
// (src/lib/settings/) et s'appliquent à l'instant.

const STORAGE_KEY = 'ramure.settings';

export type PullMode = 'ff-only' | 'rebase' | 'merge';
/** `ribbon` : pastilles d'auteur et rubans de couleur. `classic` : points et courbes, plus compact. */
export type GraphStyle = 'ribbon' | 'classic';
export type DiffMode = 'unified' | 'split';
/** `system` : clair ou sombre, comme le système. */
export type ThemeChoice = 'system' | 'light' | 'dark';
/** Thème de couleurs (charte, section 2.5). */
export type Palette = 'ardoise' | 'encre' | 'sous-bois' | 'braise';

export interface Settings {
  /** Commits chargés ; 0 pour tout l'historique. */
  limit: number;
  /** Fetch automatique, en minutes ; 0 pour le désactiver. */
  autoFetch: number;
  pullMode: PullMode;
  graphStyle: GraphStyle;
  /** Diff sur une colonne, ou ancien et nouveau côte à côte. */
  diffMode: DiffMode;
  theme: ThemeChoice;
  palette: Palette;
  /** Une connexion SSH perdue se rouvre d'elle-même (charte, section 9.15). */
  sshReconnect: boolean;
}

export const DEFAULTS: Readonly<Settings> = { limit: 10000, autoFetch: 5, pullMode: 'ff-only', graphStyle: 'ribbon', diffMode: 'unified', theme: 'system', palette: 'ardoise', sshReconnect: true };

/** Bornes de la récupération automatique, en minutes. */
export const AUTO_FETCH = { min: 0, max: 60 } as const;

function read(): Settings {
  try {
    return { ...DEFAULTS, ...JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}') };
  } catch {
    return { ...DEFAULTS };
  }
}

export const prefs = $state<Settings>(read());

/** Enregistre un ou plusieurs réglages. */
export function savePrefs(next: Partial<Settings>) {
  Object.assign(prefs, next);
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
  } catch {
    /* stockage indisponible : les réglages valent pour cette session */
  }
}
