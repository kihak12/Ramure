# Logo de Ramure

Trois pistes à comparer, sur le thème des bois de cerf.

**Décision : à prendre.** Une fois la piste choisie, elle est inscrite ici, puis viennent le logo
définitif, l'icône de l'application et la section « Logo » de la charte (issue #13).

## L'idée commune

« Ramure » désigne à la fois les bois du cerf et les branches d'un arbre. Les trois pistes
dessinent donc les bois comme des branches de Git, avec le vocabulaire de l'application :

- un tracé aux extrémités arrondies, comme les icônes ;
- des courbes souples, comme le graphe ;
- une pastille pour un commit ;
- une seule couleur, sans dégradé.

Chaque piste a deux dessins : le dessin complet, à partir de 48 px, et un dessin simplifié pour
les petites tailles (un andouiller de moins par côté, un trait plus épais).

Les dessins sont originaux, faits pour ce projet.

## En un coup d'œil

![Les trois pistes côte à côte](logo/ensemble.png)

| | A. Les bois | B. Cerf au trait | C. Cerf plein |
|---|---|---|---|
| Ce qu'on voit | Une paire de bois qui part d'un commit. | Une tête de cerf d'un seul trait : le museau est le premier commit, chaque côté de la tête une branche qui devient un bois. | Une tête de cerf pleine, de face. |
| Lien avec Git | Le plus fort : c'est un graphe. | Fort : deux branches partent d'un commit. | Faible : seules les pastilles au bout des bois le rappellent. |
| Lien avec le cerf | Par les bois seuls. | Net : museau, oreilles, bois. | Immédiat. |
| À 16 px | Un « Y » à deux pointes. Lisible, mais peut passer pour une plante. | Un « V » ramifié. | La silhouette la plus reconnaissable. |
| Singularité | Forte. | Forte. | Plus faible : la tête de cerf de face est un motif répandu. |
| Accord avec l'interface | Le même tracé que les icônes. | Le même tracé que les icônes. | Une forme pleine, là où les icônes sont au trait. |

## A. Les bois

Une paire de bois, et rien d'autre. Le tronc part d'un commit, se sépare en deux branches qui
s'écartent et remontent, et chaque branche porte ses andouillers. C'est un graphe de Git qui a la
forme d'une ramure.

**Ce qu'elle apporte.** La piste la plus abstraite, et celle qui dit le mieux « branches ». Elle
se réduit bien : il reste un « Y » net.

**Ce qu'elle coûte.** Sans tête, le cerf se devine plus qu'il ne se voit. En petit, la forme peut
évoquer une plante ou une fourche.

![Piste A : symbole, icône et nom](logo/piste-a.png)

Les petites tailles (32, 24 et 16 px), agrandies six fois :

![Piste A, petites tailles agrandies](logo/piste-a-petit.png)

## B. Cerf au trait

La tête et les bois d'un seul tracé. Tout part du museau, qui est une pastille de commit : deux
traits remontent le long de la tête, s'écartent et deviennent les bois. Deux traits courts font
les oreilles.

**Ce qu'elle apporte.** On voit un cerf, et c'est encore un graphe : un commit, deux branches. Le
dessin est fait du même trait que les icônes de l'application.

**Ce qu'elle coûte.** C'est le dessin qui a le plus de traits. À 16 px, les oreilles se
confondent avec les bois : il reste un « V » ramifié.

![Piste B : symbole, icône et nom](logo/piste-b.png)

Les petites tailles, agrandies six fois :

![Piste B, petites tailles agrandies](logo/piste-b-petit.png)

## C. Cerf plein

Une tête de cerf de face, en forme pleine, surmontée de bois au trait. Les bois gardent une
pastille à leur pointe.

**Ce qu'elle apporte.** La plus lisible : on reconnaît un cerf à toutes les tailles, y compris à
16 px.

**Ce qu'elle coûte.** La moins singulière. Une tête de cerf de face est un motif courant (chasse,
plein air, boissons), et le lien avec Git est ténu. La forme pleine tranche avec les icônes au
trait de l'interface.

![Piste C : symbole, icône et nom](logo/piste-c.png)

Les petites tailles, agrandies six fois :

![Piste C, petites tailles agrandies](logo/piste-c-petit.png)

## Recommandation

**B. Cerf au trait.** C'est la seule piste qui dit les deux choses à la fois : on voit un cerf,
et le dessin reste un commit d'où partent deux branches. Elle est faite du même trait que le
reste de l'interface.

A est le bon choix si tu préfères un signe plus abstrait. C est le bon choix si la lisibilité à
16 px passe avant tout.

## Ce qui se décide après

Ces points ne dépendent pas de la piste. Ils sont montrés dans chaque planche pour pouvoir en
juger :

- **La tuile de l'icône.** Bleue avec le symbole en blanc, ou sombre avec le symbole en bleu.
- **Le nom.** « Ramure » en DM Sans, graisse 650, à droite du symbole.
- **Les retouches.** Nombre d'andouillers, épaisseur du trait, pastilles aux pointes.

## Fichiers

- [`logo/piste-a.svg`](logo/piste-a.svg), [`logo/piste-b.svg`](logo/piste-b.svg),
  [`logo/piste-c.svg`](logo/piste-c.svg) : les dessins complets.
- [`logo/piste-a-petit.svg`](logo/piste-a-petit.svg),
  [`logo/piste-b-petit.svg`](logo/piste-b-petit.svg),
  [`logo/piste-c-petit.svg`](logo/piste-c-petit.svg) : les dessins des petites tailles.

Les SVG prennent la couleur du texte qui les entoure (`currentColor`).
