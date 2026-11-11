// Cartes de la vue d'un dépôt : la largeur des cartes latérales et si elles sont masquées, la
// hauteur du terminal. Ces choix sont communs à tous les onglets et retrouvés d'un lancement à
// l'autre (charte, sections 9.16, 9.17 et 9.20).

const STORAGE_KEY = 'ramure.layout';

/** Bornes de chaque carte, en pixels. `initial` est la largeur par défaut. */
export const SIDES = {
  sidebar: { min: 160, max: 440, initial: 204 },
  detail: { min: 260, max: 620, initial: 324 },
} as const;

/** Largeur que la carte centrale garde toujours : le graphe doit rester lisible. */
export const CENTER_MIN = 360;

export type Side = keyof typeof SIDES;

/** Hauteur du terminal d'un dépôt : par défaut, au moins, et ce que le graphe garde toujours. */
export const TERMINAL = { initial: 240, min: 120, share: 0.4 } as const;
export const GRAPH_MIN = 200;

export interface Layout {
  sidebar: number;
  detail: number;
  /** Carte masquée : sa largeur choisie est gardée pour son retour. */
  sidebarHidden: boolean;
  detailHidden: boolean;
  /** Hauteur choisie pour le terminal ; `null` tant qu'on ne l'a pas réglée. */
  terminal: number | null;
}

function read(): Layout {
  const layout: Layout = { sidebar: SIDES.sidebar.initial, detail: SIDES.detail.initial, sidebarHidden: false, detailHidden: false, terminal: null };
  try {
    const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}') as Partial<Layout>;
    for (const side of ['sidebar', 'detail'] as const) {
      const value = saved[side];
      if (typeof value === 'number' && Number.isFinite(value)) layout[side] = value;
      layout[`${side}Hidden`] = saved[`${side}Hidden`] === true;
    }
    if (typeof saved.terminal === 'number' && Number.isFinite(saved.terminal)) layout.terminal = saved.terminal;
  } catch {
    /* valeur illisible : on garde la disposition par défaut */
  }
  return layout;
}

export const layout = $state<Layout>(read());

export function saveLayout() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(layout));
  } catch {
    /* stockage indisponible : la disposition vaut pour cette session */
  }
}

export const isHidden = (side: Side) => layout[`${side}Hidden`];

/** Masque ou affiche une carte latérale. */
export function setHidden(side: Side, hidden: boolean) {
  if (layout[`${side}Hidden`] === hidden) return;
  layout[`${side}Hidden`] = hidden;
  saveLayout();
}

export const toggleSide = (side: Side) => setHidden(side, !isHidden(side));

/** Les deux cartes d'un coup : on les masque, sauf si elles le sont déjà toutes les deux. */
export function toggleSides() {
  const hidden = !(layout.sidebarHidden && layout.detailHidden);
  setHidden('sidebar', hidden);
  setHidden('detail', hidden);
}

/** Une demande explicite de voir le détail le ramène s'il était masqué. */
export const revealDetail = () => setHidden('detail', false);

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

const EDGES = 16; // marges gauche et droite de la vue
const HANDLE = 8; // la poignée d'une carte affichée tient lieu d'écart avec le centre

/**
 * Largeurs réellement affichées pour une vue de `width` pixels, et la largeur maximale que chaque
 * carte peut prendre sans priver le centre de sa place. Quand la fenêtre est trop étroite pour les
 * largeurs choisies, on reprend d'abord sur la carte de droite, puis sur celle de gauche.
 * Une carte masquée compte pour zéro, sa poignée aussi : le centre prend toute la place.
 */
export function fit(width: number) {
  const shown = { sidebar: !layout.sidebarHidden, detail: !layout.detailHidden };
  const minSidebar = shown.sidebar ? SIDES.sidebar.min : 0;
  const minDetail = shown.detail ? SIDES.detail.min : 0;
  const gutters = EDGES + (shown.sidebar ? HANDLE : 0) + (shown.detail ? HANDLE : 0);
  // Largeur encore inconnue (avant le premier affichage) : on ne restreint rien.
  const room = width > 0 ? Math.max(minSidebar + minDetail, width - gutters - CENTER_MIN) : Infinity;
  const wantedSidebar = shown.sidebar ? clamp(layout.sidebar, SIDES.sidebar.min, SIDES.sidebar.max) : 0;
  const wantedDetail = shown.detail ? clamp(layout.detail, SIDES.detail.min, SIDES.detail.max) : 0;
  const excess = Math.max(0, wantedSidebar + wantedDetail - room);
  const detail = Math.max(minDetail, wantedDetail - excess);
  const sidebar = Math.max(minSidebar, wantedSidebar - Math.max(0, excess - (wantedDetail - detail)));
  return {
    sidebar,
    detail,
    maxSidebar: clamp(room - detail, SIDES.sidebar.min, SIDES.sidebar.max),
    maxDetail: clamp(room - sidebar, SIDES.detail.min, SIDES.detail.max),
  };
}

const GAP = 8; // l'écart entre le graphe et le terminal, où se tient la poignée

/**
 * Hauteur affichée du terminal dans une colonne de `height` pixels, et ses bornes. Par défaut,
 * 240 px, ramenés à 40 % de la place quand la fenêtre est basse ; le graphe garde toujours 200 px.
 */
export function fitTerminal(height: number) {
  const max = height > 0 ? Math.max(TERMINAL.min, height - GAP - GRAPH_MIN) : Infinity;
  // Hauteur encore inconnue (avant le premier affichage) : la valeur par défaut, sans réduction.
  const room = height > 0 ? Math.round((height - GAP) * TERMINAL.share) : TERMINAL.initial;
  const initial = clamp(Math.min(TERMINAL.initial, room), TERMINAL.min, max);
  const value = clamp(layout.terminal ?? initial, TERMINAL.min, max);
  return { value, max: Number.isFinite(max) ? max : TERMINAL.initial, initial };
}
