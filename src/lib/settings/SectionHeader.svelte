<script lang="ts">
  import { tick } from 'svelte';
  import { toasts } from '../ui/toasts.svelte';

  /**
   * En-tête d'une section de réglages : son titre, et le retour aux valeurs par défaut.
   * `onreset` réinitialise la section et rend de quoi revenir en arrière : la notification qui
   * suit propose « Annuler » (charte, section 9.18).
   */
  let { title, note = '', changed, onreset }: { title: string; note?: string; changed: boolean; onreset: () => () => void } = $props();

  let button = $state<HTMLButtonElement>();

  function reset(event: MouseEvent) {
    const undo = onreset();
    // Lancé au clavier (un clic sans bouton de souris) : le bouton se grise et perdrait le focus.
    // Il va sur « Annuler », qui est ainsi à une touche, puis revient ici une fois l'annulation faite.
    const keyboard = event.detail === 0;
    toasts.ok(`« ${title} » : valeurs par défaut rétablies`, '', {
      label: 'Annuler',
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
    title={changed ? 'Revenir aux valeurs par défaut de cette section' : 'Cette section est déjà aux valeurs par défaut'}
    onclick={reset}
  >
    Tout réinitialiser
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
