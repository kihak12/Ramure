<script lang="ts" module>
  import { Annotation, Prec, RangeSet, StateEffect, StateField, Transaction, type EditorState, type RangeValue } from '@codemirror/state';
  import { Decoration, type DecorationSet, EditorView, GutterMarker, WidgetType, gutter } from '@codemirror/view';

  /** Where a conflict is in the result: from the start of its first line to the start of the line
   *  after it (`trailing`), or to the end of the file. */
  interface Region {
    from: number;
    to: number;
    trailing: boolean;
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

  /** The chip of the side a line was kept from, "1" or "2". */
  class Chip extends GutterMarker {
    readonly side: 'ours' | 'theirs';
    /** Merged by git, outside the conflicts, rather than kept. */
    readonly merged: boolean;
    constructor(side: 'ours' | 'theirs', merged = false) {
      super();
      this.side = side;
      this.merged = merged;
    }
    eq(other: Chip) {
      return other.side === this.side && other.merged === this.merged;
    }
    toDOM() {
      const chip = document.createElement('span');
      const n = this.side === 'ours' ? '1' : '2';
      chip.className = `cm-chip cm-chip-${this.side}`;
      chip.textContent = n;
      chip.title = this.merged ? `Merged by git from side ${n}` : `Kept from side ${n}`;
      return chip;
    }
  }

  /**
   * The marks of the lines git merged, after a change: a line typed in or deleted is no longer
   * git's and loses its mark; a line moved by what is typed above it keeps it (guidelines, 9.26).
   */
  function follow<T extends RangeValue>(marks: RangeSet<T>, tr: Transaction): RangeSet<T> {
    if (!tr.docChanged) return marks;
    const touched = (from: number) => {
      const line = tr.startState.doc.lineAt(from);
      let hit = false;
      tr.changes.iterChangedRanges((a, b) => {
        // A line inserted at its start only pushes it down.
        if (a <= line.to && b >= line.from && !(a === b && a === line.from)) hit = true;
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

  onMount(() => {
    const doc = State.create({ doc: value, extensions: [State.lineSeparator.of(lineSeparator)] }).doc;
    const initial = new Map<number, Region>();
    for (const r of initialRegions) {
      const first = doc.line(r.start + 1);
      const trailing = r.end + 2 <= doc.lines;
      initial.set(r.id, { from: first.from, to: trailing ? doc.line(r.end + 2).from : doc.length, trailing });
    }
    field = regionsField(initial);

    // The lines git merged from a side, outside the conflicts: their tint and their chip follow the
    // lines when the text above them is typed (guidelines, 9.26).
    const lines = merged.filter((m) => m.line < doc.lines);
    const mergedTints = StateField.define<DecorationSet>({
      create: () => Decoration.set(lines.map((m) => Decoration.line({ class: `cm-from-${m.side}` }).range(doc.line(m.line + 1).from)), true),
      update: (tints, tr) => follow(tints, tr),
      provide: (f) => EditorView.decorations.from(f),
    });
    const mergedChips = StateField.define<RangeSet<GutterMarker>>({
      create: () => RangeSet.of(lines.map((m) => new Chip(m.side, true).range(doc.line(m.line + 1).from)), true),
      update: (chips, tr) => follow(chips, tr),
    });

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
            if (side) ranges.push(Decoration.line({ class: `cm-from-${side}` }).range(state.doc.line(n).from));
          }
        }
      }
      return Decoration.set(ranges, true);
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
            if (side) marks.push(new Chip(side).range(v.state.doc.line(n).from));
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
        EditorView.lineWrapping,
        decorations,
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

  /** Brings a conflict to the middle of the view. */
  export function scrollToRegion(id: number) {
    if (!view) return;
    const region = view.state.field(field).get(id);
    if (region) view.dispatch({ effects: EditorView.scrollIntoView(region.from, { y: 'center' }) });
  }

  const theme = EditorView.theme({
    '&': { height: '100%', fontSize: 'var(--fs-code)', backgroundColor: 'var(--bg)', color: 'var(--ink)' },
    '&.cm-focused': { outline: 'none' },
    '.cm-scroller': { fontFamily: 'var(--f-mono)', lineHeight: '18px' },
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
    '.cm-chip-ours': { backgroundColor: 'color-mix(in srgb, var(--l0) 30%, var(--bg))' },
    '.cm-chip-theirs': { backgroundColor: 'color-mix(in srgb, var(--l1) 30%, var(--bg))' },
    '.cm-from-ours': { backgroundColor: 'color-mix(in srgb, var(--l0) 10%, transparent)' },
    '.cm-from-theirs': { backgroundColor: 'color-mix(in srgb, var(--l1) 10%, transparent)' },
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
