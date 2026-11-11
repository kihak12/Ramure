<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import Icon from '../ui/Icon.svelte';
  import { toasts } from '../ui/toasts.svelte';
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

  const mismatch = $derived(passphrase !== confirm);
  const ready = $derived(name.trim() !== '' && !mismatch && !busy);

  $effect(() => {
    if (!element) return;
    if (ssh.showKeys && !element.open) {
      creating = false;
      error = '';
      void ssh.loadKeys();
      element.showModal();
    } else if (!ssh.showKeys && element.open) {
      element.close();
    }
  });

  function startCreating() {
    // Premier nom libre : id_ed25519, puis id_ed25519_2, id_ed25519_3…
    const taken = new Set(ssh.keys.map((k) => k.name));
    let candidate = 'id_ed25519';
    for (let n = 2; taken.has(candidate); n++) candidate = `id_ed25519_${n}`;
    name = candidate;
    passphrase = '';
    confirm = '';
    comment = ssh.defaultUser ? `${ssh.defaultUser}@ramure` : '';
    error = '';
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
      toasts.ok('Clé créée', `${key.path}\n${key.fingerprint}`);
    } catch (err) {
      error = typeof err === 'string' ? err : String(err);
    } finally {
      busy = false;
    }
  }

  async function copy(key: KeyInfo) {
    try {
      await navigator.clipboard.writeText(key.public);
      toasts.ok('Clé publique copiée', `${key.name}.pub — à coller sur le serveur (~/.ssh/authorized_keys) ou sur la forge.`);
    } catch {
      toasts.err('Copie impossible', 'Le presse-papiers a refusé l’écriture.');
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
  onclick={(e) => {
    if (e.target === element) close();
  }}
>
  <div class="box">
    <header>
      <h2>Clés SSH</h2>
      <p>Les clés de <code>~/.ssh</code>. Elles y restent : Ramure n'en garde aucune copie.</p>
    </header>

    <div class="list">
      {#each ssh.keys as key (key.name)}
        <div class="key">
          <span class="glyph"><Icon name="tag" size={16} /></span>
          <div class="text">
            <b class="mono">{key.name}</b>
            <small>
              {key.algorithm.replace('ssh-', '')}{key.comment ? ` · ${key.comment}` : ''}
              {#if key.encrypted}· <span class="locked">protégée par une phrase secrète</span>{/if}
            </small>
            <small class="mono print">{key.fingerprint}</small>
          </div>
          <button class="btn small" type="button" onclick={() => void copy(key)}><Icon name="copy" size={14} /> Copier la clé publique</button>
        </div>
      {:else}
        <p class="none">Aucune clé dans ~/.ssh. Génère-en une pour te connecter sans mot de passe.</p>
      {/each}
    </div>

    {#if creating}
      <form class="create" onsubmit={generate}>
        <span class="label">Nouvelle clé ed25519</span>
        <div class="two">
          <label>
            <span>Nom du fichier</span>
            <!-- svelte-ignore a11y_autofocus -->
            <input class="field mono" type="text" spellcheck="false" autocomplete="off" autofocus bind:value={name} />
          </label>
          <label>
            <span>Commentaire</span>
            <input class="field mono" type="text" spellcheck="false" autocomplete="off" placeholder="moi@machine" bind:value={comment} />
          </label>
        </div>
        <div class="two">
          <label>
            <span>Phrase secrète (facultative)</span>
            <input class="field" type="password" autocomplete="new-password" bind:value={passphrase} />
          </label>
          <label>
            <span>Confirmation</span>
            <input class="field" type="password" autocomplete="new-password" bind:value={confirm} />
          </label>
        </div>
        {#if mismatch && confirm}<p class="err">Les deux phrases secrètes ne sont pas identiques.</p>{/if}
        {#if error}<p class="err">{error}</p>{/if}
        <div class="actions">
          <button class="btn" type="button" onclick={() => (creating = false)}>Annuler</button>
          <button class="btn primary" type="submit" disabled={!ready}>{busy ? 'Création…' : 'Générer la clé'}</button>
        </div>
      </form>
    {/if}

    <footer>
      {#if !creating}<button class="btn" type="button" onclick={startCreating}><Icon name="plus" size={14} /> Générer une clé</button>{/if}
      <button class="btn primary" type="button" onclick={close}>Fermer</button>
    </footer>
  </div>
</dialog>

<style>
  dialog {
    width: min(640px, calc(100vw - 48px));
    max-height: calc(100vh - 80px);
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
    padding: 18px 20px 10px;
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
  .list {
    flex: 1 1 auto;
    overflow-y: auto;
    min-height: 0;
    padding: 0 14px;
    display: grid;
    gap: 6px;
    align-content: start;
  }
  .key {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--panel);
  }
  .glyph {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: var(--radius);
    color: var(--accent);
    background: var(--accent-soft);
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
  .none {
    padding: 14px 6px;
  }
  .create {
    display: grid;
    gap: 10px;
    margin: 12px 14px 0;
    padding: 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--bg);
  }
  .two {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 10px;
  }
  .create label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: var(--fs-label);
    color: var(--muted);
  }
  .err {
    color: var(--bad);
  }
  .actions,
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  footer {
    padding: 14px 20px 16px;
  }
  footer .btn:first-child:not(.primary) {
    margin-right: auto;
  }
</style>
