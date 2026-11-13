// Cards of the view of a repository: the width of the side cards and whether they are hidden, the
// height of the terminal. These choices are shared by all the tabs and restored from one launch
// to the next (guidelines, sections 9.16, 9.17 and 9.20).

const STORAGE_KEY = 'ramure.layout';

/** Bounds of each card, in pixels. `initial` is the default width. */
export const SIDES = {
  sidebar: { min: 160, max: 440, initial: 204 },
  detail: { min: 260, max: 620, initial: 324 },
} as const;

/** Width the center card always keeps: the graph must stay readable. */
export const CENTER_MIN = 360;

export type Side = keyof typeof SIDES;

/** Height of the terminal of a repository: by default, at least, and what the graph always keeps. */
export const TERMINAL = { initial: 240, min: 130, share: 0.4 } as const;
export const GRAPH_MIN = 200;

export interface Layout {
  sidebar: number;
  detail: number;
  /** Hidden card: its chosen width is kept for when it comes back. */
  sidebarHidden: boolean;
  detailHidden: boolean;
  /** Height chosen for the terminal; `null` as long as it has not been set. */
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
    /* unreadable value: keep the default layout */
  }
  return layout;
}

export const layout = $state<Layout>(read());

export function saveLayout() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(layout));
  } catch {
    /* storage unavailable: the layout holds for this session */
  }
}

export const isHidden = (side: Side) => layout[`${side}Hidden`];

/** Hides or shows a side card. */
export function setHidden(side: Side, hidden: boolean) {
  if (layout[`${side}Hidden`] === hidden) return;
  layout[`${side}Hidden`] = hidden;
  saveLayout();
}

export const toggleSide = (side: Side) => setHidden(side, !isHidden(side));

/** Both cards at once: they are hidden, unless both already are. */
export function toggleSides() {
  const hidden = !(layout.sidebarHidden && layout.detailHidden);
  setHidden('sidebar', hidden);
  setHidden('detail', hidden);
}

/** An explicit request to see the detail brings it back if it was hidden. */
export const revealDetail = () => setHidden('detail', false);

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

const EDGES = 16; // left and right margins of the view
const HANDLE = 8; // the handle of a shown card stands for the gap with the center

/**
 * Widths actually displayed for a view of `width` pixels, and the maximum width each card can take
 * without depriving the center of its room. When the window is too narrow for the chosen widths,
 * the room is taken back from the right card first, then from the left one.
 * A hidden card counts for zero, and so does its handle: the center takes all the room.
 */
export function fit(width: number) {
  const shown = { sidebar: !layout.sidebarHidden, detail: !layout.detailHidden };
  const minSidebar = shown.sidebar ? SIDES.sidebar.min : 0;
  const minDetail = shown.detail ? SIDES.detail.min : 0;
  const gutters = EDGES + (shown.sidebar ? HANDLE : 0) + (shown.detail ? HANDLE : 0);
  // Width not known yet (before the first display): restrict nothing.
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

const GAP = 8; // the gap between the graph and the terminal, where the handle stands

/**
 * Displayed height of the terminal in a column of `height` pixels, and its bounds. By default,
 * 240 px, brought down to 40% of the room when the window is low; the graph always keeps 200 px.
 */
export function fitTerminal(height: number) {
  const max = height > 0 ? Math.max(TERMINAL.min, height - GAP - GRAPH_MIN) : Infinity;
  // Height not known yet (before the first display): the default value, with no reduction.
  const room = height > 0 ? Math.round((height - GAP) * TERMINAL.share) : TERMINAL.initial;
  const initial = clamp(Math.min(TERMINAL.initial, room), TERMINAL.min, max);
  const value = clamp(layout.terminal ?? initial, TERMINAL.min, max);
  return { value, max: Number.isFinite(max) ? max : TERMINAL.initial, initial };
}
