// Largeurs des cartes latérales de la vue d'un dépôt. Elles sont communes à tous les onglets et
// retrouvées d'un lancement à l'autre.

const STORAGE_KEY = 'ramure.layout';

/** Bornes de chaque carte, en pixels. `initial` est la largeur par défaut. */
export const SIDES = {
  sidebar: { min: 160, max: 440, initial: 204 },
  detail: { min: 260, max: 620, initial: 324 },
} as const;

/** Largeur que la carte centrale garde toujours : le graphe doit rester lisible. */
export const CENTER_MIN = 360;

export interface Layout {
  sidebar: number;
  detail: number;
}

function read(): Layout {
  const layout: Layout = { sidebar: SIDES.sidebar.initial, detail: SIDES.detail.initial };
  try {
    const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}') as Partial<Layout>;
    for (const side of ['sidebar', 'detail'] as const) {
      const value = saved[side];
      if (typeof value === 'number' && Number.isFinite(value)) layout[side] = value;
    }
  } catch {
    /* valeur illisible : on garde les largeurs par défaut */
  }
  return layout;
}

export const layout = $state<Layout>(read());

export function saveLayout() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(layout));
  } catch {
    /* stockage indisponible : les largeurs valent pour cette session */
  }
}

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

/**
 * Largeurs réellement affichées pour une vue de `width` pixels, et la largeur maximale que chaque
 * carte peut prendre sans priver le centre de sa place. Quand la fenêtre est trop étroite pour les
 * largeurs choisies, on reprend d'abord sur la carte de droite, puis sur celle de gauche.
 */
export function fit(width: number, gutters: number) {
  // Largeur encore inconnue (avant le premier affichage) : on ne restreint rien.
  const room = width > 0 ? Math.max(SIDES.sidebar.min + SIDES.detail.min, width - gutters - CENTER_MIN) : Infinity;
  const wantedSidebar = clamp(layout.sidebar, SIDES.sidebar.min, SIDES.sidebar.max);
  const wantedDetail = clamp(layout.detail, SIDES.detail.min, SIDES.detail.max);
  const excess = Math.max(0, wantedSidebar + wantedDetail - room);
  const detail = Math.max(SIDES.detail.min, wantedDetail - excess);
  const sidebar = Math.max(SIDES.sidebar.min, wantedSidebar - Math.max(0, excess - (wantedDetail - detail)));
  return {
    sidebar,
    detail,
    maxSidebar: clamp(room - detail, SIDES.sidebar.min, SIDES.sidebar.max),
    maxDetail: clamp(room - sidebar, SIDES.detail.min, SIDES.detail.max),
  };
}
