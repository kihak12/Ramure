<script lang="ts">
  import { tick } from 'svelte';
  import { veil } from '../ui/veil';
  import { flip } from 'svelte/animate';
  import Icon from '../ui/Icon.svelte';
  import Select from '../ui/Select.svelte';
  import type { Repo } from './repo.svelte';
  import type { TodoAction, TodoItem } from './types';
  import { motion } from '../ui/motion';

  let { repo }: { repo: Repo } = $props();

  interface Item {
    sha: string;
    subject: string;
    action: TodoAction;
    message: string;
  }

  const ACTIONS: { value: TodoAction; label: string; hint: string }[] = [
    { value: 'pick', label: 'Garder', hint: 'Le commit est rejoué tel quel' },
    { value: 'reword', label: 'Reformuler', hint: 'Le commit est rejoué avec un nouveau message' },
    { value: 'squash', label: 'Fusionner (messages réunis)', hint: 'Fusionné dans le commit au-dessus ; les deux messages sont réunis' },
    { value: 'fixup', label: 'Fusionner (message ignoré)', hint: 'Fusionné dans le commit au-dessus ; seul le message de celui-ci est gardé' },
    { value: 'drop', label: 'Supprimer', hint: 'Le commit disparaît de la branche' },
  ];

  let element = $state<HTMLDialogElement>();
  let items = $state<Item[]>([]);
  let dragged = $state(-1);

  const plan = $derived(repo.rebasePlan);
  const firstKept = $derived(items.find((i) => i.action !== 'drop'));
  const problem = $derived.by(() => {
    if (!firstKept) return 'Tous les commits seraient supprimés : utilise plutôt un reset.';
    if (firstKept.action === 'squash' || firstKept.action === 'fixup') {
      return 'Le premier commit conservé ne peut pas être fusionné : il n’a pas de commit au-dessus de lui.';
    }
    if (items.some((i) => i.action === 'reword' && !i.message.trim())) return 'Un commit à reformuler a un message vide.';
    return '';
  });
  const changed = $derived(
    !!plan && (items.some((i) => i.action !== 'pick') || items.some((item, n) => item.sha !== plan.commits[n]?.sha)),
  );

  $effect(() => {
    if (!element) return;
    if (plan && !element.open) {
      items = plan.commits.map((c) => ({ sha: c.sha, subject: c.subject, action: 'pick', message: c.message }));
      element.showModal();
      // Les lignes paraissent après l'ouverture : le focus va ensuite à la première liste (charte, 9.10).
      void tick().then(() => element?.querySelector<HTMLElement>('select')?.focus());
    } else if (!plan && element.open) {
      element.close();
    }
  });

  function move(from: number, to: number) {
    if (to < 0 || to >= items.length || from === to) return;
    const next = [...items];
    const [item] = next.splice(from, 1);
    next.splice(to, 0, item);
    items = next;
  }

  const cancel = () => (repo.rebasePlan = null);

  async function run() {
    if (!plan || problem) return;
    const todo: TodoItem[] = items.map((i) => ({
      action: i.action,
      sha: i.sha,
      message: i.action === 'reword' ? i.message : undefined,
    }));
    const base = plan.base;
    repo.rebasePlan = null;
    await repo.rebaseInteractive(base, todo);
  }
</script>

<dialog
  bind:this={element}
  oncancel={(e) => {
    e.preventDefault();
    cancel();
  }}
  use:veil={cancel}
>
  {#if plan}
    <!-- Un formulaire : Entrée lance le rebase (charte, 9.10), sauf dans un message à reformuler. -->
    <form
      class="box"
      onsubmit={(e) => {
        e.preventDefault();
        if (!problem && changed) run();
      }}
    >
      <header>
        <h2>Rebase interactif</h2>
        <p>
          {items.length} commit{items.length > 1 ? 's' : ''} de <b class="mono">{repo.branch ?? 'HEAD'}</b>, du plus ancien (en haut) au plus récent (en bas).
          Glisse une ligne ou utilise les flèches pour changer l’ordre.
        </p>
        {#if plan.pushed}
          <p class="warning">
            <Icon name="warning" />
            <span>Certains de ces commits sont déjà sur le dépôt distant : après le rebase, il faudra pousser en forçant.</span>
          </p>
        {/if}
      </header>

      <ol>
        {#each items as item, n (item.sha)}
          <li
            class="a-{item.action}"
            class:dragging={dragged === n}
            animate:flip={{ duration: motion.med }}
            draggable="true"
            ondragstart={(e) => {
              dragged = n;
              e.dataTransfer?.setData('text/plain', String(n));
            }}
            ondragover={(e) => {
              e.preventDefault();
              if (dragged >= 0 && dragged !== n) {
                move(dragged, n);
                dragged = n;
              }
            }}
            ondragend={() => (dragged = -1)}
          >
            <span class="grip" title="Glisser pour déplacer"><Icon name="grip" /></span>
            <div class="main">
              <div class="line">
                <span class="action">
                  <Select bind:value={item.action} title={ACTIONS.find((a) => a.value === item.action)?.hint} aria-label="Action pour {item.subject}">
                    {#each ACTIONS as action (action.value)}
                      <option value={action.value}>{action.label}</option>
                    {/each}
                  </Select>
                </span>
                <span class="sha mono">{item.sha.slice(0, 7)}</span>
                <span class="subject">{item.subject}</span>
              </div>
              {#if item.action === 'reword'}
                <textarea class="field mono" rows="3" aria-label="Nouveau message pour {item.subject}" bind:value={item.message}></textarea>
              {/if}
            </div>
            <span class="moves">
              <button type="button" aria-label="Monter" title="Monter" disabled={n === 0} onclick={() => move(n, n - 1)}><Icon name="up" size={14} /></button>
              <button type="button" aria-label="Descendre" title="Descendre" disabled={n === items.length - 1} onclick={() => move(n, n + 1)}><Icon name="down" size={14} /></button>
            </span>
          </li>
        {/each}
      </ol>

      <footer>
        <!-- La raison d'un « Lancer le rebase » désactivé, sinon la sauvegarde (charte, 9.1). -->
        <small class:bad={!!problem}>
          {problem ||
            (changed
              ? 'La position actuelle de la branche est sauvegardée dans refs/ramure/backup avant le rebase.'
              : 'Change l’ordre ou l’action d’un commit pour lancer le rebase.')}
        </small>
        <button class="btn" type="button" onclick={cancel}>Annuler</button>
        <button class="btn primary" type="submit" disabled={!!problem || !changed}>Lancer le rebase</button>
      </footer>
    </form>
  {/if}
</dialog>

<style>
  dialog {
    width: min(760px, calc(100vw - 48px));
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
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    width: 100%;
    min-height: 0;
  }
  header {
    padding: 16px 20px 8px;
    display: grid;
    gap: 8px;
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
  .warning {
    display: flex;
    gap: 8px;
    padding: 8px 10px;
    border-radius: var(--radius-s);
    background: var(--warn-soft);
    color: var(--warn);
  }
  .warning span {
    color: var(--ink);
  }
  /* Des lignes simples, séparées par un filet --line (--line-soft ne se voit pas sur --raised) :
     une ligne encadrée dans une boîte serait une carte dans une carte (charte, sections 6 et 9.6). */
  ol {
    list-style: none;
    margin: 0;
    padding: 4px 20px;
    overflow-y: auto;
    display: grid;
    align-content: start;
  }
  li {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 6px 0;
    transition: opacity var(--t-fast) var(--ease);
  }
  li + li {
    border-top: 1px solid var(--line);
  }
  /* La ligne glissée garde sa place à demi effacée ; le système la dessine sous le pointeur. */
  li.dragging {
    opacity: 0.5;
  }
  li.a-drop .subject {
    text-decoration: line-through;
    color: var(--faint);
  }
  /* Un commit fondu dans le précédent est en retrait sous lui. */
  li.a-squash,
  li.a-fixup {
    margin-left: 22px;
  }
  .grip {
    display: grid;
    padding-top: 6px;
    color: var(--faint);
    cursor: grab;
  }
  .main {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 6px;
  }
  .line {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .action {
    width: 210px;
    flex: none;
  }
  .sha {
    color: var(--muted);
    font-size: var(--fs-code);
  }
  .subject {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  textarea {
    resize: vertical;
    font-size: var(--fs-code);
  }
  .moves {
    display: flex;
    gap: 2px;
    padding-top: 3px;
  }
  .moves button {
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
  .moves button:hover:not(:disabled) {
    background: var(--hover);
    color: var(--ink);
  }
  .moves button:active:not(:disabled) {
    transform: scale(0.97);
  }
  .moves button:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 20px 16px;
  }
  footer small {
    flex: 1;
    color: var(--muted);
    font-size: var(--fs-small);
  }
  footer small.bad {
    color: var(--bad);
  }
</style>
