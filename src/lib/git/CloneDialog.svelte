<script lang="ts">
  import { Channel, invoke } from '@tauri-apps/api/core';
  import { open } from '@tauri-apps/plugin-dialog';
  import Icon from '../ui/Icon.svelte';
  import { toasts } from '../ui/toasts.svelte';
  import type { CommandLog, Done } from './types';
  import { workspace } from './workspace.svelte';

  const STORAGE_KEY = 'ramure.cloneDir';

  let element = $state<HTMLDialogElement>();
  let url = $state('');
  let parent = $state('');
  let name = $state('');
  // Tant que le nom n'a pas été retouché, il suit l'adresse.
  let nameEdited = $state(false);
  let running = $state(false);
  let progress = $state('');
  let error = $state('');

  /** Même règle que `folder_name` côté Rust : `https://hôte/moi/projet.git` → `projet`. */
  function folderName(address: string) {
    const last = address.trim().replace(/[\\/]+$/, '').split(/[\\/:]/).pop() ?? '';
    return last.replace(/\.git$/, '');
  }

  const separator = $derived(parent.includes('\\') && !parent.includes('/') ? '\\' : '/');
  const dest = $derived(parent && name ? `${parent.replace(/[\\/]+$/, '')}${separator}${name}` : '');
  const nameValid = $derived(name.trim() !== '' && !/[\\/]/.test(name) && name !== '.' && name !== '..');
  const ready = $derived(url.trim() !== '' && parent.trim() !== '' && nameValid && !running);
  // « Receiving objects:  42% (…) » → 42
  const percent = $derived(Number(/(\d+)%/.exec(progress)?.[1] ?? NaN));
  const phase = $derived(progress.split(':')[0]);

  $effect(() => {
    if (!element) return;
    if (workspace.cloning && !element.open) {
      url = '';
      name = '';
      nameEdited = false;
      progress = '';
      error = '';
      try {
        parent = localStorage.getItem(STORAGE_KEY) ?? '';
      } catch {
        parent = '';
      }
      element.showModal();
    } else if (!workspace.cloning && element.open) {
      element.close();
    }
  });

  $effect(() => {
    if (!nameEdited) name = folderName(url);
  });

  async function browse() {
    const dir = await open({ directory: true, multiple: false, title: 'Dossier où cloner le dépôt' });
    if (typeof dir === 'string') parent = dir;
  }

  async function run(event: SubmitEvent) {
    event.preventDefault();
    if (!ready) return;
    running = true;
    error = '';
    progress = 'Connexion au dépôt distant…';
    const onProgress = new Channel<string>();
    onProgress.onmessage = (line) => (progress = line);
    try {
      const reply = await invoke<Done<string>>('git_clone', { url: url.trim(), dest, onProgress });
      try {
        localStorage.setItem(STORAGE_KEY, parent);
      } catch {
        /* stockage indisponible */
      }
      workspace.cloning = false;
      toasts.ok('Dépôt cloné', reply.data);
      await workspace.openCloned(reply.data, reply.commands);
    } catch (e) {
      const failed = e as { message?: string; commands?: CommandLog[] };
      error = failed.message ?? String(e);
    } finally {
      running = false;
    }
  }

  function cancel() {
    if (running) void invoke('git_clone_cancel');
    else workspace.cloning = false;
  }
</script>

<dialog
  bind:this={element}
  oncancel={(e) => {
    e.preventDefault();
    cancel();
  }}
>
  <form onsubmit={run}>
    <h2>Cloner un dépôt</h2>

    <label class="row">
      <span class="label">Adresse du dépôt</span>
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="field mono"
        type="text"
        placeholder="https://github.com/moi/projet.git ou git@github.com:moi/projet.git"
        spellcheck="false"
        autocomplete="off"
        autofocus
        disabled={running}
        bind:value={url}
      />
    </label>

    <div class="row">
      <label class="label" for="clone-parent">Cloner dans</label>
      <div class="pair">
        <input id="clone-parent" class="field mono" type="text" placeholder="Dossier parent" spellcheck="false" disabled={running} bind:value={parent} />
        <button class="btn" type="button" disabled={running} onclick={browse}><Icon name="folder" size={14} /> Parcourir…</button>
      </div>
    </div>

    <label class="row">
      <span class="label">Nom du dossier</span>
      <input class="field mono" type="text" spellcheck="false" disabled={running} bind:value={name} oninput={() => (nameEdited = true)} />
      <small>{dest ? `Le dépôt sera cloné dans ${dest}` : 'Le nom est proposé à partir de l’adresse.'}</small>
    </label>

    {#if running}
      <div class="progress" role="status">
        <div class="bar" class:known={!Number.isNaN(percent)}><span style:width="{Number.isNaN(percent) ? 35 : percent}%"></span></div>
        <small class="mono">{progress}</small>
      </div>
    {:else if error}
      <p class="error" role="alert"><Icon name="warning" /> <span>{error}</span></p>
    {/if}

    <footer>
      <button class="btn" type="button" onclick={cancel}>{running ? 'Interrompre le clonage' : 'Annuler'}</button>
      <button class="btn primary" type="submit" disabled={!ready}>{running ? `${phase || 'Clonage'}…` : 'Cloner'}</button>
    </footer>
  </form>
</dialog>

<style>
  dialog {
    width: min(560px, calc(100vw - 48px));
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
    padding: 18px 20px 16px;
  }
  h2 {
    margin: 0;
    font-size: var(--fs-title);
    font-weight: var(--fw-strong);
  }
  .row {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .row small {
    color: var(--muted);
    font-size: var(--fs-small);
    overflow-wrap: anywhere;
  }
  .pair {
    display: flex;
    gap: 8px;
  }
  .progress {
    display: grid;
    gap: 6px;
  }
  .progress small {
    color: var(--muted);
    font-size: var(--fs-small);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .bar {
    position: relative;
    height: 6px;
    border-radius: var(--radius-pill);
    background: var(--line);
    overflow: hidden;
  }
  .bar span {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: var(--radius-pill);
    background: var(--accent);
    animation: sweep var(--t-wait) var(--ease) infinite;
  }
  /* Pourcentage connu : la barre avance au lieu de balayer. */
  .bar.known span {
    animation: none;
    transition: width var(--t-med) var(--ease);
  }
  @keyframes sweep {
    from {
      transform: translateX(-100%);
    }
    to {
      transform: translateX(290%);
    }
  }
  .error {
    display: flex;
    gap: 8px;
    margin: 0;
    padding: 8px 10px;
    border-radius: var(--radius-s);
    background: var(--bad-soft);
    color: var(--bad);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    max-height: 160px;
    overflow: auto;
  }
  .error span {
    color: var(--ink);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
</style>
