<script lang="ts">
  import type { DiffMode } from '../git/prefs.svelte';

  /** Preview of a diff layout: one column, or the old and the new side by side. */
  let { mode }: { mode: DiffMode } = $props();
</script>

<span class="diff" class:split={mode === 'split'}>
  {#if mode === 'split'}
    <span class="pane"><i></i><i class="minus"></i><i class="minus"></i><i></i></span>
    <span class="pane"><i></i><i class="plus"></i><i class="plus"></i><i></i></span>
  {:else}
    <span class="pane"><i></i><i class="minus"></i><i class="plus"></i><i class="plus"></i><i></i></span>
  {/if}
</span>

<style>
  .diff {
    display: flex;
    gap: 4px;
    height: 100%;
    padding: 6px;
    background: var(--panel);
  }
  .pane {
    flex: 1;
    min-width: 0;
    display: grid;
    align-content: center;
    gap: 4px;
  }
  i {
    height: 6px;
    border-radius: var(--radius-xs);
    background: var(--line-soft);
  }
  .minus {
    background: var(--bad-soft);
  }
  .plus {
    background: var(--ok-soft);
  }
</style>
