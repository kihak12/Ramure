# Ramure

Client Git graphique et client SSH/SFTP dans la même fenêtre, pour Windows et Ubuntu.
Libre (MIT), local, sans compte.

**État : jalon J1.** Le prototype J0 a validé le graphe et le terminal SSH sur Windows et
Ubuntu ; J1 ajoute le travail quotidien sur un dépôt : indexer, commiter, synchroniser.

## Ce que fait Ramure aujourd'hui

- **Graphe** : ouvre un dépôt, lance `git log`, calcule les lanes en Rust et dessine le graphe
  sur un canvas. Seules les lignes visibles existent dans la page. La barre d'état affiche les
  temps (git, lanes, affichage) et la fluidité du dernier défilement.
- **Modifications en cours** : la ligne « WIP » en tête du graphe ouvre le panneau d'index.
  Fichiers non indexés et indexés, indexation et retrait fichier par fichier ou en bloc,
  abandon des modifications (avec confirmation), message et commit (Ctrl + Entrée).
- **Diff** : un clic sur un fichier affiche son diff, pour le répertoire de travail, l'index
  ou n'importe quel commit de l'historique.
- **Détail d'un commit** : message complet et fichiers modifiés.
- **Synchronisation** : Récupérer (`git fetch --all`), Tirer (`git pull`, selon ta configuration
  git), Pousser (`git push`, ou publication de la branche si elle n'a pas encore d'amont), avec
  le nombre de commits en avance et en retard.
- **Terminal SSH** : connexion via russh, vérification de l'empreinte contre `known_hosts`,
  authentification par agent SSH, fichier de clé ou mot de passe, commande au démarrage,
  terminal xterm.js. L'en-tête affiche le délai d'écho.
- **Journal des commandes** : chaque commande git lancée, avec sa durée.

L'authentification des dépôts distants passe par ce que git utilise déjà : assistant
d'identifiants (Git Credential Manager) ou agent SSH. Ramure ne demande pas encore lui-même
de mot de passe ou de phrase secrète.

Pas encore là : indexation par bloc ou par ligne, modification du dernier commit, branches,
stash, cherry-pick, résolution de conflits, SFTP, tunnels, carnet d'hôtes. Voir la feuille de
route dans la spécification.

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
