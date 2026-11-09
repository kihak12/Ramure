# Ramure

Client Git graphique et client SSH/SFTP dans la même fenêtre, pour Windows et Ubuntu.
Libre (MIT), local, sans compte.

**État : prototype J0.** Il sert à valider deux choses sur les deux systèmes : le graphe reste
fluide sur un gros historique, et le terminal SSH répond sans latence.

## Ce que fait le prototype

- **Graphe** : ouvre un dépôt, lance `git log`, calcule les lanes en Rust et dessine le graphe
  sur un canvas. Seules les lignes visibles existent dans la page. La barre d'outils affiche les
  temps (git, lanes, affichage) et la fluidité du dernier défilement.
- **Terminal SSH** : connexion via russh, vérification de l'empreinte contre `known_hosts`,
  authentification par agent SSH, fichier de clé ou mot de passe, commande au démarrage,
  terminal xterm.js. L'en-tête affiche le délai d'écho.
- **Journal des commandes** : chaque commande git lancée, avec sa durée.

Pas encore là : index et commit, branches, stash, SFTP, tunnels, carnet d'hôtes. Voir la
feuille de route dans la spécification.

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
