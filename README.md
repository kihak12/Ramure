# Ramure

Client Git graphique et client SSH/SFTP dans la même fenêtre, pour Windows et Ubuntu.
Libre (MIT), local, sans compte.

**État : jalon J2 en cours.** Le prototype J0 a validé le graphe et le terminal SSH sur
Windows et Ubuntu. Le Git du quotidien (J1) et l'essentiel du Git avancé (J2) sont là ; ce qui
manque encore est listé plus bas.

## Ce que fait Ramure aujourd'hui

### Dépôt et graphe

- **Graphe** : `git log`, lanes calculées en Rust, dessin sur un canvas. Seules les lignes
  visibles existent dans la page ; 50 000 commits défilent sans saccade.
- **Colonne « Branche / Tag »** à gauche du graphe, reliée au commit par un trait. Une icône
  ordinateur marque le dernier commit local d'une branche, un nuage le dernier commit distant ;
  au même commit, ils partagent une étiquette. Les étiquettes en surnombre se déplient au survol.
- **Barre latérale** : branches locales avec leur avance et leur retard, branches distantes,
  tags, remises, et un filtre.
- **Détail d'un commit** : message complet, fichiers modifiés, diff de chaque fichier.

### Travail en cours

- **Index** : la ligne « WIP » ouvre le panneau des modifications. Indexer et retirer par
  fichier ou en bloc, abandonner après confirmation.
- **Indexation par bloc ou par ligne** : dans le diff d'un fichier modifié, clique les lignes
  à indexer (Maj + clic pour une plage), ou indexe un bloc entier.
- **Commit** : message, Ctrl + Entrée, et « Modifier le dernier commit » (amend).
- **Remises** : remiser avec les fichiers non suivis, dépiler, appliquer, supprimer, voir le contenu.

### Branches, tags et historique

Tout passe par le clic droit sur un commit ou sur une étiquette, et par la barre d'outils.

- **Branches** : basculer par double-clic, créer depuis un commit, renommer, supprimer en local
  et sur le dépôt distant, suivre une branche distante.
- **Tags** : léger ou annoté, pousser, supprimer en local et sur le dépôt distant.
- **Fusion, rebase, cherry-pick, revert, reset** (soft, mixed, hard).
- **Conflits** : quand une opération s'arrête, un bandeau propose Continuer, Passer et
  Abandonner. Chaque fichier en conflit se résout en gardant un côté, ou en le corrigeant dans
  ton éditeur avant de le marquer résolu.
- **Rebase interactif** : réordonner à la souris, reformuler, fusionner, supprimer des commits.
  git relance Ramure comme « éditeur de séquence » : aucun outil extérieur n'est nécessaire.
- **Garde-fous** : avant un reset, un rebase ou un rebase interactif, la position de la branche
  est gardée dans `refs/ramure/backup/<branche>-<date>` (`git reset --hard <cette référence>`
  y revient). Réécrire des commits déjà poussés affiche un avertissement.

### Synchronisation

- **Récupérer** (`git fetch --all --prune`), **Tirer** (avance rapide par défaut ; rebase ou
  fusion au réglage), **Pousser** (publication automatique d'une branche sans amont), et
  **push forcé** (`--force-with-lease`) après confirmation.
- **Fetch automatique** toutes les 5 minutes, réglable de 1 à 60 ou désactivable.

L'authentification des dépôts distants passe par ce que git utilise déjà : assistant
d'identifiants (Git Credential Manager) ou agent SSH. Ramure ne demande pas lui-même de mot de
passe ou de phrase secrète.

### Terminal SSH et journal

- **Terminal SSH** : connexion via russh, vérification de l'empreinte contre `known_hosts`,
  authentification par agent SSH, fichier de clé ou mot de passe, commande au démarrage.
- **Journal des commandes** : chaque commande git lancée, avec sa durée, depuis la barre d'état.

### Pas encore là

- Du jalon J1 : onglets par dépôt et dépôts récents, cloner et initialiser, palette de
  commandes (Ctrl K), diff côte à côte.
- Du jalon J2 : éditeur de conflits à trois voies, fusion ou rebase en glissant une branche
  sur une autre.
- Jalons J3 et J4 : carnet d'hôtes, clés, trousseau, terminal local, SFTP, tunnels,
  installeurs et mises à jour.

## Prérequis

- Git 2.23 ou plus récent
- Node.js 20 ou plus récent
- Rust stable (https://rustup.rs)

Windows : les « Build Tools for Visual Studio » (charge de travail C++) et WebView2, déjà
présent sur Windows 10 et 11 à jour.

Ubuntu 22.04 et 24.04 :

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

## Lancer

```bash
npm install
```

```bash
npm run tauri dev
```

Version optimisée, sans installeur (binaire dans `src-tauri/target/release/`) :

```bash
npm run tauri build -- --no-bundle
```

## Vérifier

```bash
npm run check
```

```bash
cd src-tauri && cargo test
```

Mesurer le chargement d'un vrai dépôt, sans ouvrir l'interface :

```bash
cd src-tauri && RAMURE_TEST_REPO=/chemin/du/depot cargo test real_repo -- --nocapture
```

## Organisation

| Dossier | Contenu |
| --- | --- |
| `src/` | Interface Svelte 5 : `lib/git/GraphView.svelte`, `lib/ssh/SshView.svelte` |
| `src-tauri/src/git/` | Exécution de git, parseur du log, calcul des lanes |
| `src-tauri/src/ssh/` | Sessions russh, lecture de `~/.ssh/config` |

L'interface ne touche ni au disque ni au réseau : elle appelle les commandes du cœur Rust.

## Limites connues du prototype

- Un serveur inconnu peut être accepté pour la session, mais son empreinte n'est pas encore
  écrite dans `known_hosts`.
- Les hôtes qui passent par un bastion (`ProxyJump`) ne sont pas pris en charge.
- Une seule session SSH à la fois.
- La colonne du graphe est limitée à une vingtaine de lanes.
