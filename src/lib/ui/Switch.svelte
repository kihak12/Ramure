<script lang="ts">
  /**
   * A switch: a setting that is on or off, and applies at once (guidelines, section 9.3). It is a
   * button with the role `switch`; the name of its setting is its `<label for>`, which toggles it
   * too. The word "On" or "Off" beside it says its state: never the color alone.
   */
  let {
    id,
    on,
    describedby,
    onchange,
  }: {
    id: string;
    on: boolean;
    describedby?: string;
    onchange: (on: boolean) => void;
  } = $props();
</script>

<button {id} class="switch" class:on type="button" role="switch" aria-checked={on} aria-describedby={describedby} onclick={() => onchange(!on)}>
  <span class="track" aria-hidden="true"><span class="thumb"></span></span>
  <span class="state" aria-hidden="true">{on ? 'On' : 'Off'}</span>
</button>

<style>
  /* The target is the track and its word, as high as a field: 32 px. */
  .switch {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 32px;
    padding: 0;
    border: none;
    background: none;
    color: var(--muted);
    font: inherit;
    font-size: var(--fs-label);
    cursor: pointer;
  }
  /* The track: the one pill that can be clicked (guidelines, 1 and 5), 34 × 20 px. */
  .track {
    position: relative;
    flex: none;
    width: 34px;
    height: 20px;
    border: 1px solid var(--muted);
    border-radius: var(--radius-pill);
    background: var(--bg);
    transition:
      background var(--t-fast) var(--ease),
      border-color var(--t-fast) var(--ease);
  }
  /* The thumb, 14 px, 2 px inside the border; on, it slides to the other end. */
  .thumb {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--muted);
    transition:
      transform var(--t-fast) var(--ease),
      background var(--t-fast) var(--ease);
  }
  .on .track {
    border-color: var(--accent);
    background: var(--accent);
  }
  .on .thumb {
    background: var(--on-accent);
    transform: translateX(14px);
  }
  .switch:hover .track {
    border-color: var(--ink);
  }
  .on:hover .track {
    border-color: color-mix(in srgb, var(--accent) 90%, var(--ink));
    background: color-mix(in srgb, var(--accent) 90%, var(--ink));
  }
  /* The word keeps the width of the longer one: the switch never moves. */
  .state {
    min-width: 3ch;
  }
  /* The focus ring of the application goes around the track, not around the word. */
  .switch:focus-visible {
    outline: none;
  }
  .switch:focus-visible .track {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .switch:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
</style>
