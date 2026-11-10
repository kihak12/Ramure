// Réglages de Ramure, communs à tous les dépôts ouverts.

const STORAGE_KEY = 'ramure.settings';

export type PullMode = 'ff-only' | 'rebase' | 'merge';
/** `ribbon` : pastilles d'auteur et rubans de couleur. `classic` : points et courbes, plus compact. */
export type GraphStyle = 'ribbon' | 'classic';
export type DiffMode = 'unified' | 'split';

export interface Settings {
  /** Commits chargés ; 0 pour tout l'historique. */
  limit: number;
  /** Fetch automatique, en minutes ; 0 pour le désactiver. */
  autoFetch: number;
  pullMode: PullMode;
  graphStyle: GraphStyle;
  /** Diff sur une colonne, ou ancien et nouveau côte à côte. */
  diffMode: DiffMode;
}

const DEFAULTS: Settings = { limit: 10000, autoFetch: 5, pullMode: 'ff-only', graphStyle: 'ribbon', diffMode: 'unified' };

function read(): Settings {
  try {
    return { ...DEFAULTS, ...JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}') };
  } catch {
    return { ...DEFAULTS };
  }
}

export const prefs = $state<Settings>(read());

export function savePrefs(next: Settings) {
  Object.assign(prefs, next);
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
  } catch {
    /* stockage indisponible : les réglages valent pour cette session */
  }
}
