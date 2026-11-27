<script lang="ts">
  import Icon from '../ui/Icon.svelte';
  import { toasts } from '../ui/toasts.svelte';
  import { tick, untrack } from 'svelte';
  import FileList from './FileList.svelte';
  import MessageField from './MessageField.svelte';
  import WipStrip from './WipStrip.svelte';
  import { readMessage } from './message';
  import type { Repo } from './repo.svelte';
  import type { CommitRow } from './types';
  import { dateTime, fullDateTime } from '../ui/dates';
  import { focusBack } from '../ui/focus';

  let { repo, commit }: { repo: Repo; commit: CommitRow } = $props();

  /** The date, its time kept on one line with AM or PM. */
  const fullDate = (ms: number) => fullDateTime(ms);
  const shortDate = (ms: number) => dateTime(ms);
  const numFmt = new Intl.NumberFormat('en-US');

  const detail = $derived(repo.detail);
  const body = $derived(detail ? detail.message.split('\n').slice(1).join('\n').trim() : '');
  // The description joined to the width of the panel, its trailers set apart (guidelines, 9.27).
  const view = $derived(readMessage(body));
  // Lines added and removed by the whole commit, when git counted them.
  const totals = $derived.by(() => {
    const counted = detail?.files.filter((f) => f.lines && !f.lines.binary) ?? [];
    if (!counted.length) return null;
    return {
      added: counted.reduce((sum, f) => sum + f.lines!.added, 0),
      removed: counted.reduce((sum, f) => sum + f.lines!.removed, 0),
    };
  });

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

  /**
   * A parent link: the parent becomes the selection of the graph. The detail is drawn again, which
   * would lose the keyboard: from a key, it goes to the row of the parent in the graph, with its
   * ring; the graph hidden, to the first parent link of the detail drawn again. A parent that is
   * not loaded changes nothing: the keyboard stays on its link (guidelines, 9.27).
   */
  function selectParent(e: MouseEvent, sha: string) {
    const loaded = repo.commits.some((c) => c.sha === sha);
    const view = (e.currentTarget as HTMLElement).closest('[data-repo-view]');
    repo.selectSha(sha);
    if (!loaded) return;
    // After a key with its ring, after the mouse without it (9.9, 11).
    void tick().then(() => {
      const graph = view?.querySelector<HTMLElement>('.center [role="listbox"]');
      const shown = graph && !graph.closest('[inert], [hidden]');
      focusBack(shown ? graph : (view?.querySelector<HTMLElement>('.detail .parent') ?? view?.querySelector<HTMLElement>('.detail button')));
    });
  }
</script>

<!-- The working changes stay one click away, even while another commit is shown. -->
{#if repo.wipCount > 0}<WipStrip {repo} />{/if}
<!-- Zones read in a glance, a line between them (guidelines, 9.27). -->
<div class="commit">
  <div class="head">
    <span class="label">{editing ? 'Commit message' : 'Commit'}</span>
    <span class="mono sha" title={commit.sha}>{commit.sha.slice(0, 7)}</span>
    <button class="copy" type="button" title="Copy SHA" aria-label="Copy SHA" onclick={copySha}><Icon name="copy" size={14} /></button>
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
    <hr />
    <div class="zone">{@render facts()}</div>
  {:else}
    <div class="zone">
      <p class="title">{commit.subject}</p>
      {@render facts()}
    </div>
    {#if view.paragraphs.length || view.trailers.length || (check.reason && !repo.locked)}
      <hr />
      <div class="zone message">
        {#each view.paragraphs as paragraph, i (i)}
          <p class:pre={paragraph.pre}>{paragraph.text}</p>
        {/each}
        {#if view.trailers.length}<p class="trailers">{view.trailers.join('\n')}</p>{/if}
        <!-- A disabled "Reword" says why, under the message (guidelines, 9.1 and 9.25). -->
        {#if check.reason && !repo.locked}
          <p class="why" class:nudged id="reword-why" aria-live="polite">
            {#key nudged}
              {#if nudged}<Icon name="warning" size={14} />{/if}
              <span>{check.reason}</span>
            {/key}
          </p>
        {/if}
      </div>
    {/if}
  {/if}

  <hr />
  <div class="zone">
    {#if repo.detailError}
      <p class="err" role="alert">{repo.detailError}</p>
    {:else if detail}
      <div class="files-head">
        <span class="heading">Changed files · {detail.files.length}</span>
        {#if commit.parents.length > 1}<span class="against">(against the first parent)</span>{/if}
        {#if totals}
          <span class="totals">
            {#if totals.added}<span class="plus">+{numFmt.format(totals.added)}</span>{/if}
            {#if totals.removed}<span class="minus">−{numFmt.format(totals.removed)}</span>{/if}
          </span>
        {/if}
      </div>
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
      <div class="skeleton" role="img" aria-label="Loading files">
        <span></span><span></span><span></span>
      </div>
    {/if}
  </div>
</div>

<!-- Who, when, where it is and what comes before it, under the subject or under the form. -->
{#snippet facts()}
  <div class="facts">
    <p class="byline"><span class="author">{commit.author}</span> · <span title={fullDate(commit.time * 1000)}>{shortDate(commit.time * 1000)}</span></p>
    {#if repo.hasRemote}
      <p class="fact">
        <Icon name={commit.localOnly ? 'laptop' : 'cloud'} size={14} />
        <span>{commit.localOnly ? 'Local only, not pushed yet' : 'On the remote'}</span>
      </p>
    {/if}
    {#if commit.refs.length}
      {@const refs = commit.refs.map((r) => r.name).join(', ')}
      <p class="fact"><span>Refs</span> <span class="mono refs" title={refs}>{refs}</span></p>
    {/if}
    <p class="fact parents">
      {#if commit.parents.length}
        <span>Parent{commit.parents.length === 1 ? '' : 's'}</span>
        {#each commit.parents as parent (parent)}
          <button class="btn ghost small mono parent" type="button" title="Select this commit in the graph" onclick={(e) => selectParent(e, parent)}>{parent.slice(0, 7)}</button>
        {/each}
      {:else}
        <span>No parent: first commit</span>
      {/if}
    </p>
  </div>
{/snippet}

<style>
  p {
    margin: 0;
  }
  .commit {
    width: 100%;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 24px;
  }
  .sha {
    color: var(--muted);
    font-size: var(--fs-code);
    user-select: all;
  }
  .reword-open {
    margin-left: auto;
  }
  /* The zone under the head starts 8 px lower (guidelines, 9.27). */
  .head + .zone,
  .head + .reword {
    margin-top: 8px;
  }
  .zone {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  /* Between two zones, a line the whole width of the card: 16 px under a zone, 12 px above the
     next (guidelines, 6 and 9.27). */
  hr {
    flex: none;
    width: calc(100% + 24px);
    height: 1px;
    margin: 16px -12px 12px;
    border: 0;
    background: var(--line);
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
  /* Who, when, where: 12 px, in --muted, the author and the names of branches in --ink. */
  .facts {
    display: flex;
    flex-direction: column;
    gap: 4px;
    color: var(--muted);
    font-size: var(--fs-label);
    font-variant-numeric: tabular-nums;
  }
  .author,
  .refs {
    color: var(--ink);
  }
  .fact {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .fact :global(.icon) {
    flex: none;
  }
  /* Cut on the right, the whole list in the tooltip (guidelines, 3.4). */
  .refs {
    min-width: 0;
    font-size: var(--fs-code);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .parents {
    flex-wrap: wrap;
    gap: 2px 6px;
  }
  .parent {
    font-size: var(--fs-code);
  }
  /* The description in --ink: the subject stands out by its size and weight (guidelines, 9.27). */
  .message {
    gap: 8px;
    overflow-wrap: anywhere;
  }
  .message .pre {
    white-space: pre-wrap;
  }
  .trailers {
    margin-top: 4px;
    white-space: pre-wrap;
    color: var(--muted);
    font-size: var(--fs-small);
  }
  .message .why {
    margin-top: 4px;
  }
  /* The heading and the totals on one line; for a merge, "(against the first parent)" under them. */
  .files-head {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px 6px;
    min-width: 0;
  }
  .heading {
    font-size: var(--fs-label);
    font-weight: var(--fw-strong);
    white-space: nowrap;
  }
  .against {
    order: 1;
    flex-basis: 100%;
    color: var(--muted);
    font-size: var(--fs-small);
  }
  .totals {
    margin-left: auto;
    display: flex;
    gap: 6px;
    font-size: var(--fs-small);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .plus {
    color: var(--ok);
  }
  .minus {
    color: var(--bad);
  }
  .copy {
    flex: none;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
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
  /* The rows reach 6 px past the text on each side: their names line up with the heading. The
     first row starts 6 px under the heading, the 4 px of the zone and 2 px (guidelines, 9.27). */
  .files {
    width: calc(100% + 12px);
    margin: 2px -6px 0;
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
</style>
