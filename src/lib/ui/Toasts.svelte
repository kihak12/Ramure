<script lang="ts">
  import { flip } from 'svelte/animate';
  import { fly } from 'svelte/transition';
  import Icon from './Icon.svelte';
  import { toasts } from './toasts.svelte';

  const ICON = { ok: 'check', warn: 'warning', err: 'warning' } as const;
</script>

<div class="toasts" aria-live="polite">
  {#each toasts.items as toast (toast.id)}
    <div
      class="toast k-{toast.kind}"
      role={toast.kind === 'err' ? 'alert' : 'status'}
      animate:flip={{ duration: 220 }}
      in:fly={{ y: 24, duration: 260 }}
      out:fly={{ y: 16, duration: 160 }}
    >
      <span class="badge"><Icon name={ICON[toast.kind]} size={14} /></span>
      <div class="text">
        <b>{toast.title}</b>
        {#if toast.body}<pre class="mono">{toast.body}</pre>{/if}
      </div>
      <button class="close" type="button" aria-label="Fermer la notification" onclick={() => toasts.dismiss(toast.id)}>
        <Icon name="x" size={14} />
      </button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    /* En bas au centre : c'est l'endroit qui masque le moins de commandes. */
    position: fixed;
    left: 50%;
    bottom: 40px;
    transform: translateX(-50%);
    z-index: 60;
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: min(480px, calc(100vw - 96px));
    pointer-events: none;
  }
  .toast {
    pointer-events: auto;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto;
    gap: 10px;
    align-items: start;
    padding: 10px 10px 10px 12px;
    border-radius: var(--radius);
    border: 1px solid var(--line);
    border-left: 3px solid var(--c);
    background: var(--raised);
    box-shadow: var(--shadow-pop);
    --c: var(--ok);
  }
  .k-warn {
    --c: var(--warn);
  }
  .k-err {
    --c: var(--bad);
  }
  .badge {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    color: var(--c);
    background: color-mix(in srgb, var(--c) 16%, transparent);
  }
  .text {
    min-width: 0;
    padding-top: 2px;
  }
  pre {
    margin: 4px 0 0;
    font-size: 11.5px;
    color: var(--muted);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    max-height: 180px;
    overflow: auto;
  }
  .close {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border: 0;
    border-radius: 5px;
    background: none;
    color: var(--muted);
    cursor: pointer;
    transition: background var(--t-fast) var(--ease);
  }
  .close:hover {
    background: var(--hover);
    color: var(--ink);
  }
</style>
