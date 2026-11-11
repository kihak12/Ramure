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
      <b>{OPERATION[operation]} en cours</b>
      <span>
        {#if conflicts}
          {conflicts} fichier{conflicts > 1 ? 's' : ''} en conflit : résous-les dans le panneau des modifications, puis continue.
        {:else}
          Tous les conflits sont résolus : tu peux continuer.
        {/if}
      </span>
    </div>
    <button class="btn" type="button" onclick={() => repo.showWip()}>Voir les fichiers</button>
    <button class="btn primary" type="button" disabled={repo.locked || conflicts > 0} onclick={() => void repo.operate('continue')}>
      <Icon name="play" size={14} /> Continuer
    </button>
    {#if operation !== 'merge'}
      <button class="btn" type="button" disabled={repo.locked} onclick={() => void repo.operate('skip')} title="Laisse de côté le commit en cours et passe au suivant">
        <Icon name="skip" size={14} /> Passer
      </button>
    {/if}
    <!-- L'abandon perd les conflits déjà résolus : une boîte le confirme (charte, 9.1, « Hierarchy »). -->
    <button class="btn ghost bad" type="button" disabled={repo.locked} onclick={() => void abortOperation(repo)} title="Revient à l’état d’avant l’opération">
      <Icon name="stop" size={14} /> Abandonner…
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
  .text span {
    color: var(--muted);
    font-size: var(--fs-label);
  }
</style>
