# PLAN — Prism Rush (FPS roguelike coop, Rust + Bevy)

Jeu : FPS roguelike coopératif 1–4 joueurs, map procédurale infinie, mobs en continu,
coffres d'objets/armes, réanimation, game over avec stats.

Stack : **Bevy 0.20** · **bevy_replicon 0.45** (réplication) · **bevy_replicon_iroh 0.20** (iroh 1.3)
(transport QUIC/UDP) · pas d'assets externes (modèles, sons, terrain générés en code).

## Jalons

| # | Contenu | Statut |
|---|---------|--------|
| J1 | Joueur FPS sur terrain simple : sprint, saut, dash, glissade, air-control | ✅ |
| J2 | Multijoueur local (2 instances) + lobby (héberger / rejoindre / lancer) | ✅ |
| J3 | Tir hitscan, mobs, argent | ✅ |
| J4 | Coffres d'objets et d'armes, 15 bonus, 6 armes, scaling prix & difficulté | ✅ |
| J5 | Map infinie par chunks (seed partagée, déterministe) | ✅ |
| J6 | Modèles procéduraux (joueurs, 5+ monstres, armes, coffres, décor), animations, juice | ✅ |
| J7 | Jeu en ligne sans port ni installation (iroh + liste publique) + README + scripts .bat | ✅ |
| J8 | Polish, équilibrage, menus, écran de fin, bots, mode headless | 🟡 menus, écran de fin, bots, headless faits ; équilibrage et éclairage à affiner |

## Vérification sans jouer

- `cargo test` : logique pure (économie, prix, bonus, worldgen déterministe, mouvement, sérialisation réseau).
- `prism_rush server --headless --bots 3 --duration 120` : serveur sans fenêtre, bots qui
  se déplacent / tirent / ouvrent des coffres ; logs de synthèse périodiques.
- `prism_rush bot --addr 127.0.0.1` : client headless qui se connecte en réseau et joue seul
  (teste le netcode réel : connexion, inputs, réplication).

## Architecture (voir DECISIONS.md)

```
src/
  main.rs            CLI + assemblage de l'App
  shared/            logique pure, sans rendu (testée) : rng, noise, worldgen, movement,
                     weapons, items, economy, mobs
  net/               protocole répliqué, serveur, client (prédiction/réconciliation), interpolation
  game/              systèmes autoritaires (serveur) : joueurs, combat, mobs, directeur, coffres
  bot.rs             cerveau de bot (réutilisé par bots serveur et client headless)
  render/            modèles procéduraux, terrain, caméra, viewmodel, animations, FX, audio
  ui/                menu, lobby, HUD, écran de fin
```

## V2

| # | Contenu | Statut |
|---|---------|--------|
| V2.1 | Système de vagues : fin de vague au dernier monstre, pause, prix par vague, retardataires ramenés, boss toutes les 5 vagues | ✅ |
| V2.2 | 33 améliorations en 4 catégories et 4 raretés, coffres à 3 cartes au choix, icônes dessinées en code | ✅ |
| V2.3 | 2 emplacements d'arme, armes au sol à échanger, 6 nouvelles armes (rafale, montée en régime, perforante, cloche, plasma) | ✅ |
| V2.4 | Chargeur, Gelée, Chaman, élites, Reine de la ruche, Titan de foudre, statues d'invocation, coffre légendaire de boss | ✅ |
| V2.5 | Ambiance jour/nuit par vague, nuages, SSAO + SMAA, faisceaux de coffres, impacts, flash de bouche | ✅ |
| V2.6 | Bots (cartes, armes), tests, docs, release, push | ✅ |

## V3 — « Prism Bonk » : la boucle de Megabonk, mais c'est toi qui vises

Recherche : boucle de Megabonk (XP → niveaux → choix d'armes/tomes avec raretés ×1 à ×2,
4 armes + 4 tomes, coffres payants de plus en plus chers, sanctuaires, jarres, chrono, boss,
Essaim final de fantômes qui accélèrent sans fin, monnaie méta « silver » pour débloquer).
On garde **le même monde procédural, le même moteur et le même netcode**.

### Différences voulues avec Megabonk
- **Deux catégories d'armes, séparées** :
  - **Arme principale** (1 seule, tenue en main) : **tu vises et tu tires**. Elle est choisie
    au salon parmi celles débloquées. Ses améliorations portent sur les projectiles : nombre,
    vitesse, taille, rebonds, dégâts, cadence, critique.
  - **Armes automatiques** (jusqu'à 3) : aura, foudre du ciel, orbes, haches, mines, tornades,
    fiole de poison, traînée de feu. Elles agissent seules.
- **Pas de fin** : une seule carte infinie, pas de boss final. La difficulté monte jusqu'à
  devenir impossible : **fantômes blancs** à 15 min, **fantômes noirs** à 20 min, puis plus
  rapides, plus résistants et plus forts chaque minute.
- **Coop 1-4** : l'XP est commune à l'équipe (tout le monde monte de niveau ensemble), mais chacun
  fait ses choix.

### Jalons
| # | Contenu | État |
|---|---------|------|
| V3.1 | Boucle temps réel : chrono continu à la place des vagues, gemmes d'XP, niveaux d'équipe, HUD (chrono, niveau, barre d'XP) | fait |
| V3.2 | 8 armes principales visées, à projectiles (explosion, perforation, rebond, boomerang), rayon, chaîne d'éclairs ; sans munitions | fait |
| V3.3 | 8 armes automatiques (aura, foudre, orbes, haches, mines, tornade, poison, traînée de feu), simulées par le serveur, dessinées par les clients | fait |
| V3.4 | Montée de niveau : 3 cartes (amélioration d'arme 1-2 stats, nouvelle arme auto, amélioration auto, tome), 5 raretés, 15 tomes, 3 armes auto + 4 tomes max | fait |
| V3.5 | Coffres au prix qui monte à chaque achat, caisse d'arsenal (niveau gratuit), sanctuaires de charge, power-ups (aimant, soin, bombe, hâte), hordes en meutes | fait |
| V3.6 | Difficulté continue, élites, boss à 4/8/12 min, Essaim final (fantômes blancs puis noirs, croissance exponentielle) | fait |
| V3.7 | Méta-progression : argent gagné à chaque partie, arsenal au salon (choix de l'arme de départ, déblocages), sauvegarde dans `settings.txt` | fait |
| V3.8 | Bots adaptés, tests, docs, release, push | fait |

## V4 — Bestiaire et points faibles

| # | Contenu | État |
|---|---------|------|
| V4.1 | Salon : choix de l'arme automatique de départ ; Solo passe par le salon (choix des armes), sans être annoncé en ligne | fait |
| V4.2 | 20 nouveaux monstres (3 variantes par rôle : petits rapides, gelées, tireurs ; kamikazes, tanks, soutien, chargeur), chacun avec sa mécanique | fait |
| V4.3 | Roster tiré au sort par partie (quelques monstres par rôle, au moins un du début), ordre des boss aléatoire parmi 6, annonce des nouveaux monstres | fait |
| V4.4 | Zones de dégâts : points faibles ×2/×3, carapaces et boucliers, pattes cassables, boss au sol (vulnérable), monstres sous terre (invulnérables) | fait |
| V4.5 | 3 nouveaux boss (Mère-araignée, Golem ancien, Ver des sables), modèles, zones d'attaque au sol (mortier, magma, acide, lave), laser | fait |

## V5 — Difficultés, cartes et totems

| # | Contenu | État |
|---|---------|------|
| V5.1 | 5 niveaux de difficulté (Facile → Impossible) choisis par l'hôte dans le salon, en solo comme en multi : PV, dégâts, nombre de monstres, élites, argent gagné | fait |
| V5.2 | 3 cartes (Prairie, Désert de braise, Marais maudit) : même générateur, couleurs du sol, du décor et du ciel propres, brouillard, monstres et boss propres | fait |
| V5.3 | Déblocage : tenir 15:00 sur une carte débloque la suivante (enregistré dans settings.txt) | fait |
| V5.4 | Rythme de fin : 15:00 fantômes blancs (censés tuer), 20:00 plus que des fantômes noirs, de plus en plus rapides et forts | fait (à équilibrer) |
| V5.5 | Totems de défi : plus de monstres, plus forts, mais plus de chance, d'or et d'XP ; cumulables | fait (à équilibrer) |

## Reste à faire / idées

- Équilibrage fin après des parties à plusieurs (armes, cadence des hordes, prix).
- Projectiles de l'arme principale prédits côté client (aujourd'hui ils viennent du serveur :
  invisible en local, un léger retard sur Internet).
- Sanctuaires d'avidité / de défi, jarres cassables, sons spatialisés.
- Plus de thèmes de chunks (biomes), plus d'affixes d'élites.
