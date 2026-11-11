<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { homeDir, join } from '@tauri-apps/api/path';
  import { untrack } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import Icon from '../ui/Icon.svelte';
  import { toasts } from '../ui/toasts.svelte';
  import { ssh } from './ssh.svelte';
  import { explain } from './reasons';
  import type { Host, SshError } from './types';

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
      forget();
      element.showModal();
    } else if (!editing && element.open) {
      element.close();
    }
  });

  // Fermer la fiche abandonne un test en cours : son résultat ne s'affichera nulle part.
  const close = () => {
    forget();
    ssh.editing = null;
  };

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

  // --- Tester la connexion (charte, section 9.21) ---------------------------------------------
  /** Ce que le test montre, entre les champs et les boutons. */
  type Test =
    | { step: 'idle' }
    | { step: 'running' }
    | { step: 'ok'; method: string; ms: number }
    | { step: 'fail'; message: string }
    | { step: 'hostKey'; fingerprint: string; algorithm: string }
    | { step: 'secret'; kind: 'passphrase' | 'password'; path: string; error: string };

  /** Ce qu'on sait pour ce test : rien n'est gardé une fois le résultat connu. */
  interface Attempt {
    accept?: string;
    remember?: boolean;
    keyPath?: string;
    passphrase?: string;
    password?: string;
  }

  let test = $state<Test>({ step: 'idle' });
  let attempt: Attempt = {};
  let secret = $state('');
  // Chaque test porte un numéro : le résultat d'un test abandonné (fiche fermée, champ changé) est ignoré.
  let run = 0;
  let secretEl = $state<HTMLInputElement>();

  /** Oublie le test : son résultat, ses secrets, et celui qui serait encore en cours. */
  function forget() {
    run++;
    attempt = {};
    secret = '';
    test = { step: 'idle' };
  }

  // Un champ de la connexion change : le résultat ne dit plus rien de la fiche, il s'efface.
  $effect(() => {
    if (!form) return;
    void [form.hostname, form.port, form.user, form.auth, form.keyPath, form.proxyJump];
    untrack(forget);
  });

  // Un secret est demandé : son champ prend le focus.
  $effect(() => {
    if (test.step === 'secret') secretEl?.focus();
  });

  const testable = $derived(valid && !form?.proxyJump && test.step !== 'running');

  async function startTest(next: Attempt = {}) {
    const host = current();
    if (!host || !valid || host.proxyJump) return;
    attempt = { ...attempt, ...next };
    secret = '';
    const user = host.user || ssh.defaultUser;
    if (!user) {
      test = { step: 'fail', message: `Indique un utilisateur${NB}: le compte de cette machine n'a pas pu être lu.` };
      return;
    }
    // Un mot de passe n'est jamais enregistré : il est demandé à chaque test, comme à chaque connexion.
    if (host.auth === 'password' && !attempt.password) {
      test = { step: 'secret', kind: 'password', path: '', error: '' };
      return;
    }
    const keyPath = attempt.keyPath ?? (host.auth === 'keyFile' ? host.keyPath : '');
    const auth =
      host.auth === 'password'
        ? { kind: 'password', password: attempt.password }
        : keyPath
          ? { kind: 'keyFile', path: keyPath, passphrase: attempt.passphrase ?? null }
          : { kind: 'auto' };
    const mine = ++run;
    test = { step: 'running' };
    try {
      const done = await invoke<{ knownHost: boolean; authMethod: string; connectMs: number }>('ssh_test', {
        params: {
          host: host.hostname.trim(),
          port: host.port || 22,
          user,
          auth,
          acceptFingerprint: attempt.accept ?? null,
          rememberHost: attempt.remember ?? false,
        },
      });
      if (mine !== run) return;
      attempt = {};
      test = { step: 'ok', method: done.authMethod, ms: done.connectMs };
    } catch (e) {
      if (mine !== run) return;
      const problem: SshError = e && typeof e === 'object' && 'kind' in e ? (e as SshError) : { kind: 'other', message: String(e) };
      if (problem.kind === 'hostKeyUnknown') {
        test = { step: 'hostKey', fingerprint: problem.fingerprint, algorithm: problem.algorithm };
        return;
      }
      if (problem.kind === 'needPassphrase') {
        const wrong = attempt.passphrase !== undefined;
        attempt = { ...attempt, passphrase: undefined };
        test = { step: 'secret', kind: 'passphrase', path: problem.path, error: wrong ? 'Cette phrase secrète ne déverrouille pas la clé.' : '' };
        return;
      }
      if (problem.kind === 'auth' && host.auth === 'password') {
        attempt = { ...attempt, password: undefined };
        test = { step: 'secret', kind: 'password', path: '', error: problem.message };
        return;
      }
      attempt = {};
      test = {
        step: 'fail',
        message:
          problem.kind === 'hostKeyChanged'
            ? `L'empreinte du serveur ne correspond plus à celle de la ligne ${problem.line} de known_hosts${NB}: le test est bloqué, comme une connexion.`
            : explain(problem.message),
      };
    }
  }

  function submitSecret() {
    if (test.step !== 'secret' || !secret) return;
    if (test.kind === 'passphrase') void startTest({ keyPath: test.path, passphrase: secret });
    else void startTest({ password: secret });
  }

  const NB = '\u00a0';
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

      <!-- Le test, seul sur sa ligne sous les champs ; son résultat juste dessous (charte, section 9.21). -->
      <div class="testing">
        <button
          class="btn"
          type="button"
          disabled={!testable}
          title={form.proxyJump ? 'Le rebond par bastion n’est pas encore pris en charge' : 'Se connecter avec ces valeurs, sans rien enregistrer ni ouvrir de terminal'}
          onclick={() => {
            attempt = {};
            void startTest();
          }}
        >
          {#if test.step === 'running'}<span class="spinner" aria-hidden="true"></span> Test en cours…{:else}Tester la connexion{/if}
        </button>
      </div>

      {#if test.step === 'ok'}
        <p class="test ok" role="status">
          <Icon name="check" size={14} />
          <span><b>Connexion réussie</b> <span class="how">{test.method}, en {Math.round(test.ms)}{NB}ms</span></span>
        </p>
      {:else if test.step === 'fail'}
        <p class="test bad" role="alert"><Icon name="warning" size={14} /> <span>{test.message}</span></p>
      {:else if test.step === 'hostKey'}
        <div class="test ask" role="group" aria-label="Serveur inconnu">
          <p>
            <b class="mono">{form.hostname}</b> n’est pas dans <code>known_hosts</code>. Vérifie que cette empreinte {test.algorithm} est
            bien celle du serveur avant de lui faire confiance{NB}:
          </p>
          <p class="print mono">{test.fingerprint}</p>
          <div class="row">
            <button class="btn small" type="button" onclick={() => test.step === 'hostKey' && void startTest({ accept: test.fingerprint, remember: true })}>
              Faire confiance et mémoriser
            </button>
            <button class="btn small ghost" type="button" onclick={() => test.step === 'hostKey' && void startTest({ accept: test.fingerprint, remember: false })}>
              Cette fois seulement
            </button>
          </div>
        </div>
      {:else if test.step === 'secret'}
        <div class="test ask" role="group" aria-label={test.kind === 'passphrase' ? 'Phrase secrète de la clé' : 'Mot de passe'}>
          <p>
            {#if test.kind === 'passphrase'}La clé <span class="mono">{test.path}</span> est protégée{NB}: sa phrase secrète est demandée pour le test, puis oubliée.
            {:else}Le mot de passe est demandé pour le test, puis oublié{NB}: il n’est jamais enregistré.{/if}
          </p>
          {#if test.error}<p class="err" role="alert">{test.error}</p>{/if}
          <div class="row">
            <input
              bind:this={secretEl}
              class="field"
              type="password"
              autocomplete="off"
              aria-label={test.kind === 'passphrase' ? 'Phrase secrète' : 'Mot de passe'}
              bind:value={secret}
              onkeydown={(e) => {
                // Entrée teste : elle n'enregistre pas la fiche.
                if (e.key === 'Enter') {
                  e.preventDefault();
                  submitSecret();
                }
              }}
            />
            <button class="btn small" type="button" disabled={!secret} onclick={submitSecret}>Tester</button>
          </div>
        </div>
      {/if}

      <footer>
        <button class="btn cancel" type="button" onclick={close}>Annuler</button>
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
  /* Les boutons du bas se tiennent un peu à l'écart du test et de son résultat : 24 px au lieu de 14. */
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 10px;
  }
  .testing {
    display: flex;
  }
  /* « Annuler » seul à gauche, les deux enregistrements à droite. */
  .cancel {
    margin-right: auto;
  }
  /* Le résultat du test, entre les champs et les boutons (charte, section 9.21). */
  .test {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin: 0;
    font-size: var(--fs-small);
  }
  .test :global(svg) {
    flex: none;
    margin-top: 1px;
  }
  .test.ok {
    color: var(--ok);
  }
  .test.ok .how {
    color: var(--muted);
  }
  .test.bad {
    color: var(--bad);
  }
  .test.ask {
    flex-direction: column;
    align-items: stretch;
    gap: 8px;
    padding: 10px 12px;
    border-radius: var(--radius-s);
    background: var(--bg);
    color: var(--ink);
  }
  .test.ask p {
    margin: 0;
  }
  .print {
    overflow-wrap: anywhere;
  }
  .err {
    color: var(--bad);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .row .field {
    flex: 1;
  }
</style>
