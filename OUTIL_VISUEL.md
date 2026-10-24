# Studio visuel : améliorer le jeu pièce par pièce

Le jeu a un mode **studio** qui affiche **une seule chose** (un monstre, un coffre, une
explosion, l'écran des cartes...) et enregistre une **planche** : une seule image qui contient
plusieurs vues (angles, étapes d'une animation, moments d'un effet). Le jeu se ferme tout seul
ensuite. Une image suffit pour juger un élément : c'est fait pour qu'un sous-agent Claude
améliore cet élément en regardant le résultat, sans jouer et sans gaspiller de tokens.

## Utilisation

```bash
bash scripts/studio.sh list                    # les ~190 cibles, avec le code qui les dessine
bash scripts/studio.sh mob:skitter             # -> studio_out/mob_skitter.png (+ .txt : légende des vues)
bash scripts/studio.sh model:chest:large
bash scripts/studio.sh --map 1 fx:explosion    # avec l'éclairage du Désert (0, 1 ou 2)
```

(Sans le script : `prism_rush studio <cible> [--out fichier.png]`.) La fenêtre s'ouvre derrière
les autres et ne prend ni la souris ni le clavier ; le son est coupé ; `settings.txt` n'est
jamais modifié.

| Cibles | Planche |
|---|---|
| `mob:<monstre>` (`:elite` pour la version élite) | 3x3 : marche de profil à 4 moments, 3/4 avant et arrière, préparation d'attaque, attaque, pose spéciale (au sol, sous terre, vue de dessus) |
| `model:chest|statue|powerup:<...>`, `model:shrine|totem|gem|jumppad|player`, `model:decor|solid|weapon|auto|projectile:<...>` | 3x2 : 4 angles, plongée, gros plan (coffres : ouvert) |
| `fx:<effet>`, `fx:tracer:<arme>`, `fx:mobattack:<monstre>`, `fx:chestopen:<coffre>`, `fx:powerup:<...>` | 3x2 : l'effet de 0,03 s à 1 s |
| `hazard:<zone>` (onde de choc, foudre, mortier, flaques...) | 3x2 : la zone de 0 à 1,8 s |
| `auto:<arme automatique>` | 3x2 : l'arme en action autour du joueur, face à des mannequins |
| `ui:hud|cards|chestcards|toasts|gameover|lobby|menu|pause` | l'écran (ou 4 moments de son animation) |
| `icons:items|weapons|tomes` | toutes les icônes en grille, avec leur nom |

## Lancer des sous-agents (un par élément)

Le sous-agent **`visual-polisher`** (`.claude/agents/visual-polisher.md`) connaît la méthode :
image « avant », 3 à 5 défauts repérés, corrections, 4-5 rendus maximum, `clippy`, image
« après », compte rendu. Il ne touche qu'au code de sa cible et jamais au gameplay.

Il suffit de demander à Claude, par exemple :

> Lance un visual-polisher par monstre du Désert de braise, chacun dans son worktree.

> Lance des visual-polisher sur fx:explosion, fx:crit, fx:kill et ui:cards.

Chaque sous-agent travaille dans sa **propre copie du projet** (git worktree) : ils ne se
gênent pas. Ils partagent le dossier `target/` du projet principal (Bevy n'est jamais recompilé)
et compilent chacun leur tour (verrou). Comptez 1 à 3 minutes pour la première compilation d'un
sous-agent, puis ~30 s. Au-delà de 6 à 8 sous-agents en même temps, ils attendent surtout leur
tour de compilation.

Quand ils ont fini, Claude regarde les planches avant/après (`studio_out/` de chaque worktree),
fusionne les modifications retenues, vérifie que tout compile, et committe.

## Comment ça marche (pour les curieux)

`src/render/studio.rs` : le jeu lance une partie solo sans monstres, place le sujet sur un socle
gris à 70 m au-dessus du joueur (dans le ciel, fond propre), remplace la caméra du joueur par une
caméra « objectif photo » (champ de vision de 30°) qui tourne autour du sujet, ajoute une lumière
d'appoint, pilote l'animation (marche, attaque, couvercle...) et prend les vues d'un petit
« plan de tournage ». Les captures sont ensuite assemblées en une planche avec la bibliothèque
`image`.
