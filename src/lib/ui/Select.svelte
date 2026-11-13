<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { HTMLSelectAttributes } from 'svelte/elements';
  import Icon from './Icon.svelte';

  // A drop-down list. Closed, it is drawn by the application, like the checkbox: a 32 px field and
  // the chevron of the icon set, the same on Windows and on Linux, where the list of the system is
  // taller than a field. Open, its menu stays that of the system (guidelines, section 9.3).
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
    /* The text never runs under the chevron. */
    padding-right: 29px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: pointer;
  }
  /* Disabled, like any disabled element (guidelines, section 9). */
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
