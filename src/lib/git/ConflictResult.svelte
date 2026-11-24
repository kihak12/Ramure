<script lang="ts" module>
  import { Annotation, Prec, RangeSet, StateEffect, StateField, Transaction, type EditorState, type RangeValue } from '@codemirror/state';
  import { Decoration, type DecorationSet, EditorView, GutterMarker, WidgetType, gutter } from '@codemirror/view';
  import { iconMarkup } from '../ui/Icon.svelte';
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
  /**
   * The result as it is, carried to the other window and back: its text with its history and its
   * caret, its conflicts, the lines git merged, and its scroll (guidelines, 9.26, "Its own window").
   */
  export interface ResultSnapshot {
    /** The state of the editor, `EditorState.toJSON` with its history. */
    state: { doc: string } & Record<string, unknown>;
    regions: [number, Region][];
    merged: { line: number; side: 'ours' | 'theirs' }[];
    scrollTop: number;
    /** The typings the history let go of, the oldest: see `historyDepth`. */
    dropped?: number;
  }

  export interface RegionInfo {
    id: number;
    /** `open`: still to decide, its band in place of its lines; `kept`: lines kept from the sides;
     *  `edited`: typed by hand. A conflict decided keeps its band, above its lines. */
    state: 'open' | 'kept' | 'edited';
    /** The side of each line kept, for its chip. */
    origins: ('ours' | 'theirs')[];
    /** The text of its band: "Conflict 2 of 3", then, once decided, what was kept. */
    band: string;
    /** The current conflict: its band reads in --ink and weight 650, as its headers. */
    current: boolean;
  }

  /** What the buttons of the bands do. */
  export interface BandCalls {
    /** "Keep" with the chip of side `n`. */
    keep(id: number, n: 1 | 2): void;
    /** "More": the menu of the other choices, under the button. */
    more(id: number, anchor: HTMLElement): void;
    /** "Start over": the conflict back as it was. */
    startOver(id: number): void;
    /** The label that keeps side `n`, "Keep your commit", for the tooltips. */
    keepLabel(n: 1 | 2): string;
    /** A press on the band, outside its buttons: its conflict becomes the current one. */
    reach(id: number): void;
  }

  /** A press on a band, outside its buttons, makes its conflict the current one (9.26). */
  function reachOnPress(band: HTMLElement, calls: BandCalls) {
    band.addEventListener('mousedown', (e) => {
      if (!(e.target instanceof Element && e.target.closest('button'))) calls.reach(Number(band.dataset.id));
    });
  }

  /** A small button of a band: out of the tab order, the keyboard decides in the sides (9.26). */
  function bandButton(className: string, title: string, label: string, run: (button: HTMLButtonElement) => void) {
    const b = document.createElement('button');
    b.type = 'button';
    b.tabIndex = -1;
    b.className = className;
    b.title = title;
    b.setAttribute('aria-label', label);
    // The caret stays where it was.
    b.addEventListener('mousedown', (e) => e.preventDefault());
    b.addEventListener('click', (e) => {
      e.preventDefault();
      run(b);
    });
    return b;
  }

  /** A band drawn for conflict `id`, given its new text and whether it is the current one. */
  function refresh(dom: HTMLElement, id: number, text: string, current: boolean) {
    if (dom.dataset.id !== String(id)) return false;
    dom.classList.toggle('current', current);
    const span = dom.querySelector<HTMLElement>('.cm-band-text');
    if (span && span.textContent !== text) {
      span.textContent = text;
      span.title = text;
    }
    return true;
  }

  /** The text of a band, cut first, whole in its tooltip (3.4). */
  function bandText(text: string) {
    const span = document.createElement('span');
    span.className = 'cm-band-text';
    span.textContent = text;
    span.title = text;
    return span;
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

  /**
   * The band of a conflict still to decide, in place of the markers of git: what decides it, "Keep"
   * with the chip of each side, and "More" (guidelines, 9.26).
   */
  class Band extends WidgetType {
    readonly id: number;
    readonly text: string;
    readonly current: boolean;
    readonly calls: BandCalls;
    constructor(item: RegionInfo, calls: BandCalls) {
      super();
      this.id = item.id;
      this.text = item.band;
      this.current = item.current;
      this.calls = calls;
    }
    eq(other: Band) {
      return other.id === this.id && other.text === this.text && other.current === this.current;
    }
    // Another conflict becoming the current one leaves the buttons in place: the menu of "More"
    // stays under its button.
    updateDOM(dom: HTMLElement) {
      return refresh(dom, this.id, this.text, this.current);
    }
    toDOM() {
      const band = document.createElement('div');
      band.className = `cm-conflict-band open${this.current ? ' current' : ''}`;
      band.dataset.id = String(this.id);
      const letter = document.createElement('span');
      letter.className = 'letter';
      letter.textContent = '!';
      letter.setAttribute('aria-hidden', 'true');
      const n = this.id + 1;
      const keep = (side: 1 | 2) => {
        const label = this.calls.keepLabel(side);
        const b = bandButton(`btn small cm-band-keep cm-band-${side}`, label, `${label} in conflict ${n}`, () => this.calls.keep(this.id, side));
        const chip = document.createElement('span');
        chip.className = 'cm-band-chip';
        chip.textContent = String(side);
        chip.setAttribute('aria-hidden', 'true');
        b.append('Keep', chip);
        return b;
      };
      const more = bandButton('btn ghost small cm-band-more', 'Keep both, or neither', `More choices for conflict ${n}`, (b) => this.calls.more(this.id, b));
      more.setAttribute('aria-haspopup', 'menu');
      more.innerHTML = `More${iconMarkup('chevron', 14)}`;
      band.append(letter, bandText(this.text), keep(1), keep(2), more);
      reachOnPress(band, this.calls);
      return band;
    }
    get estimatedHeight() {
      return 28;
    }
  }

  /** The band of a conflict decided, above its lines: what was kept, and "Start over" (9.26). */
  class Done extends WidgetType {
    readonly id: number;
    readonly text: string;
    readonly current: boolean;
    readonly calls: BandCalls;
    constructor(item: RegionInfo, calls: BandCalls) {
      super();
      this.id = item.id;
      this.text = item.band;
      this.current = item.current;
      this.calls = calls;
    }
    eq(other: Done) {
      return other.id === this.id && other.text === this.text && other.current === this.current;
    }
    updateDOM(dom: HTMLElement) {
      return refresh(dom, this.id, this.text, this.current);
    }
    toDOM() {
      const band = document.createElement('div');
      band.className = `cm-conflict-band done${this.current ? ' current' : ''}`;
      band.dataset.id = String(this.id);
      const again = bandButton('btn ghost small', 'Put this conflict back as it was', `Start over conflict ${this.id + 1}`, () => this.calls.startOver(this.id));
      again.textContent = 'Start over';
      band.append(bandText(this.text), again);
      reachOnPress(band, this.calls);
      return band;
    }
    get estimatedHeight() {
      return 28;
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
  import { defaultKeymap, history, historyField, historyKeymap, isolateHistory, undo, undoDepth } from '@codemirror/commands';
  import { EditorState as State } from '@codemirror/state';
  import { drawSelection, highlightActiveLine, highlightActiveLineGutter, keymap, lineNumbers } from '@codemirror/view';
  import { onMount } from 'svelte';

  let {
    value,
    regions: initialRegions,
    merged = [],
    restore = null,
    first = 'ours',
    info,
    lineSeparator = '\n',
    label,
    onchange,
    onedit,
    onundo,
    oncaret,
    onkeep,
    onmore,
    onstartover,
    keepLabel,
  }: {
    /** The file with the markers of git, as it opens. */
    value: string;
    /** The lines of each conflict in `value` (from 0, both included), by id. */
    regions: { id: number; start: number; end: number }[];
    /** The lines git merged from a side outside the conflicts, in `value` (from 0). */
    merged?: { line: number; side: 'ours' | 'theirs' }[];
    /** The result carried from the other window: it takes the place of `value`, `regions` and
     *  `merged`, read when the result opens. */
    restore?: ResultSnapshot | null;
    /** The side shown on the left, side 1: its chips read "1" and take its color. */
    first?: 'ours' | 'theirs';
    info: RegionInfo[];
    lineSeparator?: string;
    label: string;
    onchange?: (text: string) => void;
    /** Conflicts typed in by hand. */
    onedit?: (ids: number[]) => void;
    /** Ctrl + Z: true when it undid a pick, which then comes before the typing. */
    onundo?: () => boolean;
    /** The caret put in a conflict, by a click or the keys: it becomes the current one. */
    oncaret?: (id: number) => void;
    /** The buttons of the bands (guidelines, 9.26). */
    onkeep: BandCalls['keep'];
    onmore: BandCalls['more'];
    onstartover: BandCalls['startOver'];
    keepLabel: BandCalls['keepLabel'];
  } = $props();

  // The buttons of the bands call through this object: it always holds the callbacks of the last render.
  const calls: BandCalls = {
    keep: (id, n) => onkeep(id, n),
    more: (id, anchor) => onmore(id, anchor),
    startOver: (id) => onstartover(id),
    keepLabel: (n) => keepLabel(n),
    reach: (id) => oncaret?.(id),
  };

  let host = $state<HTMLDivElement>();
  let view: EditorView | undefined;
  /** The typings the history let go of, the oldest, past its depth: `historyDepth` counts them
   *  still, so that the depth a choice remembers stays its place among the typings. */
  let dropped = 0;
  let field: StateField<Map<number, Region>>;
  let chipsField: StateField<RangeSet<GutterMarker>>;
  let tintsField: StateField<DecorationSet>;

  onMount(() => {
    const doc = State.create({ doc: restore ? restore.state.doc : value, extensions: [State.lineSeparator.of(lineSeparator)] }).doc;
    const initial = new Map<number, Region>(restore ? restore.regions : []);
    for (const r of restore ? [] : initialRegions) {
      const first = doc.line(r.start + 1);
      const trailing = r.end + 2 <= doc.lines;
      initial.set(r.id, { from: first.from, to: trailing ? doc.line(r.end + 2).from : doc.length, trailing });
    }
    field = regionsField(initial);
    const num = (side: 'ours' | 'theirs'): 1 | 2 => (side === first ? 1 : 2);

    // The lines git merged from a side, outside the conflicts: their tint and their chip follow the
    // lines when the text above them is typed (guidelines, 9.26).
    const lines = (restore ? restore.merged : merged).filter((m) => m.line < doc.lines);
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
    tintsField = mergedTints;

    const decorations = EditorView.decorations.compute([field, infoField], (state) => {
      const regions = state.field(field);
      const ranges = [];
      for (const item of state.field(infoField)) {
        const region = regions.get(item.id);
        if (!region) continue;
        const at = linesOf(state, region);
        if (!at) {
          // Decided with no line: its band alone, where its lines were.
          if (item.state !== 'open') ranges.push(Decoration.widget({ widget: new Done(item, calls), block: true, side: -1 }).range(region.from));
          continue;
        }
        if (item.state === 'open') {
          ranges.push(Decoration.replace({ widget: new Band(item, calls), block: true }).range(at.first.from, at.last.to));
          continue;
        }
        // Decided, its band stays above its lines (guidelines, 9.26).
        ranges.push(Decoration.widget({ widget: new Done(item, calls), block: true, side: -1 }).range(at.first.from));
        if (item.state === 'kept') {
          for (let n = at.first.number, i = 0; n <= at.last.number; n++, i++) {
            const side = item.origins[i];
            if (side) ranges.push(Decoration.line({ class: `cm-from-${num(side)}` }).range(state.doc.line(n).from));
          }
        }
      }
      return Decoration.set(ranges, true);
    });
    // The blanks of the alignment: before a band, or before a line, or at the end. A blank before
    // the first line of a decided conflict goes above its band, which stays on its lines, as in the
    // sides; one after a band with no line goes under it, the band facing its headers.
    const blanks = EditorView.decorations.compute([field, infoField, spacersField, 'doc'], (state) => {
      const spacers = state.field(spacersField);
      if (!spacers.length) return Decoration.none;
      const rows = rowsOf(state);
      return Decoration.set(
        spacers.map((s) => {
          const row = rows[s.row];
          const widget = new Spacer(s.height);
          if (!row) return Decoration.widget({ widget, block: true, side: 1 }).range(state.doc.length);
          const prev = rows[s.row - 1];
          const under = prev?.band === 'empty' && prev.pos === row.pos;
          return Decoration.widget({ widget, block: true, side: under ? 0 : -2 }).range(row.pos);
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
    // A change typed in a conflict, whether decided or not, marks it as edited by hand. The caret
    // put in a conflict makes it the current one.
    const watch = EditorView.updateListener.of((update) => {
      if (update.selectionSet && update.transactions.some((tr) => tr.isUserEvent('select') || tr.isUserEvent('input') || tr.isUserEvent('delete'))) {
        const head = update.state.selection.main.head;
        for (const [id, r] of update.state.field(field)) {
          if (head >= r.from && head <= (r.trailing ? r.to - 1 : r.to)) {
            oncaret?.(id);
            break;
          }
        }
      }
      // The history lets go of its oldest typings once deep: the depth then falls by them.
      for (const tr of update.transactions) {
        const was = undoDepth(tr.startState);
        const is = undoDepth(tr.state);
        if (!tr.isUserEvent('undo') && is < was) dropped += was + 1 - is;
      }
      if (!update.docChanged) return;
      onchange?.(update.state.sliceDoc());
      const touched = new Set<number>();
      for (const tr of update.transactions) {
        if (!tr.docChanged || tr.annotation(picked)) continue;
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

    const config = {
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
    };
    // Carried from the other window: the text, its history and its caret as they were.
    const state = restore ? State.fromJSON(restore.state, config, { history: historyField }) : State.create({ doc: value, ...config });
    dropped = restore?.dropped ?? 0;
    view = new EditorView({ state, parent: host! });
    view.dispatch({ effects: setInfo.of(info) });
    // Its scroll, once the columns are lined up.
    if (restore) {
      const at = restore.scrollTop;
      requestAnimationFrame(() => requestAnimationFrame(() => view && (view.scrollDOM.scrollTop = at)));
    }
    // The caret waits where the eye is, scroll after scroll.
    let frame = 0;
    let top = view.scrollDOM.scrollTop;
    // Taken with Tab, the text, taller than the column, is brought into view by the browser from
    // its top, just after the focus: the three columns would go back to line 1. That jump to the
    // top, at once after the focus, is undone before the columns follow it; a scroll the editor
    // asks for is not (9.26).
    let focusedAt = -Infinity;
    const onScroll = () => {
      if (!view) return;
      if (performance.now() - focusedAt < 100 && view.scrollDOM.scrollTop === 0 && top > 0) {
        focusedAt = -Infinity;
        view.scrollDOM.scrollTop = top;
        return;
      }
      top = view.scrollDOM.scrollTop;
      if (!frame) frame = requestAnimationFrame(() => ((frame = 0), placeCaret()));
    };
    const onFocus = () => {
      focusedAt = performance.now();
    };
    view.scrollDOM.addEventListener('scroll', onScroll, { passive: true });
    view.contentDOM.addEventListener('focus', onFocus);
    // The keyboard gone elsewhere, the caret waits again where the eye is.
    const onBlur = () => requestAnimationFrame(placeCaret);
    view.contentDOM.addEventListener('blur', onBlur);
    return () => {
      view?.contentDOM.removeEventListener('blur', onBlur);
      cancelAnimationFrame(frame);
      view?.scrollDOM.removeEventListener('scroll', onScroll);
      view?.contentDOM.removeEventListener('focus', onFocus);
      view?.destroy();
    };
  });

  $effect(() => {
    const next = info;
    view?.dispatch({ effects: setInfo.of(next) });
    requestAnimationFrame(placeCaret);
  });

  /**
   * Before the keyboard comes in with Tab, the caret of the result waits where the eye is: where
   * it was when that is in view, otherwise on the current conflict when it is in view, otherwise
   * on the first line in view. Typing then scrolls nothing either (guidelines, 9.26).
   */
  function placeCaret() {
    if (!view || view.hasFocus) return;
    const v = view;
    const box = v.scrollDOM.getBoundingClientRect();
    const inView = (pos: number) => {
      const at = v.coordsAtPos(pos);
      return !!at && at.top >= box.top && at.bottom <= box.bottom;
    };
    const put = (pos: number) => {
      if (v.state.selection.main.head !== pos || !v.state.selection.main.empty) v.dispatch({ selection: { anchor: pos } });
    };
    if (inView(v.state.selection.main.head)) return;
    const current = info.find((item) => item.current);
    const region = current ? v.state.field(field).get(current.id) : undefined;
    if (region && current) {
      // A conflict to decide is its band, which takes no caret: the caret waits under it.
      const pos = current.state === 'open' ? region.to : (linesOf(v.state, region)?.first.from ?? region.from);
      if (inView(pos)) return put(pos);
    }
    // The first line in view whole, not the one the top of the column cuts.
    let line = v.state.doc.lineAt(v.posAtCoords({ x: box.left + box.width / 2, y: box.top + 4 }, false));
    if (!inView(line.from) && line.number < v.state.doc.lines) line = v.state.doc.line(line.number + 1);
    put(line.from);
  }

  /**
   * The rows of the result, as the columns line them up: the band of each conflict, in place of its
   * lines while it is to decide, above them once decided; a row per line.
   */
  function rowsOf(state: EditorState): (Row & { pos: number; band?: 'lines' | 'empty' })[] {
    const regions = state.field(field);
    const open = new Map<number, { id: number; last: number }>();
    const decided = new Map<number, { id: number; empty: boolean }[]>();
    /** The lines of the decided conflicts, by number: the conflict they are. */
    const parts = new Map<number, number>();
    for (const item of state.field(infoField)) {
      const region = regions.get(item.id);
      if (!region) continue;
      const at = linesOf(state, region);
      if (item.state === 'open') {
        if (at) open.set(at.first.number, { id: item.id, last: at.last.number });
      } else {
        const pos = at ? at.first.from : region.from;
        decided.set(pos, [...(decided.get(pos) ?? []), { id: item.id, empty: !at }]);
        if (at) for (let n = at.first.number; n <= at.last.number; n++) parts.set(n, item.id);
      }
    }
    const rows: (Row & { pos: number; band?: 'lines' | 'empty' })[] = [];
    for (let n = 1; n <= state.doc.lines; n++) {
      const line = state.doc.line(n);
      for (const d of decided.get(line.from) ?? []) rows.push({ key: conflictKey(d.id), height: 28, pos: line.from, band: d.empty ? 'empty' : 'lines' });
      const band = open.get(n);
      if (band) {
        rows.push({ key: conflictKey(band.id), height: 28, pos: line.from });
        n = band.last;
      } else {
        rows.push({ key: line.text, height: 18, pos: line.from, part: parts.get(n) });
      }
    }
    return rows;
  }

  /** The result as it is, to carry it to the other window (guidelines, 9.26, "Its own window"). */
  export function snapshot(): ResultSnapshot | null {
    if (!view) return null;
    const state = view.state;
    const lines: ResultSnapshot['merged'] = [];
    const other = first === 'ours' ? 'theirs' : 'ours';
    state.field(tintsField).between(0, state.doc.length, (from, _to, deco) => {
      lines.push({ line: state.doc.lineAt(from).number - 1, side: deco.spec.class === 'cm-from-1' ? first : other });
    });
    return {
      state: state.toJSON({ history: historyField }) as ResultSnapshot['state'],
      regions: [...state.field(field)],
      merged: lines,
      scrollTop: view.scrollDOM.scrollTop,
      dropped,
    };
  }

  /** Where the typing stands in its history, as a count: a choice remembers it, to be undone
   *  after the typing done since (guidelines, 9.26). */
  export function historyDepth(): number {
    return view ? dropped + undoDepth(view.state) : 0;
  }

  /** Whether Ctrl + Z still has typing to undo, done after the choice made at `depth`. */
  export function typedSince(depth: number): boolean {
    return !!view && undoDepth(view.state) > 0 && historyDepth() > depth;
  }

  /** Undoes the last typing in the result, wherever the keyboard is in the editor. */
  export function undoTyping(): boolean {
    return view ? undo(view) : false;
  }

  export function rows(): Row[] {
    return view ? rowsOf(view.state).map(({ key, height, part }) => ({ key, height, part })) : [];
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
      // Not in the history of the typing: its own comes first. Typing just before and just after
      // it stay two steps.
      annotations: [picked.of(true), Transaction.addToHistory.of(false), isolateHistory.of('full')],
    });
  }

  export function getText() {
    return view ? view.state.sliceDoc() : value;
  }

  /**
   * The marks of the overview strip, and the height of the result they are placed in: each conflict,
   * and each run of lines git merged from one side (guidelines, 9.26).
   */
  export function overview(): { marks: OverviewMark[]; height: number; bar: number } {
    if (!view) return { marks: [], height: 0, bar: 0 };
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
    // The height of the sideways bar, under the track of the vertical one: the strip leaves it out.
    const bar = view.scrollDOM.offsetHeight - view.scrollDOM.clientHeight;
    return { marks, height: view.contentHeight, bar };
  }

  /** Brings the height `y` of the result, in pixels from its top, to the middle of the view. */
  export function scrollToY(y: number) {
    if (!view) return;
    const scroller = view.scrollDOM;
    scroller.scrollTop = Math.max(0, y - scroller.clientHeight / 2);
  }

  /** Gives the keyboard to the result, without moving its view. */
  export function focus() {
    view?.focus();
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
    // The line of the caret is told only while the result has the keyboard: the caret that waits
    // follows the scroll, and its line would follow it too.
    '.cm-activeLine': { backgroundColor: 'transparent' },
    '&.cm-focused .cm-activeLine': { backgroundColor: 'color-mix(in srgb, var(--accent) 6%, transparent)' },
    '.cm-activeLineGutter': { backgroundColor: 'transparent' },
    '&.cm-focused .cm-activeLineGutter': { color: 'var(--ink)' },
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
    // The band of a conflict, 28 px, facing its headers in the sides (guidelines, 9.26): on
    // --warn-soft while it is to decide; decided, on --panel and in --muted, as the headers.
    '.cm-conflict-band': {
      display: 'flex',
      alignItems: 'center',
      gap: '6px',
      height: '28px',
      padding: '0 4px 0 6px',
      fontFamily: 'var(--f-ui)',
      fontSize: 'var(--fs-small)',
      backgroundColor: 'var(--warn-soft)',
      color: 'var(--ink)',
    },
    '.cm-conflict-band.done': {
      paddingLeft: '8px',
      backgroundColor: 'var(--panel)',
      borderTop: '1px solid var(--line-soft)',
      color: 'var(--muted)',
    },
    '.cm-conflict-band.current .cm-band-text': { color: 'var(--ink)', fontWeight: 'var(--fw-strong)' },
    // The buttons follow the text, at the left: a band is as wide as the longest line, which may
    // run past the view. The text is cut first.
    '.cm-band-text': { flex: '0 1 auto', minWidth: '0', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' },
    '.cm-conflict-band .btn': { flex: 'none', color: 'var(--ink)' },
    // "Keep" with the chip of its side, on 12% of its color, 24% on hover (9.26).
    '.cm-band-1': { '--c': 'var(--l0)' },
    '.cm-band-2': { '--c': 'var(--l1)' },
    '.cm-conflict-band .cm-band-keep': {
      gap: '4px',
      paddingRight: '4px',
      borderColor: 'color-mix(in srgb, var(--c) 55%, var(--line))',
      backgroundColor: 'color-mix(in srgb, var(--c) 12%, var(--raised))',
    },
    '.cm-conflict-band .cm-band-keep:hover:not(:disabled)': { backgroundColor: 'color-mix(in srgb, var(--c) 24%, var(--raised))', borderColor: 'var(--c)' },
    // The letter of a file state (9.6), as the chips of the gutter.
    '.cm-band-chip': {
      display: 'inline-grid',
      placeItems: 'center',
      minWidth: '14px',
      height: '14px',
      borderRadius: 'var(--radius-xs)',
      fontFamily: 'var(--f-mono)',
      fontSize: 'var(--fs-micro)',
      fontWeight: 'var(--fw-strong)',
      backgroundColor: 'color-mix(in srgb, var(--c) 30%, var(--raised))',
    },
    // The chevron of a menu dropped down (9.9), in --muted.
    '.cm-conflict-band .cm-band-more': { gap: '4px', paddingRight: '6px' },
    '.cm-band-more svg': { color: 'var(--muted)', transform: 'rotate(90deg)' },
    // The letter of a file state (9.6): halfway between its color and --ink, on its tint.
    '.cm-conflict-band .letter': {
      display: 'inline-grid',
      placeItems: 'center',
      minWidth: '16px',
      height: '16px',
      borderRadius: 'var(--radius-xs)',
      fontFamily: 'var(--f-mono)',
      fontSize: 'var(--fs-micro)',
      fontWeight: 'var(--fw-strong)',
      color: 'color-mix(in srgb, var(--bad) 50%, var(--ink))',
      backgroundColor: 'color-mix(in srgb, var(--bad) var(--tint-letter), transparent)',
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
