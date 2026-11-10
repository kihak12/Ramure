# Ramure

Client Git graphique et client SSH dans la même fenêtre, pour Windows et Ubuntu.
Tauri 2, Rust, Svelte 5.

## Vérifier

- `npm run check` : types, puis jetons de design (aucune valeur de style en dur dans un composant).
- `cd src-tauri && cargo test`.

## Design

- `docs/charte-graphique.md` est la référence. Quand le code et la charte divergent, la charte a
  raison ; une règle change d'abord dans la charte.
- Dès qu'un composant, un style, un jeton, une icône ou un texte d'interface change, fais relire
  le changement par le subagent `designer` avant de le proposer. Donne-lui la liste des fichiers
  modifiés : il est en lecture seule et ne peut pas lancer `git`. Corrige les violations qu'il
  relève, et tranche ses dérives avec l'auteur de la demande.

## Conventions

- L'interface, les commentaires, les commits et les PR sont en français.
- Une issue simple, puis une PR dédiée.
