# Prism Rush

**FPS roguelike coopératif, de 1 à 4 joueurs**, écrit en Rust avec Bevy 0.20.

Tu cours, tu sautes (double saut, dash, glissade, tremplins) sur une carte procédurale infinie.
Tu tires sur des vagues de monstres de plus en plus dures. Tu achètes des coffres d'objets et
d'armes, et tu réanimes tes coéquipiers. Il n'y a aucun fichier d'asset : modèles, terrain et
sons sont générés par le code.

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
| Sauter (double saut, plus avec les Plumes) | **Espace** |
| Sprinter | **Maj gauche** |
| Glisser (garde l'élan, accélère en descente) | **Ctrl** ou **C** |
| Dash | **A** (touche Q en QWERTY) ou bouton latéral de souris |
| Tirer / Zoomer | **Clic gauche** / **Clic droit** |
| Recharger | **R** |
| Ouvrir un coffre / Prendre une arme / Réveiller une statue / Réanimer (maintenir) | **E** |
| Choisir une carte d'amélioration | **Clic** ou **1 2 3 4** |
| Changer d'arme (2 emplacements) | **1 / 2** ou **molette** |
| Pause (sensibilité, volume, FOV) | **Échap** |

**Techniques** :
- enchaîne glissade puis saut pour garder ta vitesse ;
- les dashs se rechargent tout seuls ;
- les tremplins turquoise t'envoient en l'air ;
- les bons coffres sont souvent en hauteur, repère leurs **faisceaux lumineux** de loin ;
- les ondes de choc du Titan se **sautent**.

---

## 4. Le jeu (V2)

### Vagues

- La partie est une suite de **vagues**. Une vague se termine quand son **dernier monstre** meurt.
- Ensuite, une **pause de 12 s** te laisse acheter, puis la vague suivante, plus dure, commence.
- Les monstres trop loin, ou qui survivent trop longtemps en fin de vague, sont **ramenés près de
  vous** : pas de chasse interminable. Quand il en reste 3 ou moins, des marqueurs ▼ indiquent où
  ils sont.
- **Toutes les 5 vagues : un boss.** Dans l'ordre : Colosse, Reine de la ruche, Titan de foudre,
  puis on recommence, en plus fort.
- **Le prix des coffres augmente à chaque vague.**
- **L'ambiance change avec les vagues** : après-midi doré, coucher de soleil, crépuscule violet,
  nuit… puis retour au jour.

### Argent

Chaque monstre tué rapporte de l'or **à tous les joueurs** (portefeuilles individuels). Pas de vol
de kill, et chacun décide de ses achats. Les **élites** (auréole dorée, à partir de la vague 3)
valent 3 fois plus.

### Coffres et cartes

| Coffre | Faisceau | Contenu |
|---|---|---|
| Petit coffre (bois) | — | 3 cartes, surtout communes |
| Grand coffre (or) | orange | 3 cartes, meilleures raretés |
| Coffre légendaire (violet) | magenta | 3 cartes épiques ou légendaires (aussi lâché par chaque boss) |
| Caisse d'arme (acier) | turquoise | une arme posée au sol |

Ouvrir un coffre montre **3 cartes d'amélioration** (4 avec la Couronne du roi). Tu en choisis une.
Le jeu continue pendant ce temps !

### 33 améliorations, en 4 catégories et 4 raretés

Raretés : commun, rare, épique, légendaire. Chaque amélioration a son icône dessinée en code.

- **Mobilité** :
  - Pattes de grenouille (+1 saut en l'air) ;
  - Jetpack (hauteur de saut) ;
  - Bottes de course ;
  - Propulseur (+1 dash) ;
  - Bobine de dash ;
  - Cire de glisse ;
  - Cape plume (contrôle en l'air) ;
  - Pas fantôme (invincible en dash).
- **Offensif** :
  - Balles lourdes ;
  - Gâchette sensible ;
  - Lunette critique ;
  - Hache du bourreau (dégâts critiques) ;
  - Obus instables (explosions) ;
  - Bobine Tesla (arcs électriques) ;
  - Pointe creuse ;
  - Masque du berserker ;
  - Soif de sang (vitesse après un kill) ;
  - Âme de gatling.
- **Défensif** :
  - Cœur vital ;
  - Mousse régénérante ;
  - Plaque d'armure ;
  - Canines de vampire (vol de vie) ;
  - Bouclier d'énergie ;
  - Ronces (renvoi de dégâts) ;
  - Ankh (te relève une fois) ;
  - Cœur de dragon.
- **Utilitaire** :
  - Pièce fétiche (+or) ;
  - Chargeur étendu ;
  - Mains agiles (rechargement) ;
  - Carte au trésor (coffres moins chers) ;
  - Trèfle à 4 feuilles (chance) ;
  - Trousse de soin ;
  - Couronne du roi (+1 carte).

### Armes : 2 emplacements

Les caisses d'arme posent l'arme **au sol**. Appuie sur **E** pour la prendre. Si tu as déjà 2 armes,
celle que tu tiens **tombe au sol** à la place, et un coéquipier peut la récupérer.

| Rareté | Armes |
|---|---|
| Commun | Pistolet, Mitraillette, Revolver |
| Rare | Fusil à pompe, Fusil d'assaut, Fusil à rafale (3 coups) |
| Épique | Sniper, Lance-grenades (en cloche), Fusil plasma |
| Légendaire | Lance-roquettes, Minigun (monte en régime), Railgun (traverse tous les ennemis) |

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
| **Colosse** (boss) | Frappe au sol et salves de projectiles |
| **Reine de la ruche** (boss) | Vole, invoque des essaims, tire des éventails d'éclairs |
| **Titan de foudre** (boss) | Ondes de choc au sol (**saute** !) et foudre annoncée par un cercle |

Tous les monstres **télégraphient** leurs attaques. Regarde-les pour esquiver.

### Statues

Des **statues de boss** sont dispersées dans le monde. Appuie sur **E** pour en réveiller une : son
boss apparaît immédiatement. C'est un pari risqué, mais chaque boss lâche un **coffre légendaire**.

### À terre

Un coéquipier te relève en maintenant E près de toi. Si tout le monde est à terre, c'est
**game over**, avec les statistiques, puis retour au salon.

---

## 5. Pour les curieux / développeurs

```bash
cargo test --lib                     # 79 tests : monde déterministe, mouvement, armes, économie, réseau...
cargo clippy --all-targets           # aucun avertissement
cargo run --release -- server --headless --bots 3 --autostart --duration 120   # partie sans fenêtre
cargo run --release -- bot                                                     # client bot : rejoint la 1re partie de la liste
cargo run --release -- solo --autopilot --shots captures                       # captures d'écran auto
```

- **Logs** : `target\release\logs\prism_rush.log` (et `prism_rush.prev.log` pour la partie
  précédente). En cas de plantage, la raison est dans `logs\crash.log`.
- **Réglages** (pseudo, dernière adresse, sensibilité, volume, FOV) : `settings.txt`.
- **Architecture et choix techniques** : voir [`DECISIONS.md`](DECISIONS.md). Plan des jalons :
  [`PLAN.md`](PLAN.md).

Pour apprendre Rust, commence par lire, dans l'ordre :
1. `src/shared/rng.rs` et `src/shared/noise.rs` : du Rust simple.
2. `src/shared/movement.rs` : la simulation du joueur.
3. `src/net/client.rs` : la prédiction réseau.
4. `src/render/model.rs` et `src/render/models.rs` : les modèles en primitives.
