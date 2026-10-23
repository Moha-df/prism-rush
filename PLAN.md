# PLAN — Prism Rush (FPS roguelike coop, Rust + Bevy)

Jeu : FPS roguelike coopératif 1–4 joueurs, map procédurale infinie, mobs en continu,
coffres d'objets/armes, réanimation, game over avec stats.

Stack : **Bevy 0.20** · **bevy_replicon 0.45** (réplication) · **bevy_replicon_quinnet 0.21**
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
| J7 | Jeu en ligne sans ouverture de port (Tailscale) + guide README + scripts .bat | ✅ |
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

## Reste à faire / idées

- Équilibrage fin (difficulté des premières minutes, prix, dégâts des armes) après des parties à plusieurs.
- Éclairage : ombres encore un peu sombres sous les plateformes, réglage de la lumière ambiante.
- Tableau de fin de partie aligné (police à chasse fixe).
- Possible : prédiction des roquettes côté client, plus de types de chunks, sons spatialisés.
