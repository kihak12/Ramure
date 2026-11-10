// Textes du panneau « Nettoyer les branches ».

/** Résumé de la sélection, à côté de la liste. */
export function summary(count: number): string {
  if (count === 0) return 'Aucune branche sélectionnée.';
  return `${count} branches sélectionnées.`;
}

/** Texte de la confirmation, avant la suppression. */
export function confirmBody(count: number): string {
  return `Supprimer ${count} branche${count > 1 ? 's' : ''} ?`;
}

/** Message affiché quand une suppression échoue. */
export function failure(): string {
  return 'Oups, la suppression a échoué !';
}
