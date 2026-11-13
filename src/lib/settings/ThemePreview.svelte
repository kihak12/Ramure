<script lang="ts">
  import type { Palette, ThemeChoice } from '../git/prefs.svelte';

  /**
   * Preview of a theme: a small window, drawn with the tokens of that color theme, in light or in
   * dark. "System" shows both halves.
   */
  let { choice, palette }: { choice: ThemeChoice; palette: Palette } = $props();
  const themes = $derived(choice === 'system' ? (['light', 'dark'] as const) : [choice]);
</script>

<span class="preview">
  {#each themes as theme (theme)}
    <span class="window" data-theme={theme} data-palette={palette}>
      <span class="rail"><i></i><i></i></span>
      <span class="card side"><i></i><i></i><i></i></span>
      <span class="card main"><i class="a"></i><i></i><i class="b"></i><i></i></span>
    </span>
  {/each}
</span>

<style>
  .preview {
    display: flex;
    height: 100%;
  }
  .window {
    flex: 1;
    min-width: 0;
    display: flex;
    gap: 4px;
    padding: 6px 6px 6px 0;
    background: var(--side);
  }
  .rail {
    flex: none;
    display: grid;
    align-content: start;
    gap: 4px;
    width: 12px;
    padding-left: 4px;
  }
  .rail i {
    height: 4px;
    border-radius: var(--radius-xs);
    background: var(--faint);
  }
  .rail i:first-child {
    background: var(--accent);
  }
  .card {
    display: grid;
    align-content: start;
    gap: 4px;
    padding: 6px;
    border-radius: var(--radius-xs);
    background: var(--panel);
  }
  .side {
    flex: 1;
  }
  .main {
    flex: 3;
  }
  .card i {
    height: 4px;
    border-radius: var(--radius-xs);
    background: var(--line);
  }
  .side i:nth-child(2) {
    background: var(--sel);
  }
  .main i:nth-child(even) {
    width: 60%;
  }
  /* One line in the accent, one in text: what the theme changes most. */
  .main .a {
    background: var(--accent);
  }
  .main .b {
    background: var(--muted);
  }
</style>
