# Ramure

Client Git graphique et client SSH/SFTP dans la même fenêtre, pour Windows et Ubuntu.
Libre (MIT), local, sans compte.

**État : jalons J1 et J2 presque complets.** Le prototype J0 a validé le graphe et le terminal
SSH sur Windows et Ubuntu. Le Git du quotidien (J1) et le Git avancé (J2) sont là, à deux
fonctions près, listées plus bas.

## Ce que fait Ramure aujourd'hui

### Dépôts

- **Onglets** : un dépôt par onglet, rouverts au démarrage. Ctrl + T ouvre un onglet,
  Ctrl + W le ferme, Ctrl + Tab passe au suivant ; un clic du milieu ferme aussi.
- **Page d'accueil** d'un onglet neuf : ouvrir un dossier, cloner, initialiser, et la liste
  des dépôts récents.
- **Cloner** (`git clone --progress`) : progression en direct, interruption possible. Un clonage
  qui échoue ne laisse pas de dossier derrière lui.

### Graphe

- **Graphe** : `git log`, lanes calculées en Rust, dessin sur un canvas. Seules les lignes
  visibles existent dans la page ; 50 000 commits défilent sans saccade.
- **Deux rendus**, au choix dans les réglages. « Ruban » (par défaut) : une pastille aux
  initiales de l'auteur sur chaque commit, des tracés arrondis façon plan de métro, une bande
  de la couleur de la branche qui relie le commit à son message, un halo animé sur le commit
  courant. « Classique » : points et courbes, plus compact.
- **Colonne « Branche / Tag »** à gauche du graphe, reliée au commit par un trait. Une icône
  ordinateur marque le dernier commit local d'une branche, un nuage le dernier commit distant ;
  au même commit, ils partagent une étiquette. Les étiquettes en surnombre se déplient au survol.
- **Barre latérale** : branches locales avec leur avance et leur retard, branches distantes,
  tags, remises, et un filtre.
- **Détail d'un commit** : message complet, fichiers modifiés, diff de chaque fichier.
- **Diff** unifié ou côte à côte ; côte à côte, la partie modifiée d'une ligne est surlignée.
- **Palette de commandes** (Ctrl + K, ou la loupe en bas à gauche) : une recherche unique sur
  les actions, les branches, les tags, les onglets et les dépôts récents.

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
  Abandonner. Un clic sur un fichier en conflit ouvre l'**éditeur à trois voies** : les deux
  versions en haut, avec le passage en conflit surligné, et le résultat en bas. Pour chaque
  conflit, prends un côté ou les deux ; le résultat reste modifiable à la main, et l'ancêtre
  commun s'affiche à la demande. Un fichier supprimé d'un côté, ou binaire, se résout en
  gardant un côté.
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

- Du jalon J1 : raccourcis clavier configurables.
- Du jalon J2 : fusion ou rebase en glissant une branche sur une autre.
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
