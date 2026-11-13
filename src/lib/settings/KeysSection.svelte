<script lang="ts">
  import Icon from '../ui/Icon.svelte';
  import { comboOf, display, keys, refusal, SHORTCUTS } from '../ui/keys.svelte';
  import SectionHeader from './SectionHeader.svelte';
  import { settingsPage } from './settings.svelte';

  // The action whose new combination is awaited.
  let recording = $state<string | null>(null);
  /** What to say under a row: a combination refused, or taken from another action. */
  let notice = $state<{ id: string; text: string } | null>(null);
  let filter = $state('');

  // A shortcut that acts in one module only says so beside its group (guidelines, section 9.18).
  const MODULES = { git: 'Git', ssh: 'SSH' } as const;

  const custom = $derived(Object.keys(keys.overrides).length > 0);
  const fold = (text: string) => text.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase();
  const needle = $derived(fold(filter.trim()));
  // The filter searches the name, the group and the combination as it is shown.
  const shown = $derived(
    SHORTCUTS.filter((s) => !needle || fold(`${s.title} ${s.group} ${display(keys.binding(s.id))}`).includes(needle)),
  );

  // The shortcuts of the application stay silent while a new one is typed.
  $effect(() => {
    keys.recording = recording !== null;
    return () => (keys.recording = false);
  });
  // Leaving the page cancels the recording in progress.
  $effect(() => {
    if (!settingsPage.open) recording = null;
  });

  function start(id: string) {
    recording = id;
    notice = null;
  }

  function onKey(event: KeyboardEvent) {
    if (!recording) return;
    event.preventDefault();
    event.stopPropagation();
    const id = recording;
    if (event.key === 'Escape') {
      recording = null;
      notice = null;
      return;
    }
    if (event.key === 'Backspace' || event.key === 'Delete') {
      keys.set(id, '');
      recording = null;
      notice = null;
      return;
    }
    const combo = comboOf(event);
    if (!combo) return; // a modifier key alone: wait for the rest
    const problem = refusal(combo);
    if (problem) {
      notice = { id, text: `${display(combo)}: ${problem}` };
      return;
    }
    const taken = keys.rival(id, combo);
    keys.set(id, combo);
    notice = taken ? { id, text: `${display(combo)} was the shortcut of “${taken.title}”, which now has none.` } : null;
    recording = null;
  }

  function onFilterKey(event: KeyboardEvent) {
    // Escape empties the filter first; the page closes only on the next press.
    if (event.key === 'Escape' && filter) {
      event.stopPropagation();
      filter = '';
    }
  }
</script>

<!-- In the capture phase: the combination being recorded must trigger no other action. -->
<svelte:window onkeydowncapture={onKey} />

<SectionHeader
  title="Keyboard shortcuts"
  note="Click a shortcut, then press the new one. Backspace removes it, Escape cancels."
  changed={custom}
  onreset={() => {
    const before = { ...keys.overrides };
    keys.resetAll();
    notice = null;
    return () => keys.restore(before);
  }}
/>

<label class="search">
  <Icon name="search" size={14} />
  <input type="search" placeholder="Filter shortcuts" aria-label="Filter shortcuts" bind:value={filter} onkeydown={onFilterKey} />
</label>

<div class="list">
  {#each shown as shortcut, i (shortcut.id)}
    {#if i === 0 || shown[i - 1].group !== shortcut.group}
      <h3>{shortcut.group}{#if shortcut.scope}{' '}<span class="scope">· {MODULES[shortcut.scope]} module</span>{/if}</h3>
    {/if}
    {@const combo = keys.binding(shortcut.id)}
    <div class="row" class:recording={recording === shortcut.id}>
      <span class="title">{shortcut.title}</span>
      <button
        class="combo"
        type="button"
        aria-label={recording === shortcut.id
          ? `Shortcut for “${shortcut.title}”: waiting for the new shortcut.`
          : `Shortcut for “${shortcut.title}”: ${combo ? display(combo) : 'none'}. Click to change it.`}
        onclick={() => start(shortcut.id)}
      >
        {#if recording === shortcut.id}
          <span class="wait">Press the shortcut…</span>
        {:else if combo}
          {#each combo.split('+') as part, n (n)}<kbd>{display(part)}</kbd>{/each}
        {:else}
          <span class="none">none</span>
        {/if}
      </button>
      <button
        class="reset"
        type="button"
        title="Restore default shortcut"
        aria-label="Restore default shortcut for “{shortcut.title}”"
        disabled={!keys.isCustom(shortcut.id)}
        onclick={() => {
          keys.reset(shortcut.id);
          notice = null;
        }}
      >
        <Icon name="undo" size={14} />
      </button>
    </div>
    {#if notice?.id === shortcut.id}
      <p class="notice" role="status"><Icon name="warning" /> <span>{notice.text}</span></p>
    {/if}
  {:else}
    <p class="empty">No shortcut matches “{filter.trim()}”.</p>
  {/each}
</div>

<p class="foot">In a terminal, only the shortcuts with Shift, Alt or a function key act: the others go to the shell.</p>

<style>
  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 9px;
    border: 1px solid var(--line);
    border-radius: var(--radius-s);
    background: var(--bg);
    color: var(--muted);
    transition:
      border-color var(--t-fast) var(--ease),
      box-shadow var(--t-fast) var(--ease);
  }
  .search:hover:not(:focus-within) {
    border-color: color-mix(in srgb, var(--muted) 50%, var(--line));
  }
  .search:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 25%, transparent);
  }
  .search input {
    flex: 1;
    min-width: 0;
    padding: 6px 0;
    border: 0;
    background: none;
    outline: none;
    color: var(--ink);
  }
  .list {
    display: grid;
    gap: 2px;
  }
  h3 {
    margin: 0;
    padding: 12px 6px 4px;
    font-size: var(--fs-label);
    font-weight: var(--fw-strong);
    color: var(--muted);
  }
  .scope {
    font-weight: var(--fw-regular);
  }
  h3:first-child {
    padding-top: 0;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 2px 6px;
    border-radius: var(--radius);
    transition: background var(--t-fast) var(--ease);
  }
  .row:hover {
    background: var(--hover);
  }
  .row.recording {
    background: var(--accent-soft);
  }
  .title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .combo {
    display: inline-flex;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;
    min-width: 150px;
    min-height: 26px;
    padding: 2px 6px;
    border: 1px solid transparent;
    border-radius: var(--radius-s);
    background: none;
    cursor: pointer;
    transition: border-color var(--t-fast) var(--ease);
  }
  .combo:hover {
    border-color: var(--line);
    background: var(--raised);
  }
  /* The shared key (app.css), a little higher, and in the color of the text: here it is read and changed. */
  kbd {
    min-width: 0;
    padding: 0 6px;
    color: inherit;
    line-height: 18px;
    text-align: start;
  }
  .none {
    color: var(--muted);
    font-size: var(--fs-label);
  }
  /* In --ink: on --accent-soft, the accent does not have the contrast of body text in the light theme. */
  .wait {
    font-size: var(--fs-label);
    font-weight: var(--fw-strong);
  }
  .reset {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    cursor: pointer;
  }
  .reset:hover:not(:disabled) {
    background: var(--line);
    color: var(--ink);
  }
  .reset:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .notice {
    display: flex;
    gap: 8px;
    margin: 2px 0 6px;
    padding: 8px 10px;
    border-radius: var(--radius-s);
    background: var(--warn-soft);
    color: var(--warn);
  }
  .notice span {
    color: var(--ink);
  }
  .empty,
  .foot {
    margin: 0;
    color: var(--muted);
    font-size: var(--fs-small);
  }
  .empty {
    padding: 12px 6px;
    font-size: var(--fs-body);
  }
</style>
