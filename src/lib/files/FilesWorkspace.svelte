<script lang="ts">
  // The Files module: a row of panes, and under them the transfers (guidelines, 9.24).
  import { onMount } from 'svelte';
  import { files } from './files.svelte';
  import FilePane from './FilePane.svelte';
  import Transfers from './Transfers.svelte';

  onMount(() => files.start());
</script>

<div class="files">
  <div class="panes">
    {#each files.panes as pane (pane.key)}
      <FilePane {pane} />
    {/each}
  </div>
  {#if files.transfers.length}<Transfers />{/if}
</div>

<style>
  .files {
    display: flex;
    flex-direction: column;
    gap: 8px;
    height: 100%;
    min-height: 0;
    padding: 8px;
    background: var(--side);
  }
  /* The panes share the width, 300 px at the least; past the room, the row scrolls sideways. */
  .panes {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: minmax(300px, 1fr);
    gap: 8px;
    overflow-x: auto;
    overflow-y: hidden;
  }
</style>
