<script lang="ts">
  import Icon from './Icon.svelte';
  import { dialogs, type Answers, type Field } from './dialogs.svelte';

  let element = $state<HTMLDialogElement>();
  let values = $state<Answers>({});

  const open = $derived(dialogs.current);
  const spec = $derived(open?.spec ?? null);
  const fields = $derived(spec?.fields ?? []);
  const valid = $derived(fields.every((f) => !f.required || String(values[f.name] ?? '').trim() !== ''));

  const initial = (field: Field) => field.value ?? (field.type === 'checkbox' ? false : field.type === 'number' ? 0 : '');

  // `showModal()` apporte le piège de focus, la touche Échap et le voile d'arrière-plan.
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

<!-- `oncancel` : Échap. Le clic sur le voile ferme aussi. -->
<dialog
  bind:this={element}
  oncancel={(e) => {
    e.preventDefault();
    dialogs.close(null);
  }}
  onclick={(e) => {
    if (e.target === element) dialogs.close(null);
  }}
>
  {#if spec}
    <form onsubmit={submit}>
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
              <select class="field" bind:value={values[field.name]}>
                {#each field.options ?? [] as option (option.value)}
                  <option value={option.value}>{option.label}</option>
                {/each}
              </select>
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

      <footer>
        {#if spec.extra}
          {@const extra = spec.extra}
          <button
            class="btn ghost extra"
            type="button"
            onclick={() => {
              // Fermer la boîte vide `spec` : l'action est retenue avant, sinon elle ne se lit plus.
              const run = extra.run;
              dialogs.close(null);
              run();
            }}>{extra.label}</button
          >
        {/if}
        <!-- Action destructive : le focus arrive sur « Annuler », pour qu'Entrée ne détruise rien (charte, 9.10). -->
        <!-- svelte-ignore a11y_autofocus -->
        <button class="btn" type="button" autofocus={!!spec.danger} onclick={() => dialogs.close(null)}>{spec.cancelLabel ?? 'Annuler'}</button>
        <!-- svelte-ignore a11y_autofocus -->
        <button class="btn {spec.danger ? 'danger' : 'primary'}" type="submit" disabled={!valid} autofocus={fields.length === 0 && !spec.danger}>
          {spec.confirmLabel ?? 'Valider'}
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
    padding: 16px 20px;
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
    cursor: pointer;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
  .extra {
    margin-right: auto;
    color: var(--accent);
  }
</style>
