<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import Icon from '../ui/Icon.svelte';
  import { veil } from '../ui/veil';
  import { toasts } from '../ui/toasts.svelte';
  import { focusWithRing } from '../ui/focus';
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

  // Les clés dont le trousseau du système retient la phrase secrète (charte, section 9.15).
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
    try {
      await forgetSecret({ kind: 'passphrase', path: key.path });
      held[key.path] = false;
      // Le bouton a disparu : le focus va au bouton voisin de la même clé.
      focusWithRing(listEl?.querySelector<HTMLElement>(`[data-key="${CSS.escape(key.name)}"] .copy`));
    } catch (e) {
      toasts.err('Le trousseau du système n’a pas pu l’oublier', typeof e === 'string' ? e : String(e));
    }
  }

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
  use:veil={close}
>
  <div class="box">
    <header>
      <h2>Clés SSH</h2>
      <p>Les clés de <code>~/.ssh</code>. Elles y restent : Ramure n’en garde aucune copie.</p>
    </header>

    <div class="list" bind:this={listEl}>
      {#each ssh.keys as key (key.name)}
        <div class="key" data-key={key.name}>
          <span class="glyph"><Icon name="key" size={16} /></span>
          <div class="text">
            <b class="mono">{key.name}</b>
            <small>
              {key.algorithm.replace('ssh-', '')}{key.comment ? ` · ${key.comment}` : ''}
              {#if key.encrypted}· <span class="locked">protégée par une phrase secrète</span>{/if}
            </small>
            <small class="mono print">{key.fingerprint}</small>
            {#if held[key.path]}
              <!-- Ce que le trousseau retient pour cette clé, et de quoi l'oublier. -->
              <span class="kept">
                <small>Sa phrase secrète est retenue dans le trousseau du système.</small>
                <!-- Deux clés peuvent porter ce bouton : son nom accessible dit laquelle. -->
                <button class="btn ghost small" type="button" aria-label="Oublier la phrase secrète de {key.name}" onclick={() => void forgetPassphrase(key)}>
                  Oublier la phrase secrète
                </button>
              </span>
            {/if}
          </div>
          <button class="btn small copy" type="button" onclick={() => void copy(key)}><Icon name="copy" size={14} /> Copier la clé publique</button>
        </div>
      {:else}
        <p class="none">Aucune clé dans ~/.ssh. Génère-en une pour te connecter sans mot de passe.</p>
      {/each}
    </div>

    {#if creating}
      <form class="create" id="new-key" onsubmit={generate}>
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
            <input class="field" class:invalid={mismatch && !!confirm} type="password" autocomplete="new-password" aria-invalid={mismatch && confirm ? 'true' : undefined} bind:value={confirm} />
          </label>
        </div>
        <!-- Une ligne gardée pour les erreurs : quand l'une paraît, les boutons ne bougent pas (charte, 9.1). -->
        <p class="err" role="alert">
          {#if mismatch && confirm}Les deux phrases secrètes ne sont pas identiques.{:else if error}{error}{/if}
        </p>
      </form>
    {/if}

    <!-- La sortie, puis l'action, à droite (charte, 9.1, « Hierarchy »). Une simple sortie n'est jamais
         le bouton principal : au repos, la boîte n'en a pas ; pendant une création, c'est « Générer
         la clé », et « Annuler » revient à la liste. -->
    <footer>
      {#if creating}
        <button class="btn" type="button" onclick={() => (creating = false)}>Annuler</button>
        <button class="btn primary" type="submit" form="new-key" disabled={!ready}>{busy ? 'Création…' : 'Générer la clé'}</button>
      {:else}
        <button class="btn" type="button" onclick={close}>Fermer</button>
        <button class="btn" type="button" onclick={startCreating}><Icon name="plus" size={14} /> Générer une clé</button>
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
  /* Des lignes simples, séparées par un filet --line (--line-soft ne se voit pas sur --raised) :
     une ligne encadrée dans une boîte serait une carte dans une carte (charte, sections 6 et 9.6). */
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
  .glyph {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: var(--radius);
    /* Une pastille d'icône informe : elle n'a pas l'accent, réservé au principal, au focus et à
       l'élément actif (charte, section 2.3). */
    color: var(--muted);
    background: var(--hover);
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
  /* Le bouton, sans fond ni bordure, est tiré de sa marge intérieure (8 px et 1 px de bordure) :
     son texte s'aligne sur la phrase. */
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
    padding-top: 14px;
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
  /* La ligne des erreurs garde sa hauteur, vide ou non. */
  .err {
    min-height: calc(var(--fs-small) * 1.35);
    margin: 0;
    font-size: var(--fs-small);
    color: var(--bad);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 16px 20px;
  }
</style>
