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
| `jouer.bat` | Menu principal : Solo, Héberger ou Rejoindre |
| `heberger.bat` | Héberge directement une partie (`heberger.bat 2` ajoute 2 bots) |
| `rejoindre.bat 100.x.y.z` | Rejoint directement une partie |
| `scripts\installer_tailscale.bat` | Installe Tailscale (jeu en ligne) |
| `scripts\autoriser_pare_feu.bat` | Ouvre le port UDP 7777 dans le pare-feu **Windows** (à lancer en administrateur si besoin) |
| `scripts\paquet_amis.bat` | Crée `dist\PrismRush.zip` à envoyer à tes amis |
| `scripts\serveur_dedie.bat` | Serveur sans fenêtre (personne ne joue sur ce PC) |
| `scripts\test_bots.bat` | Test automatique : serveur + bots + client bot réseau |

---

## 2. Jouer en ligne avec un ami, sans ouvrir de port sur ta box

### Le principe

Ta box bloque les connexions entrantes, et ouvrir un port est pénible et risqué. On utilise
**[Tailscale](https://tailscale.com)**, un VPN gratuit qui relie directement vos PC comme s'ils
étaient sur le même réseau local, quelles que soient vos box. Chaque PC reçoit une adresse
`100.x.y.z`. Ton ami se connecte à la tienne, et Tailscale traverse les box tout seul : il passe
par un relais si une connexion directe est impossible.

Pourquoi ce choix :
- **C'est fiable.** Ça marche derrière quasiment toutes les box, y compris en 4G.
- **C'est gratuit.** Jusqu'à 6 utilisateurs, et le partage de machine est inclus.
- **Le jeu n'a rien de spécial à gérer** : pour lui, c'est un simple réseau local.

### Étapes pour TOI (l'hôte), une seule fois

1. **Installer Tailscale** : double-clique sur `scripts\installer_tailscale.bat`, ou télécharge-le
   sur <https://tailscale.com/download/windows>. Sur ce PC, il est déjà installé (adresse détectée :
   `100.112.81.107`).
2. **Te connecter** : clique sur l'icône Tailscale près de l'horloge, puis **Log in**
   (compte Google, Microsoft ou GitHub).
3. **Partager ton PC avec ton ami** :
   - ouvre la console <https://login.tailscale.com/admin/machines> ;
   - sur la ligne de ton PC, clique sur **⋯** puis **Share…** ;
   - onglet **Copy invite link**, puis **Copy share link** ;
   - envoie ce lien à ton ami (Discord, SMS…). Il est personnel, ne le publie pas.

   *Alternative : **Users → Invite users** l'ajoute à ton réseau Tailscale complet. Le partage de
   machine est plus prudent : il ne voit que ce PC.*
4. **Créer le paquet du jeu pour ton ami** : double-clique sur `scripts\paquet_amis.bat`, puis
   envoie-lui `dist\PrismRush.zip` (WeTransfer, Google Drive, Discord…).
   ⚠️ **Après chaque mise à jour du jeu, renvoie-lui un nouveau zip** : les deux versions doivent
   être identiques, sinon la connexion est refusée (« version du jeu différente »).

### Étapes pour TON AMI, une seule fois

1. Dézipper `PrismRush.zip` où il veut.
2. Installer Tailscale (`installer_tailscale.bat` dans le dossier, ou le site) et **se connecter**
   avec son propre compte.
3. **Ouvrir le lien de partage** que tu lui as envoyé, puis **Accept**. Ton PC apparaît alors dans
   sa liste Tailscale.

Il n'a pas besoin d'installer Rust : le zip contient l'exécutable prêt à l'emploi.
Si Windows signale qu'il manque `VCRUNTIME140.dll`, il faut installer
<https://aka.ms/vs/17/release/vc_redist.x64.exe>.

### À chaque partie

1. **Toi** : lance `heberger.bat` (ou `jouer.bat` → **Héberger**). Le salon affiche l'adresse à
   donner, du type **`Internet (Tailscale) : 100.112.81.107`**.
2. **Windows peut afficher « Autoriser l'accès ? » pour le pare-feu** au premier lancement : coche
   **réseaux privés ET publics**, puis **Autoriser**. Si tu as cliqué Annuler par erreur, lance
   `scripts\autoriser_pare_feu.bat` en administrateur (clic droit → *Exécuter en tant
   qu'administrateur*).
3. **Ton ami** : `jouer.bat`, puis il colle ton adresse dans **Rejoindre** (Ctrl+V fonctionne)
   et clique sur **Rejoindre**. Il peut aussi faire `rejoindre.bat 100.112.81.107`.
4. Son nom apparaît dans ton salon. Clique sur **Lancer la partie**.

Les joueurs peuvent aussi rejoindre une partie déjà en cours.

### Si ça ne se connecte pas

| Symptôme | Solution |
|---|---|
| « Délai de connexion dépassé » | Vérifier que **Tailscale est connecté des deux côtés** (icône active) et que l'ami voit ton PC dans sa liste. Tester avec `ping 100.x.y.z` dans un terminal chez l'ami. |
| Le ping marche mais pas le jeu | Pare-feu Windows de l'hôte : lancer `scripts\autoriser_pare_feu.bat` en administrateur. |
| « version du jeu différente » | Renvoyer un `PrismRush.zip` à jour (`scripts\paquet_amis.bat`). |
| « Impossible d'héberger (port occupé ?) » | Une autre instance du jeu tourne encore : la fermer (Gestionnaire des tâches). |
| Même réseau local (même box) | Pas besoin de Tailscale : utiliser l'adresse « Même réseau local » affichée dans le salon. |

*Autres VPN possibles : ZeroTier ou Radmin VPN fonctionnent aussi. N'importe quel outil qui crée
un réseau local virtuel convient : il suffit de donner l'IP correspondante.*

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
cargo run --release -- bot 127.0.0.1                                           # client bot réseau
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
