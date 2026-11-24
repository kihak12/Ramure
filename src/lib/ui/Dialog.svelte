<script lang="ts">
  import { tick } from 'svelte';
  import Icon from './Icon.svelte';
  import Select from './Select.svelte';
  import { veil } from './veil';
  import { dialogs, type Answers, type Field } from './dialogs.svelte';

  let element = $state<HTMLDialogElement>();
  let values = $state<Answers>({});

  const open = $derived(dialogs.current);
  const spec = $derived(open?.spec ?? null);
  const fields = $derived(spec?.fields ?? []);
  /** The first field takes the refusal of `check` under it; without a text field, the last line does. */
  const textFirst = $derived(fields.length > 0 && !['checkbox', 'select', 'textarea', 'number'].includes(fields[0].type));
  const valid = $derived(fields.every((f) => !f.required || String(values[f.name] ?? '').trim() !== ''));

  /** Why `check` refused what was typed: said under the first field. */
  let refusal = $state('');
  let busy = $state(false);
  let firstField = $state<HTMLInputElement>();

  const initial = (field: Field) => field.value ?? (field.type === 'checkbox' ? false : field.type === 'number' ? 0 : '');

  // `showModal()` brings the focus trap, the Escape key and the backdrop veil.
  $effect(() => {
    if (!element) return;
    if (open && !element.open) {
      values = Object.fromEntries((open.spec.fields ?? []).map((f) => [f.name, initial(f)]));
      refusal = '';
      busy = false;
      element.showModal();
      // A name is selected up to its extension: typing replaces the name and keeps the extension.
      const range = open.spec.select;
      if (range) queueMicrotask(() => firstField?.setSelectionRange(range[0], range[1]));
    } else if (!open && element.open) {
      element.close();
    }
  });

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!valid || busy) return;
    const answers: Answers = {};
    for (const field of fields) {
      const value = values[field.name];
      answers[field.name] = typeof value === 'string' ? value.trim() : value;
    }
    const check = spec?.check;
    if (check) {
      const asked = open;
      busy = true;
      const reason = await check(answers).catch((e: unknown) => String(e));
      busy = false;
      // The dialog was closed meanwhile (Escape): what the check says no longer has a place.
      if (dialogs.current !== asked) return;
      if (reason) {
        refusal = reason;
        // The keyboard goes back to the field to correct, or else to the main button.
        if (firstField) firstField.focus();
        else void tick().then(() => element?.querySelector<HTMLElement>('footer .btn:last-child')?.focus());
        return;
      }
    }
    dialogs.close(answers);
  }
</script>

<!-- `oncancel`: Escape. A click on the veil closes too. -->
<dialog
  bind:this={element}
  oncancel={(e) => {
    e.preventDefault();
    // An action that runs in the dialog lasts an instant and cannot stop halfway: neither Escape
    // nor the veil closes it meanwhile (guidelines, 9.10).
    if (!busy) dialogs.close(null);
  }}
  use:veil={() => !busy && dialogs.close(null)}
>
  {#if spec}
    <form onsubmit={submit}>
      <!-- The content scrolls if the window is too low; the last line stays in view (guidelines, 9.10). -->
      <div class="content">
      <h2>{spec.title}</h2>
      {#if spec.body}<p class="body">{spec.body}</p>{/if}
      {#if spec.warning}
        <!-- One line per thing to warn of, apart from the next: a line that wraps is not read as two. -->
        <div class="warning">
          <Icon name="warning" />
          <div class="lines">{#each spec.warning.split('\n') as line (line)}<p>{line}</p>{/each}</div>
        </div>
      {/if}
      {#if spec.note}<p class="body">{spec.note}</p>{/if}

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
              {#if i === 0}
                <input
                  class="field"
                  class:mono={field.mono}
                  class:invalid={!!refusal}
                  type="text"
                  placeholder={field.placeholder}
                  spellcheck="false"
                  autocomplete="off"
                  autofocus={!spec.danger}
                  aria-invalid={refusal ? 'true' : undefined}
                  aria-describedby={refusal ? 'dialog-refusal' : undefined}
                  bind:this={firstField}
                  bind:value={values[field.name]}
                  oninput={() => (refusal = '')}
                />
              {:else}
                <input
                  class="field"
                  class:mono={field.mono}
                  type="text"
                  placeholder={field.placeholder}
                  spellcheck="false"
                  autocomplete="off"
                  bind:value={values[field.name]}
                />
              {/if}
            {/if}
            <!-- What the action refused, under the field, announced at once (guidelines, 9.3). -->
            {#if i === 0 && textFirst && refusal}<small class="refusal" id="dialog-refusal" role="alert">{refusal}</small>{/if}
            {#if field.hint}<small>{field.hint}</small>{/if}
          </label>
        {/if}
      {/each}
      </div>

      <footer>
        <!-- What the action refused, on the left of the last line, the buttons staying where they
             are (guidelines, 9.1 and 9.21). -->
        {#if refusal && !textFirst}<p class="failure" role="alert"><Icon name="warning" size={14} /> <span>{refusal}</span></p>{/if}
        <!-- The way out, the secondary action, then the main one, at the right (guidelines, 9.1, "Hierarchy"). -->
        <!-- Destructive action: the focus lands on "Cancel", so that Enter destroys nothing (guidelines, 9.10). -->
        <!-- svelte-ignore a11y_autofocus -->
        <button class="btn" type="button" autofocus={!!spec.danger} disabled={busy} onclick={() => dialogs.close(null)}>{spec.cancelLabel ?? 'Cancel'}</button>
        {#each spec.extra ? [spec.extra].flat() : [] as extra (extra.label)}
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
        {/each}
        <!-- svelte-ignore a11y_autofocus -->
        <button class="btn {spec.danger ? 'danger' : 'primary'}" type="submit" disabled={!valid || busy} autofocus={fields.length === 0 && !spec.danger}>
          {busy && spec.busyLabel ? spec.busyLabel : spec.confirmLabel}
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
    border-radius: var(--radius);
    background: var(--warn-soft);
    color: var(--warn);
  }
  .lines {
    display: grid;
    gap: 4px;
    color: var(--ink);
  }
  .lines p {
    margin: 0;
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
  .row small.refusal {
    color: var(--bad);
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
  /* It wraps under itself, on the left: the buttons stay at the top of the line (guidelines, 9.1). */
  .failure {
    flex: 1 1 0;
    min-width: 0;
    align-self: flex-start;
    display: flex;
    gap: 6px;
    align-items: flex-start;
    margin: 0;
    color: var(--bad);
    font-size: var(--fs-small);
    overflow-wrap: anywhere;
  }
  .failure :global(.icon) {
    margin-top: 1px;
  }
</style>
