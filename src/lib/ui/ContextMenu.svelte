<script lang="ts">
  import Icon from './Icon.svelte';
  import { display } from './keys.svelte';
  import { menu, type MenuItem } from './menu.svelte';

  let element = $state<HTMLDivElement>();
  let veil = $state<HTMLDivElement>();
  let position = $state({ x: 0, y: 0 });
  // On parcourt le menu au clavier : l'élément qui a le focus le montre. La souris reprend la main en bougeant.
  let keyboard = $state(false);

  const open = $derived(menu.current);
  // Les cases d'un menu déroulant suivent leur état pendant qu'il est ouvert.
  const items = $derived(!open ? [] : typeof open.items === 'function' ? open.items() : open.items);

  // Le menu s'ouvre sous le pointeur, ou sous son bouton et aligné sur son bord droit, puis est
  // ramené dans la fenêtre s'il en dépasse.
  $effect(() => {
    if (!open) return;
    keyboard = !!open.keyboard;
    position = { x: open.x, y: open.y };
    queueMicrotask(() => {
      if (!element) return;
      // offsetWidth et offsetHeight : la taille vraie, que l'animation d'apparition ne rétrécit pas.
      const { offsetWidth: width, offsetHeight: height } = element;
      const x = open.anchor ? open.x - width : open.x;
      position = {
        x: Math.max(4, Math.min(x, window.innerWidth - width - 6)),
        y: Math.max(4, Math.min(open.y, window.innerHeight - height - 6)),
      };
      // Ouvert au clavier, le premier élément prend le focus. À la souris, c'est le menu : aucun
      // élément n'est en cours tant qu'on n'en a pas survolé ou atteint un, Espace ne coche rien.
      if (keyboard) element.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus();
      else element.focus();
    });
  });

  // L'infobulle de l'élément qui a ouvert le menu se tait tant qu'il est ouvert (charte, 9.9) : le
  // voile la couvre. Mais WebKitGTK ne relit l'infobulle qu'à un mouvement du pointeur, et un clic
  // droit donné en bougeant encore la souris ouvre le menu avant que celle de la ligne arrive : elle
  // s'afficherait par-dessus jusqu'au mouvement suivant. Le voile défile d'un pixel, bouton relâché :
  // WebKit simule alors un mouvement, trouve le voile sous le pointeur, et l'infobulle se tait.
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

  function pick(item: MenuItem) {
    if ('separator' in item || item.disabled) return;
    // Une case à cocher s'applique tout de suite et laisse le menu ouvert : on en change plusieurs d'affilée.
    if (item.checked === undefined) menu.close();
    item.run();
  }

  // L'élément survolé prend le focus : Espace et Entrée agissent sur celui qu'on voit en surbrillance.
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
    // Aucun élément en cours : ↓ va au premier, ↑ au dernier.
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
    onmousedown={() => menu.close()}
    oncontextmenu={(e) => {
      e.preventDefault();
      menu.close();
    }}
    onwheel={() => menu.close()}
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
          onclick={() => pick(item)}
          onpointerenter={(event) => event.currentTarget.focus()}
        >
          <span class="glyph">{#if item.icon}<Icon name={item.icon} size={16} />{/if}</span>
          <span class="text">{item.label}</span>
          {#if item.keys}
            <span class="keys">{#each display(item.keys).split(' + ') as part, n (n)}<kbd>{part}</kbd>{/each}</span>
          {/if}
          <!-- La place de la coche est gardée sur les cases décochées : un élément ne change pas de taille. -->
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
  /* Un pixel de plus que le voile : de quoi le faire défiler (voir plus haut). */
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
    /* Une couche de composition à lui, que l'animation finie ne lui retire pas : sans elle, WebKitGTK
       peint la barre de défilement superposée d'une liste voisine par-dessus le menu (charte, 9.9). */
    will-change: transform;
  }
  /* Déroulé sous un bouton, il vient de ce bouton : son coin haut droit. */
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
  /* Au clavier, seul l'élément qui a le focus est en surbrillance : le pointeur resté sur un autre ne compte plus. */
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
