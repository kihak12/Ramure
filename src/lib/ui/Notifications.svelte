<script lang="ts">
  import { flip } from 'svelte/animate';
  import { fly } from 'svelte/transition';
  import Icon from './Icon.svelte';
  import { toasts } from './toasts.svelte';
  import { motion } from './motion';

  const ICON = { ok: 'check', warn: 'warning', err: 'warning' } as const;

  // In a title, a word that is too long (the name of a branch, a path) is cut in its middle: its
  // start and its end stay readable. The full title is in a tooltip, and read by screen readers.
  const LONG = 36;
  const HEAD = 24;
  const TAIL = 10;
  function clip(title: string) {
    return title
      .split(' ')
      .map((word) => (word.length > LONG ? `${word.slice(0, HEAD)}…${word.slice(-TAIL)}` : word))
      .join(' ');
  }

  /** Gives the focus to the action button as soon as it appears, when asked to. */
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
      <button class="close" type="button" aria-label="Close notification" title="Close notification" onclick={() => toasts.dismiss(toast.id)}>
        <Icon name="x" size={14} />
      </button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    /* At the bottom, in the center: the place that hides the fewest controls. */
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
    /* The state is carried by the icon, not by a colored bar on the edge. */
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
  /* Off screen, but read: the whole title, when the one displayed is cut. */
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
  /* When pressed, a button shrinks to 97% (guidelines, sections 8 and 9.1). */
  .close:active {
    transform: scale(0.97);
  }
  .close:hover {
    background: var(--hover);
    color: var(--ink);
  }
</style>
