<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLSelectAttributes } from 'svelte/elements';
  import Icon from './Icon.svelte';

  // Une liste. Fermée, elle est dessinée par l'application, comme la case à cocher : un champ de
  // 32 px et le chevron du jeu d'icônes, pareils sous Windows et sous Linux, où la liste du système
  // est plus haute qu'un champ. Ouverte, son menu reste celui du système (charte, section 9.3).
  let {
    value = $bindable(),
    element = $bindable(),
    children,
    ...rest
  }: Omit<HTMLSelectAttributes, 'value' | 'class'> & {
    value?: unknown;
    element?: HTMLSelectElement;
    children: Snippet;
  } = $props();
</script>

<span class="select">
  <select class="field" bind:value bind:this={element} {...rest}>
    {@render children()}
  </select>
  <span class="chevron"><Icon name="chevron" size={14} /></span>
</span>

<style>
  .select {
    position: relative;
    display: block;
    min-width: 0;
  }
  select {
    appearance: none;
    height: 32px;
    /* Le texte ne passe jamais sous le chevron. */
    padding-right: 29px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: pointer;
  }
  /* Désactivée, comme tout élément désactivé (charte, section 9). */
  .select:has(select:disabled) {
    opacity: 0.45;
  }
  select:disabled {
    cursor: not-allowed;
  }
  .chevron {
    position: absolute;
    top: 50%;
    right: 9px;
    display: grid;
    color: var(--muted);
    pointer-events: none;
    transform: translateY(-50%) rotate(90deg);
  }
</style>
