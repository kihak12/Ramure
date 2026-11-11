# Visual direction of Ramure

This document keeps the record of how Ramure's visual direction was chosen. Everything is shown
on the real application.

**Decision: D. Clairière, chosen on 10 November 2026,** with the Ardoise colour theme as the
default. It is the basis of the design guidelines (#4).

## Where things stand

1. Three complete directions were compared: A. Sous-bois, B. Encre, C. Braise. They are kept
   [further down](#first-directions).
2. First feedback: the cards of Sous-bois are liked, but not its buttons, nor its green, nor the
   border around the commit card.
3. **D. Clairière** keeps what was liked and fixes the rest. It adds a choice of
   [colour themes](#colour-themes).
4. Clairière is chosen, with one last adjustment: in the commit list, branch and tag labels are
   slightly rounded rectangles, like buttons, and do not shrink on click.

## D. Clairière, the chosen direction

**Intent.** A "clairière" is a clearing: the undergrowth ("sous-bois") opened up. The cards and
the graph of Sous-bois are kept, the borders are removed, the buttons of the current interface
come back, and the default colours go back to neutral.

### Feedback taken into account

| Feedback | Answer |
|---|---|
| The card layout of Sous-bois is liked. | Kept: the sidebar, the graph and the detail are three rounded cards, separated by the window background. |
| The buttons of the current interface are preferred. | Buttons, fields, dialogs and the toolbar are those of the current interface: 6 px corners, icon above the label. No pill-shaped button any more. |
| The colours lean too green. | The default theme, Ardoise, is made of neutral greys, with the current blue accent. Green only says "success" and "added line". |
| The border of the commit card is in the way. | No card has a border. The coloured frame that surrounded the whole list when it was browsed from the keyboard is removed: the marker is carried by the selected commit. The selection is a rounded background, inset from the edges of the card. |
| Being able to change the colours later. | See [Colour themes](#colour-themes). |
| In the commit list, labels should have the shape of the other buttons, and not shrink on click. | Branch and tag labels are rectangles with 6 px corners. No pill, no shrinking on click. |

### Choices

| Element | Choice | Comes from |
|---|---|---|
| Font | DM Sans, 13.5 px; DM Mono for code. Section titles in lower case. | Sous-bois |
| Tabs | The active tab is a small card, made of the same material as the panels. No frame. | new |
| Toolbar | Icon above the label, sitting on the window background. | current interface |
| Buttons and fields | 6 px corners, as today. Dialogs unchanged. | current interface |
| Panels | Three cards with 14 px corners, no border, 8 px apart. | Sous-bois, without the border |
| Graph | Dots with initials, soft 3 px curves, the message in a capsule. 34 px rows. | Sous-bois |
| Selection | A rounded background under the row, inset by 6 px. No bar on the edge. | new |
| Labels | Rectangles with 6 px corners, tinted with the branch colour; the current branch is solid. No shrinking on click. | current interface for the shape, Sous-bois for the tints |
| Colours | Ardoise theme: neutral greys, blue accent. | new |

| Graph, dark | Graph, light |
|---|---|
| ![Clairière, graph, dark theme](directions/clairiere-graphe-sombre.png) | ![Clairière, graph, light theme](directions/clairiere-graphe-clair.png) |

| Changes and diff | Dialog |
|---|---|
| ![Clairière, changes and diff](directions/clairiere-travail-sombre.png) | ![Clairière, dialog](directions/clairiere-dialogue-clair.png) |

### Approved with the proposal

These points came from Sous-bois or were new. They are kept as shown:

- **The font.** DM Sans, and DM Mono for code.
- **The graph.** Soft curves and messages in capsules.
- **Tabs as cards.**
- **Density.** 21 commits visible on an 880 px screen, against 23 in the starting interface.

## Colour themes

A colour theme only changes colours: backgrounds, text, lines, accent, state colours, branch
colours and terminal colours. The layout, the shapes, the font and the sizes do not move. Each
theme exists in light and in dark.

| Theme | Backgrounds | Accent |
|---|---|---|
| **Ardoise** (slate, the default) | neutral greys | blue |
| Encre (ink) | near-black greys, or whites | indigo |
| Sous-bois (undergrowth) | very dark greens, or warm paper | moss green |
| Braise (embers) | charcoal, or cream | amber |

Ardoise is shown in the Clairière screenshots above. The other three, on the same screen:

| | Dark | Light |
|---|---|---|
| Encre | ![Encre theme, dark](directions/theme-encre-sombre.png) | ![Encre theme, light](directions/theme-encre-clair.png) |
| Sous-bois | ![Sous-bois theme, dark](directions/theme-sous-bois-sombre.png) | ![Sous-bois theme, light](directions/theme-sous-bois-clair.png) |
| Braise | ![Braise theme, dark](directions/theme-braise-sombre.png) | ![Braise theme, light](directions/theme-braise-clair.png) |

**How it would work.**

- A "Thème de couleurs" setting in the settings. The change is immediate and remembered.
- A theme is a block of colour tokens, selected by an attribute on the root of the page. The
  four themes are now in [`src/app.css`](../../src/app.css), selected by `data-palette`. This is possible because the components already go through those tokens.
- A theme only enters the application if it meets the contrasts set by the guidelines. The four
  proposed themes do.

**Worth knowing.** In the Sous-bois theme, the accent is also the colour of success; in Braise,
it is close to the warning colour. These are optional themes: Ardoise, the default, does not have
that flaw.

The implementation is tracked in issue #9. It comes after the alignment of the tokens (#5).

## What comes next

- The [design guidelines](../design-guidelines.md) set the rules of Clairière: issue #4.
- The tokens in the code are aligned with the guidelines: issue #5.
- The direction is applied to the interface: issue #10. The screenshots in this document come
  from the preview that was used to choose; the application has had the same shape since.
- The choice of the colour theme comes to the settings: issue #9.

## First directions

The three directions compared at the start. Each is a complete stance: font, tabs, toolbar,
buttons, panels, graph drawing and colours. Their colours became the Sous-bois, Encre and Braise
themes.

### At a glance

| | Current | A. Sous-bois | B. Encre | C. Braise |
|---|---|---|---|---|
| Idea | starting point | organic | technical | warm |
| Font | the system's | DM Sans | Inter | Rubik |
| Tabs | attached frame | pills | underlined text | folder tabs |
| Toolbar | icon above label | pills, icon and label side by side | one compact row | solid groups |
| Buttons | 6 px corners | pills | 4 px corners | 10 px corners, thick bottom edge |
| Panels | separated by lines | floating cards | separated by lines | separated by lines |
| Commit in the graph | dot with initials | dot with initials | point | large dot with initials |
| Branch lines | rounded corners | soft curves | sharp corners, thin line | very rounded corners, thick line |
| Link from commit to message | ribbon that fades | capsule around the message | none | whole row coloured |
| Height of a row | 32 px | 34 px | 26 px | 36 px |
| Commits visible on an 880 px screen | 23 | 21 | 29 | 20 |
| Accent | blue | moss green | indigo | amber |

What does not change from one direction to another: the organisation of the screen, the icons,
the features. Motion (durations, animation curves) is not shown by screenshots; it is settled in
the guidelines.

### Reference: the current interface

| Dark | Light |
|---|---|
| ![Current interface, dark theme](directions/actuelle-graphe-sombre.png) | ![Current interface, light theme](directions/actuelle-graphe-clair.png) |

### A. Sous-bois

**Intent.** Draw the identity from the name: a "ramure" is a set of branches. Everything is
rounded and supple, the panels float on the background like cards, and each commit message is a
capsule in the colour of its branch, hooked onto a curve.

| Element | Choice |
|---|---|
| Font | DM Sans, 13.5 px; DM Mono for code. Section titles in lower case, without spaced capitals. |
| Tabs | Pills sitting on the background; the active tab is a pill tinted with the accent. |
| Toolbar | Each action is a bordered pill, icon and label side by side. |
| Buttons and fields | Pill-shaped buttons; fields with 10 px corners; dialogs with 20 px corners. |
| Panels | Sidebar, graph and detail are three rounded cards, separated by the background. |
| Graph | Dots with initials, soft 3 px curves, the message in a capsule. 34 px rows. |
| Labels | Tinted pills; the current branch as a solid pill. |
| Colours | Very dark green or warm paper backgrounds, moss green accent, branches in natural hues. |

**What it brings.** The most recognisable: no other Git client looks like this. The link between
a commit and its message is the most readable of the three.

**What it costs.** Cards and capsules take room: fewer commits on screen, and the margins between
cards take a few pixels from each panel. The green accent is also the colour of success and of
added lines in a diff.

| Graph, dark | Graph, light |
|---|---|
| ![Sous-bois, graph, dark theme](directions/sous-bois-graphe-sombre.png) | ![Sous-bois, graph, light theme](directions/sous-bois-graphe-clair.png) |

| Changes and diff | Dialog |
|---|---|
| ![Sous-bois, changes and diff](directions/sous-bois-travail-sombre.png) | ![Sous-bois, dialog](directions/sous-bois-dialogue-clair.png) |

### B. Encre

**Intent.** A tool first. Everything is flat, dense and crisp, in greys with almost no tint: the
only bright colours on screen are those of the branches.

| Element | Choice |
|---|---|
| Font | Inter, 12.5 px; JetBrains Mono for code. Section titles in spaced small capitals. |
| Tabs | Text, and a 2 px line under the active tab. No frame. |
| Toolbar | A single compact row, icon and label side by side, no background. |
| Buttons and fields | 4 px corners, no shadow; dialogs with 8 px corners. |
| Panels | Edge to edge, separated by thin lines. |
| Graph | 9 px points, 2 px lines with sharp corners, no ribbon. The author's avatar goes back to its column. 26 px rows. |
| Labels | Rectangles in the code font, with a coloured bar on the left; the current branch is solid. |
| Colours | Neutral greys, indigo accent, branches in bright colours. |

**What it brings.** The densest: a quarter more commits on screen. Indigo is not the colour of
any state (success, warning, error), so the accent always stands for an action. The closest to
habits formed with GitKraken.

**What it costs.** The least distinctive of the three, and the one with the least visual impact:
the graph is precise, but discreet.

| Graph, dark | Graph, light |
|---|---|
| ![Encre, graph, dark theme](directions/encre-graphe-sombre.png) | ![Encre, graph, light theme](directions/encre-graphe-clair.png) |

| Changes and diff | Dialog |
|---|---|
| ![Encre, changes and diff](directions/encre-travail-sombre.png) | ![Encre, dialog](directions/encre-dialogue-clair.png) |

### C. Braise

**Intent.** A warm, tactile interface: generous shapes, solid groups, thick lines. Amber evokes
wood without going for the expected green.

| Element | Choice |
|---|---|
| Font | Rubik, 13.5 px; Fira Code for code. Section titles in capitals, in the accent colour. |
| Tabs | Wide folder tabs, with 12 px corners. |
| Toolbar | Actions are grouped in solid, rounded blocks; icons in the accent colour. |
| Buttons and fields | 10 px corners, a thick bottom edge that gives relief; dialogs with 18 px corners. |
| Panels | Edge to edge, separated by lines. |
| Graph | Large dots with initials, 3.5 px lines with very rounded corners, the whole row tinted with the branch colour. Messages in semi-bold. 36 px rows. |
| Labels | Solid pills, taller. |
| Colours | Charcoal or cream backgrounds, amber accent. |

**What it brings.** The most spectacular graph: each branch reads from one edge of the screen to
the other. Warm, comfortable in the dark theme.

**What it costs.** The least dense. Amber is close to the warning colour: a main button and a
warning look alike. On a repository with many branches, the tinted rows can clutter the screen.

| Graph, dark | Graph, light |
|---|---|
| ![Braise, graph, dark theme](directions/braise-graphe-sombre.png) | ![Braise, graph, light theme](directions/braise-graphe-clair.png) |

| Changes and diff | Dialog |
|---|---|
| ![Braise, changes and diff](directions/braise-travail-sombre.png) | ![Braise, dialog](directions/braise-dialogue-clair.png) |

### Original recommendation

**Sous-bois**, for the structure and the shapes. It is the only direction that gives Ramure a
face of its own, and the capsule is the best answer to the starting flaw: the link between a
commit and its message.

Two reservations, which a mix can settle:

- **Density.** If 21 commits on screen are too few, the shapes of Sous-bois can be kept with the
  row height of Encre.
- **The green accent.** If it is a problem next to the green of successes, the indigo of Encre
  works on the backgrounds of Sous-bois.

Encre remains the right choice if the priority is efficiency rather than identity.

## Files

- [`directions.css`](directions.css): the colours and radii of the first three directions.
- [`esquisses/`](esquisses): the interface rules that were used for the screenshots (font, tabs,
  toolbar, panels, labels). They are sketches, not code to reuse as it is.
- Graph drawing, by direction:

| | Clairière | Sous-bois | Encre | Braise |
|---|---|---|---|---|
| Row height | 34 px | 34 px | 26 px | 36 px |
| Width of a column | 24 px | 24 px | 14 px | 26 px |
| Radius of a commit | 10 px | 10 px | 4.5 px | 11.5 px |
| Thickness of the line | 3 px | 3 px | 2 px | 3.5 px |
| Change of column | curve | curve | 5 px corner | 14 px corner |
| Band behind the commit | 24 px, 15% opacity | 24 px, 15% opacity | none | 26 px, 20% opacity |

## Notes

- All colours meet the WCAG contrasts: 7:1 for body text, 4.5:1 for secondary text and text
  placed on a colour, 3:1 for the lines of the graph. For Clairière, they are also checked on the
  background of the cards, on the selection and in the capsules.
- The proposed fonts are free (SIL Open Font License) and would be embedded in the application.
- The screenshots come from the interface of pull request #2, on Ubuntu.
