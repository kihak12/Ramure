<script lang="ts">
  import Icon from '../ui/Icon.svelte';
  import { toasts } from '../ui/toasts.svelte';
  import { tick, untrack } from 'svelte';
  import FileList from './FileList.svelte';
  import MessageField from './MessageField.svelte';
  import WipStrip from './WipStrip.svelte';
  import type { Repo } from './repo.svelte';
  import type { CommitRow } from './types';

  let { repo, commit }: { repo: Repo; commit: CommitRow } = $props();

  const fullDateFmt = new Intl.DateTimeFormat('en-US', { dateStyle: 'full', timeStyle: 'short' });
  /** The full date, its time kept on one line with AM or PM. */
  const fullDate = (ms: number) => fullDateFmt.format(ms).replace(/ (AM|PM)$/, '\u00a0$1');
  const detail = $derived(repo.detail);
  const body = $derived(detail ? detail.message.split('\n').slice(1).join('\n').trim() : '');

  // --- Rewording (guidelines, 9.25) ---------------------------------------------------------
  const check = $derived(repo.rewordCheck(commit));
  const draft = $derived(repo.reword?.sha === commit.sha ? repo.reword : null);
  const editing = $derived(!!draft);
  const saving = $derived(repo.busy === 'reword');
  const staged = $derived(repo.status?.staged.length ?? 0);
  let field = $state<HTMLTextAreaElement>();

  // The whole message, once the detail of the commit is read: the draft starts from it.
  $effect(() => {
    // The detail of this commit, not that of the one looked at before.
    if (!draft || draft.text !== null || !detail || repo.detailFor !== repo.currentKey) return;
    draft.text = detail.message;
    draft.original = detail.message;
  });

  // Each request to reword gives the keyboard to the field, the caret at the end of the first
  // line, where a typo most often is; a draft taken up again too (guidelines, 9.25).
  $effect(() => {
    const ask = repo.rewordAsk;
    if (ask === repo.rewordServed || draft?.text == null) return;
    repo.rewordServed = ask;
    void tick().then(() => {
      if (!field) return;
      const end = field.value.split('\n')[0].length;
      field.focus();
      field.setSelectionRange(end, end);
    });
  });

  // F2 or the palette on a commit that cannot be reworded: the reason turns to a warning, with
  // its icon, and is announced (guidelines, 9.1).
  const refused = untrack(() => repo.rewordRefused);
  const nudged = $derived(repo.rewordRefused > refused);

  /** Why "Save message" is disabled, also its tooltip. */
  const unsaved = $derived(
    !draft || draft.text === null ? '' : !draft.text.trim() ? 'Write a commit message.' : draft.text.trim() === draft.original.trim() ? 'Change the message to save it.' : '',
  );

  /** Escape and Ctrl + Enter, from anywhere in the form, as in a dialog (guidelines, 9.25). */
  function onFormKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      repo.cancelReword();
    } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      if (!unsaved) void repo.saveReword();
    }
  }

  async function copySha() {
    try {
      await navigator.clipboard.writeText(commit.sha);
      toasts.ok('SHA copied');
    } catch {
      /* clipboard refused: the SHA stays selectable in the panel */
    }
  }
</script>

<!-- The working changes stay one click away, even while another commit is shown. -->
{#if repo.wipCount > 0}<WipStrip {repo} />{/if}
<div class="head">
  <span class="label">{editing ? 'Commit message' : 'Commit'}</span>
  {#if !editing}
    <button
      class="btn ghost small reword-open"
      type="button"
      disabled={!!check.reason || repo.locked}
      title={check.reason || 'Give this commit a new message (F2 in the graph)'}
      aria-describedby={check.reason && !repo.locked ? 'reword-why' : undefined}
      onclick={() => repo.startReword()}>Reword</button
    >
  {/if}
</div>
{#if draft}
  <!-- The message edited in place: the rest of the panel stays under it (guidelines, 9.25). -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="reword" onkeydown={onFormKey}>
    {#if draft.text === null}
      <p class="muted">Reading the message…</p>
    {:else}
      <MessageField
        bind:value={draft.text}
        bind:field
        label="Commit message"
        rows={5}
        disabled={saving}
        describedby={check.pushed || check.replayed || (check.head && staged) ? 'reword-notes' : undefined}
      >
        {#snippet notes()}
          <!-- What saving does: it describes the field and the main button (guidelines, 9.25). -->
          <div class="notes" id="reword-notes">
          {#if check.head && staged}
            <p class="note">The {staged === 1 ? 'staged file is' : `${staged} staged files are`} not part of this commit.</p>
          {/if}
          {#if !check.head && check.replayed}
            <p class="note">
              {check.replayed === 1
                ? 'The commit after this one is replayed and gets a new SHA.'
                : `The ${check.replayed} commits after this one are replayed and get new SHAs.`} The position of
              <span class="mono">{repo.branch}</span> is saved in <span class="mono">refs/ramure/backup</span>.
            </p>
          {/if}
          {#if check.pushed}
            <p class="warn"><Icon name="warning" size={14} /> <span>This commit is already on the remote: after rewording it, you will have to force push.</span></p>
          {/if}
          </div>
        {/snippet}
        {#snippet actions()}
          <span class="buttons">
            <button class="btn small" type="button" disabled={saving} onclick={() => repo.cancelReword()}>Cancel</button>
            <button
              class="btn primary small"
              type="button"
              disabled={!!unsaved || saving}
              title={unsaved || 'Ctrl + Enter'}
              aria-describedby={check.pushed || check.replayed || (check.head && staged) ? 'reword-notes' : undefined}
              onclick={() => void repo.saveReword()}
            >
              {saving ? 'Saving…' : 'Save message'}
            </button>
          </span>
        {/snippet}
      </MessageField>
    {/if}
  </div>
{:else}
  <p class="title">{commit.subject}</p>
  {#if body}<p class="message">{body}</p>{/if}
  <!-- A disabled "Reword" says why, under the message (guidelines, 9.1 and 9.25). -->
  {#if check.reason && !repo.locked}
    <p class="why" class:nudged id="reword-why" aria-live="polite">
      {#key nudged}
        {#if nudged}<Icon name="warning" size={14} />{/if}
        <span>{check.reason}</span>
      {/key}
    </p>
  {/if}
{/if}
<dl>
  <dt>SHA</dt>
  <dd class="sha">
    <span class="mono sha-full">{commit.sha}</span>
    <button class="copy" type="button" title="Copy SHA" aria-label="Copy SHA" onclick={copySha}><Icon name="copy" size={14} /></button>
  </dd>
  <dt>Author</dt>
  <dd>{commit.author}</dd>
  <dt>Date</dt>
  <dd>{fullDate(commit.time * 1000)}</dd>
  <dt>Parent{commit.parents.length === 1 ? '' : 's'}</dt>
  <dd>{#if commit.parents.length}<span class="mono parents">{commit.parents.map((p) => p.slice(0, 7)).join(', ')}</span>{:else}none (first commit){/if}</dd>
  {#if repo.hasRemote}
    <dt>Location</dt>
    <dd>{commit.localOnly ? 'Local only, not pushed yet' : 'On the remote'}</dd>
  {/if}
  {#if commit.refs.length}
    <dt>Refs</dt>
    <dd>{commit.refs.map((r) => r.name).join(', ')}</dd>
  {/if}
</dl>

{#if repo.detailError}
  <p class="err" role="alert">{repo.detailError}</p>
{:else if detail}
  <span class="label">
    Changed files · {detail.files.length}{commit.parents.length > 1 ? ' (against the first parent)' : ''}
  </span>
  {#if detail.files.length}
    <div class="files">
      <FileList
        files={detail.files}
        active={repo.openFile?.key.startsWith('c:') ? repo.openFile.key.slice(2) : null}
        onopen={(f) => repo.openCommitDiff(f)}
      />
    </div>
  {:else}
    <p class="muted">This commit changes no files.</p>
  {/if}
{:else}
  <div class="skeleton" aria-label="Loading files">
    <span></span><span></span><span></span>
  </div>
{/if}

<style>
  p {
    margin: 0;
  }
  .head {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    min-height: 24px;
  }
  /* The whole width of the panel: the buttons at its right edge (guidelines, 9.25). */
  .reword {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  /* What the rewording does, one line each, 4 px apart (guidelines, 9.10 and 9.25). */
  .notes {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .notes:empty {
    display: none;
  }
  .note,
  .why {
    color: var(--muted);
    font-size: var(--fs-small);
    overflow-wrap: anywhere;
  }
  .why {
    display: flex;
    gap: 6px;
    align-items: flex-start;
    transition: color var(--t-fast) var(--ease);
  }
  .why.nudged {
    color: var(--warn);
  }
  .why :global(.icon) {
    flex: none;
    margin-top: 1px;
  }
  .warn {
    display: flex;
    gap: 6px;
    align-items: flex-start;
    padding: 6px 8px;
    border-radius: var(--radius);
    background: var(--warn-soft);
    color: var(--ink);
    font-size: var(--fs-small);
  }
  .warn :global(.icon) {
    flex: none;
    margin-top: 1px;
    color: var(--warn);
  }
  .buttons {
    display: flex;
    gap: 8px;
  }
  .title {
    font-size: var(--fs-title);
    font-weight: var(--fw-strong);
    overflow-wrap: anywhere;
  }
  .message {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    color: var(--muted);
  }
  dl {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: 6px 12px;
    margin: 0;
    width: 100%;
  }
  dt {
    color: var(--muted);
    font-size: var(--fs-label);
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .sha {
    display: flex;
    align-items: flex-start;
    gap: 6px;
  }
  .sha-full {
    font-size: var(--fs-code);
    user-select: all;
  }
  .copy {
    flex: none;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    margin-top: -4px;
    border: 0;
    border-radius: var(--radius-s);
    background: none;
    color: var(--muted);
    cursor: pointer;
    transition:
      background var(--t-fast) var(--ease),
      transform var(--t-fast) var(--ease);
  }
  .copy:hover {
    background: var(--hover);
    color: var(--ink);
  }
  .copy:active {
    transform: scale(0.97);
  }
  .files {
    width: calc(100% + 12px);
    margin: -4px -6px 0;
  }
  .muted {
    color: var(--muted);
  }
  .err {
    color: var(--bad);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }
  /* Ghost lines while git lists the files. */
  .skeleton {
    display: grid;
    gap: 8px;
    width: 100%;
  }
  .skeleton span {
    height: 14px;
    border-radius: var(--radius-xs);
    background: linear-gradient(90deg, var(--hover) 25%, var(--line-soft) 50%, var(--hover) 75%);
    background-size: 200% 100%;
    animation: shimmer var(--t-wait) linear infinite;
  }
  .skeleton span:nth-child(2) {
    width: 70%;
  }
  .skeleton span:nth-child(3) {
    width: 85%;
  }
  @keyframes shimmer {
    to {
      background-position: -200% 0;
    }
  }
  .parents {
    font-size: var(--fs-code);
  }
</style>
