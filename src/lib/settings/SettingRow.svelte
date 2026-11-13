<script lang="ts">
  import type { Snippet } from 'svelte';

  /**
   * The row of a setting: its name and its help on the left, its control on the right (guidelines,
   * section 9.18).
   * `error`: the value entered is not valid, and it has not been applied.
   * `busy`: the setting is being applied, and what that is doing.
   * `group`: the control is a choice (section 9.19). It sits under the name, on the whole width,
   * and `{id}-name` is what names it.
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
    /* The name is level with the control: 32 px, like a field. */
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
  /* Narrow column: the control goes under its name. */
  @container (max-width: 480px) {
    .setting {
      grid-template-columns: minmax(0, 1fr);
      gap: 6px;
    }
  }
</style>
