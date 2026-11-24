<script lang="ts" module>
  import { StateEffect, StateField, type EditorState } from '@codemirror/state';
  import { Decoration, type DecorationSet, EditorView } from '@codemirror/view';

  /** Lines to highlight: from `from` to `to` included, numbered from 1. */
  export interface Mark {
    from: number;
    to: number;
    /** Gives the class `cm-mark-<kind>`. */
    kind: 'ours' | 'theirs' | 'base' | 'marker' | 'focus';
  }

  const setMarks = StateEffect.define<Mark[]>();

  function build(state: EditorState, marks: Mark[]): DecorationSet {
    const ranges = [];
    const sorted = [...marks].sort((a, b) => a.from - b.from);
    for (const mark of sorted) {
      const last = Math.min(mark.to, state.doc.lines);
      for (let n = Math.max(1, mark.from); n <= last; n++) {
        ranges.push(Decoration.line({ class: `cm-mark-${mark.kind}` }).range(state.doc.line(n).from));
      }
    }
    return Decoration.set(ranges, true);
  }

  const markField = StateField.define<DecorationSet>({
    create: () => Decoration.none,
    update(decorations, transaction) {
      let next = decorations.map(transaction.changes);
      for (const change of transaction.effects) {
        if (change.is(setMarks)) next = build(transaction.state, change.value);
      }
      return next;
    },
    provide: (field) => EditorView.decorations.from(field),
  });

  // The colors come from the tokens of the application: the light or dark theme follows on its own.
  const theme = EditorView.theme({
    '&': { height: '100%', fontSize: 'var(--fs-code)', backgroundColor: 'var(--bg)', color: 'var(--ink)' },
    '&.cm-focused': { outline: 'none' },
    '.cm-scroller': { fontFamily: 'var(--f-mono)', lineHeight: '18px' },
    '.cm-content': { caretColor: 'var(--ink)' },
    '.cm-cursor': { borderLeftColor: 'var(--ink)' },
    '.cm-gutters': {
      backgroundColor: 'var(--bg)',
      color: 'var(--muted)',
      border: 'none',
      borderRight: '1px solid var(--line-soft)',
    },
    '.cm-activeLine': { backgroundColor: 'color-mix(in srgb, var(--accent) 6%, transparent)' },
    '.cm-activeLineGutter': { backgroundColor: 'transparent', color: 'var(--ink)' },
    '.cm-selectionBackground, &.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground': {
      backgroundColor: 'color-mix(in srgb, var(--accent) 30%, transparent)',
    },
    '.cm-mark-ours': { backgroundColor: 'color-mix(in srgb, var(--l0) 18%, transparent)' },
    '.cm-mark-theirs': { backgroundColor: 'color-mix(in srgb, var(--l1) 18%, transparent)' },
    '.cm-mark-base': { backgroundColor: 'color-mix(in srgb, var(--muted) 14%, transparent)' },
    '.cm-mark-marker': { backgroundColor: 'var(--warn-soft)', color: 'var(--warn)', fontWeight: 'var(--fw-strong)' },
    // The current conflict reads by its stronger tint, with no colored bar on the edge of the line.
    '.cm-mark-focus': { backgroundColor: 'color-mix(in srgb, var(--c, var(--accent)) 30%, transparent)' },
  });
</script>

<script lang="ts">
  import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
  import { EditorState as State } from '@codemirror/state';
  import { drawSelection, highlightActiveLine, highlightActiveLineGutter, keymap, lineNumbers } from '@codemirror/view';
  import { onMount } from 'svelte';
  import { byKeyboard } from './focus';

  let {
    value,
    readonly = false,
    marks = [],
    lineSeparator = '\n',
    firstLine = 1,
    label,
    onchange,
  }: {
    /** Initial text. Changing it replaces the whole document. */
    value: string;
    readonly?: boolean;
    marks?: Mark[];
    /** Line ending of the file: `\r\n` is kept as it is when the file uses it. */
    lineSeparator?: string;
    /** The number of the first line: a part of a file keeps the numbers of the file. */
    firstLine?: number;
    label: string;
    onchange?: (text: string) => void;
  } = $props();

  let host = $state<HTMLDivElement>();
  /** Read only, the area is a stop of the tab order: its ring shows after a key, not after a click
   *  (guidelines, 9.6 and 11). WebKit would show it after a click too. */
  let ring = $state(false);
  const MODIFIERS = new Set(['Shift', 'Control', 'Alt', 'Meta', 'AltGraph']);
  let view: EditorView | undefined;
  let loaded = '';
  let loadedFirst = 1;

  function makeState(text: string) {
    return State.create({
      doc: text,
      extensions: [
        State.lineSeparator.of(lineSeparator),
        lineNumbers({ formatNumber: (n) => String(n + firstLine - 1) }),
        // Read only, it has no caret to draw.
        ...(readonly ? [] : [drawSelection()]),
        markField,
        theme,
        EditorView.contentAttributes.of({ 'aria-label': label }),
        ...(readonly
          ? // Read only, it is still a stop of the tab order: its arrows scroll it (guidelines, 11).
            [State.readOnly.of(true), EditorView.editable.of(false), EditorView.contentAttributes.of({ tabindex: '0' })]
          : [
              history(),
              highlightActiveLine(),
              highlightActiveLineGutter(),
              keymap.of([...defaultKeymap, ...historyKeymap]),
              EditorView.updateListener.of((update) => {
                if (update.docChanged) onchange?.(update.state.sliceDoc());
              }),
            ]),
      ],
    });
  }

  onMount(() => {
    loaded = value;
    loadedFirst = firstLine;
    view = new EditorView({ state: makeState(value), parent: host! });
    view.dispatch({ effects: setMarks.of(marks) });
    return () => view?.destroy();
  });

  // Another file, or another part of it: start again from a new document (and an empty undo history).
  $effect(() => {
    if (view && (value !== loaded || firstLine !== loadedFirst)) {
      loaded = value;
      loadedFirst = firstLine;
      view.setState(makeState(value));
      view.dispatch({ effects: setMarks.of(marks) });
    }
  });

  $effect(() => {
    const next = marks;
    view?.dispatch({ effects: setMarks.of(next) });
  });

  /** Current text, with the line endings of the file. */
  export function getText() {
    return view ? view.state.sliceDoc() : value;
  }
</script>

<div
  class="editor"
  class:ring={ring && readonly}
  bind:this={host}
  onfocusin={() => (ring = byKeyboard())}
  onfocusout={() => (ring = false)}
  onpointerdown={() => (ring = false)}
  onkeydown={(e) => {
    if (!MODIFIERS.has(e.key)) ring = true;
  }}
  role="presentation"
></div>

<style>
  .editor {
    position: relative;
    height: 100%;
    min-height: 0;
    overflow: hidden;
  }
  /* The layers of the editor stay inside it, under the ring. */
  .editor :global(.cm-editor) {
    isolation: isolate;
  }
  /* The ring of the keyboard, over the area, which clips: the 2 px of section 9, inside
     (guidelines, 11). */
  .editor.ring::after {
    content: '';
    position: absolute;
    inset: 0;
    border: 2px solid var(--accent);
    pointer-events: none;
  }
</style>
