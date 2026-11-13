<script lang="ts">
  import { tick } from 'svelte';
  import { toasts } from '../ui/toasts.svelte';

  /**
   * The header of a settings section: its title, and the way back to the defaults.
   * `onreset` resets the section and returns what takes it back: the notification that follows
   * offers “Undo” (guidelines, section 9.18).
   */
  let { title, note = '', changed, onreset }: { title: string; note?: string; changed: boolean; onreset: () => () => void } = $props();

  let button = $state<HTMLButtonElement>();

  function reset(event: MouseEvent) {
    const undo = onreset();
    // Started from the keyboard (a click with no mouse button): the button is disabled and would
    // lose the focus. It goes to “Undo”, which is then one key away, and comes back here once undone.
    const keyboard = event.detail === 0;
    toasts.ok(`“${title}” reset to defaults`, '', {
      label: 'Undo',
      focus: keyboard,
      run: () => {
        undo();
        if (keyboard) void tick().then(() => button?.focus());
      },
    });
  }
</script>

<header>
  <div class="text">
    <h2>{title}</h2>
    {#if note}<p>{note}</p>{/if}
  </div>
  <button
    bind:this={button}
    class="btn ghost small"
    type="button"
    disabled={!changed}
    title={changed ? 'Restore the defaults of this section' : 'This section already has its default values'}
    onclick={reset}
  >
    Reset all
  </button>
</header>

<style>
  header {
    display: flex;
    align-items: flex-start;
    gap: 12px;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 2px;
  }
  h2 {
    margin: 0;
    font-size: var(--fs-title);
    font-weight: var(--fw-strong);
  }
  p {
    margin: 0;
    color: var(--muted);
    font-size: var(--fs-small);
  }
</style>
