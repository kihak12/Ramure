<script lang="ts">
  import Icon from './Icon.svelte';
  import { menu, type MenuItem } from './menu.svelte';

  let element = $state<HTMLDivElement>();
  let position = $state({ x: 0, y: 0 });

  const open = $derived(menu.current);

  // Le menu s'ouvre sous le pointeur, puis est ramené dans la fenêtre s'il en dépasse.
  $effect(() => {
    if (!open) return;
    position = { x: open.x, y: open.y };
    queueMicrotask(() => {
      if (!element) return;
      const box = element.getBoundingClientRect();
      position = {
        x: Math.max(4, Math.min(open.x, window.innerWidth - box.width - 6)),
        y: Math.max(4, Math.min(open.y, window.innerHeight - box.height - 6)),
      };
      element.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus();
    });
  });

  function pick(item: MenuItem) {
    if ('separator' in item || item.disabled) return;
    menu.close();
    item.run();
  }

  function onKey(event: KeyboardEvent) {
    if (!element) return;
    if (event.key === 'Escape') {
      event.preventDefault();
      menu.close();
      return;
    }
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
    event.preventDefault();
    const buttons = [...element.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')];
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const next = (at + (event.key === 'ArrowDown' ? 1 : -1) + buttons.length) % buttons.length;
    buttons[next]?.focus();
  }
</script>

<svelte:window onblur={() => menu.close()} onresize={() => menu.close()} />

{#if open}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="veil"
    onmousedown={() => menu.close()}
    oncontextmenu={(e) => {
      e.preventDefault();
      menu.close();
    }}
    onwheel={() => menu.close()}
  ></div>
  <div class="menu" role="menu" tabindex="-1" bind:this={element} style:left="{position.x}px" style:top="{position.y}px" onkeydown={onKey}>
    {#each open.items as item, i (i)}
      {#if 'separator' in item}
        <hr />
      {:else}
        <button type="button" role="menuitem" class:danger={item.danger} disabled={item.disabled} title={item.hint} onclick={() => pick(item)}>
          <span class="glyph">{#if item.icon}<Icon name={item.icon} size={16} />{/if}</span>
          <span class="text">{item.label}</span>
        </button>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .veil {
    position: fixed;
    inset: 0;
    z-index: 70;
  }
  .menu {
    position: fixed;
    z-index: 71;
    min-width: 220px;
    max-width: 380px;
    padding: 5px;
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--raised);
    box-shadow: var(--shadow-pop);
    transform-origin: top left;
    animation: pop-in var(--t-fast) var(--ease);
  }
  .menu:focus {
    outline: none;
  }
  button {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 10px 6px 6px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    cursor: pointer;
    text-align: left;
    transition: background var(--t-fast) var(--ease);
  }
  button:hover:not(:disabled),
  button:focus-visible {
    background: var(--accent-soft);
    outline: none;
  }
  button:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  button.danger {
    color: var(--bad);
  }
  button.danger:hover:not(:disabled),
  button.danger:focus-visible {
    background: var(--bad-soft);
  }
  .glyph {
    display: grid;
    place-items: center;
    width: 18px;
    color: var(--muted);
  }
  .danger .glyph {
    color: inherit;
  }
  .text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  hr {
    margin: 4px 6px;
    border: 0;
    border-top: 1px solid var(--line);
  }
</style>
