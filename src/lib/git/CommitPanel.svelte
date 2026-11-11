<script lang="ts">
  import Icon from '../ui/Icon.svelte';
  import { toasts } from '../ui/toasts.svelte';
  import FileList from './FileList.svelte';
  import type { Repo } from './repo.svelte';
  import type { CommitRow } from './types';

  let { repo, commit }: { repo: Repo; commit: CommitRow } = $props();

  const fullDateFmt = new Intl.DateTimeFormat('fr-FR', { dateStyle: 'full', timeStyle: 'short' });
  const detail = $derived(repo.detail);
  const body = $derived(detail ? detail.message.split('\n').slice(1).join('\n').trim() : '');

  async function copySha() {
    try {
      await navigator.clipboard.writeText(commit.sha);
      toasts.ok('SHA copié');
    } catch {
      /* presse-papiers refusé : le SHA reste sélectionnable dans le panneau */
    }
  }
</script>

<header class="head">
  <span class="label">Commit</span>
  <!-- Les modifications en cours restent à un clic, même quand on regarde un autre commit. -->
  {#if repo.wipCount > 0}
    <button class="btn small" type="button" onclick={() => repo.showWip()}>
      <Icon name="pencil" size={14} />
      {repo.wipText}
    </button>
  {/if}
</header>
<p class="title">{commit.subject}</p>
{#if body}<p class="message">{body}</p>{/if}
<dl>
  <dt>SHA</dt>
  <dd class="sha">
    <span class="mono sha-full">{commit.sha}</span>
    <button class="copy" type="button" title="Copier le SHA" aria-label="Copier le SHA" onclick={copySha}><Icon name="copy" size={14} /></button>
  </dd>
  <dt>Auteur</dt>
  <dd>{commit.author}</dd>
  <dt>Date</dt>
  <dd>{fullDateFmt.format(commit.time * 1000)}</dd>
  <dt>Parent{commit.parents.length > 1 ? 's' : ''}</dt>
  <dd class="mono">{commit.parents.length ? commit.parents.map((p) => p.slice(0, 7)).join(', ') : 'aucun (premier commit)'}</dd>
  {#if repo.hasRemote}
    <dt>Emplacement</dt>
    <dd>{commit.localOnly ? 'Local uniquement, pas encore poussé' : 'Présent sur le dépôt distant'}</dd>
  {/if}
  {#if commit.refs.length}
    <dt>Références</dt>
    <dd>{commit.refs.map((r) => r.name).join(', ')}</dd>
  {/if}
</dl>

{#if repo.detailError}
  <p class="err">{repo.detailError}</p>
{:else if detail}
  <span class="label">
    Fichiers modifiés · {detail.files.length}{commit.parents.length > 1 ? ' (par rapport au premier parent)' : ''}
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
    <p class="muted">Ce commit ne modifie aucun fichier.</p>
  {/if}
{:else}
  <div class="skeleton" aria-label="Chargement des fichiers">
    <span></span><span></span><span></span>
  </div>
{/if}

<style>
  .head {
    /* Toute la largeur de la carte : le bouton va au bord droit. La hauteur est celle d'un petit
       bouton, pour que le titre du commit ne bouge pas quand le bouton apparaît. */
    align-self: stretch;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    min-height: 24px;
  }
  p {
    margin: 0;
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
    transform: scale(0.9);
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
  /* Lignes fantômes pendant que git liste les fichiers. */
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
