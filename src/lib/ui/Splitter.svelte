<script lang="ts">
  /**
   * Resize handle between two cards (design guidelines, section 9.16).
   * It is dragged with the mouse and set from the keyboard; a double-click, or Enter, goes back to
   * the default size. Vertical between two cards side by side (it sets a width), horizontal
   * between two cards one above the other (it sets a height).
   */
  let {
    value,
    min,
    max,
    initial,
    direction = 1,
    orientation = 'vertical',
    label,
    onchange,
    oncommit,
  }: {
    /** Current width of the card being set, in pixels. */
    value: number;
    min: number;
    max: number;
    /** Default width. */
    initial: number;
    /** 1: the card is on the left of the handle (or above it); -1: on the right (or below). */
    direction?: 1 | -1;
    /** `vertical`: the handle separates two cards side by side; `horizontal`: one above the other. */
    orientation?: 'vertical' | 'horizontal';
    label: string;
    onchange: (value: number) => void;
    /** End of the gesture: the time to save. */
    oncommit: () => void;
  } = $props();

  const STEP = 16; // one press of an arrow key
  let origin: { at: number; value: number } | null = null;
  const horizontal = $derived(orientation === 'horizontal');
  const along = (e: PointerEvent) => (horizontal ? e.clientY : e.clientX);
  let dragging = $state(false);

  const clamp = (width: number) => Math.round(Math.min(max, Math.max(min, width)));

  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault(); // no text selection during the gesture
    const handle = e.currentTarget as HTMLElement;
    handle.focus({ preventScroll: true }); // the preventDefault above also prevents taking the focus
    handle.setPointerCapture(e.pointerId);
    origin = { at: along(e), value };
    dragging = true;
  }

  function move(e: PointerEvent) {
    if (origin) onchange(clamp(origin.value + (along(e) - origin.at) * direction));
  }

  function up() {
    if (!origin) return;
    origin = null;
    dragging = false;
    oncommit();
  }

  function set(width: number) {
    onchange(clamp(width));
    oncommit();
  }

  function key(e: KeyboardEvent) {
    // An arrow moves the handle: to the right, it widens the card on the left and narrows the one
    // on the right; upward, it makes the card below taller.
    const [back, forth] = horizontal ? ['ArrowUp', 'ArrowDown'] : ['ArrowLeft', 'ArrowRight'];
    const moves: Record<string, number> = {
      [back]: value - STEP * direction,
      [forth]: value + STEP * direction,
      Home: min,
      End: max,
      Enter: initial,
    };
    if (!(e.key in moves)) return;
    e.preventDefault();
    set(moves[e.key]);
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="splitter"
  class:dragging
  class:horizontal
  role="separator"
  aria-orientation={orientation}
  aria-label={label}
  aria-valuenow={Math.round(value)}
  aria-valuemin={min}
  aria-valuemax={max}
  tabindex="0"
  title="Drag to resize. Double-click: default {horizontal ? 'height' : 'width'}."
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  ondblclick={() => set(initial)}
  onkeydown={key}
></div>

<style>
  .splitter {
    position: relative;
    min-height: 0;
    cursor: col-resize;
    touch-action: none;
  }
  /* At rest, nothing: the window background already separates the cards. The marker only appears on
     hover, on focus and during the gesture. */
  .splitter::before {
    content: '';
    position: absolute;
    top: 12px;
    bottom: 12px;
    left: 50%;
    width: 2px;
    margin-left: -1px;
    border-radius: var(--radius-pill);
    background: var(--faint);
    opacity: 0;
    transition:
      opacity var(--t-fast) var(--ease),
      background var(--t-fast) var(--ease);
  }
  .splitter:hover::before {
    opacity: 1;
  }
  /* The focus reads on the marker itself, not through a frame around the handle. */
  .splitter:focus-visible {
    outline: none;
  }
  .splitter.horizontal {
    min-height: 0;
    min-width: 0;
    cursor: row-resize;
  }
  .splitter.horizontal::before {
    top: 50%;
    bottom: auto;
    left: 12px;
    right: 12px;
    width: auto;
    height: 2px;
    margin-left: 0;
    margin-top: -1px;
  }
  .splitter.dragging::before,
  .splitter:focus-visible::before {
    opacity: 1;
    background: var(--accent);
  }
</style>
