# Direction visuelle de Ramure

Trois directions à comparer, avant d'écrire la charte graphique. Chacune est un parti pris
complet : police, onglets, barre d'outils, boutons, panneaux, dessin du graphe et couleurs.
Elles sont montrées sur l'application réelle, avec l'interface actuelle comme repère.

**Décision : à prendre.** Une fois la direction choisie (ou un mélange précis), elle est inscrite
ici et sert de base à la charte.

## En un coup d'œil

| | Actuelle | A. Sous-bois | B. Encre | C. Braise |
|---|---|---|---|---|
| Idée | point de départ | organique | technique | chaleureuse |
| Police | celle du système | DM Sans | Inter | Rubik |
| Onglets | cadre attaché | pilules | texte souligné | intercalaires |
| Barre d'outils | icône sur libellé | pilules, icône et libellé côte à côte | une ligne compacte | groupes pleins |
| Boutons | angles de 6 px | pilules | angles de 4 px | angles de 10 px, bord inférieur épais |
| Panneaux | séparés par des traits | cartes flottantes | séparés par des traits | séparés par des traits |
| Commit dans le graphe | pastille à initiales | pastille à initiales | point | grande pastille à initiales |
| Tracé des branches | angles arrondis | courbes souples | angles nets, trait fin | angles très arrondis, trait épais |
| Lien commit-message | ruban qui s'estompe | capsule autour du message | aucun | ligne entière colorée |
| Hauteur d'une ligne | 32 px | 34 px | 26 px | 36 px |
| Commits visibles sur un écran de 880 px | 23 | 21 | 29 | 20 |
| Accent | bleu | vert mousse | indigo | ambre |

Ce qui ne change pas d'une direction à l'autre : l'organisation de l'écran, les icônes, les
fonctions. Le mouvement (durées, courbes d'animation) n'est pas montré par des captures ; il sera
réglé dans la charte.

## Repère : l'interface actuelle

| Sombre | Clair |
|---|---|
| ![Interface actuelle, thème sombre](directions/actuelle-graphe-sombre.png) | ![Interface actuelle, thème clair](directions/actuelle-graphe-clair.png) |

## A. Sous-bois

**Intention.** Tirer l'identité du nom : une ramure, ce sont des branches. Tout est arrondi et
souple, les panneaux flottent sur le fond comme des cartes, et chaque message de commit est une
capsule de la couleur de sa branche, accrochée à une courbe.

| Élément | Parti pris |
|---|---|
| Police | DM Sans, 13,5 px ; DM Mono pour le code. Titres de section en minuscules, sans capitales espacées. |
| Onglets | Des pilules posées sur le fond ; l'onglet actif est une pilule teintée de l'accent. |
| Barre d'outils | Chaque action est une pilule bordée, icône et libellé côte à côte. |
| Boutons et champs | Boutons en pilule ; champs à angles de 10 px ; boîtes de dialogue à angles de 20 px. |
| Panneaux | Barre latérale, graphe et détail sont trois cartes arrondies, séparées par le fond. |
| Graphe | Pastilles à initiales, courbes souples de 3 px, message dans une capsule. Lignes de 34 px. |
| Étiquettes | Pilules teintées ; la branche courante en pilule pleine. |
| Couleurs | Fonds vert très sombre ou papier chaud, accent vert mousse, branches en teintes de nature. |

**Ce qu'elle apporte.** La plus reconnaissable : aucun autre client Git ne ressemble à cela. Le
lien entre un commit et son message est le plus lisible des trois.

**Ce qu'elle coûte.** Les cartes et les capsules prennent de la place : moins de commits à
l'écran, et les marges entre cartes retirent quelques pixels à chaque panneau. L'accent vert est
aussi la couleur du succès et des lignes ajoutées dans un diff.

| Graphe, sombre | Graphe, clair |
|---|---|
| ![Sous-bois, graphe, thème sombre](directions/sous-bois-graphe-sombre.png) | ![Sous-bois, graphe, thème clair](directions/sous-bois-graphe-clair.png) |

| Modifications et diff | Boîte de dialogue |
|---|---|
| ![Sous-bois, modifications et diff](directions/sous-bois-travail-sombre.png) | ![Sous-bois, boîte de dialogue](directions/sous-bois-dialogue-clair.png) |

## B. Encre

**Intention.** Un outil d'abord. Tout est plat, dense et net, dans des gris presque sans teinte :
les seules couleurs vives de l'écran sont celles des branches.

| Élément | Parti pris |
|---|---|
| Police | Inter, 12,5 px ; JetBrains Mono pour le code. Titres de section en petites capitales espacées. |
| Onglets | Du texte, et un trait de 2 px sous l'onglet actif. Pas de cadre. |
| Barre d'outils | Une seule ligne compacte, icône et libellé côte à côte, sans fond. |
| Boutons et champs | Angles de 4 px, sans ombre ; boîtes de dialogue à angles de 8 px. |
| Panneaux | Bord à bord, séparés par des traits fins. |
| Graphe | Points de 9 px, traits de 2 px à angles nets, aucun ruban. L'avatar de l'auteur revient dans sa colonne. Lignes de 26 px. |
| Étiquettes | Rectangles en police de code, avec une barre de couleur à gauche ; la branche courante est pleine. |
| Couleurs | Gris neutres, accent indigo, branches en couleurs vives. |

**Ce qu'elle apporte.** La plus dense : un quart de commits en plus à l'écran. L'indigo n'est la
couleur d'aucun état (succès, avertissement, erreur), donc l'accent désigne toujours une action.
La plus proche des habitudes prises avec GitKraken.

**Ce qu'elle coûte.** La moins singulière des trois, et celle qui a le moins d'impact visuel : le
graphe est précis, mais discret.

| Graphe, sombre | Graphe, clair |
|---|---|
| ![Encre, graphe, thème sombre](directions/encre-graphe-sombre.png) | ![Encre, graphe, thème clair](directions/encre-graphe-clair.png) |

| Modifications et diff | Boîte de dialogue |
|---|---|
| ![Encre, modifications et diff](directions/encre-travail-sombre.png) | ![Encre, boîte de dialogue](directions/encre-dialogue-clair.png) |

## C. Braise

**Intention.** Une interface chaude et tactile : formes généreuses, groupes pleins, gros traits.
L'ambre évoque le bois sans reprendre le vert attendu.

| Élément | Parti pris |
|---|---|
| Police | Rubik, 13,5 px ; Fira Code pour le code. Titres de section en capitales, de la couleur de l'accent. |
| Onglets | Des intercalaires de classeur, larges, à coins de 12 px. |
| Barre d'outils | Les actions sont regroupées dans des blocs pleins et arrondis ; icônes de la couleur de l'accent. |
| Boutons et champs | Angles de 10 px, bord inférieur épais qui donne du relief ; boîtes de dialogue à angles de 18 px. |
| Panneaux | Bord à bord, séparés par des traits. |
| Graphe | Grandes pastilles à initiales, traits de 3,5 px à angles très arrondis, ligne entière teintée de la couleur de la branche. Messages en caractères demi-gras. Lignes de 36 px. |
| Étiquettes | Pilules pleines, plus hautes. |
| Couleurs | Fonds charbon ou crème, accent ambre. |

**Ce qu'elle apporte.** Le graphe le plus spectaculaire : chaque branche se lit d'un bord à
l'autre de l'écran. Chaleureuse, confortable en thème sombre.

**Ce qu'elle coûte.** La moins dense. L'ambre est voisin de la couleur d'avertissement : un
bouton principal et une mise en garde se ressemblent. Sur un dépôt à nombreuses branches, les
lignes teintées peuvent charger l'écran.

| Graphe, sombre | Graphe, clair |
|---|---|
| ![Braise, graphe, thème sombre](directions/braise-graphe-sombre.png) | ![Braise, graphe, thème clair](directions/braise-graphe-clair.png) |

| Modifications et diff | Boîte de dialogue |
|---|---|
| ![Braise, modifications et diff](directions/braise-travail-sombre.png) | ![Braise, boîte de dialogue](directions/braise-dialogue-clair.png) |

## Recommandation

**Sous-bois**, pour la structure et les formes. C'est la seule direction qui donne à Ramure un
visage à lui, et la capsule est la meilleure réponse au défaut de départ : le lien entre un
commit et son message.

Deux réserves, qui peuvent se régler par un mélange :

- **La densité.** Si 21 commits à l'écran sont trop peu, on peut garder les formes de Sous-bois
  avec la hauteur de ligne d'Encre.
- **L'accent vert.** S'il gêne à côté du vert des succès, l'indigo d'Encre fonctionne sur les
  fonds de Sous-bois.

Encre reste le bon choix si la priorité est l'efficacité plutôt que l'identité.

## Fichiers

- [`directions.css`](directions.css) : les couleurs et les rayons de chaque direction.
- [`esquisses/`](esquisses) : les règles d'interface qui ont servi aux captures (police, onglets,
  barre d'outils, panneaux, étiquettes). Ce sont des esquisses, pas du code à reprendre tel quel.
- Dessin du graphe, par direction :

| | Sous-bois | Encre | Braise |
|---|---|---|---|
| Hauteur de ligne | 34 px | 26 px | 36 px |
| Largeur d'une colonne | 24 px | 14 px | 26 px |
| Rayon d'un commit | 10 px | 4,5 px | 11,5 px |
| Épaisseur du trait | 3 px | 2 px | 3,5 px |
| Changement de colonne | courbe | angle de 5 px | angle de 14 px |
| Bande derrière le commit | 24 px, opacité 15 % | aucune | 26 px, opacité 20 % |

## Notes

- Toutes les couleurs respectent les contrastes WCAG : 7:1 pour le texte courant, 4,5:1 pour le
  texte secondaire et le texte posé sur une couleur, 3:1 pour les traits du graphe.
- Les six polices sont libres (SIL Open Font License) et seraient embarquées dans l'application.
- Les captures viennent de l'interface de la PR #2, sous Ubuntu.
