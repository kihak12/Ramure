<script lang="ts">
  // The section "Files" of the settings (guidelines, 9.18): the programs that open the files of
  // the Files module (9.24, "Opening a file").
  import { invoke } from '@tauri-apps/api/core';
  import { tick } from 'svelte';
  import { open as pick } from '@tauri-apps/plugin-dialog';
  import { DEFAULTS, prefs, savePrefs, type OpenWith } from '../git/prefs.svelte';
  import { normalMatch } from '../files/opening';
  import Icon from '../ui/Icon.svelte';
  import { focusBack } from '../ui/focus';
  import SectionHeader from './SectionHeader.svelte';
  import SettingRow from './SettingRow.svelte';

  const changed = $derived(prefs.textEditor !== DEFAULTS.textEditor || prefs.openWith.length > 0);

  const found = (program: string) => invoke<boolean>('program_found', { program });
  const missing = (program: string) => `No program named ${program} was found.`;
  /** The picker of the system; its failure is said under the control (guidelines, 9.18). */
  async function choose(fail: (why: string) => void): Promise<string | null> {
    try {
      const chosen = await pick({ title: 'Choose a program', multiple: false, directory: false });
      return typeof chosen === 'string' ? chosen : null;
    } catch (e) {
      fail(`The file picker could not open. Type the name or the path of the program instead. (${e instanceof Error ? e.message : String(e)})`);
      return null;
    }
  }

  // --- The text editor: typed or chosen, applied once it is found, empty for the system's ---------
  let editor = $state(prefs.textEditor);
  let editorError = $state('');
  async function setEditor(value: string) {
    editor = value.trim();
    editorError = '';
    if (editor && !(await found(editor))) {
      editorError = missing(editor);
      return;
    }
    savePrefs({ textEditor: editor });
  }

  // --- The exceptions: a row is applied once its two fields are valid (guidelines, 9.18) -----------
  type Row = OpenWith & { id: number; matchError: string; programError: string };
  let next = 0;
  const toRow = (rule: OpenWith): Row => ({ ...rule, id: next++, matchError: '', programError: '' });
  let rows = $state<Row[]>(prefs.openWith.map(toRow));
  /** "conf" and ".conf" are the same exception. */
  const key = (match: string) => normalMatch(match).replace(/^\./, '');

  async function check(row: Row) {
    row.match = row.match.trim();
    row.program = row.program.trim();
    row.matchError = row.match && rows.some((other) => other !== row && key(other.match) === key(row.match)) ? 'Already listed.' : '';
    row.programError = row.program && !(await found(row.program)) ? missing(row.program) : '';
    save();
  }

  /** The rows that are whole and valid, in their order. */
  function save() {
    savePrefs({ openWith: rows.filter((r) => r.match && r.program && !r.matchError && !r.programError).map(({ match, program }) => ({ match, program })) });
  }

  let list = $state<HTMLElement>();
  let addButton = $state<HTMLButtonElement>();
  /** The first field of row `index`, once drawn. */
  const fieldOf = (index: number) => [...(list?.querySelectorAll<HTMLElement>('.rule:not(.head)') ?? [])][index]?.querySelector('input');

  function add() {
    rows.push(toRow({ match: '', program: '' }));
    // The new row takes the keyboard on its first field (guidelines, 9.18).
    void tick().then(() => focusBack(fieldOf(rows.length - 1)));
  }

  /** "Remove": the keyboard goes to the row that takes its place, or the one before, or "Add exception". */
  function remove(row: Row) {
    const at = rows.indexOf(row);
    rows = rows.filter((r) => r !== row);
    save();
    void tick().then(() => focusBack(fieldOf(Math.min(at, rows.length - 1)) ?? addButton));
  }

  async function browseFor(row: Row) {
    const chosen = await choose((why) => (row.programError = why));
    if (chosen === null) return;
    row.program = chosen;
    await check(row);
  }
</script>

<SectionHeader
  title="Files"
  {changed}
  onreset={() => {
    const before = { textEditor: prefs.textEditor, openWith: prefs.openWith };
    savePrefs({ textEditor: DEFAULTS.textEditor, openWith: [] });
    editor = '';
    editorError = '';
    rows = [];
    return () => {
      savePrefs(before);
      editor = before.textEditor;
      rows = before.openWith.map(toRow);
    };
  }}
/>

<section class="group" aria-labelledby="group-opening">
  <h3 id="group-opening">Opening files</h3>
  <SettingRow id="set-editor" name="Text editor" help="Opens a text file that has no exception below. Empty: the program of the system." error={editorError}>
    <div class="program">
      <input
        id="set-editor"
        class="field mono"
        class:invalid={!!editorError}
        type="text"
        spellcheck="false"
        autocomplete="off"
        placeholder="The system’s"
        aria-invalid={editorError ? 'true' : undefined}
        aria-describedby={editorError ? 'set-editor-help set-editor-error' : 'set-editor-help'}
        bind:value={editor}
        onchange={() => void setEditor(editor)}
      />
      <div class="actions">
        <button
          class="btn small"
          type="button"
          onclick={async () => {
            const chosen = await choose((why) => (editorError = why));
            if (chosen !== null) await setEditor(chosen);
          }}><Icon name="folder" size={14} /> Browse…</button
        >
        <button
          class="btn ghost small"
          type="button"
          disabled={!editor}
          onclick={async () => {
            // The system's chosen, the button is disabled: the keyboard goes to the field (11).
            const field = document.getElementById('set-editor');
            await setEditor('');
            await tick();
            if (field) focusBack(field);
          }}>Use the system’s</button
        >
      </div>
    </div>
  </SettingRow>

  <SettingRow
    id="set-open-with"
    name="Programs by extension"
    help="A file with one of these extensions or names opens in its program, instead of the text editor or the program of the system. The first that matches is used."
    group
  >
    <div class="rules" bind:this={list}>
      <!-- The labels of the two fields of a row, once there is a row (guidelines, 9.3 and 9.18). -->
      {#if rows.length}
        <div class="rule head" aria-hidden="true">
          <span class="label">Extension or name</span>
          <span></span>
          <span class="label">Program</span>
        </div>
      {/if}
      {#each rows as row (row.id)}
        <div class="rule">
          <label class="cell">
            <span class="spoken">Extension or name</span>
            <input class="field mono" class:invalid={!!row.matchError} type="text" spellcheck="false" autocomplete="off" placeholder=".conf" bind:value={row.match} onchange={() => void check(row)} />
            {#if row.matchError}<small class="error" role="alert">{row.matchError}</small>{/if}
          </label>
          <span class="to" aria-hidden="true"><Icon name="chevron" size={14} /></span>
          <label class="cell">
            <span class="spoken">Program</span>
            <input class="field mono" class:invalid={!!row.programError} type="text" spellcheck="false" autocomplete="off" placeholder="code" bind:value={row.program} onchange={() => void check(row)} />
            {#if row.programError}<small class="error" role="alert">{row.programError}</small>{/if}
          </label>
          <button class="btn small" type="button" onclick={() => void browseFor(row)}><Icon name="folder" size={14} /> Browse…</button>
          <button class="btn ghost small icon" type="button" aria-label={row.match ? `Remove the exception ${row.match}` : 'Remove this exception'} title="Remove" onclick={() => remove(row)}><Icon name="trash" size={14} /></button>
        </div>
      {:else}
        <p class="empty">No exceptions. Every file opens in the text editor or the program of the system.</p>
      {/each}
      <div><button class="btn small" type="button" bind:this={addButton} onclick={add}><Icon name="plus" size={14} /> Add exception</button></div>
    </div>
  </SettingRow>
</section>

<style>
  .program {
    display: grid;
    gap: 6px;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .rules {
    display: grid;
    gap: 6px;
  }
  /* A row: the extension or the name, the program, its "Browse…" and "Remove". */
  .rule {
    display: grid;
    grid-template-columns: 120px 14px minmax(0, 1fr) auto auto;
    align-items: start;
    gap: 6px;
  }
  /* The labels of the columns, in 12 px, weight 650, --muted, as a field's (guidelines, 9.3). */
  .head .label {
    font-size: var(--fs-label);
    font-weight: var(--fw-strong);
    color: var(--muted);
  }
  .cell {
    display: grid;
    gap: 4px;
    min-width: 0;
  }
  .to {
    display: grid;
    height: 32px;
    place-items: center;
    color: var(--muted);
  }
  .rule .btn {
    margin-top: 4px;
  }
  /* "Remove", an icon alone: 24 × 24 px at least (guidelines, 9.1). */
  .btn.icon {
    width: 24px;
    padding: 0;
    color: var(--muted);
  }
  .btn.icon:hover:not(:disabled) {
    color: var(--ink);
  }
  .error {
    color: var(--bad);
    font-size: var(--fs-small);
  }
  .empty {
    margin: 0;
    color: var(--muted);
    font-size: var(--fs-small);
  }
</style>
