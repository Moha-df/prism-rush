# Prism Rush

**FPS roguelike coopératif, de 1 à 4 joueurs**, écrit en Rust avec Bevy 0.20.

Tu cours, tu sautes (double saut, dash, glissade, tremplins) sur une carte procédurale infinie,
face à des hordes de monstres qui grossissent sans fin, comme dans Megabonk. Différence : ton arme
principale, **c'est toi qui la vises**. À côté, tes armes automatiques (aura, foudre, orbes…) font
le reste. Tu montes de niveau, tu choisis tes cartes, tu ouvres des coffres et tu réanimes tes
coéquipiers. Il n'y a aucun fichier d'asset : modèles, terrain et sons sont générés par le code.

---

## 1. Installation (hôte, une seule fois)

1. **Installer Rust** : <https://rustup.rs>, puis lancer `rustup-init.exe` avec les options par
   défaut. Sur ce PC, Rust et les outils Visual Studio sont déjà installés.
2. **Double-cliquer sur `jouer.bat`**. La première compilation prend 5 à 15 minutes. Ensuite,
   chaque lancement vérifie et recompile automatiquement en une seconde si le code a changé.

| Script | Rôle |
|---|---|
| `jouer.bat` | Menu principal : Solo, Héberger, liste des parties ouvertes |
| `heberger.bat` | Héberge directement une partie (`heberger.bat 2` ajoute 2 bots) |
| `scripts\paquet_amis.bat` | Crée `dist\PrismRush.zip` à envoyer à tes amis |
| `scripts\serveur_dedie.bat` | Serveur sans fenêtre (personne ne joue sur ce PC) |
| `scripts\test_bots.bat` | Test automatique : serveur + bots + client bot qui rejoint par Internet |

---

## 2. Jouer en ligne avec un ami

**Rien à installer, pas de compte, pas de port à ouvrir sur la box.**

### Une seule fois : envoyer le jeu à ton ami

1. Double-clique sur `scripts\paquet_amis.bat`. Il crée **`dist\PrismRush.zip`** (≈ 35 Mo).
2. Envoie ce zip à ton ami (Discord, WeTransfer, Google Drive…).

⚠️ **Après chaque mise à jour du jeu, renvoie-lui un nouveau zip.** Les deux versions doivent être
identiques : une version différente n'apparaît pas dans la liste.

### À chaque partie

1. **Toi** : `jouer.bat`, puis **Héberger**. Le salon indique « Ta partie est en ligne ! » au bout
   de quelques secondes.
2. **Ton ami** : il dézippe, double-clique sur **`Jouer.bat`**. Ta partie apparaît dans
   **« Parties ouvertes »** (« Partie de *ton pseudo* »). Il clique sur **Rejoindre**.
3. Son nom apparaît dans ton salon. Clique sur **Lancer la partie**.

On peut aussi rejoindre une partie déjà en cours.

**Plan B**, si la partie n'apparaît pas dans la liste : dans ton salon, clique sur **Copier le code**
et envoie-le à ton ami. Il le colle (Ctrl+V) dans le champ du menu, puis clique sur **Rejoindre**.

### Comment ça marche (sans serveur ni port ouvert)

- **Connexion : [iroh](https://www.iroh.computer/).** Chaque partie a une identité
  (une clé publique) au lieu d'une adresse IP.
  - Iroh perce les box des deux côtés (« NAT traversal ») pour une connexion directe pair à pair.
  - Si c'est impossible (certaines box ou la 4G), il passe automatiquement par un **relais public
    gratuit** (de l'entreprise n0, sans compte), avec un peu plus de latence.
  - La connexion est chiffrée de bout en bout.
- **Liste des parties : brokers MQTT publics et gratuits** (HiveMQ et EMQX, les deux en même temps
  pour la fiabilité).
  - L'hôte y publie une petite annonce : nom, nombre de joueurs, comment le joindre.
  - Les menus des autres joueurs lisent ces annonces.
  - Si l'hôte quitte ou plante, l'annonce disparaît d'elle-même.
  - Seule cette annonce passe par là, jamais le jeu lui-même.
- La liste est **publique** : n'importe quel joueur de Prism Rush peut voir et rejoindre une partie
  ouverte. C'est sans risque (il n'y a que ton pseudo), et tu peux quitter le salon à tout moment.

### Si ça ne marche pas

| Symptôme | Solution |
|---|---|
| Windows : « Windows a protégé votre ordinateur » | **Informations complémentaires**, puis **Exécuter quand même** (l'exe n'est pas signé). |
| Le pare-feu Windows demande l'autorisation | Cliquer **Autoriser** (réseaux privés et publics). |
| La partie n'apparaît pas dans la liste | Utiliser le plan B (**Copier le code**). Vérifier que vous avez bien la même version du zip. |
| « Impossible de rejoindre la partie » | L'hôte est-il toujours dans le salon ? Réessayer. Un pare-feu d'entreprise ou d'école peut bloquer l'UDP. |
| Ça « lag » un peu | La connexion passe par le relais (box très restrictive). C'est jouable, juste un peu moins réactif. |
| Il manque `VCRUNTIME140.dll` | Installer <https://aka.ms/vs/17/release/vc_redist.x64.exe>. |

---

## 3. Contrôles

Les touches suivent la **position** physique : ZQSD sur AZERTY, WASD sur QWERTY.

| Action | Touche |
|---|---|
| Se déplacer | **Z Q S D** |
| Sauter (double saut, plus avec les Pattes de grenouille) | **Espace** |
| Sprinter | **Maj gauche** |
| Glisser (garde l'élan, accélère en descente) | **Ctrl** ou **C** |
| Dash | **A** (touche Q en QWERTY) ou bouton latéral de souris |
| Tirer (**maintenir** : l'arme tire à sa cadence) / Zoomer | **Clic gauche** / **Clic droit** |
| Ouvrir un coffre / Réveiller une statue / Réanimer (maintenir) | **E** |
| Choisir une carte (coffre ou niveau) | **Clic** ou **1 2 3 4** |
| Pause (sensibilité, volume, FOV) | **Échap** |

**Techniques** :
- enchaîne glissade puis saut pour garder ta vitesse ;
- les dashs se rechargent tout seuls ;
- les tremplins turquoise t'envoient en l'air ;
- les bons coffres sont souvent en hauteur, repère leurs **faisceaux lumineux** de loin ;
- les ondes de choc du Titan se **sautent**.

---

## 4. Le jeu (V3, inspirée de Megabonk)

### Le principe

Une seule carte infinie, un chrono qui tourne, et des hordes de plus en plus nombreuses.
Il n'y a **pas de fin** : le but est de **survivre le plus longtemps possible**.

- Les monstres lâchent des **gemmes d'XP** bleues. Passe près d'elles pour les ramasser.
  L'XP va à **toute l'équipe** : vous montez de niveau ensemble.
- À chaque **niveau**, chaque joueur choisit **1 carte parmi 3** (pas de pause, le jeu continue).
- **Boss** à 4:00, 8:00 et 12:00 (Colosse, Reine de la ruche, Titan de foudre). Chaque boss lâche
  un coffre légendaire.
- **15:00 : l'Essaim final.** Des **fantômes blancs** traversent tout et foncent sur vous.
- **20:00 : les fantômes noirs** arrivent. Les fantômes deviennent **plus rapides, plus résistants
  et plus forts chaque minute**, sans limite : au bout d'un moment, c'est impossible. Combien de
  temps tiendrez-vous ?

### Deux catégories d'armes

**1. L'arme principale : c'est toi qui vises et qui tires.** Tu la choisis dans le salon avant la
partie. Pas de munitions : maintiens le clic gauche, elle tire à sa cadence.

| Arme | Style | Prix |
|---|---|---|
| Bâton de feu | boules de feu qui explosent | départ |
| Arc | flèches rapides, gros critiques, traverse 1 ennemi | départ |
| Épée du héros | vagues tranchantes larges qui traversent tout | départ |
| Revolver | balles qui **rebondissent** d'ennemi en ennemi | 150 argent |
| Fusil à pompe | gerbe de plombs | 200 argent |
| Bananarang | part et **revient** comme un boomerang | 200 argent |
| Bâton de foudre | l'éclair frappe la cible visée puis **saute** aux voisins | 250 argent |
| Sniper | rayon instantané qui traverse tout | 300 argent |

Ses améliorations changent ses projectiles : **plus de projectiles**, plus rapides, plus gros,
plus de rebonds, plus de dégâts, de cadence ou de critiques.

**2. Les armes automatiques (3 maximum) : elles agissent toutes seules.** On les obtient avec les
cartes de niveau, puis on les améliore.

| Arme | Effet | Prix |
|---|---|---|
| Aura | champ de force qui brûle les ennemis autour de toi | départ |
| Foudre céleste | des éclairs tombent du ciel sur les ennemis | départ |
| Orbes gardiens | des orbes tournent autour de toi | départ |
| Haches | haches lancées en cloche, qui traversent | 120 argent |
| Traînée de feu | des flammes derrière toi quand tu cours | 120 argent |
| Mines | explosent au passage des ennemis | 150 argent |
| Fiole de poison | flaques de poison sous les ennemis | 150 argent |
| Tornade | des tornades errantes broient tout | 180 argent |

### Les cartes de niveau

Chaque carte a une **rareté** (commun, peu commun, rare, épique, légendaire) qui multiplie son
effet. Une carte peut être :
- une **amélioration de l'arme principale** (1 stat, 2 à partir de rare) ;
- une **nouvelle arme automatique** ;
- une **amélioration d'une arme automatique** ;
- un **tome** (4 maximum) : un bonus pour **toutes** les armes ou pour toi. Dégâts, cadence,
  quantité (+1 projectile), taille, vélocité, durée, vitalité, régénération, armure, agilité,
  chance, expérience, or, attraction (portée de ramassage), précision.

### Argent (or) et coffres

Chaque monstre tué rapporte de l'or **à chaque joueur**. Chacun a son portefeuille, et **le prix
des coffres monte à chaque coffre que tu ouvres** (et un peu avec le temps).

| Coffre | Faisceau | Contenu |
|---|---|---|
| Petit coffre (bois) | — | 3 objets au choix, surtout communs |
| Grand coffre (or) | orange | 3 objets, meilleures raretés |
| Coffre légendaire (violet) | magenta | 3 objets épiques ou légendaires (lâché par les boss) |
| Caisse d'arsenal (acier) | turquoise | 3 cartes d'amélioration d'armes (un niveau gratuit) |

Les **33 objets** (pattes de grenouille, jetpack, canines de vampire, bobine Tesla, couronne du
roi…) s'ajoutent à tout ça. Ils ont chacun leur icône dessinée par le code.

### Sanctuaires, power-ups et statues

- **Sanctuaire de charge** (pilier avec un cercle de pierres turquoise) : reste dans le cercle
  jusqu'à ce qu'il soit plein. Tous ceux qui sont dedans gagnent un **niveau gratuit**. Plus vous
  êtes nombreux dedans, plus ça charge vite.
- **Power-ups** lâchés par les monstres (souvent par les élites) : **Aimant** (toute l'XP de la
  carte), **Soin** d'équipe, **Bombe** (rase les monstres autour), **Hâte**.
- **Statues de boss** : appuie sur **E** pour invoquer leur boss tout de suite. C'est risqué, mais
  le boss lâche un coffre légendaire.

### Argent (métal) : la progression entre les parties

À la fin de chaque partie, tu gagnes de l'**argent** (selon le temps survécu, tes kills et le niveau
atteint). Dans le **salon**, clique sur une arme verrouillée pour la **débloquer**. L'argent et les
déblocages sont enregistrés sur ton PC (`settings.txt`).

### Monstres

| Monstre | Rôle |
|---|---|
| Trotteur | Araignée rapide en meute |
| Gelée | Se divise en deux petites gelées à sa mort |
| Cracheur | Plante-canon qui tire de l'acide en cloche |
| Feu follet | Orbe volant qui tire des éclairs |
| Chargeur | Rhinocéros de cristal : il gratte le sol puis **fonce en ligne droite**, écarte-toi ! |
| Bombeur | Bombe sur pattes qui explose au contact |
| Chaman | Masque flottant qui **soigne** les autres monstres : à abattre en premier |
| Brute | Tank qui frappe le sol |
| **Fantôme blanc / noir** | L'Essaim final : traverse les murs, de plus en plus fort |
| **Colosse** (boss) | Frappe au sol et salves de projectiles |
| **Reine de la ruche** (boss) | Vole, invoque des essaims, tire des éventails d'éclairs |
| **Titan de foudre** (boss) | Ondes de choc au sol (**saute** !) et foudre annoncée par un cercle |

Les monstres montent de niveau avec le temps, et les **élites** (auréole dorée) arrivent de plus en
plus souvent. Tous les monstres **télégraphient** leurs attaques : regarde-les pour esquiver.

### À terre

Un coéquipier te relève en maintenant E près de toi. Si tout le monde est à terre, c'est
**game over** : statistiques, argent gagné, puis retour au salon.

---

## 5. Pour les curieux / développeurs

```bash
cargo test --lib                     # 81 tests : monde déterministe, mouvement, armes, économie, réseau...
cargo clippy --all-targets           # aucun avertissement
cargo run --release -- server --headless --bots 3 --autostart --duration 120   # partie sans fenêtre
cargo run --release -- bot                                                     # client bot : rejoint la 1re partie de la liste
cargo run --release -- solo --autopilot --shots captures                       # captures d'écran auto
```

- **Logs** : `target\release\logs\prism_rush.log` (et `prism_rush.prev.log` pour la partie
  précédente). En cas de plantage, la raison est dans `logs\crash.log`.
- **Réglages** (pseudo, dernière adresse, sensibilité, volume, FOV, argent et armes débloquées) :
  `settings.txt`.
- **Architecture et choix techniques** : voir [`DECISIONS.md`](DECISIONS.md). Plan des jalons :
  [`PLAN.md`](PLAN.md).

Pour apprendre Rust, commence par lire, dans l'ordre :
1. `src/shared/rng.rs` et `src/shared/noise.rs` : du Rust simple.
2. `src/shared/movement.rs` : la simulation du joueur.
3. `src/net/client.rs` : la prédiction réseau.
4. `src/render/model.rs` et `src/render/models.rs` : les modèles en primitives.
