# DECISIONS — choix techniques (et pourquoi)

Ce fichier explique les choix faits pendant le développement, pour apprendre Rust/Bevy en lisant
le code. Les plus récents sont en bas de chaque section.

## Versions (vérifiées le 2026-10-23)

| Crate | Version | Pourquoi |
|---|---|---|
| Rust | 1.99.1 stable, édition 2024 | Installé via rustup (MSVC déjà présent sur la machine). |
| `bevy` | 0.20.0 (sortie le 2026-10-08) | Dernière stable. Feature `serialize` pour que `Vec3` soit sérialisable (réseau). |
| `bevy_replicon` | 0.45 | Réplication serveur → clients, maintenue, déjà compatible Bevy 0.20. |
| `bevy_replicon_iroh` / `iroh` | 0.20 / 1.3 | Transport QUIC pair-à-pair par clé publique, traversée de NAT + relais publics (remplace quinnet, voir plus bas). |
| `rumqttc` | 0.25 | Client MQTT pour la liste publique des parties. |
| `tokio` | 1 | Runtime asynchrone requis par iroh. |
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

**Exigence** : l'ami dézippe, lance, voit la partie dans une liste et clique. Pas d'installation, pas
de compte, pas de port à ouvrir, pas de serveur à payer.

1re version : Tailscale (VPN). Fiable, mais il fallait installer un logiciel et créer un compte :
refusé. Version actuelle :

- **iroh** (`bevy_replicon_iroh`) remplace quinnet. On se connecte à une **clé publique**, pas à une
  IP. Iroh fait la traversée de NAT et se replie sur les **relais publics gratuits de n0** si le
  direct échoue. Le reste du netcode (replicon, prédiction, interpolation) n'a pas changé d'une
  ligne : c'est l'intérêt d'une couche transport interchangeable.
- **Annuaire des parties = MQTT public** (HiveMQ + EMQX en parallèle). Chaque instance publie un
  message *retenu* sur `prismrush/v<version>/games/<id>` (JSON : nom, joueurs, `EndpointAddr` iroh
  complète avec relais et IP directes). Un *last will* efface l'annonce si l'hôte disparaît ;
  l'annonce est rafraîchie toutes les 5 s et ignorée au-delà de 45 s. Le numéro de version est dans
  le sujet : les versions différentes ne se voient pas.
- Alternatives écartées :
  - Steam : demande Steam et un AppID ;
  - WebRTC/matchbox : demande d'héberger un serveur de signalisation ;
  - playit.gg : l'hôte doit configurer un tunnel, et l'UDP était payant ;
  - pkarr/DHT : permet de trouver une adresse, pas de lister des parties.
- Les opérations réseau longues (connexion, mise en ligne) tournent **en arrière-plan** (thread +
  canal `crossbeam`) : le jeu ne gèle jamais. Leçon Rust : une ressource Bevy doit être `Sync`, ce
  que n'est pas `std::sync::mpsc::Receiver` (contrairement à celui de crossbeam).
- Test de bout en bout : un serveur headless s'annonce, un client bot le trouve dans la liste,
  se connecte via iroh et joue 60 s sans aucune correction de prédiction.

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

## V2

- **Vagues plutôt que temps** : la difficulté, les prix et le niveau des monstres dépendent de
  la vague (`economy.rs`).
  - Le serveur garde un *budget* de spawn par vague (`Director`).
  - La vague finit quand le budget est dépensé et qu'il ne reste aucun monstre.
  - Les retardataires (à plus de 70 m, ou en vie plus de 30 s en fin de vague) sont **téléportés**
    près des joueurs plutôt que tués : la vague se termine toujours, sans frustration.
- **Cartes** : le coffre pose un composant répliqué `PendingChoice` sur le joueur. Le client
  affiche les cartes et renvoie `ChooseCard { index }`. Le serveur valide puis applique.
  Le serveur reste maître du tirage.
- **Raretés** : chaque type de coffre a ses poids par rareté. Le Trèfle multiplie les poids
  rares. Tirage en deux temps (rareté puis objet), sans doublon.
- **33 objets mais un seul `PlayerStats::from_inventory`** : toujours une fonction pure.
  - Les effets conditionnels (berserker, pointe creuse) passent par `conditional_damage`.
  - Les effets d'événement (ankh, ronces, bouclier, soif de sang) sont gérés au bon endroit côté
    serveur.
- **Serde et tableaux > 32** : `[u8; 33]` n'est pas sérialisable par serde. Un petit module
  `counts_serde` le (dé)sérialise comme une liste (et tolère une autre longueur).
- **Icônes dessinées en code** (`render/icons.rs`) :
  - un mini moteur 2D : formes définies par une fonction « suis-je dedans ? » ;
  - anticrénelage par sur-échantillonnage 4×4 ;
  - contour sombre façon autocollant ;
  - conversion en `Image` Bevy.
  Zéro fichier, et facile d'ajouter une icône.
- **2 armes max** : `Arsenal::give` renvoie l'arme remplacée, que le serveur fait tomber au sol
  (`WeaponPickup`, répliqué une fois). Rafales, montée en régime (minigun), perforation (railgun)
  et projectiles à gravité restent dans la simulation partagée, donc prédits côté client.
- **Hazards de boss** (`Hazard`) : répliqués une seule fois avec leur tick de départ. Le client
  anime l'onde de choc à l'heure *présente* du serveur (`ServerClock::estimate`), pour que le
  moment où il faut sauter soit exactement celui où elle touche. Le serveur ne la fait toucher que
  les joueurs au sol.
- **Élites** : un simple booléen dans `Mob`, avec des multiplicateurs de PV, dégâts et or. Le
  visuel (aura, couronne, taille ×1,3) est ajouté au modèle à la volée.
- **Ambiance par vague** (`render/atmosphere.rs`) : 4 « moods » (soleil, brouillard, ambiance,
  teinte du ciel, nuages). Chaque mood est tenu pendant la moitié de ses vagues, puis on passe
  progressivement au suivant. Un test vérifie qu'il n'y a pas de saut de lumière (il a détecté une
  aube trop brutale).
- **SSAO** : demande `Msaa::Off`. SMAA (post-process) lisse les bords à la place.
- **Tests visuels** : en mode `--shots`, une scène de démo (`render/debug.rs`) fait apparaître
  chaque nouveau monstre, une statue, une arme au sol et un choix de cartes, puis prend des
  captures. C'est ce qui a permis de vérifier le rendu sans jouer.

## V3 : la boucle de Megabonk, avec une arme qu'on vise

- **Deux catégories d'armes, deux chemins de code** :
  - l'**arme principale** fait partie de la simulation partagée (`shared/weapons.rs`) : le client
    prédit le tir (son, recul, flash, rayon du sniper) exactement comme le serveur ;
  - les **armes automatiques** n'existent que sur le serveur (`game/autos.rs`). Les clients les
    *dessinent* à partir du `Gear` répliqué (aura, orbes) ou des entités qu'elles créent
    (`Hazard` pour les mines, flaques, flammes, tornades ; `Projectile` pour les haches). Aucun
    message réseau par image pour l'aura ou les orbes : la position des orbes est une fonction du
    temps (`orb_offset`) que serveur et clients calculent chacun.
- **`build.rs` est une fonction pure de bout en bout** : `main_params(arme, améliorations, stats)`
  donne les chiffres finaux de l'arme. Une carte d'amélioration n'est qu'une liste de
  `(WStat, valeur)` ; la rareté multiplie la valeur (×1 à ×2, comme Megabonk). Les tomes sont
  stockés en dixièmes de points dans l'`Inventory`, donc `PlayerStats::from_inventory` les inclut
  sans rien changer ailleurs.
- **Un seul type de carte pour deux origines** : `PendingChoice { chest: Option<ChestKind>, options:
  Vec<CardOption> }`. Un coffre propose des objets, un niveau propose des offres ; l'écran de
  cartes, le bot et le serveur traitent les deux pareil.
- **Projectiles balayés** : une balle à 95 m/s parcourt 1,6 m par tick. On teste donc le *segment*
  parcouru pendant le tick contre les capsules des monstres (`ray_capsule`), pas un point : rien
  ne passe à travers. La perforation garde la liste des monstres déjà touchés ; le rebond crée un
  nouveau projectile vers le monstre le plus proche (un projectile répliqué une fois ne peut pas
  changer de trajectoire).
- **Boomerang sans état** : sa position est `origine + portée × sin(π·t/durée)`. Le client la
  calcule seul à partir du tick de départ, comme pour les projectiles en cloche.
- **Méta-progression locale** : l'argent et les déblocages sont dans le `settings.txt` du joueur,
  pas sur le serveur de l'hôte. Le client annonce son arme et ses armes débloquées (`Hello`, puis
  `PickLoadout` au salon) ; le serveur ne propose que les armes automatiques de cette liste.
- **Sanctuaires déterministes** : un emplacement de petit coffre sur 4 (selon son id, donc pareil
  pour tout le monde) devient un sanctuaire de charge. Pas de nouvelle génération de monde.

## V4 : bestiaire, rôles et points faibles

- **Rôle + minute d'apparition** : chaque monstre a un `Role` (petit rapide, gelée, tireur...) et
  une minute d'apparition. Au début de chaque partie, `roll_roster` garde quelques monstres par
  rôle (2 ou 3), avec la garantie d'au moins un monstre « du début » dans les rôles de base. Le
  tirage dépend de la graine de la partie : tous les joueurs voient la même chose, et deux parties
  ne se ressemblent pas. Les boss sont mélangés de la même façon (`roll_bosses`).
- **Les zones de dégâts sont des sphères** attachées au monstre (`HitZone` : position locale,
  rayon, effet). Un tir direct teste la capsule du corps *et* les sphères ; la plus proche gagne,
  avec une petite tolérance (`ZONE_SLACK`) pour que les points faibles soient agréables à viser.
  Une carapace ou un bouclier n'est qu'une zone « armure » placée devant : touché de face, c'est
  elle qu'on touche ; de dos, c'est le corps ou le point faible. Pas besoin de calculer d'angle.
- **Les dégâts de zone (explosions, aura) ne visent pas les points faibles** : seuls les tirs
  directs comptent. Ça récompense la visée, ce qui est le cœur de cette version.
- **Le multiplicateur est appliqué à un seul endroit** (`apply_mob_damage`), à partir du
  `DamageMob.zone` rempli par le code qui détecte le tir. Pattes cassées, boss au sol et monstres
  sous terre sont tous gérés là.
- **Ce que voit le client** : `MobAnim` gagne `Burrowed` (le modèle s'enfonce, un monticule de
  terre le suit) et `Toppled` (le corps tombe au sol). Les pattes cassées sont un petit composant
  répliqué (`MobParts { broken }`, un bit par patte) ; le modèle les cache avec le tag
  `Breakable(i)`. Les positions des pattes viennent d'une seule constante (`BROOD_LEGS`) utilisée
  par le modèle *et* par les zones : impossible qu'elles se décalent.
- **Zones au sol télégraphiées** : `HazardKind::Blast` (cercle qui explose après `life` secondes :
  obus du mortier, gelée de magma, sortie du ver) et des flaques (acide, lave) qui font mal toutes
  les 0,5 s. Elles sont répliquées une fois ; le client anime l'obus qui tombe à partir du tick de
  départ, comme les autres zones.
- **Solo passe par le salon** : il fallait pouvoir choisir ses armes. La partie n'est simplement
  pas annoncée dans la liste publique (`config.solo`).

## V5 : difficultés, cartes et totems

- **Un seul fichier pour l'équilibrage** : `shared/rules.rs` contient les tableaux des 5
  difficultés, des 3 cartes et des totems. `RunRules` (difficulté + carte + nombre de totems)
  est répliqué dans `MatchInfo` : le serveur s'en sert pour les calculs, les clients pour
  l'affichage (couleurs de la carte, texte du HUD).
- **Les PV sont appliqués à un seul endroit** : un système (`apply_rules_to_new_mobs`) multiplie
  la vie de tout monstre qui vient d'apparaître (`Added<Mob>`), quelle que soit son origine
  (directeur, gelée qui se divise, invocation, statue). Pas besoin de toucher chaque `spawn_mob`.
  Les dégâts sont multipliés dans l'IA, là où on calcule déjà les dégâts d'un monstre.
- **Une carte = même générateur, autre peinture** : le relief vient toujours de la graine (pas de
  nouvelle physique à tester). Chaque carte a sa palette de sol (`palette::Ground`), une fonction
  `grade` qui reteint le décor partagé (feuilles sèches dans le désert, cristaux orange...), ses 4
  ambiances de ciel, et sa liste de monstres et de boss.
- **Le déblocage des cartes appartient à chaque joueur** (comme l'argent) : il est enregistré
  dans `settings.txt` côté client quand le chrono passe 15:00. L'hôte ne peut choisir que les
  cartes qu'il a débloquées.
- **Les totems réutilisent les emplacements de coffres** (comme les sanctuaires) : une partie des
  petits coffres devient un totem, de façon déterministe. Rien à changer dans le générateur.
- **Le rythme de fin est en deux temps** : 15:00 les fantômes blancs arrivent de plus en plus vite
  (+50 % par minute) et se renforcent de 20 % par minute ; 20:00 tout le reste s'arrête et seuls
  les fantômes noirs arrivent, avec 15 % de force en plus par minute. C'est exponentiel exprès :
  la partie *doit* finir.
