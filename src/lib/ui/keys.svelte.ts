// Raccourcis clavier configurables. Chaque raccourci déclenche une commande de la palette,
// désignée par son identifiant ; l'utilisateur peut le remplacer ou le retirer.

const STORAGE_KEY = 'ramure.keys';

export interface Shortcut {
  /** Identifiant de la commande déclenchée (le même que dans la palette). */
  id: string;
  title: string;
  group: string;
  /** Raccourci par défaut, comme `Ctrl+Shift+F` ; vide s'il n'y en a pas. */
  keys: string;
}

/** Actions auxquelles un raccourci peut être attribué, avec leur raccourci par défaut. */
export const SHORTCUTS: Shortcut[] = [
  { id: 'palette', group: 'Général', title: 'Palette de commandes', keys: 'Ctrl+K' },
  { id: 'view-git', group: 'Général', title: 'Afficher les dépôts', keys: 'Ctrl+Shift+G' },
  { id: 'view-ssh', group: 'Général', title: 'Afficher le terminal SSH', keys: 'Ctrl+Shift+T' },
  { id: 'settings', group: 'Général', title: 'Réglages', keys: 'Ctrl+,' },
  { id: 'shortcuts', group: 'Général', title: 'Raccourcis clavier', keys: '' },
  { id: 'toggle-sidebar', group: 'Affichage', title: 'Afficher ou masquer la liste des branches', keys: 'Ctrl+Shift+B' },
  { id: 'toggle-detail', group: 'Affichage', title: 'Afficher ou masquer le panneau de détail', keys: 'Ctrl+Shift+E' },
  { id: 'toggle-sides', group: 'Affichage', title: 'Afficher ou masquer les panneaux latéraux', keys: '' },
  { id: 'tab-new', group: 'Onglets', title: 'Nouvel onglet', keys: 'Ctrl+T' },
  { id: 'tab-close', group: 'Onglets', title: "Fermer l'onglet", keys: 'Ctrl+W' },
  { id: 'tab-next', group: 'Onglets', title: 'Onglet suivant', keys: 'Ctrl+Tab' },
  { id: 'tab-prev', group: 'Onglets', title: 'Onglet précédent', keys: 'Ctrl+Shift+Tab' },
  { id: 'open', group: 'Dépôt', title: 'Ouvrir un dépôt', keys: 'Ctrl+O' },
  { id: 'clone', group: 'Dépôt', title: 'Cloner un dépôt', keys: '' },
  { id: 'init', group: 'Dépôt', title: 'Initialiser un dépôt', keys: '' },
  { id: 'reload', group: 'Dépôt', title: 'Recharger le dépôt', keys: 'F5' },
  { id: 'fetch', group: 'Synchronisation', title: 'Récupérer', keys: 'Ctrl+Shift+F' },
  { id: 'pull', group: 'Synchronisation', title: 'Tirer', keys: 'Ctrl+Shift+D' },
  { id: 'push', group: 'Synchronisation', title: 'Pousser', keys: 'Ctrl+Shift+U' },
  { id: 'push-force', group: 'Synchronisation', title: 'Pousser en forçant', keys: '' },
  { id: 'branch', group: 'Travail en cours', title: 'Nouvelle branche', keys: 'Ctrl+B' },
  { id: 'wip', group: 'Travail en cours', title: 'Voir les modifications en cours', keys: 'Ctrl+Shift+M' },
  { id: 'stage-all', group: 'Travail en cours', title: 'Tout indexer', keys: 'Ctrl+Shift+A' },
  { id: 'stash', group: 'Travail en cours', title: 'Remiser les modifications', keys: 'Ctrl+Shift+S' },
  { id: 'pop', group: 'Travail en cours', title: 'Dépiler la dernière remise', keys: '' },
  { id: 'op-continue', group: 'Travail en cours', title: "Continuer l'opération en suspens", keys: '' },
  { id: 'op-abort', group: 'Travail en cours', title: "Abandonner l'opération en suspens", keys: '' },
];

const DEFAULTS = new Map(SHORTCUTS.map((s) => [s.id, s.keys]));

/** Combinaisons d'édition de texte : elles ne peuvent pas être attribuées. */
const RESERVED = new Set(['Ctrl+C', 'Ctrl+V', 'Ctrl+X', 'Ctrl+A', 'Ctrl+Z', 'Ctrl+Y', 'Ctrl+Shift+Z']);

/** Combinaison d'un événement clavier, comme `Ctrl+Shift+F` ; vide pour une touche de modification seule. */
export function comboOf(event: KeyboardEvent): string {
  let key = event.key;
  if (['Control', 'Shift', 'Alt', 'Meta', 'AltGraph', 'Dead', 'Unidentified'].includes(key)) return '';
  if (key === ' ') key = 'Space';
  if (key.length === 1) key = key.toUpperCase();
  const parts = [];
  if (event.ctrlKey) parts.push('Ctrl');
  if (event.altKey) parts.push('Alt');
  if (event.shiftKey) parts.push('Shift');
  parts.push(key);
  return parts.join('+');
}

/** Pourquoi une combinaison ne peut pas servir de raccourci ; vide si elle convient. */
export function refusal(combo: string): string {
  if (RESERVED.has(combo)) return 'Cette combinaison sert à modifier du texte.';
  const functionKey = /^(Ctrl\+|Alt\+|Shift\+)*F\d{1,2}$/.test(combo);
  if (!functionKey && !combo.includes('Ctrl+') && !combo.includes('Alt+')) {
    return 'Un raccourci doit contenir Ctrl ou Alt, ou être une touche de fonction.';
  }
  return '';
}

/** Forme lisible : `Ctrl+Shift+F` → `Ctrl + Maj + F`. */
export const display = (combo: string) =>
  combo
    .split('+')
    .map((part) => (part === 'Shift' ? 'Maj' : part === 'Space' ? 'Espace' : part === 'Enter' ? 'Entrée' : part))
    .join(' + ');

/**
 * Dans un terminal, les combinaisons en Ctrl + lettre appartiennent au shell (Ctrl + K efface
 * la fin de la ligne, Ctrl + W le mot précédent). Seules passent celles qu'un shell n'utilise
 * pas : avec Maj ou Alt, ou une touche de fonction.
 */
export const allowedInTerminal = (combo: string) => /Shift\+|Alt\+|(^|\+)F\d{1,2}$/.test(combo);

function read(): Record<string, string> {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}');
  } catch {
    return {};
  }
}

class Keys {
  /** Raccourcis modifiés par l'utilisateur ; une valeur vide retire le raccourci. */
  overrides = $state<Record<string, string>>(read());
  /** Vrai tant qu'on tape une nouvelle combinaison dans les réglages : aucun raccourci ne se déclenche. */
  recording = $state(false);

  binding(id: string): string {
    return this.overrides[id] ?? DEFAULTS.get(id) ?? '';
  }

  isCustom(id: string): boolean {
    return id in this.overrides;
  }

  /** Commande attachée à une combinaison, s'il y en a une. */
  find(combo: string): string | null {
    return SHORTCUTS.find((s) => this.binding(s.id) === combo)?.id ?? null;
  }

  /** Attribue `combo` à `id`. Une combinaison n'a qu'un rôle : elle est retirée de l'action qui l'avait. */
  set(id: string, combo: string) {
    const next = { ...this.overrides };
    if (combo) {
      for (const other of SHORTCUTS) {
        if (other.id !== id && this.binding(other.id) === combo) next[other.id] = '';
      }
    }
    next[id] = combo;
    this.#save(next);
  }

  reset(id: string) {
    const next = { ...this.overrides };
    delete next[id];
    // Le raccourci par défaut qui revient peut avoir été donné à une autre action entre-temps.
    const restored = DEFAULTS.get(id) ?? '';
    if (restored) {
      for (const other of SHORTCUTS) {
        if (other.id !== id && (next[other.id] ?? DEFAULTS.get(other.id)) === restored) next[other.id] = '';
      }
    }
    this.#save(next);
  }

  resetAll() {
    this.#save({});
  }

  /** Remet des raccourcis personnalisés tels qu'ils étaient, après un « Tout réinitialiser » annulé. */
  restore(overrides: Record<string, string>) {
    this.#save({ ...overrides });
  }

  #save(next: Record<string, string>) {
    // Une valeur égale au défaut n'est pas une personnalisation.
    for (const [id, combo] of Object.entries(next)) if (combo === DEFAULTS.get(id)) delete next[id];
    this.overrides = next;
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
    } catch {
      /* stockage indisponible : les raccourcis valent pour cette session */
    }
  }
}

export const keys = new Keys();
