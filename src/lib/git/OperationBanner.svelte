<script lang="ts">
  import { slide } from 'svelte/transition';
  import Icon from '../ui/Icon.svelte';
  import { OPERATION, type Repo } from './repo.svelte';
  import { motion } from '../ui/motion';
  import { abortOperation } from './actions';

  let { repo }: { repo: Repo } = $props();

  const operation = $derived(repo.status?.operation ?? null);
  const conflicts = $derived(repo.conflicts.length);
</script>

{#if operation}
  <div class="banner" role="status" transition:slide={{ duration: motion.med }}>
    <span class="glyph"><Icon name="warning" size={18} /></span>
    <div class="text">
      <b>{OPERATION[operation]} in progress</b>
      <!-- “Show files” follows the sentence it acts on: a ghost button (guidelines, 9.1). -->
      <span class="line">
        <span>
          {#if conflicts}
            {conflicts} file{conflicts === 1 ? '' : 's'} in conflict: resolve {conflicts === 1 ? 'it' : 'them'} in the changes panel, then continue.
          {:else}
            All conflicts are resolved: you can continue.
          {/if}
        </span>
        <button class="btn ghost small" type="button" onclick={() => repo.showWip()}>Show files</button>
      </span>
    </div>
    <!-- The main button first, the way out last, as on the line of a panel (guidelines, 9.1). -->
    <button
      class="btn primary"
      type="button"
      disabled={repo.locked || conflicts > 0}
      title={conflicts > 0 ? 'Resolve the conflicts first' : undefined}
      onclick={() => void repo.operate('continue')}>
      <Icon name="play" size={14} /> Continue
    </button>
    {#if operation !== 'merge'}
      <button class="btn" type="button" disabled={repo.locked} onclick={() => void repo.operate('skip')} title="Leave out the current commit and move on to the next one">
        <Icon name="skip" size={14} /> Skip
      </button>
    {/if}
    <!-- Aborting loses the conflicts already resolved: a dialog confirms it (guidelines, 9.1, “Hierarchy”). -->
    <button class="btn ghost bad" type="button" disabled={repo.locked} onclick={() => void abortOperation(repo)} title="Go back to the state before the operation">
      <Icon name="stop" size={14} /> Abort…
    </button>
  </div>
{/if}

<style>
  .banner {
    flex: none;
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 0 8px 8px;
    padding: 8px 12px;
    border-radius: var(--radius);
    background: var(--warn-soft);
    min-width: 0;
  }
  .glyph {
    display: grid;
    color: var(--warn);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    line-height: 1.3;
  }
  .line {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    column-gap: 4px;
    min-width: 0;
  }
  .line > span {
    color: var(--muted);
    font-size: var(--fs-label);
  }
</style>
