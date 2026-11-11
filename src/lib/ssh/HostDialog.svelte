<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { homeDir, join } from '@tauri-apps/api/path';
  import { tick, untrack } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import Icon from '../ui/Icon.svelte';
  import Select from '../ui/Select.svelte';
  import { ssh } from './ssh.svelte';
  import { focusWithRing } from '../ui/focus';
  import { veil } from '../ui/veil';
  import { explain } from './reasons';
  import { forgetSecret, keyringAvailable, NO_KEYRING, secretsHeld, type SecretRef } from './secrets';
  import type { Host, SshError } from './types';

  let element = $state<HTMLDialogElement>();
  let form = $state<Host | null>(null);
  let tags = $state('');
  /** Remarque affichée sous le champ de la clé après un choix dans la fenêtre de fichiers. */
  let keyNote = $state('');
  /** Ce qui a échoué dans la fiche, dit sous l'élément qui l'a déclenché, jamais dans une
   *  notification, que le voile de la boîte recouvrirait (charte, 9.11). */
  let keyError = $state('');
  let forgetError = $state<{ password: string; passphrase: string }>({ password: '', passphrase: '' });
  /** La fiche n'a pas pu être enregistrée ou supprimée : dit dans sa dernière ligne. */
  let failure = $state('');

  const editing = $derived(ssh.editing);
  const valid = $derived(!!form && form.alias.trim() !== '' && form.hostname.trim() !== '');
  // Un hôte qu'on ajoute (ou une copie) n'est pas encore au carnet : on s'y connecte presque
  // toujours aussitôt, l'enregistrement connecte. Un hôte qu'on modifie s'enregistre seulement.
  const adding = $derived(!editing?.id);
  /** Ce qui manque tant que la fiche n'est pas valide : dit à côté du test et dans les infobulles (9.1). */
  const MISSING = 'Indique un nom et une adresse.';
  // Entrée dans une fiche incomplète : rien ne s'enregistre, la phrase passe en avertissement (9.1).
  let tried = $state(false);
  $effect(() => {
    if (valid) tried = false;
  });
  const COLORS = ['0', '1', '2', '3', '4', '5', '6', '7'];

  $effect(() => {
    if (!element) return;
    if (editing && !element.open) {
      form = { ...editing, tags: [...editing.tags] };
      tags = editing.tags.join(', ');
      keyNote = '';
      forget();
      element.showModal();
      // À la première ouverture, les champs paraissent après la boîte : `autofocus` n'y joue pas. Le
      // focus va ensuite au premier champ, « Nom » (charte, 9.10).
      void tick().then(() => element?.querySelector<HTMLInputElement>('input[autofocus]')?.focus());
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

  // « Supprimer l'hôte… » : seulement pour un hôte déjà au carnet, pas pour un ajout ni une copie.
  let removeButton = $state<HTMLButtonElement>();
  async function removeHost() {
    if (!editing?.id) return;
    // La fiche telle qu'elle est enregistrée : c'est son compte qui dit quel mot de passe part avec elle.
    const stored = ssh.hosts.find((h) => h.id === editing.id) ?? editing;
    failure = '';
    if (await ssh.removeAsked(stored, (text) => (failure = text))) {
      close();
      await ssh.focusBook();
    }
    // Annulée : la confirmation se ferme, le clavier revient au bouton qui l'a ouverte, avec l'anneau.
    else focusWithRing(removeButton);
  }

  async function save(connect: boolean) {
    const host = current();
    if (!host || !valid) return;
    failure = '';
    const saved = await ssh.save(host);
    if (typeof saved === 'string') {
      failure = saved;
      return;
    }
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
      keyError = `La fenêtre de fichiers n’a pas pu s’ouvrir : ${e instanceof Error ? e.message : String(e)}`;
      return;
    }
    // Fenêtre annulée, ou fiche fermée entre-temps : le champ ne change pas.
    if (!picked || !form) return;
    keyNote = '';
    keyError = '';
    if (picked.endsWith('.pub')) {
      // La connexion se fait avec la clé privée, rangée à côté sous le même nom.
      picked = picked.slice(0, -'.pub'.length);
      keyNote = 'Tu as choisi la clé publique : le champ pointe sur la clé privée du même nom, qui sert à se connecter.';
    }
    // Dans le dossier personnel, le chemin s'écrit `~/…`, comme les clés proposées par le champ.
    if (home && picked.startsWith(home) && /^[\\/]/.test(picked.slice(home.length))) picked = `~${picked.slice(home.length)}`;
    form.keyPath = picked;
  }

  // --- Secrets retenus dans le trousseau du système (charte, section 9.21) -----------------------
  // Le mot de passe appartient au compte (utilisateur, serveur, port), la phrase secrète à la clé :
  // la fiche montre ce que le trousseau retient pour les siens, et permet de l'oublier.
  let keyring = $state<boolean | null>(null);
  let held = $state({ password: false, passphrase: false });
  let authEl = $state<HTMLSelectElement>();
  let keyEl = $state<HTMLInputElement>();

  const passwordRef = $derived.by((): SecretRef | null => {
    if (!form || form.auth !== 'password') return null;
    const user = form.user.trim() || ssh.defaultUser;
    const host = form.hostname.trim();
    return user && host ? { kind: 'password', user, host, port: Number(form.port) || 22 } : null;
  });
  const passphraseRef = $derived.by((): SecretRef | null => {
    if (!form || form.auth !== 'keyFile' || !form.keyPath.trim()) return null;
    return { kind: 'passphrase', path: form.keyPath.trim() };
  });

  // Chaque lecture porte un numéro : la réponse d'une lecture dépassée est ignorée.
  let reading = 0;
  async function readHeld() {
    const mine = ++reading;
    const refs = [passwordRef, passphraseRef];
    const answers = await secretsHeld(refs.filter((ref) => ref !== null));
    if (mine !== reading) return;
    held = { password: !!passwordRef && answers[0] === true, passphrase: !!passphraseRef && answers[passwordRef ? 1 : 0] === true };
  }

  // La fiche s'ouvre, ou un champ de la connexion change : on relit, une fois la frappe posée.
  $effect(() => {
    if (!form) return;
    void [passwordRef, passphraseRef];
    held = { password: false, passphrase: false };
    const timer = window.setTimeout(() => void readHeld(), 250);
    return () => window.clearTimeout(timer);
  });
  $effect(() => {
    if (editing) void keyringAvailable().then((answer) => (keyring = answer));
  });

  /** Oublie le secret retenu ; le focus va au champ dont il dépend, le bouton ayant disparu. */
  async function forgetHeld(kind: 'password' | 'passphrase') {
    const ref = kind === 'password' ? passwordRef : passphraseRef;
    if (!ref) return;
    forgetError[kind] = '';
    try {
      await forgetSecret(ref);
      held[kind] = false;
      focusWithRing(kind === 'password' ? authEl : keyEl);
    } catch (e) {
      forgetError[kind] = `Le trousseau du système n’a pas pu l’oublier : ${typeof e === 'string' ? e : String(e)}`;
    }
  }

  // --- Tester la connexion (charte, section 9.21) ---------------------------------------------
  /** Ce que le test montre, entre les champs et les boutons. */
  type Test =
    | { step: 'idle' }
    | { step: 'running' }
    /** `learnFailed` : l'empreinte devait être mémorisée et `known_hosts` n'a pas pu être écrit. */
    | { step: 'ok'; method: string; ms: number; learnFailed: string | null }
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
  let testButton = $state<HTMLButtonElement>();

  // Le bouton cliqué a disparu (une question), ou s'est désactivé pendant le test : une fois le
  // résultat affiché, le clavier revient sur « Tester la connexion » plutôt que de se perdre.
  $effect(() => {
    if (test.step !== 'ok' && test.step !== 'fail') return;
    const lost = !document.activeElement || document.activeElement === document.body || document.activeElement === element;
    if (lost) void tick().then(() => testButton?.focus());
  });

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
      test = { step: 'fail', message: `Indique un utilisateur${NB}: le compte de cette machine n’a pas pu être lu.` };
      return;
    }
    const keyPath = attempt.keyPath ?? (host.auth === 'keyFile' ? host.keyPath : '');
    const auth =
      host.auth === 'password'
        ? // Sans mot de passe tapé, le test prend celui que le trousseau retient, comme une connexion.
          { kind: 'password', password: attempt.password ?? null }
        : keyPath
          ? { kind: 'keyFile', path: keyPath, passphrase: attempt.passphrase ?? null }
          : { kind: 'auto' };
    const mine = ++run;
    test = { step: 'running' };
    try {
      const done = await invoke<{ knownHost: boolean; learnFailed?: string | null; authMethod: string; connectMs: number }>('ssh_test', {
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
      test = { step: 'ok', method: done.authMethod, ms: done.connectMs, learnFailed: done.learnFailed ?? null };
    } catch (e) {
      if (mine !== run) return;
      const problem: SshError = e && typeof e === 'object' && 'kind' in e ? (e as SshError) : { kind: 'other', message: String(e) };
      if (problem.kind === 'hostKeyUnknown') {
        test = { step: 'hostKey', fingerprint: problem.fingerprint, algorithm: problem.algorithm };
        return;
      }
      if (problem.kind === 'needPassphrase') {
        attempt = { ...attempt, passphrase: undefined };
        const error =
          problem.refused === 'typed'
            ? 'Cette phrase secrète ne déverrouille pas la clé.'
            : problem.refused === 'retained'
              ? 'La phrase secrète retenue ne déverrouille plus la clé. Elle a été oubliée.'
              : '';
        test = { step: 'secret', kind: 'passphrase', path: problem.path, error };
        // Un secret retenu qui est refusé est oublié : la fiche ne le montre plus.
        if (problem.refused === 'retained') void readHeld();
        return;
      }
      if (problem.kind === 'needPassword') {
        attempt = { ...attempt, password: undefined };
        test = { step: 'secret', kind: 'password', path: '', error: problem.refused ? 'Le serveur a refusé le mot de passe retenu. Il a été oublié.' : '' };
        if (problem.refused) void readHeld();
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
            ? `L’empreinte du serveur ne correspond plus à celle de la ligne ${problem.line} de known_hosts${NB}: le test est bloqué, comme une connexion.`
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
  use:veil={close}
>
  {#if form}
    <!-- La touche Entrée est seulement observée : les champs restent les éléments interactifs. -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <form
      onsubmit={(e) => {
        e.preventDefault();
        void save(adding);
      }}
      onkeydown={(e) => {
        // Entrée dans un champ d'une fiche incomplète : le bouton grisé ne fait rien, la phrase le dit.
        if (e.key === 'Enter' && !valid && e.target instanceof HTMLInputElement) tried = true;
      }}
    >
      <!-- Les champs défilent si la fenêtre est trop basse ; la dernière ligne reste en vue (charte, 9.10). -->
      <div class="content">
      <h2>{form.id ? `Modifier ${editing?.alias ?? 'l’hôte'}` : 'Nouvel hôte'}</h2>

      {#if form.proxyJump}
        <p class="warning">
          <Icon name="warning" />
          <span>Cet hôte passe par un bastion (ProxyJump). Il est dans le carnet, mais le rebond par bastion n’est pas encore pris en charge.</span>
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
          <Select bind:element={authEl} bind:value={form.auth}>
            <option value="auto">Agent SSH, puis clés par défaut de ~/.ssh</option>
            <option value="keyFile">Fichier de clé</option>
            <option value="password">Mot de passe</option>
          </Select>
        </label>
        {#if form.auth === 'password'}
          <!-- Ce que le trousseau retient pour ce compte, et de quoi l'oublier. -->
          <div class="full kept">
            {#if held.password}
              <small role="status">Le mot de passe de ce compte est retenu dans le trousseau du système.</small>
              <button class="btn ghost small" type="button" onclick={() => void forgetHeld('password')}>Oublier le mot de passe</button>
              {#if forgetError.password}<small class="bad" role="alert">{forgetError.password}</small>{/if}
            {:else if keyring === false}
              <small>{NO_KEYRING.password}</small>
            {:else}
              <small>Demandé à la connexion, où tu peux le faire retenir par le trousseau du système.</small>
            {/if}
          </div>
        {/if}
        {#if form.auth === 'keyFile'}
          <div class="full key">
            <label for="host-key"><span class="label">Clé privée</span></label>
            <div class="pick">
              <input
                id="host-key"
                bind:this={keyEl}
                class="field mono"
                type="text"
                list="host-keys"
                placeholder="~/.ssh/id_ed25519"
                spellcheck="false"
                autocomplete="off"
                bind:value={form.keyPath}
                oninput={() => {
                  keyNote = '';
                  keyError = '';
                }}
              />
              <button class="btn" type="button" onclick={() => void browseKey()}><Icon name="folder" size={14} /> Parcourir…</button>
            </div>
            <datalist id="host-keys">
              {#each ssh.keys as key (key.path)}<option value={key.path}>{key.algorithm}{key.comment ? ` · ${key.comment}` : ''}</option>{/each}
            </datalist>
            {#if keyError}
              <small class="bad" role="alert">{keyError}</small>
            {:else if keyNote}
              <small class="note" role="status">{keyNote}</small>
            {:else if held.passphrase}
              <div class="kept">
                <small role="status">La phrase secrète de cette clé est retenue dans le trousseau du système.</small>
                <button class="btn ghost small" type="button" onclick={() => void forgetHeld('passphrase')}>Oublier la phrase secrète</button>
                {#if forgetError.passphrase}<small class="bad" role="alert">{forgetError.passphrase}</small>{/if}
              </div>
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
          bind:this={testButton}
          class="btn"
          type="button"
          disabled={!testable}
          title={form.proxyJump
            ? 'Le rebond par bastion n’est pas encore pris en charge'
            : valid
              ? 'Se connecter avec ces valeurs, sans rien enregistrer ni ouvrir de terminal'
              : MISSING}
          onclick={() => {
            attempt = {};
            void startTest();
          }}
        >
          {#if test.step === 'running'}<span class="spinner" aria-hidden="true"></span> Test en cours…{:else}Tester la connexion{/if}
        </button>
        <!-- Tant que la fiche n'est pas valide, ce qui manque, à côté du premier bouton grisé. -->
        {#if !valid && !form.proxyJump}
          <small class="why" class:tried role={tried ? 'alert' : undefined}>{#if tried}<Icon name="warning" size={14} />{/if}{MISSING}</small>
        {/if}
      </div>

      {#if test.step === 'ok'}
        <p class="test ok" role="status">
          <Icon name="check" size={14} />
          <span><b>Connexion réussie</b> <span class="how">{test.method}, en {Math.round(test.ms)}{NB}ms</span></span>
        </p>
        {#if test.learnFailed}
          <!-- Le serveur a été accepté pour ce test, mais pas inscrit dans known_hosts (charte, 9.21). -->
          <p class="test warn" role="status">
            <Icon name="warning" size={14} />
            <span><b>L’empreinte n’a pas pu être mémorisée</b> <span class="how mono">{test.learnFailed}</span></span>
          </p>
        {/if}
      {:else if test.step === 'fail'}
        <p class="test bad" role="alert"><Icon name="warning" size={14} /> <span>{test.message}</span></p>
      {:else if test.step === 'hostKey'}
        <div class="test ask" role="group" aria-label="Serveur inconnu">
          <p>
            <b class="mono">{form.hostname}</b> n’est pas dans <span class="mono">known_hosts</span>. Vérifie que cette empreinte {test.algorithm} est
            bien celle du serveur avant de lui faire confiance{NB}:
          </p>
          <p class="print mono">{test.fingerprint}</p>
          <div class="row">
            <button class="btn small" type="button" onclick={() => test.step === 'hostKey' && void startTest({ accept: test.fingerprint, remember: true })}>
              Faire confiance et mémoriser
            </button>
            <button class="btn small" type="button" onclick={() => test.step === 'hostKey' && void startTest({ accept: test.fingerprint, remember: false })}>
              Faire confiance cette fois
            </button>
          </div>
        </div>
      {:else if test.step === 'secret'}
        <div class="test ask" role="group" aria-label={test.kind === 'passphrase' ? 'Phrase secrète de la clé' : 'Mot de passe'}>
          <p>
            {#if test.kind === 'passphrase'}La clé <span class="mono">{test.path}</span> est protégée{NB}: sa phrase secrète est demandée pour le test, puis oubliée.
            {:else}Le mot de passe est demandé pour le test, puis oublié{NB}: un test ne retient rien.{/if}
          </p>
          <div class="row">
            <input
              bind:this={secretEl}
              class="field"
              class:invalid={!!test.error}
              aria-invalid={test.error ? 'true' : undefined}
              aria-describedby={test.error ? 'host-secret-error' : undefined}
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
          {#if test.error}<p class="err" id="host-secret-error" role="alert">{test.error}</p>{/if}
        </div>
      {/if}

      </div>

      <!-- « Supprimer l'hôte… » seul à gauche, s'il est au carnet ; « Annuler » puis l'unique
           enregistrement, à droite (charte, 9.1, « Hierarchy », et 9.21). -->
      <footer>
        {#if editing?.id}
          <button class="btn ghost bad remove" type="button" bind:this={removeButton} onclick={() => void removeHost()}>Supprimer l’hôte…</button>
        {/if}
        <!-- Un échec d'enregistrer ou de supprimer, à gauche des boutons, qui ne bougent pas (charte, 9.11). -->
        <div class="failure">
          {#if failure}<p role="alert"><Icon name="warning" size={14} /><span>{failure}</span></p>{/if}
        </div>
        <button class="btn" type="button" onclick={close}>Annuler</button>
        <button class="btn primary" type="submit" disabled={!valid} title={valid ? undefined : MISSING}>
          {adding ? 'Enregistrer et connecter' : 'Enregistrer'}
        </button>
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
  /* Dans une fenêtre basse (1000 × 600), les champs défilent et la dernière ligne reste en vue. */
  dialog[open] {
    display: flex;
    flex-direction: column;
  }
  form {
    display: flex;
    flex-direction: column;
    min-height: 0;
    padding: 16px 20px;
  }
  /* 4 px de marge rendus : le halo du focus d'un champ (3 px) n'est pas rogné par le défilement. */
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
  .warning {
    display: flex;
    gap: 6px;
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
    gap: 12px;
  }
  .grid label,
  .key,
  fieldset {
    display: flex;
    flex-direction: column;
    gap: 6px;
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
  small.bad {
    color: var(--bad);
  }
  /* Un secret retenu : ce que le trousseau garde et, dessous, le bouton qui l'oublie. Le bouton,
     sans fond ni bordure, est tiré de sa marge intérieure (8 px et 1 px de bordure) : son texte
     s'aligne sur la phrase. */
  .kept {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
  }
  .kept .btn {
    margin-left: -9px;
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
  /* Une pastille prise au clavier montre l'anneau de focus : son bouton radio, lui, est invisible. */
  .swatch:has(input:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
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
  /* Les boutons du bas se tiennent un peu à l'écart du test et de son résultat : 24 px au lieu de 12. */
  /* 24 px au-dessus de la dernière ligne (charte, 9.21). */
  footer {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin-top: 24px;
  }
  footer .btn {
    flex: none;
  }
  /* Entre l'entrée destructrice, tout à gauche, et les boutons de droite : l'échec, s'il y en a un. */
  .failure {
    flex: 1;
    min-width: 0;
    padding-top: 8px;
    font-size: var(--fs-small);
    line-height: 1.35;
    overflow-wrap: anywhere;
  }
  .failure p {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: 0;
    color: var(--bad);
  }
  .failure :global(svg) {
    flex: none;
  }
  .testing {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  /* Pourquoi le test et l'enregistrement sont grisés (charte, 9.1). Après un Entrée tenté quand
     même, la phrase passe en avertissement, avec son icône. */
  .why {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--muted);
    font-size: var(--fs-small);
  }
  .why.tried {
    color: var(--warn);
  }
  /* Le résultat du test, entre les champs et les boutons (charte, section 9.21). */
  .test {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: 0;
    font-size: var(--fs-small);
  }
  .test :global(svg) {
    flex: none;
    margin-top: 2px;
  }
  .test.ok {
    color: var(--ok);
  }
  .test .how {
    color: var(--muted);
  }
  .test.warn {
    color: var(--warn);
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
    margin: 0;
    color: var(--bad);
    font-size: var(--fs-small);
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
