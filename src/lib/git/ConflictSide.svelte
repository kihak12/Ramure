<script lang="ts" module>
  import { RangeSet, StateEffect, StateField, type EditorState } from '@codemirror/state';
  import { Decoration, type DecorationSet, EditorView, GutterMarker, WidgetType, gutter } from '@codemirror/view';
  import { iconMarkup } from '../ui/Icon.svelte';
  import { Spacer } from './spacer';
  import { conflictKey, type Row, type Spacer as Blank } from './threeway';

  /** A conflict as one side shows it (guidelines, 9.26). */
  export interface SideConflict {
    /** Index of the conflict in the file. */
    index: number;
    /** First line of the conflict in this side, from 0; -1 when it is not found there. */
    at: number;
    /** Its number of lines in this side. */
    count: number;
    /** This side kept, set aside, or still open. */
    status: 'kept' | 'aside' | 'open';
    /** Typed in by hand in the result. */
    edited: boolean;
  }

  /** What the view needs to draw: the conflicts, the current one, the conflict of the keyboard. */
  interface Info {
    conflicts: SideConflict[];
    current: number;
    cursor: number | null;
    focused: boolean;
  }

  /** What the buttons of the headers call: replaced at each render, read when clicked. */
  interface Calls {
    keep: (block: number) => void;
    aside: (block: number) => void;
  }

  const setInfo = StateEffect.define<Info>();
  const infoField = StateField.define<Info>({
    create: () => ({ conflicts: [], current: -1, cursor: null, focused: false }),
    update(value, tr) {
      for (const fx of tr.effects) if (fx.is(setInfo)) return fx.value;
      return value;
    },
  });

  /** The sign of a change that does not conflict, in the gutter of the signs: not the color alone (11). */
  class Plus extends GutterMarker {
    eq() {
      return true;
    }
    toDOM() {
      const sign = document.createElement('span');
      sign.className = 'cm-side-plus';
      sign.textContent = '+';
      sign.title = 'Changed from the common ancestor, merged by git';
      return sign;
    }
  }
  const plus = new Plus();

  /**
   * The header of a conflict, above its first line: its number and its lines, and the two buttons of
   * the side, at the edge toward the result: the arrow that keeps the side whole, and the cross that
   * sets it aside (guidelines, 9.26).
   */
  class Head extends WidgetType {
    readonly block: number;
    readonly count: number;
    readonly status: 'kept' | 'aside' | 'open';
    readonly edited: boolean;
    readonly current: boolean;
    readonly cursor: boolean;
    readonly place: 'left' | 'right';
    readonly label: string;
    readonly calls: Calls;
    constructor(c: SideConflict, current: boolean, cursor: boolean, place: 'left' | 'right', label: string, calls: Calls) {
      super();
      this.block = c.index;
      this.count = c.count;
      this.status = c.status;
      this.edited = c.edited;
      this.current = current;
      this.cursor = cursor;
      this.place = place;
      this.label = label;
      this.calls = calls;
    }
    eq(other: Head) {
      return (
        other.block === this.block &&
        other.count === this.count &&
        other.status === this.status &&
        other.edited === this.edited &&
        other.current === this.current &&
        other.cursor === this.cursor &&
        other.place === this.place &&
        other.label === this.label
      );
    }
    toDOM() {
      const head = document.createElement('div');
      // The current conflict reads by its header, in --ink and weight 650 (guidelines, 9.26).
      head.className = ['cm-conflict-head', this.place, this.current ? 'current' : '', this.cursor ? 'cursor' : ''].filter(Boolean).join(' ');
      const n = this.block + 1;
      const button = (kind: 'keep' | 'aside') => {
        const b = document.createElement('button');
        b.type = 'button';
        b.tabIndex = -1;
        b.className = `cm-head-btn ${kind}`;
        const keep = kind === 'keep';
        b.disabled = keep ? this.status === 'kept' : this.status === 'aside';
        b.title = keep ? 'Keep this side (Enter)' : 'Set this side aside (Delete)';
        b.setAttribute('aria-label', `${keep ? 'Keep' : 'Set aside'} ${this.label.toLowerCase()} in conflict ${n}`);
        b.innerHTML = iconMarkup(keep ? 'keep' : 'x', 14);
        b.addEventListener('mousedown', (e) => e.preventDefault());
        b.addEventListener('click', (e) => {
          e.preventDefault();
          if (keep) this.calls.keep(this.block);
          else this.calls.aside(this.block);
        });
        return b;
      };
      const text = document.createElement('span');
      text.className = 'cm-head-text';
      const state = this.status === 'kept' ? ' · kept' : this.status === 'aside' ? ' · set aside' : '';
      text.textContent = `Conflict ${n} · ${this.count} line${this.count === 1 ? '' : 's'}${state}${this.edited ? ' · edited by hand' : ''}`;
      // Cut in a narrow column, it is whole in its tooltip (3.4).
      text.title = text.textContent;
      // The arrow is the closest to the result: at the right of side 1, at the left of side 2.
      if (this.place === 'left') head.append(text, button('aside'), button('keep'));
      else head.append(button('keep'), button('aside'), text);
      return head;
    }
    get estimatedHeight() {
      return 28;
    }
  }

  /** The blanks that keep the three columns aligned (guidelines, 9.26). */
  const spacersEffect = StateEffect.define<Blank[]>();
  const spacersField = StateField.define<Blank[]>({
    create: () => [],
    update(value, tr) {
      for (const fx of tr.effects) if (fx.is(spacersEffect)) return fx.value;
      return value;
    },
  });

  /** The conflicts that show in this side: found there, with lines. */
  const shown = (info: Info) => info.conflicts.filter((c) => c.at >= 0 && c.count > 0);

  /** The rows of this side, as the columns line them up: the header of a conflict, then each line. */
  function rowsOf(state: EditorState): (Row & { pos: number; head: boolean })[] {
    const heads = new Map<number, number>();
    for (const c of shown(state.field(infoField))) heads.set(c.at, c.index);
    const rows = [];
    for (let n = 1; n <= state.doc.lines; n++) {
      const line = state.doc.line(n);
      const head = heads.get(n - 1);
      if (head !== undefined) rows.push({ key: conflictKey(head), height: 28, pos: line.from, head: true });
      rows.push({ key: line.text, height: 18, pos: line.from, head: false });
    }
    return rows;
  }

  /**
   * A blank before its row. Before the first line of a conflict, it goes above the header, which
   * stays with its lines: the header is the same height wherever it is, and faces nothing once the
   * conflict is decided.
   */
  function blanks(state: EditorState): DecorationSet {
    const spacers = state.field(spacersField);
    if (!spacers.length) return Decoration.none;
    const rows = rowsOf(state);
    return Decoration.set(
      spacers.map((s) => {
        const row = rows[s.row];
        const widget = new Spacer(s.height);
        if (!row) return Decoration.widget({ widget, block: true, side: 1 }).range(state.doc.length);
        const underHead = rows[s.row - 1]?.head === true;
        return Decoration.widget({ widget, block: true, side: row.head || underHead ? -2 : -1 }).range(row.pos);
      }),
      true,
    );
  }

  function lineOf(state: EditorState, n: number) {
    return state.doc.line(Math.max(1, Math.min(state.doc.lines, n)));
  }

  /** The headers of the conflicts, and the tints of their lines: stronger kept, faded set aside. */
  function decorate(state: EditorState, place: 'left' | 'right', label: string, calls: Calls): DecorationSet {
    const info = state.field(infoField);
    const ranges = [];
    for (const conflict of shown(info)) {
      const first = lineOf(state, conflict.at + 1);
      const cursor = info.focused && info.cursor === conflict.index;
      ranges.push(
        Decoration.widget({ widget: new Head(conflict, conflict.index === info.current, cursor, place, label, calls), block: true, side: -1 }).range(first.from),
      );
      const classes = ['cm-side-line', conflict.status === 'kept' ? 'cm-side-kept' : '', conflict.status === 'aside' ? 'cm-side-aside' : '']
        .filter(Boolean)
        .join(' ');
      for (let i = 0; i < conflict.count; i++) ranges.push(Decoration.line({ class: classes }).range(lineOf(state, conflict.at + 1 + i).from));
    }
    return Decoration.set(ranges, true);
  }

  const theme = EditorView.theme({
    '&': { height: '100%', fontSize: 'var(--fs-code)', backgroundColor: 'var(--bg)', color: 'var(--ink)' },
    '&.cm-focused': { outline: 'none' },
    // No wrapping: a column scrolls sideways on its own, and its rows stay aligned (9.26). Every
    // column keeps the room of its sideways bar, so that the three have the same height.
    '.cm-scroller': { fontFamily: 'var(--f-mono)', lineHeight: '18px', overflowX: 'scroll' },
    // The numbers of the lines are read: --muted, as in a diff (guidelines, 2.3 and 9.14).
    '.cm-gutters': { backgroundColor: 'var(--bg)', color: 'var(--muted)', border: 'none', borderRight: '1px solid var(--line-soft)' },
    '.cm-signs': { minWidth: '14px' },
    '.cm-signs .cm-gutterElement': { display: 'flex', alignItems: 'center', justifyContent: 'center' },
    '.cm-side-plus': { color: 'var(--muted)', fontWeight: 'var(--fw-strong)' },
    '.cm-side-line': { backgroundColor: 'color-mix(in srgb, var(--c) 10%, transparent)' },
    '.cm-side-kept': { backgroundColor: 'color-mix(in srgb, var(--c) 24%, transparent)' },
    // A side set aside steps back: its lines in --muted, on the tint at rest.
    '.cm-side-aside': { color: 'var(--muted)' },
    '.cm-conflict-head': {
      display: 'flex',
      alignItems: 'center',
      gap: '2px',
      height: '28px',
      padding: '0 4px 0 8px',
      fontFamily: 'var(--f-ui)',
      fontSize: 'var(--fs-small)',
      color: 'var(--muted)',
      backgroundColor: 'var(--panel)',
      borderTop: '1px solid var(--line-soft)',
    },
    '.cm-conflict-head.right': { padding: '0 8px 0 4px' },
    '.cm-conflict-head.current': { color: 'var(--ink)', fontWeight: 'var(--fw-strong)' },
    // The conflict of the keyboard: an inner ring on its header, never around the whole pane (9.6).
    '.cm-conflict-head.cursor': { boxShadow: 'inset 0 0 0 1.5px var(--accent)' },
    '.cm-head-text': { flex: '1', minWidth: '0', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' },
    '.cm-conflict-head.right .cm-head-text': { marginLeft: '6px' },
    '.cm-conflict-head.left .cm-head-text': { marginRight: '6px' },
    // Two small ghost buttons made of an icon alone, 24 px (9.1); the arrow of side 2 points left.
    '.cm-head-btn': {
      flex: 'none',
      display: 'grid',
      placeItems: 'center',
      width: '24px',
      height: '100%',
      border: 'none',
      borderRadius: 'var(--radius-s)',
      background: 'none',
      color: 'var(--muted)',
      cursor: 'pointer',
      transition: 'background var(--t-fast) var(--ease), transform var(--t-fast) var(--ease), opacity var(--t-fast) var(--ease)',
    },
    // The states of a button (9 and 9.1): hover, press at 97%, disabled at 45% with no reaction.
    '.cm-head-btn:hover:not(:disabled)': { backgroundColor: 'var(--hover)', color: 'var(--ink)' },
    '.cm-head-btn:active:not(:disabled)': { transform: 'scale(0.97)' },
    '.cm-head-btn.keep': { color: 'var(--ink)' },
    '.cm-head-btn:disabled': { opacity: '0.45', cursor: 'not-allowed' },
    '.cm-conflict-head.right .cm-head-btn.keep svg': { transform: 'scaleX(-1)' },
  });
</script>

<script lang="ts">
  import { EditorState as State } from '@codemirror/state';
  import { lineNumbers } from '@codemirror/view';
  import { onMount } from 'svelte';

  let {
    value,
    lineSeparator = '\n',
    label,
    place,
    changes = [],
    conflicts,
    current,
    onkeep,
    onaside,
  }: {
    /** Text of this side. */
    value: string;
    lineSeparator?: string;
    /** Name of the side: "Your commit", "Upstream"… */
    label: string;
    /** Its column: side 1 on the left, side 2 on the right. Its arrow points toward the result. */
    place: 'left' | 'right';
    /** Lines changed from the common ancestor outside the conflicts, from 0. */
    changes?: number[];
    conflicts: SideConflict[];
    /** The current conflict. */
    current: number;
    /** The arrow of a conflict: keep this side whole. */
    onkeep: (block: number) => void;
    /** The cross of a conflict: set this side aside. */
    onaside: (block: number) => void;
  } = $props();

  let host = $state<HTMLDivElement>();
  let paneEl = $state<HTMLDivElement>();
  let view: EditorView | undefined;
  let focused = $state(false);
  /** The ring of a conflict shows for the keyboard only: a click leaves none behind (guidelines, 9.6). */
  let keyboard = $state(false);
  let cursor = $state<number | null>(null);
  let said = $state('');
  // The buttons call through this object: it always holds the callbacks of the last render.
  const calls: Calls = {
    keep: (block) => {
      cursor = block;
      onkeep(block);
    },
    aside: (block) => {
      cursor = block;
      onaside(block);
    },
  };

  onMount(() => {
    // Where each change that does not conflict starts, in this side, which does not change.
    const doc = State.create({ doc: value, extensions: [State.lineSeparator.of(lineSeparator)] }).doc;
    const changeLines = changes.filter((n) => n < doc.lines).map((n) => doc.line(n + 1).from);
    const changeSigns = RangeSet.of(changeLines.map((from) => plus.range(from)), true);
    const state = State.create({
      doc: value,
      extensions: [
        State.lineSeparator.of(lineSeparator),
        State.readOnly.of(true),
        EditorView.editable.of(false),
        infoField,
        spacersField,
        // The sign of the changes that do not conflict.
        gutter({ class: 'cm-signs', markers: () => changeSigns }),
        lineNumbers(),
        EditorView.decorations.compute([infoField], (s) => decorate(s, place, label, calls)),
        EditorView.decorations.compute([infoField, spacersField], blanks),
        // The text of a side does not change: its changes are marked once, with the tint of a line
        // of a conflict at rest (9.26).
        EditorView.decorations.of(Decoration.set(changeLines.map((from) => Decoration.line({ class: 'cm-side-line cm-side-change' }).range(from)), true)),
        theme,
        EditorView.contentAttributes.of({ 'aria-hidden': 'true' }),
      ],
    });
    view = new EditorView({ state, parent: host! });
    return () => view?.destroy();
  });

  $effect(() => {
    const info: Info = { conflicts, current, cursor, focused: focused && keyboard };
    view?.dispatch({ effects: setInfo.of(info) });
  });

  /** The conflicts of this side, in order: what the arrows of the keyboard go through. */
  const stops = $derived(conflicts.filter((c) => c.at >= 0 && c.count > 0));

  function announce(block: number) {
    const c = conflicts.find((x) => x.index === block);
    if (!c) return;
    const state = c.status === 'kept' ? 'kept' : c.status === 'aside' ? 'set aside' : 'not kept yet';
    said = `Conflict ${block + 1}, ${label}, ${c.count} line${c.count === 1 ? '' : 's'}, ${state}`;
  }

  function moveTo(at: number) {
    const stop = stops[Math.max(0, Math.min(stops.length - 1, at))];
    if (!stop) return;
    cursor = stop.index;
    scrollToLine(stop.at + 1, 'nearest');
    announce(stop.index);
  }

  function onKey(e: KeyboardEvent) {
    // ← and →: a long line of the side, which does not wrap, read sideways (guidelines, 9.26).
    if ((e.key === 'ArrowLeft' || e.key === 'ArrowRight') && view) {
      e.preventDefault();
      view.scrollDOM.scrollLeft += e.key === 'ArrowLeft' ? -40 : 40;
      return;
    }
    if (!stops.length) return;
    if (['ArrowDown', 'ArrowUp', 'Home', 'End', 'Enter', 'Delete', 'Backspace'].includes(e.key)) keyboard = true;
    const at = cursor === null ? -1 : stops.findIndex((s) => s.index === cursor);
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      moveTo(at < 0 ? 0 : at + (e.key === 'ArrowDown' ? 1 : -1));
    } else if (e.key === 'Home' || e.key === 'End') {
      e.preventDefault();
      moveTo(e.key === 'Home' ? 0 : stops.length - 1);
    } else if (e.key === 'Enter' && cursor !== null) {
      e.preventDefault();
      onkeep(cursor);
      const block = cursor;
      queueMicrotask(() => announce(block));
    } else if ((e.key === 'Delete' || e.key === 'Backspace') && cursor !== null) {
      e.preventDefault();
      onaside(cursor);
      const block = cursor;
      queueMicrotask(() => announce(block));
    }
  }

  function onFocus(e: FocusEvent) {
    focused = true;
    keyboard = (e.target as HTMLElement).matches(':focus-visible');
    // The current conflict takes the ring, if none has it yet.
    if (cursor === null && stops.length) {
      cursor = (stops.find((s) => s.index === current) ?? stops[0]).index;
      announce(cursor);
    }
  }

  /** Brings line `line` (from 1) into view, to the middle by default. */
  export function scrollToLine(line: number, where: 'center' | 'nearest' = 'center') {
    if (!view) return;
    const target = view.state.doc.line(Math.max(1, Math.min(view.state.doc.lines, line)));
    view.dispatch({ effects: EditorView.scrollIntoView(target.from, { y: where }) });
  }

  /** The rows of this side, for the alignment of the three columns. */
  export function rows(): Row[] {
    return view ? rowsOf(view.state).map(({ key, height }) => ({ key, height })) : [];
  }

  export function setSpacers(spacers: Blank[]) {
    view?.dispatch({ effects: spacersEffect.of(spacers) });
  }

  /** What scrolls: the three columns scroll as one. */
  export function scroller(): HTMLElement | undefined {
    return view?.scrollDOM;
  }

  /** The ring follows the current conflict when it changes from the bar. */
  export function follow(block: number) {
    if (stops.some((s) => s.index === block)) cursor = block;
  }
</script>

<!-- The conflicts of a side are a list for the keyboard: ↑ and ↓ go through them, Enter keeps the
     side, Delete sets it aside (guidelines, 9.26). -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="pane"
  tabindex={stops.length ? 0 : -1}
  role="group"
  aria-label="{label}: ↑ and ↓ go through the conflicts, Enter keeps this side, Delete sets it aside"
  bind:this={paneEl}
  onkeydown={onKey}
  onfocus={onFocus}
  onblur={() => (focused = false)}
  onpointerdown={(e) => {
    // The text of a side cannot take the focus: the pane takes it, so that its keys, and those of
    // the editor (Ctrl + Z, Ctrl + S, F8), work after a click.
    keyboard = false;
    if (document.activeElement !== paneEl) paneEl?.focus({ preventScroll: true });
    // A click on a conflict makes it the conflict of the keyboard: the arrows go on from it.
    const at = view?.posAtCoords({ x: e.clientX, y: e.clientY });
    if (at === null || at === undefined || !view) return;
    const n = view.state.doc.lineAt(at).number - 1;
    const hit = stops.find((c) => n >= c.at && n < c.at + c.count);
    if (hit) cursor = hit.index;
  }}
>
  <div class="editor" bind:this={host}></div>
  <p class="spoken" aria-live="polite">{said}</p>
</div>

<style>
  .pane {
    height: 100%;
    min-height: 0;
    outline: none;
  }
  .editor {
    height: 100%;
    min-height: 0;
    overflow: hidden;
  }
</style>
