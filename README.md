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
| Ouvrir un coffre / Réanimer (maintenir) | **E** |
| Changer d'arme | **1 à 6** ou **molette** |
| Pause (sensibilité, volume, FOV) | **Échap** |

**Techniques** : enchaîne glissade puis saut pour garder ta vitesse. Les dashs se rechargent
tout seuls. Les tremplins turquoise t'envoient en l'air, et les coffres d'armes sont souvent en
hauteur.

---

## 4. Le jeu

- **Argent** : chaque monstre tué rapporte de l'or **à tous les joueurs** (portefeuilles
  individuels). Pas de vol de kill, et chacun décide de ses achats.
- **Coffres d'objets** (doré) et **coffres d'armes** (acier et turquoise, plus chers). Leur prix
  augmente avec le temps.
- **Difficulté** : elle monte avec le temps, et plus vite à plusieurs. Il y a plus de monstres,
  ils sont plus résistants et de nouveaux types arrivent. Un **boss** apparaît toutes les 5 minutes.
- **À terre** : un coéquipier te relève en maintenant E près de toi. Si tout le monde est à terre,
  c'est **game over**, avec les statistiques, puis retour au salon.

**15 objets cumulables** : Plume (+1 saut), Bottes véloces (vitesse), Chargeur étendu,
Mains agiles (rechargement), Balles lourdes (dégâts), Cœur vital (PV max), Mousse régénérante,
Gâchette sensible (cadence), Lentille critique, Plaque d'armure, Propulseur (+1 dash),
Croc vampire (vol de vie), Pièce fétiche (+or), Obus instables (explosions), Ressort (hauteur de
saut).

**6 armes** : Pistolet, Mitraillette, Fusil à pompe, Fusil d'assaut, Sniper, Lance-roquettes.

**Monstres** :

| Monstre | Rôle |
|---|---|
| Trotteur | Araignée rapide en meute |
| Cracheur | Plante-canon qui tire de l'acide en cloche |
| Feu follet | Orbe volant qui tire des éclairs |
| Bombeur | Bombe sur pattes qui explose au contact |
| Brute | Tank qui frappe le sol |
| **Colosse** | Boss : frappe au sol et salves de projectiles |

Tous **télégraphient** leurs attaques : ils se figent, gonflent ou lèvent les bras. Regarde-les
pour esquiver.

---

## 5. Pour les curieux / développeurs

```bash
cargo test --lib                     # 67 tests : monde déterministe, mouvement, armes, économie, réseau...
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
