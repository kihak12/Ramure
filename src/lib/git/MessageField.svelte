<script lang="ts">
  // The field of a commit message: the commit of the changes (9.22) and the rewording of a commit
  // (9.25) share it, so that a message reads and counts the same in both. Under it, what is said
  // about the message, then the length of its first line and the buttons.
  import type { Snippet } from 'svelte';

  let {
    value = $bindable(''),
    field = $bindable(),
    id,
    label,
    rows = 4,
    disabled = false,
    describedby,
    onkeydown,
    notes,
    actions,
  }: {
    value?: string;
    field?: HTMLTextAreaElement;
    id?: string;
    /** The accessible name, when no visible label names the field. */
    label?: string;
    rows?: number;
    disabled?: boolean;
    describedby?: string;
    onkeydown?: (e: KeyboardEvent) => void;
    /** What is said about the message, between the field and the buttons. */
    notes?: Snippet;
    actions: Snippet;
  } = $props();

  const subjectLength = $derived(value.split('\n')[0].length);
</script>

<textarea
  {id}
  class="field"
  {rows}
  placeholder="Summary on the first line, details after"
  aria-label={label}
  aria-describedby={describedby}
  bind:value
  bind:this={field}
  {onkeydown}
  {disabled}
></textarea>
{@render notes?.()}
<div class="send">
  <small class:long={subjectLength > 72} title="Length of the first line: past 72 characters, many tools cut it.">
    {subjectLength} / 72
  </small>
  {@render actions()}
</div>

<style>
  textarea {
    resize: vertical;
    min-height: 64px;
    max-height: 220px;
    font-family: var(--f-mono);
    font-size: var(--fs-code);
  }
  .send {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  small {
    color: var(--muted);
    font-size: var(--fs-small);
    font-variant-numeric: tabular-nums;
  }
  small.long {
    color: var(--warn);
  }
</style>
