<script lang="ts" generics="T extends string | number">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  /**
   * Un choix : quelques options, toutes visibles, dont une est toujours retenue (charte, section 9.19).
   * C'est un groupe de boutons radio : les flèches déplacent le choix, qui s'applique aussitôt.
   * `rows` : une option par ligne, avec ce qu'elle entraîne. `tiles` : côte à côte, avec un aperçu.
   */
  let {
    name,
    value,
    options,
    layout = 'rows',
    labelledby,
    describedby,
    preview,
    onchange,
  }: {
    /** Nom du groupe de boutons radio, unique dans la page. */
    name: string;
    value: T;
    options: readonly { value: T; label: string; help?: string }[];
    layout?: 'rows' | 'tiles';
    labelledby: string;
    describedby?: string;
    /** Ce qui se voit mieux qu'il ne se dit : l'aperçu d'une option. */
    preview?: Snippet<[T]>;
    onchange: (value: T) => void;
  } = $props();

  // L'anneau de focus ne se montre qu'au clavier. On le suit ici : `:focus-visible` ne répond pas
  // sur un bouton radio atteint par les flèches dans tous les moteurs. Un focus qui arrive juste
  // après un appui de la souris vient d'elle ; tout autre vient du clavier.
  const POINTER_WINDOW = 500; // ms entre l'appui de la souris et le focus qu'il donne
  let keyboard = $state(false);
  let pointerAt = -Infinity;
</script>

<!-- Le groupe ne prend pas le focus lui-même : ce sont ses boutons radio qui le prennent. -->
<!-- svelte-ignore a11y_interactive_supports_focus -->
<div
  class="choice {layout}"
  class:keyboard
  role="radiogroup"
  aria-labelledby={labelledby}
  aria-describedby={describedby}
  style:--count={options.length}
  onpointerdown={() => {
    pointerAt = performance.now();
    keyboard = false;
  }}
  onfocusin={() => (keyboard = performance.now() - pointerAt > POINTER_WINDOW)}
  onkeydown={() => (keyboard = true)}
>
  {#each options as option (option.value)}
    {@const on = option.value === value}
    <label class="option" class:on>
      <input type="radio" {name} value={option.value} checked={on} onchange={() => onchange(option.value)} />
      <span class="ring"></span>
      {#if preview}<span class="preview" aria-hidden="true">{@render preview(option.value)}</span>{/if}
      <span class="text">
        <span class="name">{option.label}</span>
        {#if option.help}<span class="help">{option.help}</span>{/if}
      </span>
      <!-- La coche garde sa place sur les options non retenues : une option ne change pas de taille. -->
      <span class="mark" aria-hidden="true"><Icon name="check" size={14} /></span>
    </label>
  {/each}
</div>

<style>
  .choice {
    display: grid;
    gap: 2px;
    min-width: 0;
  }
  .tiles {
    grid-template-columns: repeat(var(--count), minmax(0, 1fr));
    gap: 8px;
  }
  .option {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    gap: 6px 8px;
    min-height: 28px;
    padding: 6px 8px;
    border-radius: var(--radius);
    cursor: pointer;
    transition: background var(--t-fast) var(--ease);
  }
  .tiles .option {
    align-content: start;
    padding: 6px;
    border: 1px solid var(--line);
  }
  .option:hover {
    background: var(--hover);
  }
  .option.on {
    background: var(--sel);
  }
  /* Le bouton radio reste là pour le clavier et les lecteurs d'écran ; la ligne entière le dessine. */
  input {
    position: absolute;
    inset: 0;
    margin: 0;
    opacity: 0;
    pointer-events: none;
  }
  /* Focus au clavier : un anneau intérieur sur l'option, comme sur une ligne de liste. */
  .ring {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    pointer-events: none;
  }
  .keyboard input:focus + .ring {
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  .preview {
    grid-column: 1 / -1;
    display: block;
    height: 64px;
    overflow: hidden;
    border-radius: var(--radius-s);
  }
  .text {
    display: grid;
    gap: 2px;
    min-width: 0;
  }
  .tiles .text {
    padding-left: 2px;
  }
  .on .name {
    font-weight: var(--fw-strong);
  }
  .help {
    color: var(--muted);
    font-size: var(--fs-small);
  }
  .mark {
    display: grid;
    color: var(--accent);
    visibility: hidden;
  }
  .on .mark {
    visibility: visible;
  }
</style>
