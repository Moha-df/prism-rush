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
| # | Contenu |
|---|---------|
| V3.1 | Boucle temps réel : chrono continu à la place des vagues, gemmes d'XP (aimant), niveaux d'équipe, or au sol, HUD (chrono, niveau, barre d'XP) |
| V3.2 | Armes principales visées (bâton de feu, arc, revolver, fusil à pompe, sniper, bâton de foudre, bananarang, épée) à projectiles, sans munitions ; visuels prédits côté client ; choix de l'arme de départ au salon |
| V3.3 | Armes automatiques (aura, foudre, orbes, haches, mines, tornade, poison, traînée de feu) simulées par le serveur |
| V3.4 | Montée de niveau : 3 choix (nouvelle arme auto, amélioration d'arme avec 1-2 stats selon la rareté, nouveau tome, tome +1), 5 raretés, 16 tomes, 3 armes auto + 4 tomes max |
| V3.5 | Monde : coffres = objet aléatoire, prix qui monte à chaque achat ; jarres ; sanctuaires (charge, avidité, défi, aimant) ; bonus au sol (soin, aimant, bombe, rage) |
| V3.6 | Difficulté continue, élites, boss à 4/8/12 min, Essaim final (fantômes blancs puis noirs, croissance exponentielle) |
| V3.7 | Méta-progression : silver gagné à chaque partie, boutique au menu (armes principales et armes auto à débloquer), profil sauvegardé |
| V3.8 | Bots, tests, docs, release, push |

## Reste à faire / idées

- Équilibrage fin après des parties à plusieurs (vagues tardives, prix, armes légendaires).
- Prédiction des roquettes côté client ; sons spatialisés.
- Plus de thèmes de chunks (biomes), plus d'affixes d'élites.
