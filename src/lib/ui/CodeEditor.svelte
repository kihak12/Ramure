<script lang="ts" module>
  import { StateEffect, StateField, type EditorState } from '@codemirror/state';
  import { Decoration, type DecorationSet, EditorView } from '@codemirror/view';

  /** Lignes à mettre en évidence : de `from` à `to` inclus, numérotées à partir de 1. */
  export interface Mark {
    from: number;
    to: number;
    /** Donne la classe `cm-mark-<kind>`. */
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

  // Les couleurs viennent des jetons de l'application : le thème clair ou sombre suit tout seul.
  const theme = EditorView.theme({
    '&': { height: '100%', fontSize: '12px', backgroundColor: 'var(--bg)', color: 'var(--ink)' },
    '&.cm-focused': { outline: 'none' },
    '.cm-scroller': { fontFamily: 'var(--f-mono)', lineHeight: '18px' },
    '.cm-content': { caretColor: 'var(--ink)' },
    '.cm-cursor': { borderLeftColor: 'var(--ink)' },
    '.cm-gutters': {
      backgroundColor: 'var(--bg)',
      color: 'var(--faint)',
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
    '.cm-mark-marker': { backgroundColor: 'var(--warn-soft)', color: 'var(--warn)', fontWeight: '600' },
    '.cm-mark-focus': {
      backgroundColor: 'color-mix(in srgb, var(--c, var(--accent)) 22%, transparent)',
      boxShadow: 'inset 3px 0 0 var(--c, var(--accent))',
    },
  });
</script>

<script lang="ts">
  import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
  import { EditorState as State } from '@codemirror/state';
  import { drawSelection, highlightActiveLine, highlightActiveLineGutter, keymap, lineNumbers } from '@codemirror/view';
  import { onMount } from 'svelte';

  let {
    value,
    readonly = false,
    marks = [],
    lineSeparator = '\n',
    label,
    onchange,
  }: {
    /** Texte initial. Le changer remplace tout le document. */
    value: string;
    readonly?: boolean;
    marks?: Mark[];
    /** Fin de ligne du fichier : `\r\n` est conservé tel quel quand le fichier l'utilise. */
    lineSeparator?: string;
    label: string;
    onchange?: (text: string) => void;
  } = $props();

  let host = $state<HTMLDivElement>();
  let view: EditorView | undefined;
  let loaded = '';

  function makeState(text: string) {
    return State.create({
      doc: text,
      extensions: [
        State.lineSeparator.of(lineSeparator),
        lineNumbers(),
        drawSelection(),
        markField,
        theme,
        EditorView.contentAttributes.of({ 'aria-label': label }),
        ...(readonly
          ? [State.readOnly.of(true), EditorView.editable.of(false)]
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
    view = new EditorView({ state: makeState(value), parent: host! });
    view.dispatch({ effects: setMarks.of(marks) });
    return () => view?.destroy();
  });

  // Un autre fichier : on repart d'un document neuf (et d'un historique d'annulation vide).
  $effect(() => {
    if (view && value !== loaded) {
      loaded = value;
      view.setState(makeState(value));
    }
  });

  $effect(() => {
    const next = marks;
    view?.dispatch({ effects: setMarks.of(next) });
  });

  /** Texte courant, avec les fins de ligne du fichier. */
  export function getText() {
    return view ? view.state.sliceDoc() : value;
  }

  /** Remplace les lignes `from` à `to` (incluses, à partir de 1) par `lines` ; une liste vide les supprime. */
  export function replaceLines(from: number, to: number, lines: string[]) {
    if (!view) return;
    const doc = view.state.doc;
    const first = doc.line(Math.max(1, from));
    const last = doc.line(Math.min(doc.lines, to));
    if (lines.length) {
      view.dispatch({ changes: { from: first.from, to: last.to, insert: lines.join(view.state.lineBreak) } });
    } else if (last.to < doc.length) {
      view.dispatch({ changes: { from: first.from, to: last.to + 1 } });
    } else {
      view.dispatch({ changes: { from: Math.max(0, first.from - 1), to: last.to } });
    }
  }

  /** Amène la ligne `line` (à partir de 1) au milieu de la vue. */
  export function scrollToLine(line: number) {
    if (!view) return;
    const target = view.state.doc.line(Math.max(1, Math.min(view.state.doc.lines, line)));
    view.dispatch({ effects: EditorView.scrollIntoView(target.from, { y: 'center' }) });
  }
</script>

<div class="editor" bind:this={host}></div>

<style>
  .editor {
    height: 100%;
    min-height: 0;
    overflow: hidden;
  }
</style>
