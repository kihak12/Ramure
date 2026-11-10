<script lang="ts">
  import { flip } from 'svelte/animate';
  import { fly } from 'svelte/transition';
  import Icon from './Icon.svelte';
  import { toasts } from './toasts.svelte';
  import { motion } from './motion';

  const ICON = { ok: 'check', warn: 'warning', err: 'warning' } as const;

  // Dans un titre, un nom trop long (une branche, un chemin) est coupé en son milieu : son début
  // et sa fin restent lisibles, et le titre tient sur une ligne. Le titre complet est en infobulle.
  const LONG = 36;
  const HEAD = 24;
  const TAIL = 10;
  function clip(title: string) {
    return title
      .split(' ')
      .map((word) => (word.length > LONG ? `${word.slice(0, HEAD)}…${word.slice(-TAIL)}` : word))
      .join(' ');
  }
</script>

<div class="toasts" aria-live="polite">
  {#each toasts.items as toast (toast.id)}
    {@const title = clip(toast.title)}
    <div
      class="toast k-{toast.kind}"
      role={toast.kind === 'err' ? 'alert' : 'status'}
      animate:flip={{ duration: motion.med }}
      in:fly={{ y: 24, duration: motion.med }}
      out:fly={{ y: 16, duration: motion.fast }}
    >
      <span class="badge"><Icon name={ICON[toast.kind]} size={14} /></span>
      <div class="text">
        <b title={title === toast.title ? undefined : toast.title}>{title}</b>
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
    /* L'état est porté par l'icône, pas par un trait de couleur sur le bord. */
    border: 1px solid var(--line);
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
    font-size: var(--fs-small);
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
    border-radius: var(--radius-s);
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
