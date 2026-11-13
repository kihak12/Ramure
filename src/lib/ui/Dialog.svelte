<script lang="ts">
  import Icon from './Icon.svelte';
  import Select from './Select.svelte';
  import { veil } from './veil';
  import { dialogs, type Answers, type Field } from './dialogs.svelte';

  let element = $state<HTMLDialogElement>();
  let values = $state<Answers>({});

  const open = $derived(dialogs.current);
  const spec = $derived(open?.spec ?? null);
  const fields = $derived(spec?.fields ?? []);
  const valid = $derived(fields.every((f) => !f.required || String(values[f.name] ?? '').trim() !== ''));

  const initial = (field: Field) => field.value ?? (field.type === 'checkbox' ? false : field.type === 'number' ? 0 : '');

  // `showModal()` brings the focus trap, the Escape key and the backdrop veil.
  $effect(() => {
    if (!element) return;
    if (open && !element.open) {
      values = Object.fromEntries((open.spec.fields ?? []).map((f) => [f.name, initial(f)]));
      element.showModal();
    } else if (!open && element.open) {
      element.close();
    }
  });

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!valid) return;
    const answers: Answers = {};
    for (const field of fields) {
      const value = values[field.name];
      answers[field.name] = typeof value === 'string' ? value.trim() : value;
    }
    dialogs.close(answers);
  }
</script>

<!-- `oncancel`: Escape. A click on the veil closes too. -->
<dialog
  bind:this={element}
  oncancel={(e) => {
    e.preventDefault();
    dialogs.close(null);
  }}
  use:veil={() => dialogs.close(null)}
>
  {#if spec}
    <form onsubmit={submit}>
      <!-- The content scrolls if the window is too low; the last line stays in view (guidelines, 9.10). -->
      <div class="content">
      <h2>{spec.title}</h2>
      {#if spec.body}<p class="body">{spec.body}</p>{/if}
      {#if spec.warning}
        <p class="warning"><Icon name="warning" /> <span>{spec.warning}</span></p>
      {/if}

      {#each fields as field, i (field.name)}
        {#if field.type === 'checkbox'}
          <label class="check">
            <input type="checkbox" bind:checked={values[field.name] as boolean} />
            <span>{field.label}</span>
          </label>
        {:else}
          <label class="row">
            <span class="label">{field.label}</span>
            {#if field.type === 'textarea'}
              <textarea class="field" class:mono={field.mono} rows="4" placeholder={field.placeholder} bind:value={values[field.name]}></textarea>
            {:else if field.type === 'select'}
              <Select bind:value={values[field.name]}>
                {#each field.options ?? [] as option (option.value)}
                  <option value={option.value}>{option.label}</option>
                {/each}
              </Select>
            {:else if field.type === 'number'}
              <input class="field" type="number" min={field.min} max={field.max} bind:value={values[field.name]} />
            {:else}
              <!-- svelte-ignore a11y_autofocus -->
              <input
                class="field"
                class:mono={field.mono}
                type="text"
                placeholder={field.placeholder}
                spellcheck="false"
                autocomplete="off"
                autofocus={i === 0 && !spec.danger}
                bind:value={values[field.name]}
              />
            {/if}
            {#if field.hint}<small>{field.hint}</small>{/if}
          </label>
        {/if}
      {/each}
      </div>

      <footer>
        <!-- The way out, the secondary action, then the main one, at the right (guidelines, 9.1, "Hierarchy"). -->
        <!-- Destructive action: the focus lands on "Cancel", so that Enter destroys nothing (guidelines, 9.10). -->
        <!-- svelte-ignore a11y_autofocus -->
        <button class="btn" type="button" autofocus={!!spec.danger} onclick={() => dialogs.close(null)}>{spec.cancelLabel ?? 'Cancel'}</button>
        {#if spec.extra}
          {@const extra = spec.extra}
          <button
            class="btn"
            type="button"
            onclick={() => {
              // Closing the dialog empties `spec`: the action is kept beforehand, or it can no longer be read.
              const run = extra.run;
              dialogs.close(null);
              run();
            }}>{extra.label}</button
          >
        {/if}
        <!-- svelte-ignore a11y_autofocus -->
        <button class="btn {spec.danger ? 'danger' : 'primary'}" type="submit" disabled={!valid} autofocus={fields.length === 0 && !spec.danger}>
          {spec.confirmLabel}
        </button>
      </footer>
    </form>
  {/if}
</dialog>

<style>
  dialog {
    width: min(460px, calc(100vw - 48px));
    padding: 0;
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--raised);
    color: var(--ink);
    box-shadow: var(--shadow-pop);
  }
  dialog[open] {
    display: flex;
    flex-direction: column;
    animation: pop-in var(--t-med) var(--ease);
  }
  dialog::backdrop {
    background: var(--scrim);
    backdrop-filter: blur(2px);
    animation: fade-in var(--t-med) var(--ease);
  }
  form {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
    padding: 16px 20px;
  }
  /* 4 px of margin given back: the focus halo of a field (3 px) is not clipped by the scrolling. */
  .content {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
    margin: -4px;
    padding: 4px;
    overflow-y: auto;
  }
  h2 {
    margin: 0;
    font-size: var(--fs-title);
    font-weight: var(--fw-strong);
  }
  .body {
    margin: 0;
    color: var(--muted);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  .warning {
    display: flex;
    gap: 8px;
    margin: 0;
    padding: 8px 10px;
    border-radius: var(--radius-s);
    background: var(--warn-soft);
    color: var(--warn);
  }
  .warning span {
    color: var(--ink);
  }
  .row {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .row small {
    color: var(--muted);
    font-size: var(--fs-small);
  }
  textarea {
    resize: vertical;
    min-height: 70px;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  /* Too many buttons for one line: they wrap to the next line, the main one stays last,
     at the bottom right (guidelines, 9.1, "Hierarchy"). */
  footer {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
</style>
