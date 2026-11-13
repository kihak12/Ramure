<script lang="ts">
  import Icon from './Icon.svelte';
  import { display } from './keys.svelte';
  import { menu, type MenuItem } from './menu.svelte';

  let element = $state<HTMLDivElement>();
  let veil = $state<HTMLDivElement>();
  let position = $state({ x: 0, y: 0 });
  // The menu is being browsed from the keyboard: the item that has the focus shows it. The mouse takes over again by moving.
  let keyboard = $state(false);

  const open = $derived(menu.current);
  // The checkable items of a drop-down menu follow their state while it is open.
  const items = $derived(!open ? [] : typeof open.items === 'function' ? open.items() : open.items);

  // The menu opens under the pointer, or under its button and aligned on its right edge, then is
  // brought back inside the window if it overflows it.
  $effect(() => {
    if (!open) return;
    keyboard = !!open.keyboard;
    position = { x: open.x, y: open.y };
    queueMicrotask(() => {
      if (!element) return;
      // offsetWidth and offsetHeight: the true size, which the appearing animation does not shrink.
      const { offsetWidth: width, offsetHeight: height } = element;
      const x = open.anchor ? open.x - width : open.x;
      position = {
        x: Math.max(4, Math.min(x, window.innerWidth - width - 6)),
        y: Math.max(4, Math.min(open.y, window.innerHeight - height - 6)),
      };
      // Opened from the keyboard, the first item takes the focus. With the mouse, the menu does: no
      // item is current until one has been hovered or reached, and Space checks nothing.
      if (keyboard) element.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus();
      else element.focus();
    });
  });

  // The tooltip of the element that opened the menu keeps quiet while it is open (guidelines, 9.9):
  // the veil covers it. But WebKitGTK only reads the tooltip again on a move of the pointer, and a
  // right-click given while the mouse is still moving opens the menu before the tooltip of the row
  // arrives: it would show on top until the next move. The veil scrolls by one pixel once the button
  // is released: WebKit then simulates a move, finds the veil under the pointer, and the tooltip
  // keeps quiet.
  $effect(() => {
    if (!open) return;
    const nudge = () =>
      setTimeout(() => {
        if (veil) veil.scrollTop = veil.scrollTop ? 0 : 1;
      });
    nudge();
    window.addEventListener('mouseup', nudge, { capture: true, once: true });
    return () => window.removeEventListener('mouseup', nudge, { capture: true });
  });

  /** `pointer`: chosen with a click, not from the keyboard (Enter or Space give a click without `detail`). */
  function pick(item: MenuItem, pointer = false) {
    if ('separator' in item || item.disabled) return;
    // A checkable item applies at once and leaves the menu open: several can be changed in a row.
    if (item.checked === undefined) menu.close(false, pointer);
    item.run();
  }

  // The hovered item takes the focus: Space and Enter act on the one seen highlighted.
  function onKey(event: KeyboardEvent) {
    if (!element) return;
    if (event.key === 'Escape') {
      event.preventDefault();
      menu.close(true);
      return;
    }
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
    event.preventDefault();
    keyboard = true;
    const buttons = [...element.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')];
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    // No current item: ↓ goes to the first, ↑ to the last.
    const from = at < 0 && event.key === 'ArrowUp' ? 0 : at;
    const next = (from + (event.key === 'ArrowDown' ? 1 : -1) + buttons.length) % buttons.length;
    buttons[next]?.focus();
  }
</script>

<svelte:window onblur={() => menu.close()} onresize={() => menu.close()} />

{#if open}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="veil"
    bind:this={veil}
    onmousedown={(e) => {
      // A click on the veil does not move the focus: the menu gives it back to whoever had it.
      e.preventDefault();
      menu.close(false, true);
    }}
    oncontextmenu={(e) => {
      e.preventDefault();
      menu.close(false, true);
    }}
    onwheel={() => menu.close(false, true)}
  >
    <div class="room"></div>
  </div>
  <div
    class="menu"
    class:under={!!open.anchor}
    class:keyboard
    role="menu"
    tabindex="-1"
    bind:this={element}
    style:left="{position.x}px"
    style:top="{position.y}px"
    onkeydown={onKey}
    onpointermove={() => (keyboard = false)}
  >
    {#each items as item, i (i)}
      {#if 'separator' in item}
        <hr />
      {:else}
        <button
          type="button"
          role={item.checked === undefined ? 'menuitem' : 'menuitemcheckbox'}
          aria-checked={item.checked}
          class:danger={item.danger}
          disabled={item.disabled}
          title={item.hint}
          onclick={(e) => pick(item, e.detail > 0)}
          onpointerenter={(event) => event.currentTarget.focus()}
        >
          <span class="glyph">{#if item.icon}<Icon name={item.icon} size={16} />{/if}</span>
          <span class="text">{item.label}</span>
          {#if item.keys}
            <span class="keys">{#each display(item.keys).split(' + ') as part, n (n)}<kbd>{part}</kbd>{/each}</span>
          {/if}
          <!-- The place of the check mark is kept on unchecked items: an item does not change size. -->
          {#if item.checked !== undefined}
            <span class="check">{#if item.checked}<Icon name="check" size={16} />{/if}</span>
          {/if}
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
    overflow: hidden;
  }
  /* One pixel more than the veil: enough to make it scroll (see above). */
  .room {
    height: calc(100% + 1px);
  }
  .menu {
    position: fixed;
    z-index: 71;
    min-width: 220px;
    max-width: 380px;
    padding: 4px;
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--raised);
    box-shadow: var(--shadow-pop);
    transform-origin: top left;
    animation: pop-in var(--t-fast) var(--ease);
  }
  /* Dropped down under a button, it comes from that button: its top right corner. */
  .menu.under {
    transform-origin: top right;
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
  button:focus-visible,
  .keyboard button:focus {
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
  button.danger:focus-visible,
  .keyboard button.danger:focus {
    background: var(--bad-soft);
  }
  /* From the keyboard, only the item that has the focus is highlighted: the pointer left on another no longer counts. */
  .keyboard button:hover:not(:focus) {
    background: none;
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
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .keys {
    flex: none;
    display: flex;
    gap: 2px;
    margin-left: 12px;
  }
  .check {
    flex: none;
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    color: var(--accent);
  }
  hr {
    margin: 4px 6px;
    border: 0;
    border-top: 1px solid var(--line);
  }
</style>
