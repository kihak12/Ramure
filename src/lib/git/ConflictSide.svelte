<script lang="ts" module>
  import { RangeSet, StateEffect, StateField, type EditorState } from '@codemirror/state';
  import { Decoration, type DecorationSet, EditorView, GutterMarker, WidgetType, gutter } from '@codemirror/view';

  /** A conflict as one side shows it (guidelines, 9.26). */
  export interface SideConflict {
    /** Index of the conflict in the file. */
    index: number;
    /** First line of the conflict in this side, from 0; -1 when it is not found there. */
    at: number;
    /** Lines of this side kept, one per line of the conflict. */
    kept: boolean[];
    state: 'on' | 'off' | 'mixed';
    /** Typed in by hand in the result: the boxes step back. */
    edited: boolean;
  }

  /** What the view needs to draw: the conflicts, the current one, the line of the keyboard. */
  interface Info {
    conflicts: SideConflict[];
    current: number;
    cursor: { block: number; line: number } | null;
    focused: boolean;
  }

  /** What the widgets call back: replaced at each render, read when clicked. */
  interface Calls {
    pick: (block: number, line: number, shift: boolean) => void;
    group: (block: number) => void;
  }

  const setInfo = StateEffect.define<Info>();
  const infoField = StateField.define<Info>({
    create: () => ({ conflicts: [], current: -1, cursor: null, focused: false }),
    update(value, tr) {
      for (const fx of tr.effects) if (fx.is(setInfo)) return fx.value;
      return value;
    },
  });

  /** The checkbox of a line, in its gutter: the mouse picks with it, the keyboard goes through the lines. */
  class Box extends GutterMarker {
    readonly block: number;
    readonly line: number;
    readonly kept: boolean;
    readonly faded: boolean;
    readonly calls: Calls;
    constructor(block: number, line: number, kept: boolean, faded: boolean, calls: Calls) {
      super();
      this.block = block;
      this.line = line;
      this.kept = kept;
      this.faded = faded;
      this.calls = calls;
    }
    eq(other: Box) {
      return other.block === this.block && other.line === this.line && other.kept === this.kept && other.faded === this.faded;
    }
    toDOM() {
      const box = document.createElement('input');
      box.type = 'checkbox';
      box.tabIndex = -1;
      box.checked = this.kept;
      box.className = this.faded ? 'faded' : '';
      box.setAttribute('aria-hidden', 'true');
      box.addEventListener('mousedown', (e) => e.preventDefault());
      box.addEventListener('click', (e) => {
        e.preventDefault();
        this.calls.pick(this.block, this.line, e.shiftKey);
      });
      return box;
    }
  }

  /** The header of a conflict, above its first line: a box for the whole side. */
  class Head extends WidgetType {
    readonly block: number;
    readonly count: number;
    readonly state: 'on' | 'off' | 'mixed';
    readonly edited: boolean;
    readonly calls: Calls;
    constructor(block: number, count: number, state: 'on' | 'off' | 'mixed', edited: boolean, calls: Calls) {
      super();
      this.block = block;
      this.count = count;
      this.state = state;
      this.edited = edited;
      this.calls = calls;
    }
    eq(other: Head) {
      return other.block === this.block && other.count === this.count && other.state === this.state && other.edited === this.edited;
    }
    toDOM() {
      const head = document.createElement('div');
      head.className = 'cm-conflict-head';
      const label = document.createElement('label');
      const box = document.createElement('input');
      box.type = 'checkbox';
      box.tabIndex = -1;
      box.checked = this.state === 'on';
      box.indeterminate = this.state === 'mixed';
      if (this.edited) box.className = 'faded';
      box.addEventListener('mousedown', (e) => e.preventDefault());
      box.addEventListener('click', (e) => {
        e.preventDefault();
        this.calls.group(this.block);
      });
      const text = document.createElement('span');
      text.textContent = `Conflict ${this.block + 1} · ${this.count} line${this.count === 1 ? '' : 's'}${this.edited ? ' · edited by hand' : ''}`;
      label.append(box, text);
      head.append(label);
      return head;
    }
    get estimatedHeight() {
      return 24;
    }
  }

  function lineOf(state: EditorState, n: number) {
    return state.doc.line(Math.max(1, Math.min(state.doc.lines, n)));
  }

  /** Tints of the lines, the headers and the ring of the keyboard. */
  function decorate(state: EditorState, calls: Calls): DecorationSet {
    const info = state.field(infoField);
    const ranges = [];
    for (const conflict of info.conflicts) {
      if (conflict.at < 0 || !conflict.kept.length) continue;
      const first = lineOf(state, conflict.at + 1);
      ranges.push(
        Decoration.widget({ widget: new Head(conflict.index, conflict.kept.length, conflict.state, conflict.edited, calls), block: true, side: -1 }).range(first.from),
      );
      conflict.kept.forEach((kept, i) => {
        const line = lineOf(state, conflict.at + 1 + i);
        const cursor = info.focused && info.cursor?.block === conflict.index && info.cursor.line === i;
        const classes = ['cm-side-line', kept ? 'cm-side-kept' : '', conflict.index === info.current ? 'cm-side-current' : '', cursor ? 'cm-side-cursor' : '']
          .filter(Boolean)
          .join(' ');
        ranges.push(Decoration.line({ class: classes }).range(line.from));
      });
    }
    return Decoration.set(ranges, true);
  }

  function boxes(state: EditorState, calls: Calls): RangeSet<GutterMarker> {
    const info = state.field(infoField);
    const marks = [];
    for (const conflict of info.conflicts) {
      if (conflict.at < 0) continue;
      for (let i = 0; i < conflict.kept.length; i++) {
        marks.push(new Box(conflict.index, i, conflict.kept[i], conflict.edited, calls).range(lineOf(state, conflict.at + 1 + i).from));
      }
    }
    return RangeSet.of(marks, true);
  }

  const theme = EditorView.theme({
    '&': { height: '100%', fontSize: 'var(--fs-code)', backgroundColor: 'var(--bg)', color: 'var(--ink)' },
    '&.cm-focused': { outline: 'none' },
    '.cm-scroller': { fontFamily: 'var(--f-mono)', lineHeight: '18px' },
    '.cm-gutters': { backgroundColor: 'var(--bg)', color: 'var(--faint)', border: 'none', borderRight: '1px solid var(--line-soft)' },
    '.cm-picks': { minWidth: '22px' },
    '.cm-picks .cm-gutterElement': { display: 'flex', alignItems: 'center', justifyContent: 'center' },
    '.cm-picks input, .cm-conflict-head input': { margin: '0' },
    '.cm-picks input.faded, .cm-conflict-head input.faded': { opacity: '0.45' },
    '.cm-side-line': { backgroundColor: 'color-mix(in srgb, var(--c) 10%, transparent)' },
    '.cm-side-kept': { backgroundColor: 'color-mix(in srgb, var(--c) 24%, transparent)' },
    // The line of the keyboard: an inner ring, never around the whole pane (guidelines, 9.6).
    '.cm-side-cursor': { boxShadow: 'inset 0 0 0 1.5px var(--accent)' },
    '.cm-conflict-head': {
      display: 'flex',
      alignItems: 'center',
      height: '24px',
      padding: '0 6px',
      fontFamily: 'var(--f-ui)',
      fontSize: 'var(--fs-small)',
      color: 'var(--muted)',
      backgroundColor: 'var(--panel)',
      borderTop: '1px solid var(--line-soft)',
    },
    '.cm-conflict-head label': { display: 'flex', alignItems: 'center', gap: '6px', cursor: 'pointer' },
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
    conflicts,
    current,
    lines,
    onpick,
    ongroup,
  }: {
    /** Text of this side. */
    value: string;
    lineSeparator?: string;
    /** Name of the side: "Upstream", "Your commit"… */
    label: string;
    conflicts: SideConflict[];
    /** The current conflict. */
    current: number;
    /** The lines of each conflict on this side, to announce them. */
    lines: string[][];
    onpick: (block: number, line: number, shift: boolean) => void;
    ongroup: (block: number) => void;
  } = $props();

  let host = $state<HTMLDivElement>();
  let view: EditorView | undefined;
  let focused = $state(false);
  let cursor = $state<{ block: number; line: number } | null>(null);
  let said = $state('');
  // The widgets call through this object: it always holds the callbacks of the last render.
  const calls: Calls = {
    pick: (block, line, shift) => {
      cursor = { block, line };
      onpick(block, line, shift);
    },
    group: (block) => ongroup(block),
  };

  onMount(() => {
    const state = State.create({
      doc: value,
      extensions: [
        State.lineSeparator.of(lineSeparator),
        State.readOnly.of(true),
        EditorView.editable.of(false),
        EditorView.lineWrapping,
        infoField,
        gutter({ class: 'cm-picks', markers: (v) => boxes(v.state, calls) }),
        lineNumbers(),
        EditorView.decorations.compute([infoField], (s) => decorate(s, calls)),
        theme,
        EditorView.contentAttributes.of({ 'aria-hidden': 'true' }),
      ],
    });
    view = new EditorView({ state, parent: host! });
    return () => view?.destroy();
  });

  $effect(() => {
    const info: Info = { conflicts, current, cursor, focused };
    view?.dispatch({ effects: setInfo.of(info) });
  });

  /** Every line of the conflicts of this side, in order: what the arrows go through. */
  const stops = $derived(conflicts.filter((c) => c.at >= 0).flatMap((c) => c.kept.map((_, line) => ({ block: c.index, line }))));

  function announce(stop: { block: number; line: number }) {
    const conflict = conflicts.find((c) => c.index === stop.block);
    const text = lines[stop.block]?.[stop.line] ?? '';
    said = `Line ${stop.line + 1} of conflict ${stop.block + 1}, ${conflict?.kept[stop.line] ? 'kept' : 'not kept'}: ${text}`;
  }

  function moveTo(index: number, extend: boolean) {
    const stop = stops[Math.max(0, Math.min(stops.length - 1, index))];
    if (!stop) return;
    cursor = stop;
    scrollToStop(stop);
    // Shift with an arrow extends from the last line picked, in the same conflict.
    if (extend) onpick(stop.block, stop.line, true);
    announce(stop);
  }

  function onKey(e: KeyboardEvent) {
    if (!stops.length) return;
    const at = cursor ? stops.findIndex((s) => s.block === cursor!.block && s.line === cursor!.line) : -1;
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      moveTo(at < 0 ? 0 : at + (e.key === 'ArrowDown' ? 1 : -1), e.shiftKey && at >= 0);
    } else if (e.key === 'Home' || e.key === 'End') {
      e.preventDefault();
      moveTo(e.key === 'Home' ? 0 : stops.length - 1, false);
    } else if (e.key === ' ' && cursor) {
      e.preventDefault();
      onpick(cursor.block, cursor.line, false);
      queueMicrotask(() => cursor && announce(cursor));
    }
  }

  function onFocus() {
    focused = true;
    // The first line of the current conflict takes the ring, if none has it yet.
    if (!cursor && stops.length) {
      cursor = stops.find((s) => s.block === current) ?? stops[0];
      announce(cursor);
    }
  }

  function scrollToStop(stop: { block: number; line: number }) {
    const conflict = conflicts.find((c) => c.index === stop.block);
    if (conflict && conflict.at >= 0) scrollToLine(conflict.at + 1 + stop.line, 'nearest');
  }

  /** Brings line `line` (from 1) into view, to the middle by default. */
  export function scrollToLine(line: number, where: 'center' | 'nearest' = 'center') {
    if (!view) return;
    const target = view.state.doc.line(Math.max(1, Math.min(view.state.doc.lines, line)));
    view.dispatch({ effects: EditorView.scrollIntoView(target.from, { y: where }) });
  }

  /** The ring follows the current conflict when it changes from the bar. */
  export function follow(block: number) {
    const stop = stops.find((s) => s.block === block);
    if (stop) cursor = stop;
  }
</script>

<!-- The lines of the conflicts are a list for the keyboard: the arrows go through them, Space
     keeps one or lets it go (guidelines, 9.26). -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="pane"
  tabindex="0"
  role="group"
  aria-label="{label}: the arrows go through the lines of the conflicts, Space keeps a line or lets it go"
  onkeydown={onKey}
  onfocus={onFocus}
  onblur={() => (focused = false)}
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
