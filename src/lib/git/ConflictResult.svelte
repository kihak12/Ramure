<script lang="ts" module>
  import { Annotation, Prec, RangeSet, StateEffect, StateField, Transaction, type EditorState, type RangeValue } from '@codemirror/state';
  import { Decoration, type DecorationSet, EditorView, GutterMarker, WidgetType, gutter } from '@codemirror/view';
  import { Spacer } from './spacer';
  import { conflictKey, type Row, type Spacer as Blank } from './threeway';

  /** Where a conflict is in the result: from the start of its first line to the start of the line
   *  after it (`trailing`), or to the end of the file. */
  interface Region {
    from: number;
    to: number;
    trailing: boolean;
  }

  /** A mark of the overview strip: a conflict or a change merged by git, and where it is in the
   *  height of the result, in pixels from its top (guidelines, 9.26). */
  export interface OverviewMark {
    kind: 'conflict' | 'change';
    /** The conflict, for a conflict. */
    id: number;
    /** The side of a change, 1 on the left. */
    n: 1 | 2;
    /** Its state, for a conflict. */
    state: RegionInfo['state'];
    top: number;
    bottom: number;
    /** Its lines in the result, from 1; none for a conflict still to decide or decided with no line. */
    lines: [number, number] | null;
  }

  /** What the result shows of a conflict (guidelines, 9.26). */
  export interface RegionInfo {
    id: number;
    /** `open`: still to decide, shown as a band; `kept`: lines kept from the sides; `edited`: typed by hand. */
    state: 'open' | 'kept' | 'edited';
    /** The side of each line kept, for its chip. */
    origins: ('ours' | 'theirs')[];
    /** The text of the band of a conflict still to decide. */
    band: string;
  }

  /** A change made by the editor itself (a pick), not typed: it marks nothing as edited. */
  const picked = Annotation.define<boolean>();
  const setRegion = StateEffect.define<{ id: number; region: Region }>();
  const setInfo = StateEffect.define<RegionInfo[]>();

  function regionsField(initial: Map<number, Region>) {
    return StateField.define<Map<number, Region>>({
      create: () => initial,
      update(regions, tr) {
        let next = regions;
        if (tr.docChanged) {
          next = new Map();
          for (const [id, r] of regions) {
            // The start stays before what is typed at it, the end too: a line typed at the end of
            // the last line belongs to the conflict, a line typed after it does not.
            next.set(id, { ...r, from: tr.changes.mapPos(r.from, -1), to: tr.changes.mapPos(r.to, -1) });
          }
        }
        for (const fx of tr.effects) {
          if (fx.is(setRegion)) {
            next = new Map(next);
            next.set(fx.value.id, fx.value.region);
          }
        }
        return next;
      },
    });
  }

  const infoField = StateField.define<RegionInfo[]>({
    create: () => [],
    update(value, tr) {
      for (const fx of tr.effects) if (fx.is(setInfo)) return fx.value;
      return value;
    },
  });

  /** The band of a conflict still to decide, in place of the markers of git. */
  class Band extends WidgetType {
    readonly text: string;
    constructor(text: string) {
      super();
      this.text = text;
    }
    eq(other: Band) {
      return other.text === this.text;
    }
    toDOM() {
      const band = document.createElement('div');
      band.className = 'cm-conflict-band';
      const letter = document.createElement('span');
      letter.className = 'letter';
      letter.textContent = '!';
      letter.setAttribute('aria-hidden', 'true');
      const text = document.createElement('span');
      text.textContent = this.text;
      band.append(letter, text);
      return band;
    }
    get estimatedHeight() {
      return 24;
    }
  }

  /** The chip of the side a line comes from, "1" or "2": its number is its place, 1 on the left. */
  class Chip extends GutterMarker {
    readonly n: 1 | 2;
    /** Merged by git, outside the conflicts, rather than kept. */
    readonly merged: boolean;
    constructor(n: 1 | 2, merged = false) {
      super();
      this.n = n;
      this.merged = merged;
    }
    eq(other: Chip) {
      return other.n === this.n && other.merged === this.merged;
    }
    toDOM() {
      const chip = document.createElement('span');
      chip.className = `cm-chip cm-chip-${this.n}`;
      chip.textContent = String(this.n);
      chip.title = this.merged ? `Merged by git from side ${this.n}` : `Kept from side ${this.n}`;
      return chip;
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

  /**
   * The marks of the lines git merged, after a change: a line touched by it, typed in, split or
   * deleted, is no longer git's and loses its mark; a line moved by what is typed above it keeps it
   * (guidelines, 9.26).
   */
  function follow<T extends RangeValue>(marks: RangeSet<T>, tr: Transaction): RangeSet<T> {
    if (!tr.docChanged) return marks;
    const touched = (from: number) => {
      const line = tr.startState.doc.lineAt(from);
      let hit = false;
      tr.changes.iterChangedRanges((a, b) => {
        if (a <= line.to && b >= line.from) hit = true;
      });
      return hit;
    };
    return marks.update({ filter: (from) => !touched(from) }).map(tr.changes);
  }

  /** The lines of a region, in the document as it is now. */
  function linesOf(state: EditorState, region: Region) {
    if (region.to <= region.from) return null;
    const end = region.trailing ? region.to - 1 : region.to;
    return { first: state.doc.lineAt(region.from), last: state.doc.lineAt(Math.max(region.from, end)) };
  }
</script>

<script lang="ts">
  import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
  import { EditorState as State } from '@codemirror/state';
  import { drawSelection, highlightActiveLine, highlightActiveLineGutter, keymap, lineNumbers } from '@codemirror/view';
  import { onMount } from 'svelte';

  let {
    value,
    regions: initialRegions,
    merged = [],
    first = 'ours',
    info,
    lineSeparator = '\n',
    label,
    onchange,
    onedit,
    ontype,
    onundo,
  }: {
    /** The file with the markers of git, as it opens. */
    value: string;
    /** The lines of each conflict in `value` (from 0, both included), by id. */
    regions: { id: number; start: number; end: number }[];
    /** The lines git merged from a side outside the conflicts, in `value` (from 0). */
    merged?: { line: number; side: 'ours' | 'theirs' }[];
    /** The side shown on the left, side 1: its chips read "1" and take its color. */
    first?: 'ours' | 'theirs';
    info: RegionInfo[];
    lineSeparator?: string;
    label: string;
    onchange?: (text: string) => void;
    /** Conflicts typed in by hand. */
    onedit?: (ids: number[]) => void;
    /** Anything typed, in a conflict or not: Ctrl + Z then undoes the typing first. */
    ontype?: () => void;
    /** Ctrl + Z: true when it undid a pick, which then comes before the typing. */
    onundo?: () => boolean;
  } = $props();

  let host = $state<HTMLDivElement>();
  let view: EditorView | undefined;
  let field: StateField<Map<number, Region>>;
  let chipsField: StateField<RangeSet<GutterMarker>>;

  onMount(() => {
    const doc = State.create({ doc: value, extensions: [State.lineSeparator.of(lineSeparator)] }).doc;
    const initial = new Map<number, Region>();
    for (const r of initialRegions) {
      const first = doc.line(r.start + 1);
      const trailing = r.end + 2 <= doc.lines;
      initial.set(r.id, { from: first.from, to: trailing ? doc.line(r.end + 2).from : doc.length, trailing });
    }
    field = regionsField(initial);
    const num = (side: 'ours' | 'theirs'): 1 | 2 => (side === first ? 1 : 2);

    // The lines git merged from a side, outside the conflicts: their tint and their chip follow the
    // lines when the text above them is typed (guidelines, 9.26).
    const lines = merged.filter((m) => m.line < doc.lines);
    const mergedTints = StateField.define<DecorationSet>({
      create: () => Decoration.set(lines.map((m) => Decoration.line({ class: `cm-from-${num(m.side)}` }).range(doc.line(m.line + 1).from)), true),
      update: (tints, tr) => follow(tints, tr),
      provide: (f) => EditorView.decorations.from(f),
    });
    const mergedChips = StateField.define<RangeSet<GutterMarker>>({
      create: () => RangeSet.of(lines.map((m) => new Chip(num(m.side), true).range(doc.line(m.line + 1).from)), true),
      update: (chips, tr) => follow(chips, tr),
    });
    chipsField = mergedChips;

    const decorations = EditorView.decorations.compute([field, infoField], (state) => {
      const regions = state.field(field);
      const ranges = [];
      for (const item of state.field(infoField)) {
        const region = regions.get(item.id);
        const at = region && linesOf(state, region);
        if (!region || !at) continue;
        if (item.state === 'open') {
          ranges.push(Decoration.replace({ widget: new Band(item.band), block: true }).range(at.first.from, at.last.to));
        } else if (item.state === 'kept') {
          for (let n = at.first.number, i = 0; n <= at.last.number; n++, i++) {
            const side = item.origins[i];
            if (side) ranges.push(Decoration.line({ class: `cm-from-${num(side)}` }).range(state.doc.line(n).from));
          }
        }
      }
      return Decoration.set(ranges, true);
    });
    // The blanks of the alignment: before a band, or before a line, or at the end.
    const blanks = EditorView.decorations.compute([field, infoField, spacersField, 'doc'], (state) => {
      const spacers = state.field(spacersField);
      if (!spacers.length) return Decoration.none;
      const rows = rowsOf(state);
      return Decoration.set(
        spacers.map((s) => {
          const row = rows[s.row];
          const widget = new Spacer(s.height);
          return row
            ? Decoration.widget({ widget, block: true, side: -1 }).range(row.pos)
            : Decoration.widget({ widget, block: true, side: 1 }).range(state.doc.length);
        }),
        true,
      );
    });
    // The caret steps over a band: the markers under it cannot be typed into.
    const atomic = EditorView.atomicRanges.of((v) => {
      const set: DecorationSet = Decoration.none;
      const regions = v.state.field(field);
      const ranges = [];
      for (const item of v.state.field(infoField)) {
        const region = regions.get(item.id);
        const at = region && linesOf(v.state, region);
        if (item.state === 'open' && at) ranges.push(Decoration.mark({}).range(at.first.from, at.last.to));
      }
      return ranges.length ? Decoration.set(ranges, true) : set;
    });
    const chips = gutter({
      class: 'cm-chips',
      markers: (v) => {
        const regions = v.state.field(field);
        const marks = [];
        for (const item of v.state.field(infoField)) {
          const region = regions.get(item.id);
          const at = region && linesOf(v.state, region);
          if (item.state !== 'kept' || !at) continue;
          for (let n = at.first.number, i = 0; n <= at.last.number; n++, i++) {
            const side = item.origins[i];
            if (side) marks.push(new Chip(num(side)).range(v.state.doc.line(n).from));
          }
        }
        return [RangeSet.of(marks, true), v.state.field(mergedChips)];
      },
    });
    // A change typed in a conflict, whether decided or not, marks it as edited by hand.
    const watch = EditorView.updateListener.of((update) => {
      if (!update.docChanged) return;
      onchange?.(update.state.sliceDoc());
      const touched = new Set<number>();
      for (const tr of update.transactions) {
        if (!tr.docChanged || tr.annotation(picked)) continue;
        ontype?.();
        const before = tr.startState.field(field);
        tr.changes.iterChangedRanges((fromA, toA) => {
          for (const [id, r] of before) {
            const end = r.trailing ? r.to - 1 : r.to;
            if (fromA <= end && toA >= r.from) touched.add(id);
          }
        });
      }
      if (touched.size) onedit?.([...touched]);
    });

    const state = State.create({
      doc: value,
      extensions: [
        State.lineSeparator.of(lineSeparator),
        field,
        infoField,
        mergedTints,
        mergedChips,
        chips,
        lineNumbers(),
        drawSelection(),
        spacersField,
        decorations,
        blanks,
        atomic,
        history(),
        highlightActiveLine(),
        highlightActiveLineGutter(),
        Prec.highest(keymap.of([{ key: 'Mod-z', run: () => onundo?.() ?? false }])),
        keymap.of([...defaultKeymap, ...historyKeymap]),
        watch,
        theme,
        EditorView.contentAttributes.of({ 'aria-label': label }),
      ],
    });
    view = new EditorView({ state, parent: host! });
    view.dispatch({ effects: setInfo.of(info) });
    return () => view?.destroy();
  });

  $effect(() => {
    const next = info;
    view?.dispatch({ effects: setInfo.of(next) });
  });

  /** The rows of the result, as the columns line them up: a band for a conflict to decide, a row per line. */
  function rowsOf(state: EditorState): (Row & { pos: number })[] {
    const regions = state.field(field);
    const bands = new Map<number, { id: number; last: number }>();
    for (const item of state.field(infoField)) {
      const region = regions.get(item.id);
      const at = region && linesOf(state, region);
      if (item.state === 'open' && at) bands.set(at.first.number, { id: item.id, last: at.last.number });
    }
    const rows = [];
    for (let n = 1; n <= state.doc.lines; n++) {
      const line = state.doc.line(n);
      const band = bands.get(n);
      if (band) {
        rows.push({ key: conflictKey(band.id), height: 24, pos: line.from });
        n = band.last;
      } else {
        rows.push({ key: line.text, height: 18, pos: line.from });
      }
    }
    return rows;
  }

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

  /** The text of a conflict in the result, its lines joined, without the break after them. */
  export function regionText(id: number): string {
    if (!view) return '';
    const region = view.state.field(field).get(id);
    if (!region) return '';
    return view.state.sliceDoc(region.from, region.trailing && region.to > region.from ? region.to - 1 : region.to);
  }

  /**
   * Puts `text` (lines joined with the line break of the file; empty: no line at all) in place of a
   * conflict. A pick of the editor, not typing: it goes neither into the history of the typing nor
   * into the conflicts edited by hand.
   */
  export function setRegionText(id: number, text: string, empty = false) {
    if (!view) return;
    const region = view.state.field(field).get(id);
    if (!region) return;
    const brk = view.state.lineBreak;
    // A region keeps the break after its last line: no line at all takes it away with them.
    const insert = empty ? '' : region.trailing ? text + brk : text;
    view.dispatch({
      changes: { from: region.from, to: region.to, insert },
      effects: setRegion.of({ id, region: { from: region.from, to: region.from + insert.length, trailing: region.trailing } }),
      annotations: [picked.of(true), Transaction.addToHistory.of(false)],
    });
  }

  export function getText() {
    return view ? view.state.sliceDoc() : value;
  }

  /**
   * The marks of the overview strip, and the height of the result they are placed in: each conflict,
   * and each run of lines git merged from one side (guidelines, 9.26).
   */
  export function overview(): { marks: OverviewMark[]; height: number } {
    if (!view) return { marks: [], height: 0 };
    const state = view.state;
    const regions = state.field(field);
    const marks: OverviewMark[] = [];
    for (const item of state.field(infoField)) {
      const region = regions.get(item.id);
      if (!region) continue;
      const at = linesOf(state, region);
      const top = view.lineBlockAt(at ? at.first.from : region.from).top;
      const bottom = at ? view.lineBlockAt(at.last.to).bottom : top;
      const lines: [number, number] | null = at && item.state !== 'open' ? [at.first.number, at.last.number] : null;
      marks.push({ kind: 'conflict', id: item.id, n: 1, state: item.state, top, bottom, lines });
    }
    // The lines merged by git, one mark per run of lines of the same side.
    let run: OverviewMark | null = null;
    const cursor = state.field(chipsField).iter();
    for (; cursor.value; cursor.next()) {
      const line = state.doc.lineAt(cursor.from);
      const n = (cursor.value as Chip).n;
      const block = view.lineBlockAt(line.from);
      if (run && run.n === n && run.lines && run.lines[1] === line.number - 1) {
        run.lines[1] = line.number;
        run.bottom = block.bottom;
      } else {
        run = { kind: 'change', id: -1, n, state: 'kept', top: block.top, bottom: block.bottom, lines: [line.number, line.number] };
        marks.push(run);
      }
    }
    return { marks, height: view.contentHeight };
  }

  /** Brings the height `y` of the result, in pixels from its top, to the middle of the view. */
  export function scrollToY(y: number) {
    if (!view) return;
    const scroller = view.scrollDOM;
    scroller.scrollTop = Math.max(0, y - scroller.clientHeight / 2);
  }

  /** Brings a conflict to the middle of the view. */
  export function scrollToRegion(id: number) {
    if (!view) return;
    const region = view.state.field(field).get(id);
    if (region) view.dispatch({ effects: EditorView.scrollIntoView(region.from, { y: 'center' }) });
  }

  const theme = EditorView.theme({
    '&': { height: '100%', fontSize: 'var(--fs-code)', backgroundColor: 'var(--bg)', color: 'var(--ink)' },
    '&.cm-focused': { outline: 'none' },
    // No wrapping: a column scrolls sideways on its own, and its rows stay aligned (9.26). Every
    // column keeps the room of its sideways bar, so that the three have the same height.
    '.cm-scroller': { fontFamily: 'var(--f-mono)', lineHeight: '18px', overflowX: 'scroll' },
    '.cm-content': { caretColor: 'var(--ink)' },
    '.cm-cursor': { borderLeftColor: 'var(--ink)' },
    '.cm-gutters': { backgroundColor: 'var(--bg)', color: 'var(--muted)', border: 'none', borderRight: '1px solid var(--line-soft)' },
    '.cm-activeLine': { backgroundColor: 'color-mix(in srgb, var(--accent) 6%, transparent)' },
    '.cm-activeLineGutter': { backgroundColor: 'transparent', color: 'var(--ink)' },
    '.cm-selectionBackground, &.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground': {
      backgroundColor: 'color-mix(in srgb, var(--accent) 30%, transparent)',
    },
    '.cm-chips': { minWidth: '22px' },
    '.cm-chips .cm-gutterElement': { display: 'flex', alignItems: 'center', justifyContent: 'center' },
    // The letter of a file state (guidelines, 9.6), on the tint of its side.
    '.cm-chip': {
      display: 'inline-grid',
      placeItems: 'center',
      minWidth: '14px',
      height: '14px',
      borderRadius: 'var(--radius-xs)',
      fontFamily: 'var(--f-mono)',
      fontSize: 'var(--fs-micro)',
      fontWeight: 'var(--fw-strong)',
      color: 'var(--ink)',
    },
    // Side 1, on the left, takes the first color of the graph, side 2 the second (2.2, 9.26).
    '.cm-chip-1': { backgroundColor: 'color-mix(in srgb, var(--l0) 30%, var(--bg))' },
    '.cm-chip-2': { backgroundColor: 'color-mix(in srgb, var(--l1) 30%, var(--bg))' },
    '.cm-from-1': { backgroundColor: 'color-mix(in srgb, var(--l0) 10%, transparent)' },
    '.cm-from-2': { backgroundColor: 'color-mix(in srgb, var(--l1) 10%, transparent)' },
    '.cm-conflict-band': {
      display: 'flex',
      alignItems: 'center',
      gap: '6px',
      height: '24px',
      padding: '0 6px',
      fontFamily: 'var(--f-ui)',
      fontSize: 'var(--fs-small)',
      backgroundColor: 'var(--warn-soft)',
      color: 'var(--ink)',
    },
    '.cm-conflict-band .letter': {
      display: 'inline-grid',
      placeItems: 'center',
      minWidth: '16px',
      height: '16px',
      borderRadius: 'var(--radius-xs)',
      fontSize: 'var(--fs-micro)',
      fontWeight: 'var(--fw-strong)',
      color: 'var(--bad)',
      backgroundColor: 'var(--bad-soft)',
    },
  });
</script>

<div class="editor" bind:this={host}></div>

<style>
  .editor {
    height: 100%;
    min-height: 0;
    overflow: hidden;
  }
</style>
