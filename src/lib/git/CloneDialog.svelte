<script lang="ts">
  import { veil } from '../ui/veil';
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
  // Until the name has been edited, it follows the URL.
  let nameEdited = $state(false);
  let running = $state(false);
  let progress = $state('');
  let error = $state('');

  /** Same rule as `folder_name` on the Rust side: `https://host/me/project.git` → `project`. */
  function folderName(address: string) {
    const last = address.trim().replace(/[\\/]+$/, '').split(/[\\/:]/).pop() ?? '';
    return last.replace(/\.git$/, '');
  }

  const separator = $derived(parent.includes('\\') && !parent.includes('/') ? '\\' : '/');
  const dest = $derived(parent && name ? `${parent.replace(/[\\/]+$/, '')}${separator}${name}` : '');
  const nameValid = $derived(name.trim() !== '' && !/[\\/]/.test(name) && name !== '.' && name !== '..');
  const ready = $derived(url.trim() !== '' && parent.trim() !== '' && nameValid && !running);
  // “Receiving objects:  42% (…)” → 42
  const percent = $derived(Number(/(\d+)%/.exec(progress)?.[1] ?? NaN));

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
    const dir = await open({ directory: true, multiple: false, title: 'Folder to clone the repository into' });
    if (typeof dir === 'string') parent = dir;
  }

  async function run(event: SubmitEvent) {
    event.preventDefault();
    if (!ready) return;
    running = true;
    error = '';
    progress = 'Connecting to the remote repository…';
    const onProgress = new Channel<string>();
    onProgress.onmessage = (line) => (progress = line);
    try {
      const reply = await invoke<Done<string>>('git_clone', { url: url.trim(), dest, onProgress });
      try {
        localStorage.setItem(STORAGE_KEY, parent);
      } catch {
        /* storage unavailable */
      }
      workspace.cloning = false;
      toasts.ok('Repository cloned', reply.data);
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
  use:veil={() => {
    // During a clone, a stray click on the veil must not stop it (guidelines, 9.10).
    if (!running) cancel();
  }}
>
  <form onsubmit={run}>
    <!-- The content scrolls if the window is too low; the last line stays in view (guidelines, 9.10). -->
    <div class="content">
    <h2>Clone repository</h2>

    <label class="row">
      <span class="label">Repository URL</span>
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="field mono"
        type="text"
        placeholder="https://github.com/me/project.git or git@github.com:me/project.git"
        spellcheck="false"
        autocomplete="off"
        autofocus
        disabled={running}
        bind:value={url}
      />
    </label>

    <div class="row">
      <label class="label" for="clone-parent">Clone into</label>
      <div class="pair">
        <input id="clone-parent" class="field mono" type="text" placeholder="Parent folder" spellcheck="false" disabled={running} bind:value={parent} />
        <button class="btn" type="button" disabled={running} onclick={browse}><Icon name="folder" size={14} /> Browse…</button>
      </div>
    </div>

    <label class="row">
      <span class="label">Folder name</span>
      <input class="field mono" type="text" spellcheck="false" disabled={running} bind:value={name} oninput={() => (nameEdited = true)} />
    </label>

    <!-- A place kept under the fields: the destination sentence, then the progress of the clone in
         its place, without the buttons moving on a click on “Clone” (guidelines, 9.1, “Hierarchy”). -->
    <div class="status">
      {#if running}
        <div class="progress" role="status">
          <div class="bar" class:known={!Number.isNaN(percent)}><span style:width="{Number.isNaN(percent) ? 35 : percent}%"></span></div>
          <small class="mono">{progress}</small>
        </div>
      {:else if error}
        <p class="error" role="alert"><Icon name="warning" /> <span>{error}</span></p>
      {:else}
        <small>{dest ? `The repository will be cloned into ${dest}` : 'The name is suggested from the URL.'}</small>
      {/if}
    </div>
    </div>

    <footer>
      <button class="btn" type="button" onclick={cancel}>{running ? 'Stop cloning' : 'Cancel'}</button>
      <button class="btn primary" type="submit" disabled={!ready}>{running ? 'Cloning…' : 'Clone'}</button>
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
  /* High enough for the progress (a bar and a line) and for a one-line error: the sentence, the
     bar, then the error fit in it without pushing the buttons. */
  .status {
    min-height: 36px;
  }
  .status > small {
    color: var(--muted);
    font-size: var(--fs-small);
    overflow-wrap: anywhere;
  }
  h2 {
    margin: 0;
    font-size: var(--fs-title);
    font-weight: var(--fw-strong);
  }
  .row {
    display: flex;
    flex-direction: column;
    gap: 6px;
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
  /* Known percentage: the bar moves forward instead of sweeping. */
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
