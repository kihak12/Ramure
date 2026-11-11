<script lang="ts">
  import type { Snippet } from 'svelte';

  /**
   * Une ligne de réglage : son nom et son aide à gauche, son contrôle à droite (charte, section 9.18).
   * `error` : la valeur saisie n'est pas valide, elle n'a pas été appliquée.
   * `busy` : l'application du réglage est en cours, et ce qu'elle fait.
   * `group` : le contrôle est un choix (section 9.19). Il se place sous le nom, sur toute la
   * largeur, et c'est `{id}-name` qui le nomme.
   */
  let {
    id,
    name,
    help = '',
    error = '',
    busy = '',
    group = false,
    children,
  }: { id: string; name: string; help?: string; error?: string; busy?: string; group?: boolean; children: Snippet } = $props();
</script>

<div class="setting" class:group>
  <div class="text">
    {#if group}
      <span class="name" id="{id}-name">{name}</span>
    {:else}
      <label class="name" for={id}>{name}</label>
    {/if}
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
  .name {
    font-size: var(--fs-body);
  }
  .group {
    grid-template-columns: minmax(0, 1fr);
    gap: 8px;
  }
  .group .text {
    padding-top: 0;
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
