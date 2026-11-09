<script lang="ts">
  import { Channel, invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { FitAddon } from '@xterm/addon-fit';
  import { Terminal } from '@xterm/xterm';
  import '@xterm/xterm/css/xterm.css';
  import { onMount } from 'svelte';

  // Miroir des structures de src-tauri/src/ssh.
  interface HostEntry {
    alias: string;
    hostname: string;
    user: string | null;
    port: number;
    identityFile: string | null;
    proxyJump: boolean;
  }
  interface Defaults {
    user: string;
    hosts: HostEntry[];
    keyFiles: string[];
  }
  interface Connected {
    id: number;
    fingerprint: string;
    knownHost: boolean;
    authMethod: string;
    connectMs: number;
  }
  interface Closed {
    id: number;
    exitStatus: number | null;
    reason: string | null;
  }
  type SshError =
    | { kind: 'hostKeyUnknown'; fingerprint: string; algorithm: string }
    | { kind: 'hostKeyChanged'; fingerprint: string; line: number }
    | { kind: 'needPassphrase'; path: string }
    | { kind: 'auth'; message: string }
    | { kind: 'other'; message: string };
  type AuthKind = 'auto' | 'keyFile' | 'password';

  let { active = false }: { active?: boolean } = $props();

  let defaults = $state.raw<Defaults | null>(null);
  let alias = $state('');
  let form = $state({
    host: '',
    port: 22,
    user: '',
    auth: 'auto' as AuthKind,
    keyPath: '~/.ssh/id_ed25519',
    passphrase: '',
    password: '',
    startup: '',
  });
  let status = $state<'idle' | 'connecting' | 'connected'>('idle');
  let session = $state.raw<Connected | null>(null);
  let target = $state('');
  let problem = $state<SshError | null>(null);
  let closedInfo = $state('');
  let echo = $state<number | null>(null);
  let viaBastion = $state(false);

  let termEl = $state<HTMLDivElement>();
  let term: Terminal | undefined;
  let fit: FitAddon | undefined;
  // Hors réactivité : lus dans les rappels du terminal, à chaque frappe.
  let sessionId: number | null = null;
  let lastKey = 0;
  let echoes: number[] = [];

  const canConnect = $derived(
    status === 'idle' &&
      form.host.trim() !== '' &&
      form.user.trim() !== '' &&
      (form.auth !== 'password' || form.password !== '') &&
      (form.auth !== 'keyFile' || form.keyPath.trim() !== ''),
  );

  function median(values: number[]) {
    const sorted = [...values].sort((a, b) => a - b);
    return sorted[Math.floor(sorted.length / 2)];
  }

  function normalize(e: unknown): SshError {
    if (e && typeof e === 'object' && 'kind' in e) return e as SshError;
    return { kind: 'other', message: typeof e === 'string' ? e : String(e) };
  }

  function pickHost() {
    const host = defaults?.hosts.find((h) => h.alias === alias);
    if (!host) return;
    form.host = host.hostname;
    form.port = host.port;
    if (host.user) form.user = host.user;
    if (host.identityFile) {
      form.auth = 'keyFile';
      form.keyPath = host.identityFile;
    } else {
      form.auth = 'auto';
    }
    viaBastion = host.proxyJump;
    problem = null;
  }

  async function connect(acceptFingerprint?: string) {
    if (!term || !fit) return;
    problem = null;
    closedInfo = '';
    status = 'connecting';
    term.reset();
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

    const auth =
      form.auth === 'password'
        ? { kind: 'password', password: form.password }
        : form.auth === 'keyFile'
          ? { kind: 'keyFile', path: form.keyPath.trim(), passphrase: form.passphrase || null }
          : { kind: 'auto' };

    try {
      const connected = await invoke<Connected>('ssh_connect', {
        params: {
          host: form.host.trim(),
          port: Number(form.port) || 22,
          user: form.user.trim(),
          auth,
          cols: term.cols,
          rows: term.rows,
          acceptFingerprint: acceptFingerprint ?? null,
          startupCommand: form.startup.trim() || null,
        },
        onData,
      });
      session = connected;
      sessionId = connected.id;
      target = `${form.user.trim()}@${form.host.trim()}${Number(form.port) !== 22 ? `:${form.port}` : ''}`;
      status = 'connected';
      // Les secrets ne restent pas dans le formulaire une fois la session ouverte.
      form.password = '';
      form.passphrase = '';
      term.focus();
    } catch (e) {
      status = 'idle';
      problem = normalize(e);
      if (problem.kind === 'needPassphrase') {
        form.auth = 'keyFile';
        form.keyPath = problem.path;
      }
    }
  }

  function disconnect() {
    if (sessionId !== null) void invoke('ssh_close', { id: sessionId }).catch(() => {});
  }

  function onSubmit(e: SubmitEvent) {
    e.preventDefault();
    if (canConnect) void connect();
  }

  onMount(() => {
    const css = getComputedStyle(document.documentElement);
    const v = (name: string) => css.getPropertyValue(name).trim();
    const theme = () => ({
      background: v('--term-bg'),
      foreground: v('--term-ink'),
      cursor: v('--accent'),
      selectionBackground: v('--sel'),
    });

    const terminal = new Terminal({
      fontFamily: v('--f-mono'),
      fontSize: 13,
      lineHeight: 1.2,
      cursorBlink: true,
      scrollback: 5000,
      theme: theme(),
    });
    const fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    if (termEl) terminal.open(termEl);
    term = terminal;
    fit = fitAddon;

    terminal.onData((data) => {
      if (sessionId === null) return;
      if (!lastKey) lastKey = performance.now();
      void invoke('ssh_write', { id: sessionId, data }).catch(() => {});
    });
    terminal.onResize(({ cols, rows }) => {
      if (sessionId !== null) void invoke('ssh_resize', { id: sessionId, cols, rows }).catch(() => {});
    });

    const observer = new ResizeObserver(() => {
      // Vue masquée : la taille est nulle, on ne recalcule rien.
      if (termEl && termEl.offsetParent !== null) fitAddon.fit();
    });
    if (termEl) observer.observe(termEl);

    const media = window.matchMedia('(prefers-color-scheme: dark)');
    const onTheme = () => (terminal.options.theme = theme());
    media.addEventListener('change', onTheme);

    const unlisten = listen<Closed>('ssh-closed', (event) => {
      if (event.payload.id !== sessionId) return;
      sessionId = null;
      session = null;
      status = 'idle';
      const { exitStatus, reason } = event.payload;
      closedInfo = reason
        ? `Connexion interrompue : ${reason}`
        : exitStatus === null
          ? 'Session terminée.'
          : `Session terminée (code de sortie ${exitStatus}).`;
    });

    void invoke<Defaults>('ssh_defaults')
      .then((d) => {
        defaults = d;
        if (!form.user) form.user = d.user;
        if (d.keyFiles.length) form.keyPath = d.keyFiles[0];
      })
      .catch(() => {});

    return () => {
      observer.disconnect();
      media.removeEventListener('change', onTheme);
      void unlisten.then((stop) => stop());
      terminal.dispose();
    };
  });

  $effect(() => {
    // À l'affichage de la vue : la taille du terminal a pu changer pendant qu'elle était masquée.
    if (!active) return;
    const connected = status === 'connected';
    requestAnimationFrame(() => {
      fit?.fit();
      if (connected) term?.focus();
    });
  });
</script>

<div class="view">
  <form class="panel" onsubmit={onSubmit}>
    <span class="label">Connexion SSH</span>

    {#if defaults?.hosts.length}
      <label>
        <span>Hôte de ~/.ssh/config</span>
        <select class="field" id="ssh-alias" bind:value={alias} onchange={pickHost} disabled={status !== 'idle'}>
          <option value="">Choisir…</option>
          {#each defaults.hosts as host (host.alias)}
            <option value={host.alias}>{host.alias}</option>
          {/each}
        </select>
      </label>
    {/if}

    <div class="two">
      <label>
        <span>Adresse</span>
        <input class="field" id="ssh-host" bind:value={form.host} placeholder="serveur.example.net" autocomplete="off" spellcheck="false" disabled={status !== 'idle'} />
      </label>
      <label class="port">
        <span>Port</span>
        <input class="field" id="ssh-port" type="number" min="1" max="65535" bind:value={form.port} disabled={status !== 'idle'} />
      </label>
    </div>

    <label>
      <span>Utilisateur</span>
      <input class="field" id="ssh-user" bind:value={form.user} autocomplete="off" spellcheck="false" disabled={status !== 'idle'} />
    </label>

    <label>
      <span>Authentification</span>
      <select class="field" id="ssh-auth" bind:value={form.auth} disabled={status !== 'idle'}>
        <option value="auto">Agent SSH, puis clés de ~/.ssh</option>
        <option value="keyFile">Fichier de clé</option>
        <option value="password">Mot de passe</option>
      </select>
    </label>

    {#if form.auth === 'keyFile'}
      <label>
        <span>Clé privée</span>
        <input class="field mono" id="ssh-key" bind:value={form.keyPath} autocomplete="off" spellcheck="false" disabled={status !== 'idle'} />
      </label>
      <label>
        <span>Passphrase (si la clé est chiffrée)</span>
        <input class="field" id="ssh-passphrase" type="password" bind:value={form.passphrase} autocomplete="off" disabled={status !== 'idle'} />
      </label>
    {:else if form.auth === 'password'}
      <label>
        <span>Mot de passe</span>
        <input class="field" id="ssh-password" type="password" bind:value={form.password} autocomplete="off" disabled={status !== 'idle'} />
      </label>
    {/if}

    <label>
      <span>Commande au démarrage (facultatif)</span>
      <input class="field mono" id="ssh-startup" bind:value={form.startup} placeholder="cd /srv/docker" autocomplete="off" spellcheck="false" disabled={status !== 'idle'} />
    </label>

    {#if viaBastion && status === 'idle'}
      <p class="note warn">Cet hôte passe par un bastion (ProxyJump). Le rebond n'est pas encore pris en charge : la connexion directe peut échouer.</p>
    {/if}

    {#if status === 'connected'}
      <button class="btn" type="button" onclick={disconnect}>Déconnecter</button>
    {:else}
      <button class="btn primary" type="submit" disabled={!canConnect}>
        {status === 'connecting' ? 'Connexion…' : 'Connecter'}
      </button>
    {/if}

    {#if problem?.kind === 'hostKeyUnknown'}
      <div class="note warn" role="alert">
        <b>Serveur inconnu</b>
        <p>Ce serveur n'est pas dans <span class="mono">known_hosts</span>. Vérifie son empreinte {problem.algorithm} avant d'accepter :</p>
        <p class="mono finger">{problem.fingerprint}</p>
        <div class="actions">
          <button class="btn primary" type="button" onclick={() => problem?.kind === 'hostKeyUnknown' && connect(problem.fingerprint)}>Accepter pour cette session</button>
          <button class="btn" type="button" onclick={() => (problem = null)}>Annuler</button>
        </div>
        <p class="small">Le prototype n'écrit pas dans known_hosts : l'empreinte sera redemandée à la prochaine connexion.</p>
      </div>
    {:else if problem?.kind === 'hostKeyChanged'}
      <div class="note bad" role="alert">
        <b>Empreinte modifiée : connexion bloquée</b>
        <p>La clé de ce serveur ne correspond plus à celle enregistrée à la ligne {problem.line} de <span class="mono">known_hosts</span>.</p>
        <p class="mono finger">{problem.fingerprint}</p>
        <p class="small">Si le serveur a été réinstallé, retire l'ancienne ligne avec <span class="mono">ssh-keygen -R</span>. Sinon, ne te connecte pas.</p>
      </div>
    {:else if problem?.kind === 'needPassphrase'}
      <p class="note warn" role="alert">La clé <span class="mono">{problem.path}</span> est chiffrée. Saisis sa passphrase puis reconnecte.</p>
    {:else if problem}
      <p class="note bad" role="alert">{problem.message}</p>
    {/if}

    {#if closedInfo}
      <p class="note">{closedInfo}</p>
    {/if}
  </form>

  <div class="term-side">
    <header class="bar">
      {#if session}
        <b>{target}</b>
        <span class="ok">connecté</span>
        <span title={session.fingerprint}>empreinte {session.knownHost ? 'connue' : 'acceptée pour la session'}</span>
        <span>{session.authMethod}</span>
        <span>connexion en {Math.round(session.connectMs)} ms</span>
        <span title="Délai médian entre une frappe et la réponse du serveur, réseau compris.">
          écho {echo === null ? '—' : `${echo.toFixed(0)} ms`}
        </span>
      {:else}
        <span>{status === 'connecting' ? 'Connexion en cours…' : 'Aucune session ouverte'}</span>
      {/if}
    </header>
    <div class="term" bind:this={termEl}></div>
  </div>
</div>

<style>
  .view {
    display: grid;
    grid-template-columns: 320px minmax(0, 1fr);
    height: 100%;
    min-height: 0;
  }
  .panel {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px;
    border-right: 1px solid var(--line);
    background: var(--panel);
    overflow: auto;
    min-height: 0;
  }
  label {
    display: grid;
    gap: 4px;
    min-width: 0;
  }
  label > span {
    font-size: 12px;
    color: var(--muted);
  }
  .two {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 84px;
    gap: 8px;
  }
  .note {
    margin: 0;
    padding: 10px 12px;
    border-radius: 8px;
    border: 1px solid var(--line);
    background: var(--bg);
    display: grid;
    gap: 6px;
    overflow-wrap: anywhere;
  }
  .note p {
    margin: 0;
  }
  .note.warn {
    border-color: var(--warn);
    background: var(--warn-soft);
  }
  .note.bad {
    border-color: var(--bad);
    background: var(--bad-soft);
  }
  .finger {
    font-size: 11.5px;
    user-select: all;
  }
  .small {
    font-size: 11.5px;
    color: var(--muted);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .term-side {
    display: grid;
    grid-template-rows: 34px minmax(0, 1fr);
    min-width: 0;
    min-height: 0;
    background: var(--term-bg);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 0 12px;
    border-bottom: 1px solid var(--line);
    background: var(--panel);
    color: var(--muted);
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    font-variant-numeric: tabular-nums;
  }
  .bar b {
    color: var(--ink);
    font-family: var(--f-mono);
    font-weight: 500;
  }
  .ok {
    color: var(--ok);
  }
  .term {
    min-width: 0;
    min-height: 0;
    padding: 8px 4px 4px 10px;
    overflow: hidden;
  }
</style>
