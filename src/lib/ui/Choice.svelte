<script lang="ts" generics="T extends string | number">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  /**
   * A choice: a few options, all in view, one of which is always selected (guidelines, section 9.19).
   * It is a group of radio buttons: the arrows move the choice, which applies at once.
   * `rows`: one option per line, with what it entails. `tiles`: side by side, with a preview.
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
    /** Name of the group of radio buttons, unique in the page. */
    name: string;
    value: T;
    options: readonly { value: T; label: string; help?: string }[];
    layout?: 'rows' | 'tiles';
    labelledby: string;
    describedby?: string;
    /** What is better shown than said: the preview of an option. */
    preview?: Snippet<[T]>;
    onchange: (value: T) => void;
  } = $props();

  // The focus ring only shows from the keyboard. It is tracked here: `:focus-visible` does not
  // respond, in every engine, on a radio button reached with the arrows. A focus that arrives just
  // after a mouse press comes from the mouse; any other comes from the keyboard.
  const POINTER_WINDOW = 500; // ms between the mouse press and the focus it gives
  let keyboard = $state(false);
  let pointerAt = -Infinity;
</script>

<!-- The group does not take the focus itself: its radio buttons take it. -->
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
      <!-- The check mark keeps its place on the options not selected: an option does not change size. -->
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
  /* The radio button stays there for the keyboard and screen readers; the whole line draws it. */
  input {
    position: absolute;
    inset: 0;
    margin: 0;
    opacity: 0;
    pointer-events: none;
  }
  /* Keyboard focus: an inner ring on the option, as on a row of a list. */
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
