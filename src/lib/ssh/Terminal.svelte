<script lang="ts">
  import { theme as appTheme } from '../settings/theme.svelte';
  import { Channel, invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { FitAddon } from '@xterm/addon-fit';
  import { SearchAddon, type ISearchOptions } from '@xterm/addon-search';
  import { tick } from 'svelte';
  import { Terminal } from '@xterm/xterm';
  import '@xterm/xterm/css/xterm.css';
  import { onMount } from 'svelte';
  import Icon from '../ui/Icon.svelte';
  import { prefs } from '../git/prefs.svelte';
  import { STATUS, type Session } from './ssh.svelte';
  import { explain } from './reasons';
  import type { Closed, Connected, SshError } from './types';

  let {
    session,
    active,
    onclose,
    bare = false,
    autofocus = true,
    onoutput,
  }: {
    session: Session;
    active: boolean;
    onclose: () => void;
    /** Sans en-tête : le panneau qui l'accueille dit lui-même ce qu'est le terminal (charte, 9.20). */
    bare?: boolean;
    /** Le terminal prend le clavier quand son onglet s'affiche. Sous le graphe, jamais de lui-même. */
    autofocus?: boolean;
    /** Appelé à chaque sortie du shell : le dépôt se relit quand elle se tait. */
    onoutput?: () => void;
  } = $props();

  /** Ce que la session attend de l'utilisateur, ou son état. */
  type Phase =
    | { step: 'connecting' }
    | { step: 'connected' }
    | { step: 'hostKey'; fingerprint: string; algorithm: string }
    | { step: 'hostKeyChanged'; fingerprint: string; line: number }
    | { step: 'passphrase'; path: string; error: string }
    | { step: 'password'; error: string }
    | { step: 'error'; message: string }
    | { step: 'closed'; message: string; lost?: boolean }
    /** Connexion perdue : la tentative `attempt` (de 1 à 5) part à l'instant `at`. */
    | { step: 'lost'; attempt: number; at: number; reason: string };

  // Délais entre deux tentatives de reconnexion, en secondes (charte, section 9.15).
  const RETRIES = [2, 5, 10, 20, 30];
  const NB = '\u00a0'; // espace insécable, avant « : » et entre un nombre et son unité

  /** Ce qu'on sait déjà pour cette tentative : rien n'est conservé une fois la session ouverte. */
  interface Attempt {
    accept?: string;
    remember?: boolean;
    keyPath?: string;
    passphrase?: string;
    password?: string;
  }

  let phase = $state<Phase>({ step: 'connecting' });
  // Reconnexion automatique : la tentative en cours (0 quand il n'y en a pas), son minuteur, et
  // l'heure, rafraîchie chaque seconde pour le compte à rebours.
  let retry = $state(0);
  let retryTimer: number | undefined;
  let now = $state(Date.now());
  $effect(() => {
    if (phase.step !== 'lost') return;
    const tick = window.setInterval(() => (now = Date.now()), 1000);
    return () => window.clearInterval(tick);
  });
  const waitText = $derived.by(() => {
    if (phase.step !== 'lost') return '';
    const seconds = Math.max(0, Math.ceil((phase.at - now) / 1000));
    const which = `${phase.attempt}${NB}sur${NB}${RETRIES.length}`;
    return seconds > 0 ? `Nouvelle tentative dans ${seconds}${NB}s, ${which}.` : `Tentative ${which}…`;
  });
  // Ce qu'un lecteur d'écran entend : une fois par tentative, pas à chaque seconde.
  const announce = $derived(
    phase.step === 'lost' ? `Connexion perdue. Tentative ${phase.attempt} sur ${RETRIES.length} dans ${RETRIES[phase.attempt - 1]} secondes.` : '',
  );
  let retryButton = $state<HTMLButtonElement>();
  let endButton = $state<HTMLButtonElement>();
  // Un panneau de fin apparaît : le focus va sur son bouton principal, que le clavier n'atteindrait
  // pas autrement, le terminal gardant la touche Tab (charte, section 9.15).
  $effect(() => {
    if (phase.step !== 'closed' && phase.step !== 'error') return;
    void tick().then(() => endButton?.focus());
  });
  // La carte apparaît : le focus va sur « Reconnecter maintenant ». Le terminal, coupé, ne sert plus,
  // et il garde la touche Tab pour lui : sans cela, le clavier n'atteindrait pas la carte.
  $effect(() => {
    if (phase.step !== 'lost' || phase.attempt !== 1) return;
    void tick().then(() => retryButton?.focus());
  });

  /** Programme la tentative `attempt`, ou s'arrête après la dernière. */
  function scheduleRetry(attempt: number, reason: string) {
    window.clearTimeout(retryTimer);
    if (attempt > RETRIES.length) {
      retry = 0;
      session.status = 'closed';
      phase = { step: 'closed', lost: true, message: `Les ${RETRIES.length} tentatives de reconnexion ont échoué${reason ? `${NB}: ${explain(reason)}` : '.'}` };
      return;
    }
    retry = attempt;
    session.status = 'reconnecting';
    const delay = RETRIES[attempt - 1] * 1000;
    now = Date.now();
    phase = { step: 'lost', attempt, at: now + delay, reason: explain(reason) };
    retryTimer = window.setTimeout(() => void connect({}, attempt), delay);
  }

  /** « Reconnecter maintenant » : la tentative prévue part sans attendre. */
  function retryNow() {
    if (phase.step !== 'lost') return;
    window.clearTimeout(retryTimer);
    void connect({}, phase.attempt);
  }

  /** « Arrêter » : plus de tentative, l'onglet attend un clic. */
  function stopRetrying() {
    window.clearTimeout(retryTimer);
    retry = 0;
    session.status = 'closed';
    phase = { step: 'closed', lost: true, message: 'La reconnexion automatique est arrêtée.' };
  }
  let info = $state.raw<Connected | null>(null);
  let echo = $state<number | null>(null);
  let secret = $state('');

  let termEl = $state<HTMLDivElement>();
  let term: Terminal | undefined;
  let fit: FitAddon | undefined;
  let search: SearchAddon | undefined;

  // --- Recherche dans le terminal (charte, section 9.15) -------------------------------------
  const MATCH = 0.3; // part de l'accent dans le fond d'un résultat, comme dans verifier-contrastes.mjs
  let query = $state('');
  let caseSensitive = $state(false);
  let regex = $state(false);
  /** Résultats de la dernière recherche ; `index` vaut -1 au-delà de 1 000 résultats surlignés. */
  let results = $state<{ index: number; count: number } | null>(null);
  let searchEl = $state<HTMLInputElement>();
  const invalid = $derived.by(() => {
    if (!regex || !query) return false;
    try {
      new RegExp(query);
      return false;
    } catch {
      return true;
    }
  });
  const countText = $derived(
    !query || !results
        ? ''
        : results.count === 0
          ? 'Aucun résultat'
          : results.index < 0
            ? `${results.count} résultats`
            : `${results.index + 1} sur ${results.count}`,
  );

  /** `#rrggbb` mêlé à `p` sur `#rrggbb` : le terminal veut une couleur pleine, pas `color-mix()`. */
  function blend(hex: string, p: number, over: string) {
    const channels = (h: string) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));
    const [a, b] = [channels(hex), channels(over)];
    return `#${a.map((v, i) => Math.round(v * p + b[i] * (1 - p)).toString(16).padStart(2, '0')).join('')}`;
  }

  /** Les couleurs des résultats, lues dans les jetons au moment de chercher : elles suivent le thème.
   *  Un résultat est l'accent à 30 % sur le fond du terminal (charte, section 9.15). */
  function searchOptions(incremental = false): ISearchOptions {
    const css = getComputedStyle(document.documentElement);
    const v = (name: string) => css.getPropertyValue(name).trim();
    const match = blend(v('--accent'), MATCH, v('--term-bg'));
    return {
      caseSensitive,
      regex,
      incremental,
      decorations: {
        matchBackground: match,
        matchOverviewRuler: v('--accent'),
        activeMatchBackground: match,
        activeMatchBorder: v('--accent'),
        activeMatchColorOverviewRuler: v('--accent'),
      },
    };
  }

  function find(direction: 'next' | 'previous', incremental = false) {
    if (!search) return;
    if (!query || invalid) {
      search.clearDecorations();
      results = null;
      return;
    }
    if (direction === 'next') search.findNext(query, searchOptions(incremental));
    else search.findPrevious(query, searchOptions());
  }

  function closeSearch() {
    session.searching = false;
    search?.clearDecorations();
    results = null;
    term?.focus();
  }

  function onFieldKey(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      find(e.shiftKey ? 'previous' : 'next');
    }
  }

  /** Échap ferme la barre, d'où que vienne la frappe : le champ, un bouton ou une bascule. */
  function onBarKey(e: KeyboardEvent) {
    if (e.key !== 'Escape') return;
    e.preventDefault();
    e.stopPropagation();
    closeSearch();
  }

  // Ouverture demandée (raccourci, palette), même barre déjà ouverte : le champ prend le focus et
  // son texte est sélectionné.
  $effect(() => {
    void session.searchAsk;
    if (!session.searching) return;
    void tick().then(() => {
      searchEl?.focus();
      searchEl?.select();
    });
  });

  // Les options changent : la recherche est refaite sur place.
  $effect(() => {
    void caseSensitive;
    void regex;
    if (session.searching && query) find('next', true);
  });
  // Hors réactivité : lus dans les rappels du terminal, à chaque frappe.
  let sessionId: number | null = null;
  let lastKey = 0;
  let echoes: number[] = [];
  let attempt: Attempt = {};

  const host = $derived(session.host);
  // Un terminal local parle au cœur Rust par ses propres commandes ; le reste du composant est commun.
  // La nature d'un onglet ne change pas : la lire une fois suffit.
  // svelte-ignore state_referenced_locally
  const local = session.kind === 'local';
  const API = local
    ? { write: 'local_write', resize: 'local_resize', close: 'local_close', closed: 'local-closed' }
    : { write: 'ssh_write', resize: 'ssh_resize', close: 'ssh_close', closed: 'ssh-closed' };
  /** Ce que le cœur dit d'un terminal local ouvert : son shell et son dossier de départ. */
  let opened = $state<{ shell: string; cwd: string } | null>(null);
  const onWindows = navigator.userAgent.includes('Windows');

  /** Lance le shell de cette machine dans un pseudo-terminal. */
  async function openLocal() {
    if (!term || !fit) return;
    phase = { step: 'connecting' };
    session.status = 'connecting';
    fit.fit();
    const onData = new Channel<ArrayBuffer | number[]>();
    onData.onmessage = (chunk) => {
      term?.write(new Uint8Array(chunk));
      onoutput?.();
    };
    try {
      const result = await invoke<{ id: number; shell: string; cwd: string }>('local_open', {
        cols: term.cols,
        rows: term.rows,
        cwd: session.cwd || null,
        onData,
      });
      sessionId = result.id;
      opened = { shell: result.shell, cwd: result.cwd };
      session.opened = opened;
      phase = { step: 'connected' };
      session.status = 'connected';
      if (autofocus || relaunched) term.focus();
      relaunched = false;
    } catch (e) {
      session.status = 'closed';
      const what = onWindows ? 'Vérifie que PowerShell est installé.' : 'Vérifie la variable SHELL de ton compte.';
      phase = { step: 'error', message: `${typeof e === 'string' ? e : String(e)} ${what}` };
    }
  }
  const target = $derived(`${host.user}@${host.hostname}${host.port !== 22 ? `:${host.port}` : ''}`);

  function median(values: number[]) {
    const sorted = [...values].sort((a, b) => a - b);
    return sorted[Math.floor(sorted.length / 2)];
  }

  function normalize(e: unknown): SshError {
    if (e && typeof e === 'object' && 'kind' in e) return e as SshError;
    return { kind: 'other', message: typeof e === 'string' ? e : String(e) };
  }

  /** Ouvre la session. `auto` : numéro de la tentative de reconnexion automatique, 0 sinon. */
  async function connect(next: Attempt = {}, auto = 0) {
    if (!term || !fit) return;
    if (local) return openLocal();
    retry = auto;
    attempt = { ...attempt, ...next };
    if (host.proxyJump) {
      phase = { step: 'error', message: 'Cet hôte passe par un bastion (ProxyJump). Le rebond par bastion n’est pas encore pris en charge.' };
      session.status = 'closed';
      return;
    }
    if (!host.user) {
      phase = { step: 'error', message: 'Cet hôte n’a pas d’utilisateur : ajoute-le dans sa fiche.' };
      session.status = 'closed';
      return;
    }
    // Un mot de passe n'est jamais enregistré : il est demandé à chaque connexion.
    if (host.auth === 'password' && !attempt.password) {
      // La série de tentatives s'arrête : l'onglet attend l'utilisateur.
      retry = 0;
      session.status = 'closed';
      phase = { step: 'password', error: '' };
      return;
    }

    phase = { step: 'connecting' };
    session.status = auto ? 'reconnecting' : 'connecting';
    secret = '';
    fit.fit();
    echoes = [];
    echo = null;

    const onData = new Channel<ArrayBuffer | number[]>();
    onData.onmessage = (chunk) => {
      if (lastKey) {
        echoes.push(performance.now() - lastKey);
        lastKey = 0;
        if (echoes.length > 40) echoes.shift();
        echo = median(echoes);
      }
      term?.write(new Uint8Array(chunk));
    };

    const keyPath = attempt.keyPath ?? (host.auth === 'keyFile' ? host.keyPath : '');
    const auth =
      host.auth === 'password'
        ? { kind: 'password', password: attempt.password }
        : keyPath
          ? { kind: 'keyFile', path: keyPath, passphrase: attempt.passphrase ?? null }
          : { kind: 'auto' };

    try {
      const connected = await invoke<Connected>('ssh_connect', {
        params: {
          host: host.hostname,
          port: host.port || 22,
          user: host.user,
          auth,
          cols: term.cols,
          rows: term.rows,
          acceptFingerprint: attempt.accept ?? null,
          rememberHost: attempt.remember ?? false,
          startupCommand: host.startupCommand.trim() || null,
        },
        onData,
      });
      info = connected;
      sessionId = connected.id;
      phase = { step: 'connected' };
      session.status = 'connected';
      // Les secrets ne survivent pas à l'ouverture de la session.
      attempt = {};
      retry = 0;
      term.focus();
    } catch (e) {
      const problem = normalize(e);
      session.status = 'closed';
      if (problem.kind === 'hostKeyUnknown') {
        phase = { step: 'hostKey', fingerprint: problem.fingerprint, algorithm: problem.algorithm };
      } else if (problem.kind === 'hostKeyChanged') {
        phase = { step: 'hostKeyChanged', fingerprint: problem.fingerprint, line: problem.line };
      } else if (problem.kind === 'needPassphrase') {
        const wrong = attempt.passphrase !== undefined;
        attempt.passphrase = undefined;
        phase = { step: 'passphrase', path: problem.path, error: wrong ? 'Cette phrase secrète ne déverrouille pas la clé.' : '' };
      } else if (problem.kind === 'auth' && host.auth === 'password') {
        attempt.password = undefined;
        phase = { step: 'password', error: problem.message };
      } else if (auto) {
        // Le serveur ne répond toujours pas : la tentative suivante est programmée.
        scheduleRetry(auto + 1, problem.message);
      } else {
        phase = { step: 'error', message: problem.message };
      }
    }
  }

  function submitSecret(e: SubmitEvent) {
    e.preventDefault();
    if (phase.step === 'passphrase') void connect({ keyPath: phase.path, passphrase: secret });
    else if (phase.step === 'password') void connect({ password: secret });
  }

  /** « Relancer » vient d'être cliqué : le terminal rouvert reprend le clavier, même sous le graphe. */
  let relaunched = false;

  function reconnect() {
    relaunched = true;
    window.clearTimeout(retryTimer);
    attempt = {};
    term?.reset();
    void connect();
  }

  onMount(() => {
    const css = getComputedStyle(document.documentElement);
    const v = (name: string) => css.getPropertyValue(name).trim();
    const theme = () => ({
      background: v('--term-bg'),
      foreground: v('--term-ink'),
      cursor: v('--accent'),
      selectionBackground: v('--sel'),
      // La marge des résultats de recherche, sans trait sur son bord.
      overviewRulerBorder: v('--term-bg'),
    });

    const terminal = new Terminal({
      // La recherche surligne ses résultats par des décorations, une API encore « proposée ».
      allowProposedApi: true,
      overviewRuler: { width: 8 },
      fontFamily: v('--f-mono'),
      fontSize: parseFloat(v('--fs-term')),
      lineHeight: 1.2,
      cursorBlink: true,
      scrollback: 5000,
      theme: theme(),
    });
    const fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    const searchAddon = new SearchAddon();
    terminal.loadAddon(searchAddon);
    searchAddon.onDidChangeResults((e) => (results = { index: e.resultIndex, count: e.resultCount }));
    search = searchAddon;
    if (termEl) terminal.open(termEl);
    term = terminal;
    fit = fitAddon;

    terminal.onData((data) => {
      if (sessionId === null) return;
      if (!lastKey) lastKey = performance.now();
      void invoke(API.write, { id: sessionId, data }).catch(() => {});
    });
    // Le pseudo-terminal ne suit qu'une fois le geste fini : pendant qu'on glisse une poignée, le
    // shell redessinerait son invite à chaque image.
    let resizeTimer: number | undefined;
    terminal.onResize(({ cols, rows }) => {
      window.clearTimeout(resizeTimer);
      resizeTimer = window.setTimeout(() => {
        if (sessionId !== null) void invoke(API.resize, { id: sessionId, cols, rows }).catch(() => {});
      }, 120);
    });

    const observer = new ResizeObserver(() => {
      // Onglet masqué : la taille est nulle, on ne recalcule rien.
      if (termEl && termEl.offsetParent !== null && termEl.clientWidth > 0) fitAddon.fit();
    });
    if (termEl) observer.observe(termEl);

    // Le terminal lit ses couleurs dans les jetons : il les relit quand le thème change.
    const stopTheme = $effect.root(() => {
      $effect(() => {
        void appTheme.key;
        terminal.options.theme = theme();
      });
    });

    const unlisten = listen<Closed>(API.closed, (event) => {
      if (event.payload.id !== sessionId) return;
      sessionId = null;
      info = null;
      const { exitStatus, reason, ending } = event.payload;
      // Terminal local : une fin normale ferme l'onglet ; une erreur le garde, avec « Relancer ».
      if (local) {
        if (exitStatus === 0) return onclose();
        session.status = 'closed';
        phase = {
          step: 'closed',
          message: `${exitStatus === null ? 'Le shell s’est arrêté.' : `Le shell s’est arrêté (code de sortie ${exitStatus}).`} Relance-le, ou ferme l’onglet.`,
        };
        return;
      }
      // Seule une connexion perdue se rouvre d'elle-même, et seulement si le réglage le permet.
      if (ending === 'lost' && prefs.sshReconnect) {
        term?.write('\r\n');
        scheduleRetry(1, reason ?? '');
        return;
      }
      session.status = 'closed';
      phase =
        ending === 'lost'
          ? { step: 'closed', lost: true, message: reason ? explain(reason) : 'La connexion au serveur a été coupée.' }
          : {
              step: 'closed',
              message: reason
                ? `Connexion interrompue\u00a0: ${reason}`
                : exitStatus === null || exitStatus === 0
                  ? 'Le serveur a fermé la session.'
                  : `Session terminée (code de sortie ${exitStatus}).`,
            };
    });

    void connect();

    return () => {
      observer.disconnect();
      window.clearTimeout(resizeTimer);
      stopTheme();
      void unlisten.then((stop) => stop());
      window.clearTimeout(retryTimer);
      if (sessionId !== null) void invoke(API.close, { id: sessionId }).catch(() => {});
      terminal.dispose();
    };
  });

  $effect(() => {
    // À l'affichage de l'onglet : la taille du terminal a pu changer pendant qu'il était masqué.
    if (!active) return;
    const connected = phase.step === 'connected';
    requestAnimationFrame(() => {
      fit?.fit();
      if (connected && autofocus) term?.focus();
    });
  });
</script>

<div class="session">
  {#if !bare}
  <header>
    {#if local}
      <!-- Un terminal local : l'ordinateur, comme partout où « local » est dit (charte, section 7). -->
      <span class="what"><Icon name="laptop" size={14} /> <b>Terminal local</b></span>
      {#if opened}
        <span class="mono">{opened.shell}</span>
        <span>ouvert dans <span class="mono">{opened.cwd}</span></span>
      {/if}
    {:else}
      <span class="dot s-{session.status}" role="img" aria-label={STATUS[session.status]} title={STATUS[session.status]}></span>
      <b class="mono">{target}</b>
    {/if}
    {#if local}
      <!-- Pas de « Déconnecter » : la croix de l'onglet ferme le terminal. -->
    {:else if phase.step === 'connected' && info}
      <span>{info.knownHost ? 'serveur connu' : 'empreinte acceptée pour cette session'}</span>
      <span>{info.authMethod}</span>
      <span>connexion en {Math.round(info.connectMs)} ms</span>
      {#if echo !== null}<span title="Délai médian entre une frappe et son affichage">écho {Math.round(echo)} ms</span>{/if}
      <button class="btn small" type="button" onclick={() => sessionId !== null && void invoke(API.close, { id: sessionId })}>Déconnecter</button>
    {:else if phase.step === 'connecting'}
      <span>connexion…</span>
    {/if}
  </header>
  {/if}

  {#if session.searching}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="search" role="search" aria-label="Chercher dans le terminal" onkeydown={onBarKey}>
      <label class="filter" class:invalid>
        <Icon name="search" size={14} />
        <input
          bind:this={searchEl}
          type="search"
          placeholder="Chercher dans le terminal"
          aria-label="Chercher dans le terminal"
          aria-invalid={invalid ? 'true' : undefined}
          aria-describedby="search-count-{session.key}"
          spellcheck="false"
          autocomplete="off"
          bind:value={query}
          oninput={() => find('next', true)}
          onkeydown={onFieldKey}
        />
      </label>
      {#if invalid}
        <span class="count bad" id="search-count-{session.key}" role="alert">Expression non valide</span>
      {:else}
        <span class="count" id="search-count-{session.key}" role="status">{countText}</span>
      {/if}
      <button class="btn ghost small" type="button" aria-label="Résultat précédent" title="Résultat précédent (Maj + Entrée)" disabled={!results?.count} onclick={() => find('previous')}>
        <Icon name="up" size={14} />
      </button>
      <button class="btn ghost small" type="button" aria-label="Résultat suivant" title="Résultat suivant (Entrée)" disabled={!results?.count} onclick={() => find('next')}>
        <Icon name="down" size={14} />
      </button>
      <button class="btn ghost small toggle" type="button" aria-pressed={caseSensitive} title="Respecter la casse" aria-label="Respecter la casse" onclick={() => (caseSensitive = !caseSensitive)}>Aa</button>
      <button class="btn ghost small toggle mono" type="button" aria-pressed={regex} title="Expression régulière" aria-label="Expression régulière" onclick={() => (regex = !regex)}>.*</button>
      <button class="btn ghost small close" type="button" aria-label="Fermer la recherche" title="Fermer la recherche (Échap)" onclick={closeSearch}>
        <Icon name="x" size={14} />
      </button>
    </div>
  {/if}

  <div class="stage">
    <div class="term" bind:this={termEl}></div>

    {#if phase.step !== 'connected'}
      <div class="veil">
        <div class="card" class:bad={phase.step === 'hostKeyChanged'}>
          {#if phase.step === 'connecting'}
            <span class="spinner wait" aria-hidden="true"></span>
            <p class="lead">
              {#if local}Ouverture du terminal local…{:else}{retry ? 'Reconnexion' : 'Connexion'} à <b class="mono">{target}</b>…{/if}
            </p>
          {:else if phase.step === 'hostKey'}
            <h3><Icon name="warning" /> Serveur inconnu</h3>
            <p>
              <b class="mono">{host.hostname}</b> n’est pas dans <code>known_hosts</code>. Vérifie que cette empreinte
              {phase.algorithm} est bien celle du serveur avant de lui faire confiance :
            </p>
            <p class="print mono">{phase.fingerprint}</p>
            <div class="row">
              <button class="btn primary" type="button" onclick={() => void connect({ accept: phase.step === 'hostKey' ? phase.fingerprint : undefined, remember: true })}>
                Faire confiance et mémoriser
              </button>
              <button class="btn" type="button" onclick={() => void connect({ accept: phase.step === 'hostKey' ? phase.fingerprint : undefined, remember: false })}>
                Faire confiance cette fois
              </button>
              <button class="btn ghost" type="button" onclick={onclose}>Annuler</button>
            </div>
          {:else if phase.step === 'hostKeyChanged'}
            <h3><Icon name="warning" /> L’empreinte du serveur a changé</h3>
            <p>
              L’empreinte présentée par <b class="mono">{host.hostname}</b> ne correspond plus à celle enregistrée à la ligne
              {phase.line} de <code>known_hosts</code>. La connexion est bloquée : le serveur a peut-être été réinstallé, ou
              quelqu’un s’interpose entre toi et lui.
            </p>
            <p class="print mono">{phase.fingerprint}</p>
            <p class="muted">Si le changement est légitime, retire l’ancienne ligne de <code>known_hosts</code>, puis reconnecte-toi.</p>
            <div class="row">
              <button class="btn" type="button" onclick={reconnect}>Réessayer</button>
              <button class="btn ghost" type="button" onclick={onclose}>Fermer l’onglet</button>
            </div>
          {:else if phase.step === 'passphrase' || phase.step === 'password'}
            <form onsubmit={submitSecret}>
              <h3>{phase.step === 'passphrase' ? 'Phrase secrète de la clé' : `Mot de passe de ${host.user}`}</h3>
              <p class="muted">
                {#if phase.step === 'passphrase'}
                  La clé <span class="mono">{phase.path}</span> est protégée.
                {:else}
                  Pour <span class="mono">{target}</span>. Il n’est pas enregistré.
                {/if}
              </p>
              {#if phase.error}<p class="err">{phase.error}</p>{/if}
              <!-- svelte-ignore a11y_autofocus -->
              <input class="field" type="password" autocomplete="off" autofocus aria-label={phase.step === 'passphrase' ? 'Phrase secrète' : 'Mot de passe'} bind:value={secret} />
              <div class="row">
                <button class="btn primary" type="submit" disabled={!secret}>Se connecter</button>
                <button class="btn ghost" type="button" onclick={onclose}>Annuler</button>
              </div>
            </form>
          {:else if phase.step === 'lost'}
            <h3><Icon name="warning" /> Connexion perdue</h3>
            <!-- Le compte à rebours change chaque seconde : il n'est pas lu, seule l'annonce l'est. -->
            <p aria-hidden="true">{waitText}</p>
            <p class="spoken" role="status">{announce}</p>
            <!-- La ligne du motif garde sa place : les boutons ne bougent pas d'une tentative à l'autre. -->
            <p class="muted reason">{phase.reason || NB}</p>
            <div class="row">
              <button class="btn primary" type="button" bind:this={retryButton} onclick={retryNow}><Icon name="refresh" size={14} /> Reconnecter maintenant</button>
              <button class="btn ghost" type="button" onclick={stopRetrying}>Arrêter</button>
            </div>
          {:else}
            <h3>
              {#if local}Shell arrêté{:else}{phase.step === 'error'
                  ? 'Connexion impossible'
                  : phase.lost
                    ? 'Connexion perdue'
                    : 'Session terminée'}{/if}
            </h3>
            <p class:err={phase.step === 'error'}>{phase.message}</p>
            <div class="row">
              <!-- Un terminal local n'est jamais « reconnecté » : il est relancé. -->
              <button class="btn primary" type="button" bind:this={endButton} onclick={reconnect}><Icon name="refresh" size={14} /> {local ? 'Relancer' : 'Reconnecter'}</button>
              <button class="btn ghost" type="button" onclick={onclose}>Fermer l’onglet</button>
            </div>
          {/if}
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .session {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  /* Barre de recherche : une bande sous l'en-tête, qui pousse le terminal sans cacher son texte. */
  .search {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    border-bottom: 1px solid var(--line-soft);
    background: var(--panel);
  }
  .filter {
    flex: none;
    display: flex;
    align-items: center;
    gap: 6px;
    width: 240px;
    padding: 0 9px;
    border: 1px solid var(--line);
    border-radius: var(--radius-s);
    background: var(--bg);
    color: var(--muted);
    transition:
      border-color var(--t-fast) var(--ease),
      box-shadow var(--t-fast) var(--ease);
  }
  .filter:hover:not(:focus-within) {
    border-color: color-mix(in srgb, var(--muted) 50%, var(--line));
  }
  .filter:focus-within {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 25%, transparent);
  }
  .filter.invalid,
  .filter.invalid:focus-within {
    border-color: var(--bad);
  }
  .filter.invalid:focus-within {
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--bad) 25%, transparent);
  }
  .filter input {
    flex: 1;
    min-width: 0;
    min-height: 30px;
    padding: 5px 0;
    border: 0;
    background: none;
    outline: none;
    color: var(--ink);
  }
  /* Assez large pour « Expression non valide » : les boutons ne bougent pas quand le texte change. */
  .count {
    min-width: 124px;
    color: var(--muted);
    font-size: var(--fs-small);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .count.bad {
    color: var(--bad);
  }
  /* Bascule : un bouton fantôme, enfoncé en --accent-soft bordé d'accent (charte, section 9.1). */
  .toggle[aria-pressed='true'],
  .toggle[aria-pressed='true']:hover {
    background: var(--accent-soft);
    border-color: var(--accent);
  }
  .close {
    margin-left: auto;
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    /* Hauteur fixe : celle d'un en-tête avec son bouton « Déconnecter ». Le terminal ne saute pas
       d'un onglet à l'autre. */
    height: 35px;
    padding: 5px 12px;
    border-bottom: 1px solid var(--line-soft);
    color: var(--muted);
    font-size: var(--fs-label);
    white-space: nowrap;
    overflow: hidden;
  }
  header b {
    color: var(--ink);
  }
  header .btn {
    margin-left: auto;
  }
  .dot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--faint);
  }
  .dot.s-connected {
    background: var(--ok);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--ok) 25%, transparent);
  }
  .dot.s-connecting,
  .dot.s-reconnecting {
    background: var(--warn);
    animation: blink var(--t-wait) var(--ease) infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0.35;
    }
  }
  .stage {
    flex: 1;
    position: relative;
    min-height: 0;
    background: var(--term-bg);
  }
  .term {
    position: absolute;
    inset: 8px 4px 4px 10px;
  }
  .veil {
    position: absolute;
    inset: 0;
    /* Dans un terminal bas (sous le graphe), le panneau défile plutôt que de déborder ; ses marges
       automatiques le centrent sans rogner son haut quand il dépasse. */
    display: flex;
    padding: 24px;
    overflow: auto;
    background: color-mix(in srgb, var(--term-bg) 82%, transparent);
    backdrop-filter: blur(1.5px);
    animation: fade-in var(--t-med) var(--ease);
  }
  .card {
    margin: auto;
    width: min(520px, 100%);
    display: grid;
    gap: 10px;
    padding: 16px 20px;
    border: 1px solid var(--line);
    border-radius: var(--radius-l);
    background: var(--raised);
    box-shadow: var(--shadow-pop);
    animation: pop-in var(--t-med) var(--ease);
  }
  .card.bad {
    border-color: var(--bad);
    background: var(--bad-soft);
  }
  .card form {
    display: grid;
    gap: 10px;
  }
  h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-size: var(--fs-title);
    font-weight: var(--fw-strong);
  }
  p {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .lead {
    text-align: center;
  }
  .muted {
    color: var(--muted);
  }
  .err {
    color: var(--bad);
    white-space: pre-wrap;
  }
  .print {
    padding: 8px 10px;
    border-radius: var(--radius-s);
    background: var(--bg);
    border: 1px solid var(--line);
    font-size: var(--fs-code);
    user-select: all;
  }
  code {
    font-family: var(--f-mono);
    font-size: var(--fs-code);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 4px;
  }
  .wait {
    justify-self: center;
  }
  /* Hors de l'écran, mais lu : l'annonce d'une tentative de reconnexion. */
  .spoken {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
  .what {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
</style>
