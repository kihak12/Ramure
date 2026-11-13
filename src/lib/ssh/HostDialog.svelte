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
  /** Note shown under the key field after a choice in the file picker. */
  let keyNote = $state('');
  /** What failed in the form, said under the element that triggered it, never in a
   *  notification, which the veil of the dialog would cover (guidelines, 9.11). */
  let keyError = $state('');
  let forgetError = $state<{ password: string; passphrase: string }>({ password: '', passphrase: '' });
  /** The host could not be saved or deleted: said in the last line of the form. */
  let failure = $state('');

  const editing = $derived(ssh.editing);
  const valid = $derived(!!form && form.alias.trim() !== '' && form.hostname.trim() !== '');
  // A host being added (or a copy) is not in the book yet: one nearly always connects to it at
  // once, so saving connects. A host being edited is only saved.
  const adding = $derived(!editing?.id);
  /** What is missing while the form is not valid: said beside the test and in the tooltips (9.1). */
  const MISSING = 'Enter a name and an address.';
  // Enter in an incomplete form: nothing is saved, the sentence turns to a warning (9.1).
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
      keyError = '';
      failure = '';
      forgetError = { password: '', passphrase: '' };
      forget();
      element.showModal();
      // On the first opening, the fields appear after the dialog: `autofocus` does not act there.
      // The focus then goes to the first field, "Name" (guidelines, 9.10).
      void tick().then(() => element?.querySelector<HTMLInputElement>('input[autofocus]')?.focus());
    } else if (!editing && element.open) {
      element.close();
    }
  });

  // Closing the form drops a test in progress: its result will show nowhere.
  const close = () => {
    forget();
    ssh.editing = null;
  };

  /** The host as the form has it, tags included. */
  function current(): Host | null {
    if (!form) return null;
    return { ...form, port: Number(form.port) || 22, tags: tags.split(',').map((t) => t.trim()).filter(Boolean) };
  }

  // "Delete host…": only for a host already in the book, not for one being added nor for a copy.
  let removeButton = $state<HTMLButtonElement>();
  async function removeHost() {
    if (!editing?.id) return;
    // The host as it is saved: its account says which password goes with it.
    const stored = ssh.hosts.find((h) => h.id === editing.id) ?? editing;
    failure = '';
    if (await ssh.removeAsked(stored, (text) => (failure = text))) {
      close();
      await ssh.focusBook();
    }
    // Canceled: the confirmation closes, the keyboard comes back to the button that opened it, with the ring.
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

  /** Chooses the private key in the file picker of the system, opened in `~/.ssh`. */
  async function browseKey() {
    let home = '';
    let folder: string | undefined;
    try {
      home = (await homeDir()).replace(/[\\/]+$/, '');
      folder = await join(home, '.ssh');
    } catch {
      folder = undefined; // home folder not found: the picker opens where the system decides
    }
    let picked: string | null;
    try {
      picked = await open({ title: 'Choose a private key', multiple: false, directory: false, defaultPath: folder });
    } catch (e) {
      keyError = `The file picker could not open: ${e instanceof Error ? e.message : String(e)}`;
      return;
    }
    // Picker canceled, or form closed in the meantime: the field does not change.
    if (!picked || !form) return;
    keyNote = '';
    keyError = '';
    if (picked.endsWith('.pub')) {
      // The connection uses the private key, kept beside it under the same name.
      picked = picked.slice(0, -'.pub'.length);
      keyNote = 'You chose the public key: the field points to the private key of the same name, which is the one used to connect.';
    }
    // In the home folder, the path is written `~/…`, like the keys the field suggests.
    if (home && picked.startsWith(home) && /^[\\/]/.test(picked.slice(home.length))) picked = `~${picked.slice(home.length)}`;
    form.keyPath = picked;
  }

  // --- Secrets kept in the system keyring (guidelines, section 9.21) -----------------------------
  // The password belongs to the account (user, server, port), the passphrase to the key: the form
  // shows what the keyring keeps for them, and lets the user forget it.
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

  // Each read carries a number: the answer of an outdated read is ignored.
  let reading = 0;
  async function readHeld() {
    const mine = ++reading;
    const refs = [passwordRef, passphraseRef];
    const answers = await secretsHeld(refs.filter((ref) => ref !== null));
    if (mine !== reading) return;
    held = { password: !!passwordRef && answers[0] === true, passphrase: !!passphraseRef && answers[passwordRef ? 1 : 0] === true };
  }

  // The form opens, or a field of the connection changes: read again, once typing has settled.
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

  /** Forgets the kept secret; the focus goes to the field it depends on, since the button is gone. */
  async function forgetHeld(kind: 'password' | 'passphrase') {
    const ref = kind === 'password' ? passwordRef : passphraseRef;
    if (!ref) return;
    forgetError[kind] = '';
    try {
      await forgetSecret(ref);
      held[kind] = false;
      focusWithRing(kind === 'password' ? authEl : keyEl);
    } catch (e) {
      forgetError[kind] = `The system keyring could not forget it: ${typeof e === 'string' ? e : String(e)}`;
    }
  }

  // --- Test connection (guidelines, section 9.21) --------------------------------------------
  /** What the test shows, between the fields and the buttons. */
  type Test =
    | { step: 'idle' }
    | { step: 'running' }
    /** `learnFailed`: the fingerprint was to be remembered and `known_hosts` could not be written. */
    | { step: 'ok'; method: string; ms: number; learnFailed: string | null }
    | { step: 'fail'; message: string }
    | { step: 'hostKey'; fingerprint: string; algorithm: string }
    | { step: 'secret'; kind: 'passphrase' | 'password'; path: string; error: string };

  /** What is known for this test: nothing is kept once the result is known. */
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
  // Each test carries a number: the result of a dropped test (form closed, field changed) is ignored.
  let run = 0;
  let secretEl = $state<HTMLInputElement>();
  let testButton = $state<HTMLButtonElement>();

  // The clicked button is gone (a question), or was disabled during the test: once the result
  // shows, the keyboard comes back to "Test connection" rather than getting lost.
  $effect(() => {
    if (test.step !== 'ok' && test.step !== 'fail') return;
    const lost = !document.activeElement || document.activeElement === document.body || document.activeElement === element;
    if (lost) void tick().then(() => testButton?.focus());
  });

  /** Forgets the test: its result, its secrets, and the one that may still be running. */
  function forget() {
    run++;
    attempt = {};
    secret = '';
    test = { step: 'idle' };
  }

  // A field of the connection changes: the result no longer says anything about the form, it goes away.
  $effect(() => {
    if (!form) return;
    void [form.hostname, form.port, form.user, form.auth, form.keyPath, form.proxyJump];
    untrack(forget);
  });

  // A secret is asked for: its field takes the focus.
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
      test = { step: 'fail', message: 'Enter a user: the account of this machine could not be read.' };
      return;
    }
    const keyPath = attempt.keyPath ?? (host.auth === 'keyFile' ? host.keyPath : '');
    const auth =
      host.auth === 'password'
        ? // Without a typed password, the test takes the one the keyring keeps, as a connection does.
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
            ? 'This passphrase does not unlock the key.'
            : problem.refused === 'retained'
              ? 'The remembered passphrase no longer unlocks the key. It has been forgotten.'
              : '';
        test = { step: 'secret', kind: 'passphrase', path: problem.path, error };
        // A kept secret that is refused is forgotten: the form no longer shows it.
        if (problem.refused === 'retained') void readHeld();
        return;
      }
      if (problem.kind === 'needPassword') {
        attempt = { ...attempt, password: undefined };
        test = { step: 'secret', kind: 'password', path: '', error: problem.refused ? 'The server refused the remembered password. It has been forgotten.' : '' };
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
            ? `The server fingerprint no longer matches the one on line ${problem.line} of known_hosts: the test is blocked, as a connection would be.`
            : explain(problem.message),
      };
    }
  }

  function submitSecret() {
    if (test.step !== 'secret' || !secret) return;
    if (test.kind === 'passphrase') void startTest({ keyPath: test.path, passphrase: secret });
    else void startTest({ password: secret });
  }
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
    <!-- The Enter key is only watched: the fields stay the interactive elements. -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <form
      onsubmit={(e) => {
        e.preventDefault();
        void save(adding);
      }}
      onkeydown={(e) => {
        // Enter in a field of an incomplete form: the disabled button does nothing, the sentence says so.
        if (e.key === 'Enter' && !valid && e.target instanceof HTMLInputElement) tried = true;
      }}
    >
      <!-- The fields scroll if the window is too low; the last line stays in view (guidelines, 9.10). -->
      <div class="content">
      <h2>{form.id ? `Edit ${editing?.alias ?? 'host'}` : 'New host'}</h2>

      {#if form.proxyJump}
        <p class="warning">
          <Icon name="warning" />
          <span>This host goes through a bastion (ProxyJump). It is in your hosts, but connecting through a bastion is not supported yet.</span>
        </p>
      {/if}

      <div class="grid">
        <label class="full">
          <span class="label">Name</span>
          <!-- svelte-ignore a11y_autofocus -->
          <input class="field" type="text" placeholder="vps-prod" spellcheck="false" autocomplete="off" autofocus bind:value={form.alias} />
        </label>
        <label class="most">
          <span class="label">Address</span>
          <input class="field mono" type="text" placeholder="server.example.net" spellcheck="false" autocomplete="off" bind:value={form.hostname} />
        </label>
        <label>
          <span class="label">Port</span>
          <input class="field mono" type="number" min="1" max="65535" bind:value={form.port} />
        </label>
        <label class="full">
          <span class="label">User</span>
          <input class="field mono" type="text" placeholder={ssh.defaultUser || 'user'} spellcheck="false" autocomplete="off" bind:value={form.user} />
        </label>

        <label class="full">
          <span class="label">Authentication</span>
          <Select bind:element={authEl} bind:value={form.auth}>
            <option value="auto">SSH agent, then the default keys of ~/.ssh</option>
            <option value="keyFile">Key file</option>
            <option value="password">Password</option>
          </Select>
        </label>
        {#if form.auth === 'password'}
          <!-- What the keyring keeps for this account, and the way to forget it. -->
          <div class="full kept">
            {#if held.password}
              <small role="status">The password of this account is kept in the system keyring.</small>
              <button class="btn ghost small" type="button" onclick={() => void forgetHeld('password')}>Forget password</button>
              {#if forgetError.password}<small class="bad" role="alert">{forgetError.password}</small>{/if}
            {:else if keyring === false}
              <small>{NO_KEYRING.password}</small>
            {:else}
              <small>Asked for when you connect. You can have the system keyring remember it.</small>
            {/if}
          </div>
        {/if}
        {#if form.auth === 'keyFile'}
          <div class="full key">
            <label for="host-key"><span class="label">Private key</span></label>
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
              <button class="btn" type="button" onclick={() => void browseKey()}><Icon name="folder" size={14} /> Browse…</button>
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
                <small role="status">The passphrase of this key is kept in the system keyring.</small>
                <button class="btn ghost small" type="button" onclick={() => void forgetHeld('passphrase')}>Forget passphrase</button>
                {#if forgetError.passphrase}<small class="bad" role="alert">{forgetError.passphrase}</small>{/if}
              </div>
            {:else}
              <small>If the key is protected, its passphrase is asked for when you connect.</small>
            {/if}
          </div>
        {/if}

        <label class="full">
          <span class="label">Startup command</span>
          <input class="field mono" type="text" placeholder="cd /srv/docker" spellcheck="false" autocomplete="off" bind:value={form.startupCommand} />
          <small>Sent to the terminal as soon as the shell is ready.</small>
        </label>

        <!-- The log of every session: its folder is in the settings, a single place (9.18). -->
        <div class="full journal">
          <label class="check">
            <input type="checkbox" bind:checked={form.logSessions} />
            <span>Record a log of every session</span>
          </label>
          <small>One text file per session, in the log folder of the settings. It holds everything the terminal shows, sensitive data included.</small>
        </div>

        <label class="half">
          <span class="label">Group</span>
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
          <legend class="label">Color</legend>
          <label class="swatch none" class:on={form.color === ''} title="None">
            <input type="radio" name="host-color" value="" bind:group={form.color} /><span class="spoken">None</span>
          </label>
          {#each COLORS as color (color)}
            <label class="swatch" class:on={form.color === color} style:--c="var(--l{color})" title="Color {Number(color) + 1}">
              <input type="radio" name="host-color" value={color} bind:group={form.color} /><span class="spoken">Color {Number(color) + 1}</span>
            </label>
          {/each}
        </fieldset>
      </div>

      <!-- The test, alone on its line under the fields; its result right under it (guidelines, section 9.21). -->
      <div class="testing">
        <button
          bind:this={testButton}
          class="btn"
          type="button"
          disabled={!testable}
          title={form.proxyJump
            ? 'Connecting through a bastion is not supported yet'
            : valid
              ? 'Connect with these values, without saving anything or opening a terminal'
              : MISSING}
          onclick={() => {
            attempt = {};
            void startTest();
          }}
        >
          {#if test.step === 'running'}<span class="spinner" aria-hidden="true"></span> Testing…{:else}Test connection{/if}
        </button>
        <!-- While the form is not valid, what is missing, beside the first disabled button. -->
        {#if !valid && !form.proxyJump}
          <small class="why" class:tried role={tried ? 'alert' : undefined}>{#if tried}<Icon name="warning" size={14} />{/if}{MISSING}</small>
        {/if}
      </div>

      {#if test.step === 'ok'}
        <p class="test ok" role="status">
          <Icon name="check" size={14} />
          <span><b>Connection succeeded</b> <span class="how">{test.method}, in {Math.round(test.ms)} ms</span></span>
        </p>
        {#if test.learnFailed}
          <!-- The server was accepted for this test, but not written in known_hosts (guidelines, 9.21). -->
          <p class="test warn" role="status">
            <Icon name="warning" size={14} />
            <span><b>The fingerprint could not be remembered</b> <span class="how mono">{test.learnFailed}</span></span>
          </p>
        {/if}
      {:else if test.step === 'fail'}
        <p class="test bad" role="alert"><Icon name="warning" size={14} /> <span>{test.message}</span></p>
      {:else if test.step === 'hostKey'}
        <div class="test ask" role="group" aria-label="Unknown server">
          <p>
            <b class="mono">{form.hostname}</b> is not in <span class="mono">known_hosts</span>. Check that this {test.algorithm} fingerprint
            is the server’s before you trust it:
          </p>
          <p class="print mono">{test.fingerprint}</p>
          <div class="row">
            <button class="btn small" type="button" onclick={() => test.step === 'hostKey' && void startTest({ accept: test.fingerprint, remember: true })}>
              Trust and remember
            </button>
            <button class="btn small" type="button" onclick={() => test.step === 'hostKey' && void startTest({ accept: test.fingerprint, remember: false })}>
              Trust this time
            </button>
          </div>
        </div>
      {:else if test.step === 'secret'}
        <div class="test ask" role="group" aria-label={test.kind === 'passphrase' ? 'Key passphrase' : 'Password'}>
          <p>
            {#if test.kind === 'passphrase'}The key <span class="mono">{test.path}</span> is protected: enter its passphrase for the test. It is forgotten afterwards.
            {:else}Enter the password for the test. It is forgotten afterwards: a test keeps nothing.{/if}
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
              aria-label={test.kind === 'passphrase' ? 'Passphrase' : 'Password'}
              bind:value={secret}
              onkeydown={(e) => {
                // Enter tests: it does not save the form.
                if (e.key === 'Enter') {
                  e.preventDefault();
                  submitSecret();
                }
              }}
            />
            <button class="btn small" type="button" disabled={!secret} onclick={submitSecret}>Test</button>
          </div>
          {#if test.error}<p class="err" id="host-secret-error" role="alert">{test.error}</p>{/if}
        </div>
      {/if}

      </div>

      <!-- "Delete host…" alone on the left, if the host is in the book; "Cancel" then the single
           save, on the right (guidelines, 9.1, "Hierarchy", and 9.21). -->
      <footer>
        {#if editing?.id}
          <button class="btn ghost bad remove" type="button" bind:this={removeButton} onclick={() => void removeHost()}>Delete host…</button>
        {/if}
        <!-- A failure to save or to delete, on the left of the buttons, which do not move (guidelines, 9.11). -->
        <div class="failure">
          {#if failure}<p role="alert"><Icon name="warning" size={14} /><span>{failure}</span></p>{/if}
        </div>
        <button class="btn" type="button" onclick={close}>Cancel</button>
        <button class="btn primary" type="submit" disabled={!valid} title={valid ? undefined : MISSING}>
          {adding ? 'Save and connect' : 'Save'}
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
  /* In a low window (1000 × 600), the fields scroll and the last line stays in view. */
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
  /* 4 px of margin given back: the focus halo of a field (3 px) is not cut off by the scrolling. */
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
  /* The key field and its "Browse…" button side by side. */
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
  /* A checkbox and its text, 6 px away, then its help (guidelines, 9.3 and 9.21). */
  .journal {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  /* More specific than `.grid label`, which stacks a label above its field. */
  .grid .check {
    flex-direction: row;
    align-items: center;
    align-self: flex-start;
    gap: 6px;
    font-size: var(--fs-body);
    color: var(--ink);
  }
  /* A kept secret: what the keyring keeps and, under it, the button that forgets it. The button,
     without background or border, is pulled back by its inner margin (8 px and 1 px of border): its
     text lines up with the sentence. */
  .kept {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
  }
  .kept .btn {
    margin-left: -9px;
  }
  /* Six columns: the address takes five, the port one. */
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
  /* A swatch reached with the keyboard shows the focus ring: its radio button is invisible. */
  .swatch:has(input:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .swatch input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }
  /* The buttons at the bottom stand a little apart from the test and its result: 24 px instead of 12. */
  /* 24 px above the last line (guidelines, 9.21). */
  footer {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    margin-top: 24px;
  }
  footer .btn {
    flex: none;
  }
  /* Between the destructive button, far left, and the buttons on the right: the failure, if any. */
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
  /* Why the test and the save are disabled (guidelines, 9.1). After Enter is pressed all the
     same, the sentence turns to a warning, with its icon. */
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
  /* The result of the test, between the fields and the buttons (guidelines, section 9.21). */
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
