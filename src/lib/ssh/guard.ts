// Avant d'arrêter des terminaux locaux : y tourne-t-il un programme ? Fermer un onglet ou la
// fenêtre arrête les shells, et avec eux un serveur de développement ou une longue compilation
// (charte, sections 9.15 et 9.20).

import { invoke } from '@tauri-apps/api/core';
import { dialogs } from '../ui/dialogs.svelte';
import type { Session } from './ssh.svelte';

/**
 * Ce qui tourne dans `sessions`. `programs` : les terminaux où un programme tourne, autre que le
 * shell. `unknown` : ceux dont le shell tourne sans qu'on puisse en savoir plus (sous Windows).
 */
async function running(sessions: Session[]) {
  const open = sessions.filter((s) => s.kind === 'local' && s.localId !== undefined);
  if (!open.length) return { programs: 0, unknown: 0 };
  try {
    const busy = await invoke<(boolean | null)[]>('local_busy', { ids: open.map((s) => s.localId) });
    return { programs: busy.filter((b) => b === true).length, unknown: busy.filter((b) => b === null).length };
  } catch {
    // Le cœur ne répond pas : on ne sait pas. Comme sous Windows, le dépôt et la fenêtre demandent
    // alors, et l'onglet d'un seul terminal, qu'on a sous les yeux, se ferme sans question.
    return { programs: 0, unknown: open.length };
  }
}

const LOST = 'et sa sortie perdue';

/** Fermer un onglet de terminal local : on ne demande que si l'on sait qu'un programme y tourne. */
export async function mayCloseTerminal(session: Session): Promise<boolean> {
  const { programs } = await running([session]);
  if (!programs) return true;
  return dialogs.confirm({
    title: 'Fermer ce terminal ?',
    body: `Un programme tourne dans ce terminal. Il sera arrêté ${LOST}.`,
    confirmLabel: 'Fermer le terminal',
    danger: true,
  });
}

/**
 * Fermer l'onglet d'un dépôt arrête ses terminaux, qu'on ne voit pas toujours. On demande quand un
 * programme y tourne, et aussi quand on ne peut pas le savoir et qu'un shell est ouvert.
 */
export async function mayCloseRepo(name: string, sessions: Session[]): Promise<boolean> {
  const { programs, unknown } = await running(sessions);
  if (!programs && !unknown) return true;
  return dialogs.confirm({
    title: `Fermer le dépôt ${name} ?`,
    body: programs
      ? programs === 1
        ? `Un programme tourne dans le terminal de ce dépôt. Il sera arrêté ${LOST}.`
        : `Des programmes tournent dans ${programs} terminaux de ce dépôt. Ils seront arrêtés et leur sortie perdue.`
      : unknown === 1
        ? `Un terminal est ouvert dans ce dépôt. Ce qui y tourne sera arrêté, ${LOST}.`
        : `${unknown} terminaux sont ouverts dans ce dépôt. Ce qui y tourne sera arrêté, ${LOST}.`,
    confirmLabel: 'Fermer le dépôt',
    danger: true,
  });
}

/** Quitter l'application arrête tous les terminaux locaux, ceux des dépôts et ceux du module SSH. */
export async function mayQuit(sessions: Session[]): Promise<boolean> {
  const { programs, unknown } = await running(sessions);
  if (!programs && !unknown) return true;
  const body = programs
    ? programs === 1
      ? `Un programme tourne dans un terminal. Il sera arrêté ${LOST}.`
      : `Des programmes tournent dans ${programs} terminaux. Ils seront arrêtés et leur sortie perdue.`
    : unknown === 1
      ? `Un terminal est ouvert. Ce qui y tourne sera arrêté, ${LOST}.`
      : `${unknown} terminaux sont ouverts. Ce qui y tourne sera arrêté, ${LOST}.`;
  return dialogs.confirm({ title: 'Quitter Ramure ?', body, confirmLabel: 'Quitter', danger: true });
}
