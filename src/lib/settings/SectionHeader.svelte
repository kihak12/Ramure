<script lang="ts">
  /** En-tête d'une section de réglages : son titre, et le retour aux valeurs par défaut. */
  let { title, note = '', changed, onreset }: { title: string; note?: string; changed: boolean; onreset: () => void } = $props();
</script>

<header>
  <div class="text">
    <h2>{title}</h2>
    {#if note}<p>{note}</p>{/if}
  </div>
  <button
    class="btn ghost small"
    type="button"
    disabled={!changed}
    title={changed ? 'Revenir aux valeurs par défaut de cette section' : 'Cette section est déjà aux valeurs par défaut'}
    onclick={onreset}
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
