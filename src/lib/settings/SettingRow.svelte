<script lang="ts">
  import type { Snippet } from 'svelte';

  /**
   * Une ligne de réglage : son nom et son aide à gauche, son contrôle à droite (charte, section 9.18).
   * `error` : la valeur saisie n'est pas valide, elle n'a pas été appliquée.
   * `busy` : l'application du réglage est en cours, et ce qu'elle fait.
   */
  let { id, name, help = '', error = '', busy = '', children }: { id: string; name: string; help?: string; error?: string; busy?: string; children: Snippet } = $props();
</script>

<div class="setting">
  <div class="text">
    <label for={id}>{name}</label>
    {#if help}<p id="{id}-help">{help}</p>{/if}
  </div>
  <div class="control">
    {@render children()}
    {#if error}
      <p class="error" id="{id}-error" role="alert">{error}</p>
    {:else if busy}
      <p class="busy" role="status"><span class="spinner" aria-hidden="true"></span>{busy}</p>
    {/if}
  </div>
</div>

<style>
  .setting {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 240px;
    gap: 16px;
    align-items: start;
  }
  .text {
    display: grid;
    gap: 2px;
    /* Le nom est à la hauteur du contrôle : 32 px, comme un champ. */
    padding-top: 6px;
  }
  label {
    font-size: var(--fs-body);
  }
  p {
    margin: 0;
    color: var(--muted);
    font-size: var(--fs-small);
  }
  .control {
    display: grid;
    gap: 6px;
    min-width: 0;
  }
  .error {
    color: var(--bad);
  }
  .busy {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  /* Colonne étroite : le contrôle passe sous son nom. */
  @container (max-width: 480px) {
    .setting {
      grid-template-columns: minmax(0, 1fr);
      gap: 6px;
    }
  }
</style>
