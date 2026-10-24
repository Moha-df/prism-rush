//! Visual studio: renders ONE thing of the game (a 3D model, a monster and
//! its animations, a particle effect, a ground hazard, an automatic weapon,
//! a UI screen, the icons) and saves a single **contact sheet** PNG: several
//! frames (angles or moments) tiled in one image, then quits.
//!
//! Made for polishing the visuals piece by piece (one helper agent per
//! target): one command, one small image to look at, no need to play.
//!
//! ```text
//! prism_rush studio list               # every target, with the code to edit
//! prism_rush studio mob:skitter        # 3x3 sheet: walk cycle, angles, attack...
//! prism_rush studio model:chest:large --out shots/chest.png
//! ```
//!
//! How it works: the game starts a normal solo run (no monster spawns), and
//! the subject is placed on a grey plinth 70 m above the player, in the sky.
//! A studio camera (narrow lens) replaces the player camera and takes the
//! shots of a small plan (see `build_plan`). The frame labels are printed
//! and saved next to the PNG (`.txt`), in reading order.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};
use bevy_replicon::prelude::{SendTargets, ToClients};

use super::avatars::{AnimState, HasModel};
use super::model::{MeshBuffer, Part, PrimCache, spawn_model};
use super::{MainCamera, ViewModelCamera, models};
use crate::config::AppConfig;
use crate::net::protocol::*;
use crate::shared::build::{AutoWeapon, MainWeapon, Tome, WeaponLevel};
use crate::shared::economy::ChestKind;
use crate::shared::items::ItemKind;
use crate::shared::mobs::MobKind;
use crate::shared::worldgen::{Decor, DecorKind, Shape, Solid, SolidKind};
use crate::ui::AppState;

/// Height of the stage above the player spawn.
const STAGE_HEIGHT: f32 = 70.0;
/// Seconds after the game is ready before the first shot (models load).
const WARMUP: f32 = 2.0;

// ---------------------------------------------------------------------------
// Catalog of targets
// ---------------------------------------------------------------------------

/// Every monster with a model.
fn all_mobs() -> Vec<MobKind> {
    let mut v: Vec<MobKind> = MobKind::SPAWNABLE.to_vec();
    v.extend(MobKind::BOSSES);
    v.extend([MobKind::Jellet, MobKind::WhiteGhost, MobKind::BlackGhost]);
    v
}

const CHESTS: [ChestKind; 4] = [
    ChestKind::Small,
    ChestKind::Large,
    ChestKind::Legendary,
    ChestKind::Weapon,
];
const DECORS: [DecorKind; 6] = [
    DecorKind::Grass,
    DecorKind::Flower,
    DecorKind::Pebble,
    DecorKind::Mushroom,
    DecorKind::Canopy,
    DecorKind::Shard,
];
const SOLIDS: [SolidKind; 6] = [
    SolidKind::Rock,
    SolidKind::Pillar,
    SolidKind::Platform,
    SolidKind::TreeTrunk,
    SolidKind::Ruin,
    SolidKind::Crystal,
];
const PROJECTILES: [ProjectileKind; 12] = [
    ProjectileKind::Acid,
    ProjectileKind::Bolt,
    ProjectileKind::Thorn,
    ProjectileKind::Shard,
    ProjectileKind::BossOrb,
    ProjectileKind::Fireball,
    ProjectileKind::Arrow,
    ProjectileKind::Bullet,
    ProjectileKind::Pellet,
    ProjectileKind::SwordWave,
    ProjectileKind::Banana,
    ProjectileKind::Axe,
];
const HAZARDS: [HazardKind; 9] = [
    HazardKind::Shockwave,
    HazardKind::Lightning,
    HazardKind::Mine,
    HazardKind::Poison,
    HazardKind::Fire,
    HazardKind::Tornado,
    HazardKind::Blast,
    HazardKind::AcidPool,
    HazardKind::LavaPool,
];
const FX_SIMPLE: [&str; 13] = [
    "hit",
    "crit",
    "weak",
    "armor",
    "kill",
    "explosion",
    "lightning",
    "heal",
    "warp",
    "arc",
    "beam",
    "partbroken",
    "xp",
];
const FX_EVENTS: [&str; 4] = ["statue", "shrine", "totem", "levelup"];
const UI_TARGETS: [&str; 8] = [
    "hud",
    "cards",
    "chestcards",
    "toasts",
    "gameover",
    "lobby",
    "menu",
    "pause",
];
const ICON_SETS: [&str; 3] = ["items", "weapons", "tomes"];

/// Lowercase name of an enum value (`MobKind::StoneGolem` -> "stonegolem").
fn key<T: std::fmt::Debug>(v: T) -> String {
    format!("{v:?}").to_lowercase()
}

/// Every target with the code that draws it (what a helper agent edits).
pub fn catalog() -> Vec<(String, &'static str)> {
    let mut c: Vec<(String, &'static str)> = Vec::new();
    for m in all_mobs() {
        c.push((format!("mob:{}", key(m)), "src/render/monsters.rs ou src/render/models.rs (fn mob → modèle), animations : src/render/anim.rs"));
    }
    c.push((
        "mob:<nom>:elite".into(),
        "variante élite (src/render/models.rs: mob_with_elite)",
    ));
    for k in CHESTS {
        c.push((
            format!("model:chest:{}", key(k)),
            "src/render/models.rs: chest / chest_body",
        ));
    }
    for b in MobKind::BOSSES {
        c.push((
            format!("model:statue:{}", key(b)),
            "src/render/models.rs: statue / petrify",
        ));
    }
    c.push(("model:shrine".into(), "src/render/models.rs: shrine"));
    c.push(("model:totem".into(), "src/render/models.rs: totem"));
    c.push(("model:gem".into(), "src/render/models.rs: xp_gem"));
    c.push(("model:jumppad".into(), "src/render/models.rs: jump_pad"));
    c.push(("model:player".into(), "src/render/models.rs: player"));
    for p in PowerupKind::ALL {
        c.push((
            format!("model:powerup:{}", key(p)),
            "src/render/models.rs: powerup",
        ));
    }
    for d in DECORS {
        c.push((
            format!("model:decor:{}", key(d)),
            "src/render/models.rs: decor",
        ));
    }
    for s in SOLIDS {
        c.push((
            format!("model:solid:{}", key(s)),
            "src/render/models.rs: solid",
        ));
    }
    for w in MainWeapon::ALL {
        c.push((format!("model:weapon:{}", key(w)), "src/render/models.rs: weapon (arme en main, vue 1re personne : src/render/viewmodel.rs)"));
    }
    for a in AutoWeapon::ALL {
        c.push((
            format!("model:auto:{}", key(a)),
            "src/render/models.rs: auto_weapon",
        ));
    }
    for p in PROJECTILES {
        c.push((
            format!("model:projectile:{}", key(p)),
            "src/render/models.rs: projectile",
        ));
    }
    for f in FX_SIMPLE {
        c.push((
            format!("fx:{f}"),
            "src/render/fx.rs (on_fx: particules, flashs, chiffres de dégâts)",
        ));
    }
    for w in MainWeapon::ALL {
        c.push((
            format!("fx:tracer:{}", key(w)),
            "src/render/fx.rs (Fx::Tracer, couleurs weapon_tracer_color)",
        ));
    }
    for m in all_mobs() {
        c.push((
            format!("fx:mobattack:{}", key(m)),
            "src/render/fx.rs (Fx::MobAttack : télégraphe d'attaque)",
        ));
    }
    for k in CHESTS {
        c.push((
            format!("fx:chestopen:{}", key(k)),
            "src/render/fx.rs (GameEvent::ChestOpened)",
        ));
    }
    for p in PowerupKind::ALL {
        c.push((
            format!("fx:powerup:{}", key(p)),
            "src/render/fx.rs (GameEvent::PowerupTaken)",
        ));
    }
    for f in FX_EVENTS {
        c.push((format!("fx:{f}"), "src/render/fx.rs (on_events)"));
    }
    for h in HAZARDS {
        c.push((
            format!("hazard:{}", key(h)),
            "src/render/avatars.rs (modèles des zones : attach_models, grow_shockwaves)",
        ));
    }
    for a in AutoWeapon::ALL {
        c.push((
            format!("auto:{}", key(a)),
            "src/render/autos.rs (+ src/render/fx.rs) : visuels des armes automatiques en jeu",
        ));
    }
    for u in UI_TARGETS {
        let src = match u {
            "hud" | "toasts" => "src/ui/hud.rs",
            "cards" | "chestcards" => "src/ui/cards.rs",
            "gameover" => "src/ui/gameover.rs",
            "lobby" => "src/ui/lobby.rs + src/ui/arsenal.rs",
            "menu" => "src/ui/menu.rs",
            _ => "src/ui/mod.rs (menu pause)",
        };
        c.push((format!("ui:{u}"), src));
    }
    for i in ICON_SETS {
        c.push((
            format!("icons:{i}"),
            "src/render/icons.rs (icônes dessinées par le code)",
        ));
    }
    c
}

// ---------------------------------------------------------------------------
// Plans: what to shoot
// ---------------------------------------------------------------------------

/// Camera around the subject: yaw 0 = in front of it (models face -Z).
#[derive(Clone, Copy, Debug)]
struct Cam {
    yaw: f32,
    pitch: f32,
    /// Distance multiplier (< 1 = closer).
    zoom: f32,
}

const fn cam(yaw: f32, pitch: f32, zoom: f32) -> Cam {
    Cam { yaw, pitch, zoom }
}

/// Something to do right before waiting for a shot.
#[derive(Clone, Debug)]
enum Act {
    None,
    /// Monster pose (`walk` = walk speed of the legs, m/s).
    Anim {
        action: MobAnim,
        walk: f32,
    },
    Broken(u8),
    Burrow(bool),
    Open(bool),
    Fx(Fx),
    Event(GameEvent),
    Hazard(HazardKind, f32, f32),
    /// Level-up cards (false) or chest cards (true).
    Cards(bool),
    Toasts,
    GameOver,
    Pause,
}

struct Step {
    label: String,
    cam: Cam,
    act: Act,
    /// Seconds to wait after the action before taking the shot.
    wait: f32,
}

fn step(label: impl Into<String>, cam: Cam, act: Act, wait: f32) -> Step {
    Step {
        label: label.into(),
        cam,
        act,
        wait,
    }
}

/// How the frames are laid out in the sheet.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Layout {
    /// Square crop of the center, `tile` px, `cols` per row.
    Square { tile: u32, cols: u32 },
    /// Full frames scaled to `width` px wide, `cols` per row.
    Full { width: u32, cols: u32 },
}

/// What is shown.
enum Subject {
    /// A model on the plinth (`stride` for the walk cycle, `sink` for burrowing).
    Model { part: Part, stride: f32, sink: f32 },
    /// Only the plinth: effects and hazards happen on it.
    Stage { radius: f32 },
    /// Around the player, with dummy monsters (automatic weapons).
    Player { auto: AutoWeapon },
    /// Normal view with the HUD (UI screens).
    Screen,
    /// A full-screen grid of icons.
    Icons(&'static str),
}

struct Plan {
    subject: Subject,
    steps: Vec<Step>,
    layout: Layout,
}

fn find<T: Copy + std::fmt::Debug>(all: impl IntoIterator<Item = T>, name: &str) -> Option<T> {
    all.into_iter().find(|v| key(*v) == name)
}

fn turntable(extra: Option<Step>) -> Vec<Step> {
    let mut s = vec![
        step("3/4 avant droit", cam(35.0, 15.0, 1.0), Act::None, 0.0),
        step("3/4 arrière droit", cam(125.0, 15.0, 1.0), Act::None, 0.05),
        step("3/4 arrière gauche", cam(215.0, 15.0, 1.0), Act::None, 0.05),
        step("3/4 avant gauche", cam(305.0, 15.0, 1.0), Act::None, 0.05),
        step("plongée", cam(35.0, 60.0, 1.0), Act::None, 0.05),
    ];
    s.push(extra.unwrap_or_else(|| step("gros plan face", cam(15.0, 8.0, 0.55), Act::None, 0.05)));
    s
}

fn model_plan(part: Part) -> Plan {
    Plan {
        subject: Subject::Model {
            part,
            stride: 1.0,
            sink: 0.0,
        },
        steps: turntable(None),
        layout: Layout::Square { tile: 512, cols: 3 },
    }
}

fn mob_plan(kind: MobKind, elite: bool) -> Plan {
    let def = kind.def();
    let walk = def.speed.clamp(2.0, 8.0);
    let mv = |w: f32| Act::Anim {
        action: MobAnim::Move,
        walk: w,
    };
    let side = cam(90.0, 10.0, 1.0);
    let front = cam(35.0, 12.0, 1.0);
    let mut steps = vec![
        step("marche, profil (t)", side, mv(walk), 0.8),
        step("marche, profil (t+0.1s)", side, Act::None, 0.1),
        step("marche, profil (t+0.2s)", side, Act::None, 0.1),
        step("marche, profil (t+0.3s)", side, Act::None, 0.1),
        step("marche, 3/4 avant", front, Act::None, 0.1),
        step("marche, 3/4 arrière", cam(215.0, 12.0, 1.0), Act::None, 0.1),
        step(
            "préparation d'attaque (Windup)",
            front,
            Act::Anim {
                action: MobAnim::Windup,
                walk: 0.0,
            },
            0.35,
        ),
        step(
            "attaque (Attack)",
            front,
            Act::Anim {
                action: MobAnim::Attack,
                walk: 0.0,
            },
            0.12,
        ),
    ];
    let toppled = Act::Anim {
        action: MobAnim::Toppled,
        walk: 0.0,
    };
    steps.push(match kind {
        MobKind::Broodmother => step(
            "4 pattes cassées, au sol (Toppled)",
            front,
            Act::Broken(0b1111),
            0.0,
        ),
        MobKind::AncientGolem => step("à genoux (Toppled)", front, toppled.clone(), 0.6),
        MobKind::Burrower | MobKind::Sandworm => {
            step("sous terre (Burrowed)", front, Act::Burrow(true), 0.8)
        }
        _ => step("plongée, immobile", cam(35.0, 55.0, 1.0), mv(0.0), 0.3),
    });
    if kind == MobKind::Broodmother {
        // The toppled pose comes right after the legs break.
        steps.push(step("au sol (Toppled)", front, toppled, 0.6));
    }
    let part = models::mob_with_elite(kind, elite);
    let sink = if matches!(kind, MobKind::Burrower | MobKind::Sandworm) {
        def.height * 1.15
    } else {
        0.0
    };
    let cols = if steps.len() > 9 { 5 } else { 3 };
    Plan {
        subject: Subject::Model {
            part,
            stride: (def.height * 0.6).max(0.8),
            sink,
        },
        steps,
        layout: Layout::Square { tile: 400, cols },
    }
}

/// A short time series right after an action.
fn timeline(act: Act, view: Cam, times: &[f32]) -> Vec<Step> {
    let mut steps = Vec::new();
    let mut last = 0.0;
    for (i, t) in times.iter().enumerate() {
        let a = if i == 0 { act.clone() } else { Act::None };
        steps.push(step(format!("t = {t:.2} s"), view, a, t - last));
        last = *t;
    }
    steps
}

const FX_TIMES: [f32; 6] = [0.03, 0.1, 0.2, 0.35, 0.6, 1.0];

fn stage_plan(radius: f32, act: Act, view: Cam, times: &[f32]) -> Plan {
    Plan {
        subject: Subject::Stage { radius },
        steps: timeline(act, view, times),
        layout: Layout::Square { tile: 512, cols: 3 },
    }
}

/// Point of the stage where effects happen (relative to the stage center).
const FX_AT: Vec3 = Vec3::new(0.0, 1.2, 0.0);

fn fx_plan(name: &str, rest: &str, at: Vec3) -> Option<Plan> {
    let view = cam(30.0, 18.0, 1.0);
    let x = Vec3::X;
    let hit = |amount: u16, crit: bool, killed: bool, zone: u8| {
        Act::Fx(Fx::Hit {
            pos: at,
            amount,
            crit,
            killed,
            by_slot: 0,
            zone,
        })
    };
    let ev = |e: GameEvent| Act::Event(e);
    let ground = at - Vec3::Y * FX_AT.y;
    let (radius, act) = match name {
        "hit" => (2.5, hit(24, false, false, 0)),
        "crit" => (2.5, hit(61, true, false, 0)),
        "weak" => (2.5, hit(56, false, false, 1)),
        "armor" => (2.5, hit(6, false, false, 2)),
        "kill" => (2.5, hit(40, false, true, 0)),
        "explosion" => (
            5.0,
            Act::Fx(Fx::Explosion {
                pos: at,
                radius: 4.0,
            }),
        ),
        "lightning" => (4.0, Act::Fx(Fx::Lightning { pos: ground })),
        "heal" => (
            6.0,
            Act::Fx(Fx::Heal {
                pos: at,
                radius: 6.0,
            }),
        ),
        "warp" => (
            5.0,
            Act::Fx(Fx::Warp {
                from: at - x * 4.0,
                to: at + x * 4.0,
            }),
        ),
        "arc" => (
            5.0,
            Act::Fx(Fx::Arc {
                from: at - x * 4.0,
                to: at + x * 4.0,
            }),
        ),
        "beam" => (
            5.0,
            Act::Fx(Fx::EnemyBeam {
                from: at - x * 5.0 + Vec3::Y * 2.0,
                to: at + x * 2.0,
            }),
        ),
        "partbroken" => (3.0, Act::Fx(Fx::PartBroken { pos: at })),
        "xp" => (3.0, Act::Fx(Fx::XpCollected { from: at, slot: 0 })),
        "statue" => (5.0, ev(GameEvent::StatueAwakened { pos: ground })),
        "shrine" => (5.0, ev(GameEvent::ShrineCharged { pos: ground })),
        "totem" => (
            5.0,
            ev(GameEvent::TotemActivated {
                pos: ground,
                slot: 0,
                totems: 1,
            }),
        ),
        "levelup" => (4.0, ev(GameEvent::LevelUp { level: 5 })),
        "tracer" => {
            let weapon = find(MainWeapon::ALL, rest)?;
            (
                5.0,
                Act::Fx(Fx::Tracer {
                    slot: 0,
                    weapon,
                    from: at - x * 5.0,
                    to: at + x * 5.0,
                    size: 1.0,
                }),
            )
        }
        "mobattack" => (
            3.0,
            Act::Fx(Fx::MobAttack {
                pos: ground,
                kind: find(all_mobs(), rest)?,
            }),
        ),
        "chestopen" => (
            3.5,
            ev(GameEvent::ChestOpened {
                pos: ground,
                slot: 0,
                kind: find(CHESTS, rest)?,
            }),
        ),
        "powerup" => (
            3.5,
            ev(GameEvent::PowerupTaken {
                slot: 0,
                kind: find(PowerupKind::ALL, rest)?,
                pos: at,
            }),
        ),
        _ => return None,
    };
    Some(stage_plan(radius, act, view, &FX_TIMES))
}

fn solid_part(kind: SolidKind, at: Vec3) -> Part {
    let (shape, h) = match kind {
        SolidKind::Rock => (
            Shape::Box {
                half: Vec3::new(2.0, 1.4, 1.7),
                yaw: 0.3,
            },
            1.4,
        ),
        SolidKind::Pillar => (
            Shape::Cylinder {
                radius: 1.0,
                half_height: 4.0,
            },
            4.0,
        ),
        SolidKind::Platform => (
            Shape::Box {
                half: Vec3::new(3.0, 0.4, 3.0),
                yaw: 0.0,
            },
            0.4,
        ),
        SolidKind::TreeTrunk => (
            Shape::Cylinder {
                radius: 0.5,
                half_height: 3.0,
            },
            3.0,
        ),
        SolidKind::Ruin => (
            Shape::Box {
                half: Vec3::new(2.5, 2.0, 0.6),
                yaw: 0.0,
            },
            2.0,
        ),
        SolidKind::Crystal => (
            Shape::Cylinder {
                radius: 0.7,
                half_height: 2.5,
            },
            2.5,
        ),
    };
    // World-space models: built at the stage, shown with an identity root.
    models::solid(
        &Solid {
            center: at + Vec3::Y * h,
            shape,
            kind,
        },
        0.4,
    )
}

/// The plan of a target, or `None` if the name is unknown.
fn build_plan(target: &str, stage: Vec3) -> Option<Plan> {
    let parts: Vec<&str> = target.split(':').collect();
    let at = |i: usize| parts.get(i).copied().unwrap_or("");
    Some(match at(0) {
        "mob" => mob_plan(find(all_mobs(), at(1))?, at(2) == "elite"),
        "model" => {
            let part = match at(1) {
                "chest" => {
                    let open = step("ouvert", cam(35.0, 28.0, 1.0), Act::Open(true), 0.9);
                    let part = models::chest(find(CHESTS, at(2))?);
                    return Some(Plan {
                        subject: Subject::Model {
                            part,
                            stride: 1.0,
                            sink: 0.0,
                        },
                        steps: turntable(Some(open)),
                        layout: Layout::Square { tile: 512, cols: 3 },
                    });
                }
                "statue" => models::statue(find(MobKind::BOSSES, at(2))?),
                "shrine" => models::shrine(),
                "totem" => models::totem(),
                "gem" => models::xp_gem(5.0),
                "jumppad" => models::jump_pad(Vec3::ZERO, 1.2),
                "player" => models::player(player_color(0)),
                "powerup" => models::powerup(find(PowerupKind::ALL, at(2))?),
                "decor" => models::decor(&Decor {
                    pos: Vec3::ZERO,
                    kind: find(DECORS, at(2))?,
                    scale: 1.0,
                    yaw: 0.0,
                    variant: 0.4,
                }),
                // Solids are built in world space: shift them back to the origin.
                "solid" => Part::pivot().with(solid_part(find(SOLIDS, at(2))?, Vec3::ZERO)),
                "weapon" => models::weapon(find(MainWeapon::ALL, at(2))?, player_color(0)),
                "auto" => models::auto_weapon(find(AutoWeapon::ALL, at(2))?),
                "projectile" => models::projectile(find(PROJECTILES, at(2))?, 1.0),
                _ => return None,
            };
            model_plan(part)
        }
        "fx" => fx_plan(at(1), at(2), stage + FX_AT)?,
        "hazard" => {
            let kind = find(HAZARDS, at(1))?;
            let (radius, life) = match kind {
                HazardKind::Shockwave => (14.0, 3.0),
                HazardKind::Lightning => (LIGHTNING_RADIUS, 0.0),
                HazardKind::Blast => (3.5, 1.2),
                _ => (3.0, 6.0),
            };
            let times = [0.05, 0.3, 0.6, 0.9, 1.3, 1.8];
            stage_plan(
                radius.max(3.0) + 1.0,
                Act::Hazard(kind, radius, life),
                cam(30.0, 35.0, 1.0),
                &times,
            )
        }
        "auto" => {
            let auto = find(AutoWeapon::ALL, at(1))?;
            let times = [0.4, 0.8, 1.2, 1.6, 2.0, 2.4];
            Plan {
                subject: Subject::Player { auto },
                steps: timeline(Act::None, cam(35.0, 35.0, 1.0), &times),
                layout: Layout::Square { tile: 512, cols: 3 },
            }
        }
        "ui" => {
            let full = |w: u32, c: u32| Layout::Full { width: w, cols: c };
            let one = |label: &str, act: Act, wait: f32| Plan {
                subject: Subject::Screen,
                steps: vec![step(label, cam(0.0, 0.0, 1.0), act, wait)],
                layout: full(1600, 1),
            };
            let anim = |act: Act, times: &[f32]| Plan {
                subject: Subject::Screen,
                steps: timeline(act, cam(0.0, 0.0, 1.0), times),
                layout: full(800, 2),
            };
            match at(1) {
                "hud" => one("HUD en jeu", Act::None, 0.5),
                "cards" => anim(Act::Cards(false), &[0.05, 0.15, 0.3, 0.8]),
                "chestcards" => anim(Act::Cards(true), &[0.05, 0.15, 0.3, 0.8]),
                "toasts" => anim(Act::Toasts, &[0.15, 0.8, 3.0, 6.3]),
                "gameover" => one("fin de partie", Act::GameOver, 1.5),
                "pause" => one("menu pause", Act::Pause, 0.6),
                "lobby" => one("salon", Act::None, 0.0),
                "menu" => one("menu principal", Act::None, 0.0),
                _ => return None,
            }
        }
        "icons" => {
            let set = ICON_SETS.into_iter().find(|s| *s == at(1))?;
            Plan {
                subject: Subject::Icons(set),
                steps: vec![step(
                    format!("icônes : {set}"),
                    cam(0.0, 0.0, 1.0),
                    Act::None,
                    0.5,
                )],
                layout: Layout::Full {
                    width: 1600,
                    cols: 1,
                },
            }
        }
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Runtime
// ---------------------------------------------------------------------------

/// Captured frames, filled by the screenshot observers.
type Frames = Arc<Mutex<Vec<Option<Image>>>>;

#[derive(Resource)]
struct Studio {
    target: String,
    out: PathBuf,
    plan: Option<Plan>,
    /// Real time when the scene was ready (None = not yet).
    ready_at: Option<f32>,
    started: bool,
    step: usize,
    /// Real time of the current step's action.
    step_at: f32,
    shot_requested: bool,
    /// The action of the current step was done.
    applied: bool,
    frames: Frames,
    stage: Vec3,
    /// Point the camera looks at, and the radius it must fit.
    focus: Vec3,
    radius: f32,
    cam: Cam,
    scene_mode: bool,
    done: bool,
}

/// The shown model (its animation is driven by the studio).
#[derive(Component)]
struct Subject3d;

/// Animation wanted for the subject.
#[derive(Component, Default)]
struct StudioAnim {
    action: MobAnim,
    walk: f32,
    phase: f32,
    broken: u8,
    open: bool,
    burrow: bool,
    sink: f32,
}

#[derive(Component)]
struct StudioFill;

pub struct StudioPlugin;

impl Plugin for StudioPlugin {
    fn build(&self, app: &mut App) {
        let enabled = |c: Res<AppConfig>| c.studio.is_some();
        app.add_systems(Startup, init.run_if(enabled))
            .add_systems(Update, (prepare, run_plan, finish).chain().run_if(enabled))
            .add_systems(
                Update,
                drive_anim
                    .after(super::avatars::update_anim_states)
                    .before(super::anim::animate_parts)
                    .run_if(enabled),
            )
            .add_systems(
                PostUpdate,
                studio_camera
                    .after(super::camera::CameraSystems)
                    .before(bevy::transform::TransformSystems::Propagate)
                    .run_if(enabled),
            );
    }
}

fn init(mut commands: Commands, config: Res<AppConfig>) {
    let target = config.studio.clone().unwrap_or_default().to_lowercase();
    let out = config
        .studio_out
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let name: String = target
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                .collect();
            PathBuf::from("studio_out").join(format!("{name}.png"))
        });
    if build_plan(&target, Vec3::ZERO).is_none() {
        error!("studio: cible inconnue « {target} » (voir `prism_rush studio list`)");
        println!("STUDIO ERROR: unknown target {target} (run `prism_rush studio list`)");
        std::process::exit(2);
    }
    commands.insert_resource(Studio {
        target,
        out,
        plan: None,
        ready_at: None,
        started: false,
        step: 0,
        step_at: 0.0,
        shot_requested: false,
        applied: false,
        frames: Arc::new(Mutex::new(Vec::new())),
        stage: Vec3::ZERO,
        focus: Vec3::ZERO,
        radius: 3.0,
        cam: cam(35.0, 15.0, 1.0),
        scene_mode: false,
        done: false,
    });
}

/// Waits for the right screen, then builds the scene of the plan.
#[allow(clippy::too_many_arguments)]
fn prepare(
    mut commands: Commands,
    mut studio: ResMut<Studio>,
    time: Res<Time<Real>>,
    state: Res<State<AppState>>,
    local: Res<crate::net::LocalPlayer>,
    mut players: Query<(Entity, &Player, &PlayerState, &mut Gear, &mut Items)>,
    terrain: Res<crate::game::world::TerrainRes>,
    icons: Res<super::icons::Icons>,
    mut cache: ResMut<PrimCache>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if studio.started {
        return;
    }
    let wanted = match studio.target.as_str() {
        "ui:menu" => AppState::Menu,
        "ui:lobby" => AppState::Lobby,
        _ => AppState::InGame,
    };
    let in_game = *state.get() == AppState::InGame;
    let ready = *state.get() == wanted
        && (!in_game || players.iter().any(|(_, p, ..)| Some(p.slot) == local.slot));
    if !ready {
        studio.ready_at = None;
        return;
    }
    let now = time.elapsed_secs();
    let ready_at = *studio.ready_at.get_or_insert(now);
    if now - ready_at < WARMUP {
        return;
    }
    let stage = Vec3::new(
        0.0,
        terrain.wgen.ground_height(0.0, 0.0) + STAGE_HEIGHT,
        0.0,
    );
    let Some(plan) = build_plan(&studio.target, stage) else {
        return;
    };
    studio.stage = stage;
    studio.started = true;
    let plinth = |commands: &mut Commands,
                  cache: &mut PrimCache,
                  meshes: &mut Assets<Mesh>,
                  materials: &mut Assets<StandardMaterial>,
                  y: f32,
                  r: f32| {
        let part = Part::cylinder(r * 2.0, 0.2)
            .at(0.0, y - 0.1, 0.0)
            .color(Color::srgb(0.5, 0.5, 0.55))
            .with(
                Part::cylinder(r * 2.04, 0.04)
                    .at(0.0, 0.1, 0.0)
                    .color(Color::srgb(0.38, 0.38, 0.43)),
            );
        let root = commands
            .spawn((Transform::from_translation(stage), Visibility::default()))
            .id();
        spawn_model(commands, cache, meshes, materials, root, root, &part, ());
    };
    match &plan.subject {
        Subject::Model { part, stride, sink } => {
            // Bounds of the model, to frame it and size the plinth.
            let mut buf = MeshBuffer::default();
            let mut glow = MeshBuffer::default();
            buf.add_part(&mut cache, part, &Transform::IDENTITY, &mut glow);
            let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
            // Lit parts, plus the glowing parts close to them (eyes, flames),
            // but not the long light beams of chests.
            let tris = if buf.tris.is_empty() { &glow.tris } else { &buf.tris };
            for v in tris.iter().flatten() {
                lo = lo.min(*v);
                hi = hi.max(*v);
            }
            let (near_lo, near_hi) = (lo - Vec3::splat(1.5), hi + Vec3::splat(1.5));
            for v in glow.tris.iter().flatten() {
                if v.cmpge(near_lo).all() && v.cmple(near_hi).all() {
                    lo = lo.min(*v);
                    hi = hi.max(*v);
                }
            }
            if lo.x > hi.x {
                (lo, hi) = (Vec3::splat(-0.5), Vec3::splat(0.5));
            }
            studio.focus = stage + (lo + hi) * 0.5;
            studio.radius = ((hi - lo) * 0.5).length().max(0.3);
            let root = commands
                .spawn((
                    Transform::from_translation(stage),
                    Visibility::default(),
                    Subject3d,
                    StudioAnim {
                        sink: *sink,
                        ..default()
                    },
                ))
                .id();
            let model = commands
                .spawn((Transform::IDENTITY, Visibility::default(), ChildOf(root)))
                .id();
            spawn_model(
                &mut commands,
                &mut cache,
                &mut meshes,
                &mut materials,
                root,
                model,
                part,
                (),
            );
            commands
                .entity(root)
                .insert((HasModel { model }, AnimState::with_stride(*stride)));
            let r = Vec2::new(hi.x - lo.x, hi.z - lo.z).length() * 0.5 * 1.1 + 0.2;
            plinth(
                &mut commands,
                &mut cache,
                &mut meshes,
                &mut materials,
                lo.y.min(0.0),
                r,
            );
            studio.scene_mode = true;
        }
        Subject::Stage { radius } => {
            studio.focus = stage + Vec3::Y * 1.0;
            studio.radius = *radius;
            plinth(
                &mut commands,
                &mut cache,
                &mut meshes,
                &mut materials,
                0.0,
                radius * 1.1,
            );
            studio.scene_mode = true;
        }
        Subject::Player { auto } => {
            if let Some((_, _, state, mut gear, _)) = players
                .iter_mut()
                .find(|(_, p, ..)| Some(p.slot) == local.slot)
            {
                gear.0.autos = vec![(
                    *auto,
                    WeaponLevel {
                        level: 5,
                        ..default()
                    },
                )];
                let center = state.sim.mv.pos;
                studio.focus = center + Vec3::Y * 1.0;
                studio.radius = 8.0;
                // Dummies to shoot at.
                for i in 0..6 {
                    let a = i as f32 / 6.0 * std::f32::consts::TAU;
                    let d = 6.0 + (i % 2) as f32 * 3.0;
                    let (x, z) = (center.x + a.cos() * d, center.z + a.sin() * d);
                    let p = Vec3::new(x, terrain.wgen.ground_height(x, z) + 0.2, z);
                    let e = crate::game::mobs::spawn_mob(
                        &mut commands,
                        MobKind::Brute,
                        p,
                        1,
                        0,
                        a,
                        false,
                    );
                    commands.entity(e).insert(crate::game::mobs::Frozen);
                }
            }
            studio.scene_mode = true;
        }
        Subject::Screen => {
            if studio.target == "ui:hud"
                && let Some((_, _, _, mut gear, mut items)) = players
                    .iter_mut()
                    .find(|(_, p, ..)| Some(p.slot) == local.slot)
            {
                for a in [AutoWeapon::Aura, AutoWeapon::Orbs, AutoWeapon::Lightning] {
                    if !gear.0.has_auto(a) {
                        gear.0.autos.push((
                            a,
                            WeaponLevel {
                                level: 3,
                                ..default()
                            },
                        ));
                    }
                }
                for item in [
                    ItemKind::FrogLegs,
                    ItemKind::FrogLegs,
                    ItemKind::Jetpack,
                    ItemKind::VampireFangs,
                    ItemKind::Crown,
                ] {
                    items.0.add(item);
                }
            }
        }
        Subject::Icons(set) => spawn_icon_grid(&mut commands, &icons, set),
    }
    *studio.frames.lock().expect("frames") = vec![None; plan.steps.len()];
    studio.plan = Some(plan);
    studio.step_at = now;
    info!(
        "studio: {} ({} vues) -> {}",
        studio.target,
        studio.plan.as_ref().map(|p| p.steps.len()).unwrap_or(0),
        studio.out.display()
    );
}

/// A full-screen grid of every icon of a set, with names.
fn spawn_icon_grid(commands: &mut Commands, icons: &super::icons::Icons, set: &str) {
    let mut list: Vec<(Handle<Image>, String)> = Vec::new();
    match set {
        "items" => {
            for i in ItemKind::ALL {
                list.push((icons.items[&i].clone(), i.name().to_string()));
            }
        }
        "weapons" => {
            for w in MainWeapon::ALL {
                list.push((icons.mains[&w].clone(), w.def().name.to_string()));
            }
            for a in AutoWeapon::ALL {
                list.push((icons.autos[&a].clone(), a.def().name.to_string()));
            }
        }
        _ => {
            for t in Tome::ALL {
                list.push((icons.tomes[&t].clone(), t.name().to_string()));
            }
        }
    }
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_wrap: FlexWrap::Wrap,
                align_content: AlignContent::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(8.0),
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(16.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.12, 0.1, 0.16)),
            GlobalZIndex(100),
        ))
        .id();
    for (icon, name) in list {
        commands.spawn((
            Node {
                width: Val::Px(140.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            ChildOf(root),
            children![
                (
                    ImageNode::new(icon),
                    Node {
                        width: Val::Px(96.0),
                        height: Val::Px(96.0),
                        ..default()
                    }
                ),
                (
                    Text::new(name),
                    crate::ui::widgets::font(14.0),
                    TextColor(Color::WHITE),
                    TextLayout::justify(Justify::Center)
                ),
            ],
        ));
    }
}

/// Steps through the plan: action, wait, screenshot, next step.
#[allow(clippy::too_many_arguments)]
fn run_plan(
    mut commands: Commands,
    mut studio: ResMut<Studio>,
    time: Res<Time<Real>>,
    tick: Res<crate::game::SimTick>,
    local: Res<crate::net::LocalPlayer>,
    mut subject: Query<&mut StudioAnim, With<Subject3d>>,
    mut players: Query<(Entity, &Player, &Gear, &Items)>,
    mut info: Query<&mut MatchInfo>,
    mut fx: MessageWriter<Fx>,
    mut events: MessageWriter<GameEvent>,
    mut server_events: MessageWriter<ToClients<GameEvent>>,
    mut paused: ResMut<crate::ui::Paused>,
) {
    if !studio.started || studio.done {
        return;
    }
    let now = time.elapsed_secs();
    let i = studio.step;
    let Some((act, wait, cam, label)) = studio
        .plan
        .as_ref()
        .and_then(|p| p.steps.get(i))
        .map(|s| (s.act.clone(), s.wait, s.cam, s.label.clone()))
    else {
        return;
    };
    if !studio.applied {
        studio.applied = true;
        studio.step_at = now;
        studio.cam = cam;
        apply(
            &mut commands,
            &mut studio,
            act,
            &tick,
            &local,
            &mut subject,
            &mut players,
            &mut info,
            &mut fx,
            &mut events,
            &mut server_events,
            &mut paused,
        );
        return;
    }
    if !studio.shot_requested {
        if now - studio.step_at >= wait {
            let frames = studio.frames.clone();
            commands.spawn(Screenshot::primary_window()).observe(
                move |shot: On<ScreenshotCaptured>| {
                    if let Some(slot) = frames.lock().expect("frames").get_mut(i) {
                        *slot = Some(shot.image.clone());
                    }
                },
            );
            info!("studio: vue {} — {label}", i + 1);
            studio.shot_requested = true;
        }
        return;
    }
    if studio
        .frames
        .lock()
        .expect("frames")
        .get(i)
        .is_some_and(|f| f.is_some())
    {
        studio.step += 1;
        studio.shot_requested = false;
        studio.applied = false;
    }
}

#[allow(clippy::too_many_arguments)]
fn apply(
    commands: &mut Commands,
    studio: &mut Studio,
    act: Act,
    tick: &crate::game::SimTick,
    local: &crate::net::LocalPlayer,
    subject: &mut Query<&mut StudioAnim, With<Subject3d>>,
    players: &mut Query<(Entity, &Player, &Gear, &Items)>,
    info: &mut Query<&mut MatchInfo>,
    fx: &mut MessageWriter<Fx>,
    events: &mut MessageWriter<GameEvent>,
    server_events: &mut MessageWriter<ToClients<GameEvent>>,
    paused: &mut crate::ui::Paused,
) {
    let mut anim = |f: &mut dyn FnMut(&mut StudioAnim)| {
        for mut a in subject.iter_mut() {
            f(&mut a);
        }
    };
    match act {
        Act::None => {}
        Act::Anim { action, walk } => anim(&mut |a| {
            a.action = action;
            a.walk = walk;
        }),
        Act::Broken(b) => anim(&mut |a| a.broken = b),
        Act::Burrow(b) => anim(&mut |a| a.burrow = b),
        Act::Open(o) => anim(&mut |a| a.open = o),
        Act::Fx(f) => {
            fx.write(f);
        }
        Act::Event(e) => {
            events.write(e);
        }
        Act::Hazard(kind, radius, life) => {
            let mut h = Hazard::enemy(kind, studio.stage, tick.0, radius, life);
            if kind == HazardKind::Tornado {
                h.vel = Vec3::X * 2.0;
            }
            // Server logic too (explosions, life time), harmless here.
            commands.spawn((h, crate::game::combat::HazardServer::new(0.0)));
        }
        Act::Cards(chest) => {
            let Some((e, _, gear, items)) = players
                .iter_mut()
                .find(|(_, p, ..)| Some(p.slot) == local.slot)
            else {
                return;
            };
            let mut rng = crate::shared::rng::Rng::new(7);
            let choice = if chest {
                let options = crate::shared::items::roll_cards(ChestKind::Large, 3.0, 3, &mut rng)
                    .into_iter()
                    .map(CardOption::Item)
                    .collect();
                PendingChoice {
                    chest: Some(ChestKind::Large),
                    options,
                }
            } else {
                let options = crate::shared::build::roll_offers(
                    &gear.0,
                    &items.0,
                    &AutoWeapon::ALL,
                    4.0,
                    3,
                    &mut rng,
                )
                .into_iter()
                .map(CardOption::Offer)
                .collect();
                PendingChoice {
                    chest: None,
                    options,
                }
            };
            commands.entity(e).insert(choice);
        }
        Act::Toasts => {
            let slot = local.slot.unwrap_or(0);
            for e in [
                GameEvent::LevelUp { level: 7 },
                GameEvent::Gained {
                    slot,
                    reward: Reward::Item(ItemKind::Crown),
                },
                GameEvent::NewMonster {
                    kind: MobKind::Scarab,
                },
                GameEvent::TotemActivated {
                    pos: Vec3::ZERO,
                    slot,
                    totems: 2,
                },
                GameEvent::BossSpawned,
            ] {
                events.write(e);
            }
        }
        Act::GameOver => {
            let Ok(mut m) = info.single_mut() else { return };
            m.phase = Phase::GameOver;
            let summary = RunSummary {
                duration: 754.0,
                level: 18,
                rules: m.rules,
                players: (0..3u8)
                    .map(|s| PlayerSummary {
                        slot: s,
                        name: ["Soki", "Zoé", "Bot Alpha"][s as usize].into(),
                        kills: 412 - u32::from(s) * 90,
                        damage: 48_000.0 - f32::from(s) * 9_000.0,
                        gold_earned: 1_800,
                        chests: 14,
                        revives: u32::from(s),
                        downs: 1,
                        items: 22,
                    })
                    .collect(),
            };
            server_events.write(ToClients {
                targets: SendTargets::All,
                message: GameEvent::GameOver(Box::new(summary)),
            });
        }
        Act::Pause => paused.0 = true,
    }
}

/// Writes the contact sheet once every frame is captured, then quits.
fn finish(mut studio: ResMut<Studio>, mut exit: MessageWriter<AppExit>) {
    if studio.done || !studio.started {
        return;
    }
    let Some(plan) = studio.plan.as_ref() else {
        return;
    };
    let frames: Vec<Image> = {
        let lock = studio.frames.lock().expect("frames");
        if lock.iter().any(|f| f.is_none()) {
            return;
        }
        lock.iter().flatten().cloned().collect()
    };
    let labels: Vec<String> = plan.steps.iter().map(|s| s.label.clone()).collect();
    let layout = plan.layout;
    studio.done = true;
    match compose(frames, layout) {
        Ok(sheet) => {
            if let Some(dir) = studio.out.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            match sheet.save(&studio.out) {
                Ok(()) => {
                    let cols = match layout {
                        Layout::Square { cols, .. } | Layout::Full { cols, .. } => cols as usize,
                    };
                    let mut txt = format!(
                        "{}  ({} vues, {} par ligne, ordre de lecture)\n",
                        studio.target,
                        labels.len(),
                        cols
                    );
                    for (i, l) in labels.iter().enumerate() {
                        txt.push_str(&format!(
                            "{:>2}. [ligne {}, col {}] {l}\n",
                            i + 1,
                            i / cols + 1,
                            i % cols + 1
                        ));
                    }
                    let _ = std::fs::write(studio.out.with_extension("txt"), &txt);
                    println!("{txt}STUDIO OUTPUT: {}", studio.out.display());
                    info!("studio: planche enregistrée -> {}", studio.out.display());
                }
                Err(e) => println!("STUDIO ERROR: cannot save {}: {e}", studio.out.display()),
            }
        }
        Err(e) => println!("STUDIO ERROR: {e}"),
    }
    exit.write(AppExit::Success);
}

/// Tiles the frames into one image.
fn compose(frames: Vec<Image>, layout: Layout) -> Result<image::RgbaImage, String> {
    use image::imageops::{self, FilterType};
    let mut tiles = Vec::new();
    for f in frames {
        let img = f
            .try_into_dynamic()
            .map_err(|e| format!("screenshot: {e:?}"))?
            .to_rgba8();
        let (w, h) = img.dimensions();
        let tile = match layout {
            Layout::Square { tile, .. } => {
                let side = w.min(h);
                let crop =
                    imageops::crop_imm(&img, (w - side) / 2, (h - side) / 2, side, side).to_image();
                imageops::resize(&crop, tile, tile, FilterType::Triangle)
            }
            Layout::Full { width, .. } if width >= w => img,
            Layout::Full { width, .. } => {
                imageops::resize(&img, width, h * width / w, FilterType::Triangle)
            }
        };
        tiles.push(tile);
    }
    let cols = match layout {
        Layout::Square { cols, .. } | Layout::Full { cols, .. } => cols,
    }
    .min(tiles.len() as u32)
    .max(1);
    let (tw, th) = tiles.first().map(|t| t.dimensions()).ok_or("no frame")?;
    let rows = (tiles.len() as u32).div_ceil(cols);
    let gap = 4;
    let mut sheet = image::RgbaImage::from_pixel(
        cols * tw + (cols - 1) * gap,
        rows * th + (rows - 1) * gap,
        image::Rgba([20, 18, 26, 255]),
    );
    for (i, t) in tiles.iter().enumerate() {
        let (c, r) = (i as u32 % cols, i as u32 / cols);
        imageops::overlay(
            &mut sheet,
            t,
            i64::from(c * (tw + gap)),
            i64::from(r * (th + gap)),
        );
    }
    Ok(sheet)
}

/// Studio camera: narrow lens around the subject (scene modes only).
#[allow(clippy::too_many_arguments)]
fn studio_camera(
    mut commands: Commands,
    studio: Option<Res<Studio>>,
    mut camera: Query<(&mut Transform, &mut Projection), With<MainCamera>>,
    mut vm: Query<&mut Camera, (With<ViewModelCamera>, Without<MainCamera>)>,
    mut hud: Query<&mut Visibility, With<crate::ui::hud::HudRoot>>,
    mut fill: Query<(&mut Transform, &mut PointLight), (With<StudioFill>, Without<MainCamera>)>,
) {
    let Some(studio) = studio else { return };
    if !studio.started || !studio.scene_mode {
        return;
    }
    let fov: f32 = 30.0;
    let Ok((mut t, mut proj)) = camera.single_mut() else {
        return;
    };
    if let Projection::Perspective(p) = proj.as_mut() {
        p.fov = fov.to_radians();
    }
    let c = studio.cam;
    let (yaw, pitch) = (c.yaw.to_radians(), c.pitch.to_radians());
    let dir = Vec3::new(
        yaw.sin() * pitch.cos(),
        pitch.sin(),
        -yaw.cos() * pitch.cos(),
    );
    let dist = studio.radius / (fov.to_radians() * 0.5).sin() * 0.85 * c.zoom;
    let eye = studio.focus + dir * dist;
    *t = Transform::from_translation(eye).looking_at(studio.focus, Vec3::Y);
    for mut cam in &mut vm {
        cam.is_active = false;
    }
    for mut v in &mut hud {
        *v = Visibility::Hidden;
    }
    // Soft fill light from the camera side: the sun alone leaves the front
    // of the models in the shade.
    let lux = 1_500.0;
    let pos = eye + Vec3::Y * dist * 0.3;
    let d2 = (pos - studio.focus).length_squared();
    match fill.single_mut() {
        Ok((mut ft, mut light)) => {
            ft.translation = pos;
            light.intensity = lux * 4.0 * std::f32::consts::PI * d2;
            light.range = dist * 4.0;
        }
        Err(_) => {
            commands.spawn((
                StudioFill,
                PointLight {
                    intensity: lux * 4.0 * std::f32::consts::PI * d2,
                    range: dist * 4.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::from_translation(pos),
            ));
        }
    }
}

/// Poses the subject as asked by the plan (after the game's own anim update).
fn drive_anim(
    time: Res<Time>,
    mut q: Query<(&mut StudioAnim, &mut AnimState, &HasModel)>,
    mut models: Query<&mut Transform, Without<HasModel>>,
) {
    let dt = time.delta_secs();
    for (mut s, mut st, has) in &mut q {
        if st.action != s.action {
            st.action = s.action;
            st.action_time = 0.0;
        }
        s.phase += dt * s.walk / st.stride.max(0.3) * 1.6;
        st.phase = s.phase;
        st.speed = s.walk;
        st.broken = s.broken;
        let k = 1.0 - (-dt * 6.0).exp();
        st.open += (if s.open { 1.0 } else { 0.0 } - st.open) * k;
        st.burrow += (if s.burrow { 1.0 } else { 0.0 } - st.burrow) * k;
        if s.sink > 0.0
            && let Ok(mut m) = models.get_mut(has.model)
        {
            m.translation.y = -s.sink * st.burrow;
        }
    }
}
