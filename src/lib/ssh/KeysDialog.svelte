<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { homeDir } from '@tauri-apps/api/path';
  import { open } from '@tauri-apps/plugin-dialog';
  import Icon from '../ui/Icon.svelte';
  import { veil } from '../ui/veil';
  import { focusWithRing } from '../ui/focus';
  import { tick } from 'svelte';
  import { forgetSecret, secretsHeld } from './secrets';
  import { ssh } from './ssh.svelte';
  import type { KeyInfo } from './types';

  let element = $state<HTMLDialogElement>();
  let creating = $state(false);
  let busy = $state(false);
  let name = $state('');
  let passphrase = $state('');
  let confirm = $state('');
  let comment = $state('');
  let error = $state('');
  /** The result of the last action, said in the last line of the dialog and not in a
   *  notification, which the veil of the dialog would cover (guidelines, 9.11 and 9.15, "Keys"). */
  let result = $state<{ ok: boolean; text: string } | null>(null);
  const said = (e: unknown) => (typeof e === 'string' ? e : e instanceof Error ? e.message : String(e));

  // The keys whose passphrase the system keyring keeps (guidelines, section 9.15).
  let held = $state<Record<string, boolean>>({});
  let listEl = $state<HTMLElement>();
  $effect(() => {
    if (!ssh.showKeys) return;
    const locked = ssh.keys.filter((key) => key.encrypted);
    void secretsHeld(locked.map((key) => ({ kind: 'passphrase', path: key.path }))).then((answers) => {
      held = Object.fromEntries(locked.map((key, i) => [key.path, answers[i] === true]));
    });
  });

  async function forgetPassphrase(key: KeyInfo) {
    result = null;
    try {
      await forgetSecret({ kind: 'passphrase', path: key.path });
      held[key.path] = false;
      // The button is gone: the focus goes to the button next to it, of the same key.
      focusWithRing(listEl?.querySelector<HTMLElement>(`[data-key="${CSS.escape(key.name)}"] .copy`));
    } catch (e) {
      result = { ok: false, text: `The system keyring could not forget the passphrase of ${key.name}: ${said(e)}` };
    }
  }

  const mismatch = $derived(passphrase !== confirm);
  const ready = $derived(name.trim() !== '' && !mismatch && !busy);

  $effect(() => {
    if (!element) return;
    if (ssh.showKeys && !element.open) {
      creating = false;
      error = '';
      result = null;
      void ssh.loadKeys();
      element.showModal();
    } else if (!ssh.showKeys && element.open) {
      element.close();
    }
  });

  function startCreating() {
    // First free name: id_ed25519, then id_ed25519_2, id_ed25519_3…
    const taken = new Set(ssh.keys.map((k) => k.name));
    let candidate = 'id_ed25519';
    for (let n = 2; taken.has(candidate); n++) candidate = `id_ed25519_${n}`;
    name = candidate;
    passphrase = '';
    confirm = '';
    comment = ssh.defaultUser ? `${ssh.defaultUser}@ramure` : '';
    error = '';
    result = null;
    creating = true;
  }

  async function generate(e: SubmitEvent) {
    e.preventDefault();
    if (!ready) return;
    busy = true;
    error = '';
    try {
      const key = await invoke<KeyInfo>('ssh_key_generate', { name: name.trim(), passphrase, comment });
      await ssh.loadKeys();
      creating = false;
      passphrase = '';
      confirm = '';
      result = { ok: true, text: `Key created: ${key.path}` };
    } catch (err) {
      error = typeof err === 'string' ? err : String(err);
    } finally {
      busy = false;
    }
  }

  async function copy(key: KeyInfo) {
    try {
      await navigator.clipboard.writeText(key.public);
      result = { ok: true, text: `Public key of ${key.name} copied, ready to paste on the server or your Git host.` };
    } catch {
      result = { ok: false, text: 'The public key could not be copied: the clipboard refused it.' };
    }
  }

  // Importing a key from elsewhere: it is copied into ~/.ssh with its public key.
  let importing = $state(false);
  let importEl = $state<HTMLButtonElement>();
  async function importKey() {
    let home: string | undefined;
    try {
      home = await homeDir();
    } catch {
      home = undefined; // no home folder: the picker opens where the system decides
    }
    let picked: string | null;
    try {
      picked = await open({ title: 'Choose a private key', multiple: false, directory: false, defaultPath: home });
    } catch (e) {
      result = { ok: false, text: `The file picker could not open: ${said(e)}` };
      return;
    }
    if (!picked) return;
    importing = true;
    result = null;
    try {
      const key = await invoke<KeyInfo>('ssh_key_import', { path: picked });
      await ssh.loadKeys();
      result = { ok: true, text: `Key imported: ${key.path}` };
    } catch (err) {
      result = { ok: false, text: said(err) };
    } finally {
      importing = false;
      // The button, disabled during the import, had lost the focus, which fell on the page or on
      // the dialog: it gets it back, with its ring (guidelines, 11).
      const active = document.activeElement;
      if (!active || active === document.body || active === element) await tick().then(() => focusWithRing(importEl));
    }
  }

  const close = () => (ssh.showKeys = false);
</script>

<dialog
  bind:this={element}
  oncancel={(e) => {
    e.preventDefault();
    close();
  }}
  use:veil={close}
>
  <div class="box">
    <header>
      <h2>SSH keys</h2>
      <p>The keys of <code>~/.ssh</code>. They stay there: Ramure keeps no copy of them.</p>
    </header>

    <div class="list" bind:this={listEl}>
      {#each ssh.keys as key (key.name)}
        <div class="key" data-key={key.name}>
          <span class="glyph"><Icon name="key" size={16} /></span>
          <div class="text">
            <b class="mono">{key.name}</b>
            <small>
              {key.algorithm.replace('ssh-', '')}{key.comment ? ` · ${key.comment}` : ''}
              {#if key.encrypted}· <span class="locked">protected by a passphrase</span>{/if}
            </small>
            <small class="mono print">{key.fingerprint}</small>
            {#if held[key.path]}
              <!-- What the keyring keeps for this key, and how to forget it. -->
              <span class="kept">
                <small>Its passphrase is kept in the system keyring.</small>
                <!-- Two keys can carry this button: its accessible name says which. -->
                <button class="btn ghost small" type="button" aria-label="Forget passphrase of {key.name}" onclick={() => void forgetPassphrase(key)}>
                  Forget passphrase
                </button>
              </span>
            {/if}
          </div>
          <button class="btn small copy" type="button" onclick={() => void copy(key)}><Icon name="copy" size={14} /> Copy public key</button>
        </div>
      {:else}
        <p class="none">No keys in ~/.ssh. Generate one to connect without a password.</p>
      {/each}
    </div>

    {#if creating}
      <form class="create" id="new-key" onsubmit={generate}>
        <span class="label">New ed25519 key</span>
        <div class="two">
          <label>
            <span>File name</span>
            <!-- svelte-ignore a11y_autofocus -->
            <input class="field mono" type="text" spellcheck="false" autocomplete="off" autofocus bind:value={name} />
          </label>
          <label>
            <span>Comment</span>
            <input class="field mono" type="text" spellcheck="false" autocomplete="off" placeholder="me@machine" bind:value={comment} />
          </label>
        </div>
        <div class="two">
          <label>
            <span>Passphrase (optional)</span>
            <input class="field" type="password" autocomplete="new-password" bind:value={passphrase} />
          </label>
          <label>
            <span>Confirm passphrase</span>
            <input class="field" class:invalid={mismatch && !!confirm} type="password" autocomplete="new-password" aria-invalid={mismatch && confirm ? 'true' : undefined} bind:value={confirm} />
          </label>
        </div>
        <!-- A line kept for the errors: when one shows, the buttons do not move (guidelines, 9.1). -->
        <p class="err" role="alert">
          {#if mismatch && confirm}The two passphrases do not match.{:else if error}{error}{/if}
        </p>
      </form>
    {/if}

    <!-- The way out, then the action, on the right (guidelines, 9.1, "Hierarchy"). A plain way out is
         never the main button: at rest, the dialog has none; while a key is being created, it is
         "Create key", and "Cancel" goes back to the list. -->
    <footer>
      <!-- The result of the last action, on the left of the buttons, which stay at the top of the line. -->
      <div class="result">
        <p role="status">{#if result?.ok && !creating}<Icon name="check" size={14} /><span>{result.text}</span>{/if}</p>
        {#if result && !result.ok && !creating}<p class="bad" role="alert"><Icon name="warning" size={14} /><span>{result.text}</span></p>{/if}
      </div>
      {#if creating}
        <button class="btn" type="button" onclick={() => (creating = false)}>Cancel</button>
        <button class="btn primary" type="submit" form="new-key" disabled={!ready}>{busy ? 'Creating…' : 'Create key'}</button>
      {:else}
        <button class="btn" type="button" onclick={close}>Close</button>
        <button class="btn" type="button" bind:this={importEl} disabled={importing} onclick={() => void importKey()}>
          <Icon name="import" size={14} />
          {importing ? 'Importing…' : 'Import key…'}
        </button>
        <button class="btn" type="button" onclick={startCreating}><Icon name="plus" size={14} /> Generate key</button>
      {/if}
    </footer>
  </div>
</dialog>

<style>
  dialog {
    width: min(640px, calc(100vw - 48px));
    padding: 0;
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--raised);
    color: var(--ink);
    box-shadow: var(--shadow-pop);
  }
  dialog[open] {
    display: flex;
    animation: pop-in var(--t-med) var(--ease);
  }
  dialog::backdrop {
    background: var(--scrim);
    backdrop-filter: blur(2px);
    animation: fade-in var(--t-med) var(--ease);
  }
  .box {
    display: flex;
    flex-direction: column;
    width: 100%;
    min-height: 0;
  }
  header {
    padding: 16px 20px 8px;
    display: grid;
    gap: 6px;
  }
  h2 {
    margin: 0;
    font-size: var(--fs-title);
    font-weight: var(--fw-strong);
  }
  p {
    margin: 0;
    color: var(--muted);
  }
  code {
    font-family: var(--f-mono);
    font-size: var(--fs-code);
  }
  /* Plain rows, separated by a --line rule (--line-soft does not show on --raised): a framed row
     inside a dialog would be a card in a card (guidelines, sections 6 and 9.6). */
  .list {
    flex: 1 1 auto;
    overflow-y: auto;
    min-height: 0;
    padding: 0 20px;
    display: grid;
    align-content: start;
  }
  .key {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 0;
  }
  .key + .key {
    border-top: 1px solid var(--line);
  }
  /* The icon alone, with no square behind it: a --hover background would not show on --raised in
     dark. It informs: no accent, which is kept for the primary, the focus and the active element
     (guidelines, 2.3). */
  .glyph {
    flex: none;
    display: grid;
    color: var(--muted);
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    line-height: 1.35;
  }
  .text small {
    color: var(--muted);
    font-size: var(--fs-small);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .print {
    font-size: var(--fs-small) !important;
  }
  .locked {
    color: var(--ok);
  }
  /* The button, with no background nor border, is pulled back by its inner margin (8 px and 1 px
     of border): its text lines up with the sentence. */
  .kept {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    margin-top: 4px;
  }
  .kept small {
    white-space: normal;
  }
  .kept .btn {
    margin-left: -9px;
  }
  .none {
    padding: 12px 0;
  }
  .create {
    display: grid;
    gap: 12px;
    margin: 4px 20px 0;
    padding-top: 12px;
    border-top: 1px solid var(--line);
  }
  .two {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 12px;
  }
  .create label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: var(--fs-label);
    color: var(--muted);
  }
  /* The line of the errors keeps its height, empty or not. */
  .err {
    min-height: calc(var(--fs-small) * 1.35);
    margin: 0;
    font-size: var(--fs-small);
    color: var(--bad);
  }
  footer {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 16px 20px;
  }
  footer .btn {
    flex: none;
  }
  /* The result: a status region always there for a success, an alert for a failure. The text
     lines up roughly with that of the buttons, 30 px high. */
  .result {
    flex: 1;
    min-width: 0;
    padding-top: 8px;
    font-size: var(--fs-small);
    line-height: 1.35;
    overflow-wrap: anywhere;
  }
  .result p {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: 0;
    color: var(--ok);
  }
  .result p.bad {
    color: var(--bad);
  }
  .result :global(svg) {
    flex: none;
  }
</style>
