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

## Reste à faire / idées

- Équilibrage fin après des parties à plusieurs (vagues tardives, prix, armes légendaires).
- Prédiction des roquettes côté client ; sons spatialisés.
- Plus de thèmes de chunks (biomes), plus d'affixes d'élites.
