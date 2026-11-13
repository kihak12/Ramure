<script lang="ts" module>
  // Ramure's icon set: 1.4 px strokes on a 16 grid, with no fill.
  const PATHS = {
    branch:
      '<circle cx="4.5" cy="3.5" r="1.6"/><circle cx="4.5" cy="12.5" r="1.6"/><circle cx="11.5" cy="5.5" r="1.6"/><path d="M4.5 5.1v5.8M11.5 7.1c0 3-7 2-7 3.8"/>',
    merge:
      '<circle cx="4.5" cy="3.5" r="1.6"/><circle cx="4.5" cy="12.5" r="1.6"/><circle cx="11.5" cy="8" r="1.6"/><path d="M4.5 5.1v5.8M4.5 5.1c0 2.9 3 2.9 5.4 2.9"/>',
    laptop: '<rect x="3" y="3.5" width="10" height="7" rx="1"/><path d="M1.5 12.5h13"/>',
    cloud: '<path d="M4.6 12.5a3 3 0 0 1-.5-5.96 4 4 0 0 1 7.72.9A2.55 2.55 0 0 1 11.4 12.5z"/>',
    tag: '<path d="M2.5 2.5h5.2l5.8 5.8-5.2 5.2-5.8-5.8z"/><circle cx="5.6" cy="5.6" r=".6"/>',
    key: '<circle cx="5.5" cy="10.5" r="3"/><path d="m7.6 8.4 5.9-5.9M11.5 4.5l1.8 1.8M9.6 6.4l1.5 1.5"/>',
    import: '<path d="M5.5 5.5v-2a1 1 0 0 1 1-1h4l3 3v7a1 1 0 0 1-1 1h-6a1 1 0 0 1-1-1v-2M2 8h7M7 5.8 9.2 8 7 10.2"/>',
    log: '<rect x="3.5" y="2.5" width="9" height="11" rx="1"/><path d="M6 6h4M6 8.5h4M6 11h2.5"/>',
    export: '<path d="M10.5 5.5v-2a1 1 0 0 0-1-1h-4l-3 3v7a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1v-2M7 8h7M11.8 5.8 14 8l-2.2 2.2"/>',
    stash:
      '<path d="M2.5 9.5v3a1 1 0 0 0 1 1h9a1 1 0 0 0 1-1v-3M2.5 9.5h3l1 1.5h3l1-1.5h3M8 2.5v5M5.8 5.5 8 7.7l2.2-2.2"/>',
    pop: '<path d="M2.5 9.5v3a1 1 0 0 0 1 1h9a1 1 0 0 0 1-1v-3M2.5 9.5h3l1 1.5h3l1-1.5h3M8 7.5v-5M5.8 4.7 8 2.5l2.2 2.2"/>',
    fetch:
      '<path d="M13.5 8a5.5 5.5 0 0 1-9.6 3.7M2.5 8a5.5 5.5 0 0 1 9.6-3.7M12.3 1.8v2.7H9.6M3.7 14.2v-2.7h2.7"/>',
    pull: '<path d="M8 2.5v8M4.8 7.5 8 10.7l3.2-3.2M3 13.5h10"/>',
    push: '<path d="M8 13.5v-8M4.8 8.5 8 5.3l3.2 3.2M3 2.5h10"/>',
    up: '<path d="M8 12.5v-9M4.5 7 8 3.5 11.5 7"/>',
    down: '<path d="M8 3.5v9M4.5 9 8 12.5 11.5 9"/>',
    plus: '<path d="M8 3.5v9M3.5 8h9"/>',
    minus: '<path d="M3.5 8h9"/>',
    undo: '<path d="M3 6.5h6.5a3.5 3.5 0 0 1 0 7H6M5.5 4 3 6.5 5.5 9"/>',
    reset: '<path d="M3 8a5 5 0 1 0 1.5-3.6M3 2.5V5h2.5"/>',
    check: '<path d="m3.5 8.5 3 3 6-7"/>',
    x: '<path d="m4 4 8 8M12 4l-8 8"/>',
    chevron: '<path d="m6 3.5 4.5 4.5L6 12.5"/>',
    settings:
      '<path d="M2.5 4.5h7M12 4.5h1.5M2.5 11.5H4M6.5 11.5h7"/><circle cx="10.5" cy="4.5" r="1.5"/><circle cx="5.2" cy="11.5" r="1.5"/>',
    folder: '<path d="M2 4.5a1 1 0 0 1 1-1h3l1.5 1.5H13a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1z"/>',
    terminal: '<rect x="2" y="3" width="12" height="10" rx="1.5"/><path d="m5 6.5 2 1.5-2 1.5M8.5 10h2.5"/>',
    warning: '<path d="M8 2.5 14 13H2zM8 6.5v3M8 11.2v.1"/>',
    copy: '<rect x="5.5" y="5.5" width="8" height="8" rx="1"/><path d="M10.5 5.5v-2a1 1 0 0 0-1-1h-6a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2"/>',
    trash:
      '<path d="M3 4.5h10M6.5 4.5v-2h3v2M4.5 4.5l.6 8.5a1 1 0 0 0 1 .9h3.8a1 1 0 0 0 1-.9l.6-8.5"/>',
    pencil: '<path d="m3 13 .6-3L10.5 3.1a1.2 1.2 0 0 1 1.7 0l.7.7a1.2 1.2 0 0 1 0 1.7L6 12.4z"/>',
    play: '<path d="M5 3.5v9l7-4.5z"/>',
    skip: '<path d="M4 3.5v9l6-4.5zM12 3.5v9"/>',
    stop: '<rect x="4" y="4" width="8" height="8" rx="1"/>',
    search: '<circle cx="7" cy="7" r="4"/><path d="m10 10 3.5 3.5"/>',
    commit: '<circle cx="8" cy="8" r="2.5"/><path d="M1.5 8h4M10.5 8h4"/>',
    list: '<path d="M6 4h7.5M6 8h7.5M6 12h7.5M2.5 4h.1M2.5 8h.1M2.5 12h.1"/>',
    pick: '<circle cx="5" cy="11" r="2.2"/><circle cx="11" cy="11.5" r="2.2"/><path d="M5 8.8C5 6 7 3.5 10 2.5c-1 2.5.5 4.5 1 6.8"/>',
    grip: '<path d="M6 4h.1M10 4h.1M6 8h.1M10 8h.1M6 12h.1M10 12h.1" stroke-width="2"/>',
    eye: '<path d="M1.5 8S4 3.5 8 3.5 14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8z"/><circle cx="8" cy="8" r="1.8"/>',
    refresh: '<path d="M13 8a5 5 0 1 1-1.5-3.6M13 2.5V5h-2.5"/>',
    'panel-left': '<rect x="2" y="3" width="12" height="10" rx="1.5"/><path d="M6 3v10"/>',
    'panel-right': '<rect x="2" y="3" width="12" height="10" rx="1.5"/><path d="M10 3v10"/>',
    panels: '<rect x="2" y="3" width="12" height="10" rx="1.5"/><path d="M6 3v10M10 3v10"/>',
    'panel-bottom': '<rect x="2" y="3" width="12" height="10" rx="1.5"/><path d="M2 9.5h12"/>',
  } as const;

  export type IconName = keyof typeof PATHS;
</script>

<script lang="ts">
  let { name, size = 16 }: { name: IconName; size?: number } = $props();
</script>

<!-- The paths are constants of this file: nothing from outside is injected. -->
<svg class="icon" viewBox="0 0 16 16" width={size} height={size} aria-hidden="true">{@html PATHS[name]}</svg>
