<script lang="ts">
  import { flip } from 'svelte/animate';
  import { fly } from 'svelte/transition';
  import Icon from './Icon.svelte';
  import { toasts } from './toasts.svelte';
  import { motion } from './motion';

  const ICON = { ok: 'check', warn: 'warning', err: 'warning' } as const;

  // Dans un titre, un mot trop long (un nom de branche, un chemin) est coupé en son milieu : son
  // début et sa fin restent lisibles. Le titre complet est en infobulle, et lu par les lecteurs d'écran.
  const LONG = 36;
  const HEAD = 24;
  const TAIL = 10;
  function clip(title: string) {
    return title
      .split(' ')
      .map((word) => (word.length > LONG ? `${word.slice(0, HEAD)}…${word.slice(-TAIL)}` : word))
      .join(' ');
  }

  /** Donne le focus au bouton d'action dès qu'il apparaît, si on le demande. */
  function focused(node: HTMLButtonElement, wanted: boolean) {
    if (wanted) node.focus();
  }
</script>

<div class="toasts" aria-live="polite">
  {#each toasts.items as toast (toast.id)}
    {@const title = clip(toast.title)}
    <div
      class="toast k-{toast.kind}"
      role={toast.kind === 'err' ? 'alert' : 'status'}
      onpointerenter={() => toasts.hold(toast.id)}
      onpointerleave={() => toasts.release(toast.id)}
      onfocusin={() => toasts.hold(toast.id)}
      onfocusout={() => toasts.release(toast.id)}
      animate:flip={{ duration: motion.med }}
      in:fly={{ y: 24, duration: motion.med }}
      out:fly={{ y: 16, duration: motion.fast }}
    >
      <span class="badge"><Icon name={ICON[toast.kind]} size={14} /></span>
      <div class="text">
        {#if title === toast.title}
          <b>{title}</b>
        {:else}
          <b title={toast.title}><span aria-hidden="true">{title}</span><span class="spoken">{toast.title}</span></b>
        {/if}
        {#if toast.body}<pre class="mono">{toast.body}</pre>{/if}
      </div>
      {#if toast.action}
        {@const action = toast.action}
        <button
          class="btn small action"
          type="button"
          use:focused={action.focus === true}
          onclick={() => {
            toasts.dismiss(toast.id);
            action.run();
          }}
        >
          {action.label}
        </button>
      {/if}
      <button class="close" type="button" aria-label="Fermer la notification" title="Fermer la notification" onclick={() => toasts.dismiss(toast.id)}>
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
    grid-auto-flow: column;
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
  .text b {
    font-weight: var(--fw-strong);
  }
  /* Hors de l'écran, mais lu : le titre entier, quand celui qui est affiché est coupé. */
  pre {
    margin: 4px 0 0;
    font-size: var(--fs-small);
    color: var(--muted);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    max-height: 180px;
    overflow: auto;
  }
  .action {
    min-height: 24px;
  }
  .close {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    cursor: pointer;
    transition:
      background var(--t-fast) var(--ease),
      transform var(--t-fast) var(--ease);
  }
  /* À l'appui, un bouton se resserre à 97 % (charte, sections 8 et 9.1). */
  .close:active {
    transform: scale(0.97);
  }
  .close:hover {
    background: var(--hover);
    color: var(--ink);
  }
</style>
