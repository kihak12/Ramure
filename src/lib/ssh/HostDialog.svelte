<script lang="ts">
  import { homeDir, join } from '@tauri-apps/api/path';
  import { open } from '@tauri-apps/plugin-dialog';
  import Icon from '../ui/Icon.svelte';
  import { toasts } from '../ui/toasts.svelte';
  import { ssh } from './ssh.svelte';
  import type { Host } from './types';

  let element = $state<HTMLDialogElement>();
  let form = $state<Host | null>(null);
  let tags = $state('');
  /** Remarque affichée sous le champ de la clé après un choix dans la fenêtre de fichiers. */
  let keyNote = $state('');

  const editing = $derived(ssh.editing);
  const valid = $derived(!!form && form.alias.trim() !== '' && form.hostname.trim() !== '');
  const COLORS = ['0', '1', '2', '3', '4', '5', '6', '7'];

  $effect(() => {
    if (!element) return;
    if (editing && !element.open) {
      form = { ...editing, tags: [...editing.tags] };
      tags = editing.tags.join(', ');
      keyNote = '';
      element.showModal();
    } else if (!editing && element.open) {
      element.close();
    }
  });

  const close = () => (ssh.editing = null);

  /** Fiche à jour, tags compris. */
  function current(): Host | null {
    if (!form) return null;
    return { ...form, port: Number(form.port) || 22, tags: tags.split(',').map((t) => t.trim()).filter(Boolean) };
  }

  async function save(connect: boolean) {
    const host = current();
    if (!host || !valid) return;
    const saved = await ssh.save(host);
    if (!saved) return;
    close();
    if (connect) ssh.open(saved);
  }

  /** Choisit la clé privée dans la fenêtre de fichiers du système, ouverte dans `~/.ssh`. */
  async function browseKey() {
    let home = '';
    let folder: string | undefined;
    try {
      home = (await homeDir()).replace(/[\\/]+$/, '');
      folder = await join(home, '.ssh');
    } catch {
      folder = undefined; // dossier personnel introuvable : la fenêtre s'ouvre où le système le décide
    }
    let picked: string | null;
    try {
      picked = await open({ title: 'Choisir une clé privée', multiple: false, directory: false, defaultPath: folder });
    } catch (e) {
      toasts.err("La fenêtre de fichiers n'a pas pu s'ouvrir", e instanceof Error ? e.message : String(e));
      return;
    }
    // Fenêtre annulée, ou fiche fermée entre-temps : le champ ne change pas.
    if (!picked || !form) return;
    keyNote = '';
    if (picked.endsWith('.pub')) {
      // La connexion se fait avec la clé privée, rangée à côté sous le même nom.
      picked = picked.slice(0, -'.pub'.length);
      keyNote = 'Tu as choisi la clé publique : le champ pointe sur la clé privée du même nom, qui sert à se connecter.';
    }
    // Dans le dossier personnel, le chemin s'écrit `~/…`, comme les clés proposées par le champ.
    if (home && picked.startsWith(home) && /^[\\/]/.test(picked.slice(home.length))) picked = `~${picked.slice(home.length)}`;
    form.keyPath = picked;
  }

  /** Connexion ponctuelle : rien n'est inscrit dans le carnet. */
  function connectOnce() {
    const host = current();
    if (!host || !valid) return;
    close();
    ssh.open(host);
  }
</script>

<dialog
  bind:this={element}
  oncancel={(e) => {
    e.preventDefault();
    close();
  }}
>
  {#if form}
    <form
      onsubmit={(e) => {
        e.preventDefault();
        void save(false);
      }}
    >
      <h2>{form.id ? `Modifier ${editing?.alias ?? "l'hôte"}` : 'Nouvel hôte'}</h2>

      {#if form.proxyJump}
        <p class="warning">
          <Icon name="warning" />
          <span>Cet hôte passe par un bastion (ProxyJump). Il est dans le carnet, mais le rebond par bastion n'est pas encore pris en charge.</span>
        </p>
      {/if}

      <div class="grid">
        <label class="full">
          <span class="label">Nom</span>
          <!-- svelte-ignore a11y_autofocus -->
          <input class="field" type="text" placeholder="vps-prod" spellcheck="false" autocomplete="off" autofocus bind:value={form.alias} />
        </label>
        <label class="most">
          <span class="label">Adresse</span>
          <input class="field mono" type="text" placeholder="serveur.example.net" spellcheck="false" autocomplete="off" bind:value={form.hostname} />
        </label>
        <label>
          <span class="label">Port</span>
          <input class="field mono" type="number" min="1" max="65535" bind:value={form.port} />
        </label>
        <label class="full">
          <span class="label">Utilisateur</span>
          <input class="field mono" type="text" placeholder={ssh.defaultUser || 'utilisateur'} spellcheck="false" autocomplete="off" bind:value={form.user} />
        </label>

        <label class="full">
          <span class="label">Authentification</span>
          <select class="field" bind:value={form.auth}>
            <option value="auto">Agent SSH, puis clés par défaut de ~/.ssh</option>
            <option value="keyFile">Fichier de clé</option>
            <option value="password">Mot de passe (demandé à chaque connexion)</option>
          </select>
        </label>
        {#if form.auth === 'keyFile'}
          <div class="full key">
            <label for="host-key"><span class="label">Clé privée</span></label>
            <div class="pick">
              <input
                id="host-key"
                class="field mono"
                type="text"
                list="host-keys"
                placeholder="~/.ssh/id_ed25519"
                spellcheck="false"
                autocomplete="off"
                bind:value={form.keyPath}
                oninput={() => (keyNote = '')}
              />
              <button class="btn" type="button" onclick={() => void browseKey()}><Icon name="folder" size={14} /> Parcourir…</button>
            </div>
            <datalist id="host-keys">
              {#each ssh.keys as key (key.path)}<option value={key.path}>{key.algorithm}{key.comment ? ` · ${key.comment}` : ''}</option>{/each}
            </datalist>
            {#if keyNote}
              <small class="note" role="status">{keyNote}</small>
            {:else}
              <small>Si la clé est protégée, sa phrase secrète est demandée à la connexion.</small>
            {/if}
          </div>
        {/if}

        <label class="full">
          <span class="label">Commande au démarrage</span>
          <input class="field mono" type="text" placeholder="cd /srv/docker" spellcheck="false" autocomplete="off" bind:value={form.startupCommand} />
          <small>Envoyée dans le terminal dès que le shell est prêt.</small>
        </label>

        <label class="half">
          <span class="label">Groupe</span>
          <input class="field" type="text" list="host-groups" placeholder="Production" autocomplete="off" bind:value={form.group} />
          <datalist id="host-groups">
            {#each ssh.groups as group (group)}<option value={group}></option>{/each}
          </datalist>
        </label>
        <label class="half">
          <span class="label">Tags</span>
          <input class="field" type="text" placeholder="web, docker" autocomplete="off" bind:value={tags} />
        </label>

        <fieldset class="full">
          <legend class="label">Couleur</legend>
          <label class="swatch none" class:on={form.color === ''} title="Aucune">
            <input type="radio" name="host-color" value="" bind:group={form.color} /><span class="sr">Aucune</span>
          </label>
          {#each COLORS as color (color)}
            <label class="swatch" class:on={form.color === color} style:--c="var(--l{color})" title="Couleur {Number(color) + 1}">
              <input type="radio" name="host-color" value={color} bind:group={form.color} /><span class="sr">Couleur {Number(color) + 1}</span>
            </label>
          {/each}
        </fieldset>
      </div>

      <footer>
        <button class="btn ghost once" type="button" disabled={!valid} onclick={connectOnce} title="Ouvre un terminal sans inscrire l'hôte dans le carnet">
          Connecter sans enregistrer
        </button>
        <button class="btn" type="button" onclick={close}>Annuler</button>
        <button class="btn" type="button" disabled={!valid} onclick={() => void save(true)}>Enregistrer et connecter</button>
        <button class="btn primary" type="submit" disabled={!valid}>Enregistrer</button>
      </footer>
    </form>
  {/if}
</dialog>

<style>
  dialog {
    width: min(600px, calc(100vw - 48px));
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
    gap: 14px;
    padding: 18px 20px 16px;
  }
  h2 {
    margin: 0;
    font-size: var(--fs-title);
    font-weight: var(--fw-strong);
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
  .grid {
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    gap: 10px 12px;
  }
  .grid label,
  .key,
  fieldset {
    display: flex;
    flex-direction: column;
    gap: 5px;
    min-width: 0;
  }
  /* Le champ de la clé et son bouton « Parcourir… » côte à côte. */
  .pick {
    display: flex;
    gap: 8px;
  }
  .pick .field {
    flex: 1;
  }
  .note {
    color: var(--warn);
  }
  /* Six colonnes : l'adresse en prend cinq, le port une. */
  .full {
    grid-column: 1 / -1;
  }
  .most {
    grid-column: span 5;
  }
  .half {
    grid-column: span 3;
  }
  small {
    color: var(--muted);
    font-size: var(--fs-small);
  }
  fieldset {
    flex-direction: row;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin: 0;
    padding: 0;
    border: 0;
  }
  legend {
    float: left;
    width: 100%;
    padding: 0;
    margin-bottom: 4px;
  }
  .swatch {
    flex: none;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: var(--c);
    cursor: pointer;
    border: 2px solid transparent;
    transition:
      transform var(--t-fast) var(--ease),
      box-shadow var(--t-fast) var(--ease);
  }
  .swatch.none {
    background: none;
    border: 2px dashed var(--line);
  }
  .swatch:hover {
    transform: scale(1.12);
  }
  .swatch.on {
    box-shadow:
      0 0 0 2px var(--raised),
      0 0 0 4px var(--c, var(--muted));
  }
  .swatch input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .once {
    margin-right: auto;
    color: var(--accent);
  }
</style>
