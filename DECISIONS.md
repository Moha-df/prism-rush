# DECISIONS — choix techniques (et pourquoi)

Ce fichier explique les choix faits pendant le développement, pour apprendre Rust/Bevy en lisant
le code. Les plus récents sont en bas de chaque section.

## Versions (vérifiées le 2026-10-23)

| Crate | Version | Pourquoi |
|---|---|---|
| Rust | 1.99.1 stable, édition 2024 | Installé via rustup (MSVC déjà présent sur la machine). |
| `bevy` | 0.20.0 (sortie le 2026-10-08) | Dernière stable. Feature `serialize` pour que `Vec3` soit sérialisable (réseau). |
| `bevy_replicon` | 0.45 | Réplication serveur → clients, maintenue, déjà compatible Bevy 0.20. |
| `bevy_replicon_quinnet` / `bevy_quinnet` | 0.21 / 0.22 | Transport QUIC (UDP chiffré). Le backend `renet` n'était **pas encore** porté sur Bevy 0.20. |
| `clap` | 4 | Ligne de commande (`host`, `join`, `server --headless`, `bot`...). |

**Alternative écartée : lightyear 0.31.** Il fournit prédiction et interpolation clés en main, mais
la 0.31 était sortie 2 jours avant le début du projet et dépendait de versions *release candidate*
(`aeronet_io 0.22.0-rc`). Risqué sans pouvoir tester à la main. Replicon est plus simple et stable ;
la prédiction est écrite à la main (≈200 lignes, voir `net/client.rs`) — c'est aussi plus instructif.

**Méthode** : l'API de Bevy change à chaque version, donc on lit les sources/exemples des crates
téléchargées (`~/.cargo/registry/src/.../bevy-0.20.0/examples`) plutôt que la mémoire.
Exemples utilisés : `first_person_view_model`, `fog`, `generate_custom_mesh`, `mouse_grab`,
`audio/decodable`, `ui/widgets/button`, et l'exemple `tic_tac_toe` de `bevy_replicon_quinnet`.

## Architecture

- **`lib.rs` + `main.rs`** : toute la logique est dans une bibliothèque, `main.rs` ne fait que lire la
  ligne de commande. Avantage : `cargo test --lib` teste tout sans fenêtre.
- **`shared/`** = logique *pure* (pas d'ECS, pas de rendu) : génération du monde, mouvement, armes,
  objets, économie. Ce sont de simples fonctions `fn(état, entrée) -> résultat`, faciles à tester et
  **déterministes** (même entrée → même sortie sur toutes les machines).
- **`game/`** = systèmes Bevy qui n'existent que sur le serveur (autorité).
- **`net/`** = protocole, connexion, prédiction, interpolation.
- **`render/` et `ui/`** = uniquement côté client avec fenêtre ; jamais ajoutés en mode `--headless`.

## Réseau

- **Serveur autoritaire** : le client n'envoie *que ses entrées* (`PlayerInput` : axes, angles,
  boutons). Le serveur simule, décide des dégâts, de l'argent, des coffres. Impossible de tricher
  sur l'or ou les PV.
- **Listen server** : l'hôte fait tourner le serveur *et* joue dans le même processus. Replicon gère
  ça nativement (`ClientId::Server`) : les messages de l'hôte vers « le serveur » sont redistribués
  localement, donc le code est le même pour l'hôte et les clients distants.
- **Tick fixe 60 Hz** (`FixedUpdate`) pour la simulation et la réplication.
- **Prédiction + réconciliation** (`net/client.rs`) : le client simule immédiatement ses propres
  entrées avec *exactement* la même fonction que le serveur (`shared::sim::simulate_player`), garde
  l'historique des entrées non confirmées, et quand l'état serveur arrive (avec le numéro de la
  dernière entrée appliquée, `last_seq`), il repart de cet état et rejoue l'historique. L'écart
  éventuel est lissé visuellement (`smoothing`) au lieu de téléporter la caméra.
  Test réel avec un client bot en réseau : **1 correction en 30 s** de jeu.
- **Redondance des entrées** : chaque paquet (non fiable) contient les 4 dernières entrées ; le serveur
  ignore les doublons (`InputQueue::push_new`). Une perte de paquet ne coûte rien.
- **Interpolation** (`net/interp.rs`) : les autres joueurs et monstres sont affichés 6 ticks (100 ms)
  dans le passé, entre deux snapshots. Chaque état répliqué porte son `tick` serveur ; le client
  estime le tick serveur courant (`ServerClock`).
- **Compensation de lag** (`game/combat.rs`) : le client indique à quel tick il *voit* le monde
  (`view_tick`) ; le serveur teste les tirs hitscan contre la position des monstres à ce tick
  (`MobHistory`). Ce que tu vises est ce que tu touches.
- **Projectiles** : répliqués *une seule fois* (origine, vitesse, gravité, tick de départ) ; chaque
  client calcule la trajectoire lui-même. Très économe en bande passante.
- **Le terrain n'est jamais envoyé** : seulement la seed. Chaque machine génère le même monde.

## Monde procédural

- Bruit de valeur + fBm + bruit « ridged » maison (`shared/noise.rs`), hachage SplitMix64 : pas de
  dépendance, et déterminisme garanti (testé).
- Le sol de collision est **exactement** le maillage affiché : hauteurs échantillonnées sur une
  grille de 2 m et interpolées sur les mêmes triangles (`ground_height`). Pas de « flottement ».
- Chunks de 32 m, générés à la demande et mis en cache (`ChunkCache`), purgés quand plus aucun
  joueur n'est proche → monde infini, mémoire bornée.
- Chaque chunk a un « thème » (rochers, ruines avec plateformes en escalier, forêt, tour avec
  tremplin, cristaux) pour varier et encourager le jeu vertical ; les coffres d'armes sont souvent
  en hauteur.

## Gameplay

- **Argent individuel, mais chaque kill paie tout le monde.** Pas de vol de kill, le joueur qui
  réanime ou fait diversion n'est pas pénalisé, et personne ne dépense « l'argent de l'équipe » pour
  un coffre que les autres voulaient. (`shared/economy.rs`)
- Prix des coffres ∝ difficulté^1.25, gains ∝ difficulté : dépenser tôt est récompensé.
- Difficulté façon Risk of Rain 2 : croît avec le temps, plus vite à plusieurs. Un « directeur »
  gagne des crédits et les dépense pour faire apparaître des monstres ; boss toutes les 5 minutes.
- Les monstres **télégraphient** leurs attaques (temps de préparation) : lisible et esquivable.

## Direction artistique et rendu

- Recherches faites avant de modéliser (guides low-poly, silhouettes, palettes) :
  silhouette lisible d'abord (test de la silhouette noire), **palette stricte** (`render/palette.rs`),
  **flat shading** avec une densité de facettes constante, lumière directionnelle chaude + ambiance
  froide, brouillard teinté, accents **émissifs** (yeux, visières) pour la lisibilité.
- Monde chaud et naturel (verts, grès, roches lavande) / monstres prune sombre avec accents lumineux
  (magenta, orange, vert toxique, cyan) / joueurs saturés avec visière lumineuse de leur couleur.
- **Un seul système de modèles** (`render/model.rs`) : une description en arbre de `Part`
  (primitive, taille, position, rotation, couleur, lueur, tag d'animation), utilisée de deux façons :
  instanciée en hiérarchie animable (personnages, coffres, armes) ou **cuite** dans un seul maillage
  à couleurs de sommets (décor des chunks : 3 draw calls par chunk quel que soit le contenu).
- Animations 100 % procédurales (`render/anim.rs`) : une fonction pure `pose(tag, état, temps)`.
- Arme en vue FPS rendue par une 2e caméra (calque 1) : elle ne traverse jamais les murs.
- Sons synthétisés au démarrage (oscillateurs, bruit filtré, enveloppes) via une source audio Bevy
  personnalisée (`render/audio.rs`), sans aucun fichier.
- Vérification visuelle automatique : `--autopilot` (le bot pilote le joueur) + `--shots` (captures).

## Jeu en ligne sans ouvrir de port

- **Tailscale** (VPN maillé gratuit) plutôt que :
  - playit.gg : l'UDP semblait réservé à l'offre payante, avec des retours de fiabilité mitigés ;
  - Steam Networking : demande Steam et un AppID ;
  - WebRTC : demande un serveur de signalisation, et échoue derrière certains NAT.
- Tailscale traverse les NAT, avec repli sur relais, et le jeu le voit comme un réseau local.
- Le salon affiche automatiquement l'IP Tailscale (`tailscale ip -4`) et l'IP locale.
- Partage de machine (« Share ») plutôt qu'invitation dans tout le réseau : l'ami ne voit que ce PC.
- Le serveur écoute sur `0.0.0.0` (toutes les interfaces). QUIC (quinnet) chiffre la connexion,
  avec un certificat auto-signé et une vérification ignorée côté client (acceptable entre amis).

## Vérification sans jouer

- `cargo test` : ~50 tests (déterminisme du monde, mouvement, armes, objets, économie,
  sérialisation réseau, file d'entrées, interpolation, rayon/capsule...).
- `prism_rush server --headless --bots 3 --autostart --duration 60` : partie complète sans fenêtre,
  logs de synthèse toutes les 10 s.
- `prism_rush bot 127.0.0.1` : client headless piloté par un bot, qui passe par **tout** le chemin
  réseau (connexion, prédiction, envoi d'entrées, réconciliation) et affiche RTT, pertes et
  nombre de corrections.

## Pièges rencontrés

- `gen` est un mot réservé en Rust 2024 → renommé `wgen`.
- `bevy_quinnet` avec ses features par défaut active `recv_channels`, incompatible avec la façon dont
  `bevy_replicon_quinnet` l'utilise → `default-features = false`.
- Les tests de génération ont révélé que les escaliers de ruines pouvaient déborder sur la zone de
  spawn : filtrage final dans `generate_chunk`.
- **Bug du « mauvais monstre qui meurt »** : le test rayon/capsule faisait 0/0 = NaN pour les
  petits monstres (capsule réduite à une sphère), et `NaN > r²` est faux → chaque tir « touchait »
  un Trotteur au hasard sur la carte. Trouvé en journalisant les kills (impact à 10–45 m du monstre
  tué). Leçon Rust/maths : toujours traiter les cas dégénérés, et tester `is_finite()`.
- **Fermetures du jeu** : par défaut Bevy *panique* quand une commande vise une entité déjà
  détruite (fréquent en réseau). Gestionnaire d'erreurs réglé sur `warn`, `try_despawn`/`try_insert`,
  et tous les logs écrits dans `logs/prism_rush.log` (+ `crash.log` en cas de panique).
- Les scripts `.bat` recompilent à chaque lancement (incrémental) : sinon on relance un vieil exe.
- `system_font_discovery` nécessaire pour la police système (accents français) ;
  `system_clipboard` pour coller l'IP avec Ctrl+V.
- Le bot fonçait vers des coffres perchés inaccessibles : il ignore les coffres trop hauts,
  abandonne après 8 s, et combat en priorité si un monstre est proche.
