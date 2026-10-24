//! Every model of the game, described with the `Part` builder.
//!
//! Conventions: origin at the feet (y = 0), facing -Z (Bevy's forward).
//! Sizes are in meters and match the gameplay collision sizes.
//!
//! Design notes (silhouette first — each monster must be recognizable from
//! its black outline alone):
//! - Skitter: wide and low, many legs (spider).
//! - Spitter: tall thin stalk with a big cannon-head (turret plant).
//! - Wisp: floating glowing orb with rings (nothing touches the ground).
//! - Bomber: a ball with a lit fuse (cartoon bomb with feet).
//! - Brute: top-heavy gorilla, enormous arms.
//! - Colossus: giant rock golem with a halo of floating stones.

use bevy::prelude::*;

use super::model::{AnimTag, Part};
use super::monsters;
use super::palette::*;
use crate::net::protocol::{PowerupKind, ProjectileKind, SHRINE_RADIUS};
use crate::shared::economy::ChestKind;
use crate::shared::mobs::MobKind;
use crate::shared::build::{AutoWeapon, MainWeapon};
use crate::shared::worldgen::{Decor, DecorKind, Shape, Solid, SolidKind};

// ---------------------------------------------------------------------------
// Players
// ---------------------------------------------------------------------------

pub fn player(color: Color) -> Part {
    let leg_l = Part::pivot()
        .at(0.17, 0.9, 0.0)
        .tag(AnimTag::LegL)
        .with(Part::cylinder(0.24, 0.5).at(0.0, -0.25, 0.0).color(SUIT_DARK))
        .with(Part::cylinder(0.21, 0.42).at(0.0, -0.64, 0.0).color(color))
        .with(Part::cube(0.26, 0.16, 0.38).at(0.0, -0.84, -0.05).color(ARMOR_WHITE));

    let arm_l = Part::pivot()
        .at(0.4, 0.62, 0.0)
        .tag(AnimTag::ArmL)
        .with(Part::sphere(0.26).color(color))
        .with(Part::cylinder(0.17, 0.42).at(0.0, -0.26, 0.0).color(SUIT_DARK))
        .with(Part::sphere(0.17).at(0.0, -0.5, 0.0).color(ARMOR_WHITE));
    // Right arm = mirrored left arm + a small gun in the hand.
    let arm_r = arm_l
        .mirrored_x()
        .with(Part::cube(0.1, 0.14, 0.42).at(0.0, -0.5, -0.2).color(METAL_DARK))
        .with(Part::cube(0.03, 0.03, 0.1).at(0.0, -0.42, -0.36).color(color).glow(2.0));

    let head = Part::pivot()
        .at(0.0, 0.72, 0.0)
        .tag(AnimTag::Head)
        .with(Part::sphere(0.46).at(0.0, 0.2, 0.0).color(ARMOR_WHITE))
        .with(Part::cube(0.36, 0.14, 0.12).at(0.0, 0.22, -0.18).color(color).glow(3.0))
        .with(Part::cylinder(0.03, 0.26).at(0.16, 0.48, 0.06).color(SUIT_DARK))
        .with(Part::ico(0.08, 0.08, 0.08).at(0.16, 0.62, 0.06).color(color).glow(3.0));

    let body = Part::pivot()
        .at(0.0, 0.9, 0.0)
        .tag(AnimTag::Body)
        .with(Part::cube(0.5, 0.2, 0.3).at(0.0, 0.05, 0.0).color(SUIT_DARK))
        .with(Part::cube(0.64, 0.55, 0.38).at(0.0, 0.42, 0.0).color(color))
        .with(Part::cube(0.46, 0.3, 0.06).at(0.0, 0.48, -0.2).color(ARMOR_WHITE))
        .with(Part::ico(0.12, 0.12, 0.06).at(0.0, 0.5, -0.24).color(color).glow(2.5))
        .with(Part::cube(0.44, 0.5, 0.22).at(0.0, 0.42, 0.28).color(SUIT_DARK))
        .with(Part::cube(0.06, 0.38, 0.03).at(0.0, 0.42, 0.4).color(color).glow(2.5))
        .with(head)
        .with(arm_l)
        .with(arm_r);

    Part::pivot().with(leg_l.mirrored_x()).with(leg_l).with(body)
}

// ---------------------------------------------------------------------------
// Monsters
// ---------------------------------------------------------------------------

/// A monster model, with the elite look (bigger, golden ring and crown).
pub fn mob_with_elite(kind: MobKind, elite: bool) -> Part {
    if !elite {
        return mob(kind);
    }
    let def = kind.def();
    scale_part(mob(kind), 1.3)
        .with(Part::torus(def.radius * 3.0, 0.1).at(0.0, 0.15, 0.0).color(GOLD).glow(4.0).tag(AnimTag::Pulse))
        .with(Part::cone(0.3, 0.35).at(0.0, def.height * 1.3 + 0.3, 0.0).color(GOLD).glow(2.0).tag(AnimTag::Bob))
}

pub fn mob(kind: MobKind) -> Part {
    match kind {
        MobKind::Skitter => skitter(),
        MobKind::Spitter => spitter(),
        MobKind::Wisp => wisp(),
        MobKind::Bomber => bomber(),
        MobKind::Brute => brute(),
        MobKind::Colossus => colossus(),
        MobKind::Charger => charger(),
        MobKind::Jelly => jelly(),
        MobKind::Jellet => scale_part(jelly(), 0.5),
        MobKind::Shaman => shaman(),
        MobKind::HiveQueen => hive_queen(),
        MobKind::StormTitan => storm_titan(),
        MobKind::WhiteGhost => ghost(false),
        MobKind::BlackGhost => ghost(true),
        MobKind::Flea => monsters::flea(),
        MobKind::Burrower => monsters::burrower(),
        MobKind::Scarab => monsters::scarab(),
        MobKind::ToxicJelly => monsters::toxic_jelly(),
        MobKind::MagmaJelly => monsters::magma_jelly(),
        MobKind::MegaJelly => monsters::mega_jelly(),
        MobKind::Mortar => monsters::mortar(),
        MobKind::Eye => monsters::eye(),
        MobKind::Thornback => monsters::thornback(),
        MobKind::SkyJelly => monsters::sky_jelly(),
        MobKind::Sporeling => monsters::sporeling(),
        MobKind::FireBat => monsters::fire_bat(),
        MobKind::StoneGolem => monsters::stone_golem(),
        MobKind::CrystalGolem => monsters::crystal_golem(),
        MobKind::ShieldKnight => monsters::shield_knight(),
        MobKind::Summoner => monsters::summoner(),
        MobKind::LavaBoar => monsters::lava_boar(),
        MobKind::Broodmother => monsters::broodmother(),
        MobKind::AncientGolem => monsters::ancient_golem(),
        MobKind::Sandworm => monsters::sandworm(),
    }
}

/// Final Swarm ghost: a floating sheet with a wavy tail and empty eyes.
/// White ghosts glow softly; black ghosts are dark with red eyes and horns.
fn ghost(black: bool) -> Part {
    let (body, glow, eyes, eye_glow) = if black {
        (rgb(0.08, 0.05, 0.12), 0.0, GLOW_MAGENTA, 6.0)
    } else {
        (rgb(0.92, 0.95, 1.0), 0.8, rgb(0.05, 0.05, 0.1), 0.0)
    };
    let mut tail = Part::pivot().at(0.0, 0.35, 0.0).tag(AnimTag::Wing(0.0));
    for i in 0..6 {
        let a = i as f32 / 6.0 * std::f32::consts::TAU;
        tail = tail.with(Part::cone(0.3, 0.6).at(a.cos() * 0.4, -0.25, a.sin() * 0.4).rot_x(180.0).color(body).glow(glow));
    }
    let mut ghost = Part::pivot()
        .at(0.0, 0.0, 0.0)
        .tag(AnimTag::Bob)
        .with(Part::sphere(1.1).at(0.0, 1.0, 0.0).color(body).glow(glow))
        .with(Part::frustum(1.15, 0.7).at(0.0, 0.5, 0.0).rot_x(180.0).color(body).glow(glow))
        .with(tail)
        .with(Part::ellipsoid(0.2, 0.32, 0.1).at(0.2, 1.1, -0.5).color(eyes).glow(eye_glow))
        .with(Part::ellipsoid(0.2, 0.32, 0.1).at(-0.2, 1.1, -0.5).color(eyes).glow(eye_glow))
        .with(Part::ellipsoid(0.25, 0.15, 0.1).at(0.0, 0.8, -0.52).color(eyes).glow(eye_glow).tag(AnimTag::Jaw))
        .with(
            Part::pivot()
                .at(0.55, 0.9, -0.1)
                .tag(AnimTag::ArmL)
                .with(Part::cone(0.25, 0.6).at(0.0, 0.0, -0.3).rot_x(-80.0).color(body).glow(glow)),
        )
        .with(
            Part::pivot()
                .at(-0.55, 0.9, -0.1)
                .tag(AnimTag::ArmR)
                .with(Part::cone(0.25, 0.6).at(0.0, 0.0, -0.3).rot_x(-80.0).color(body).glow(glow)),
        );
    if black {
        ghost = ghost
            .with(Part::cone(0.18, 0.5).at(0.32, 1.55, 0.0).rot_z(-25.0).color(PLUM_DARK))
            .with(Part::cone(0.18, 0.5).at(-0.32, 1.55, 0.0).rot_z(25.0).color(PLUM_DARK))
            .with(Part::torus(1.6, 0.06).at(0.0, 0.2, 0.0).color(GLOW_MAGENTA).glow(3.0).tag(AnimTag::Spin(4.0)));
    }
    Part::pivot().with(ghost)
}

/// XP gem: a small spinning cyan crystal.
pub fn xp_gem(value: f32) -> Part {
    let s = (0.25 + value.sqrt() * 0.05).min(0.6);
    let color = if value >= 15.0 { GLOW_MAGENTA } else if value >= 6.0 { GOLD } else { GLOW_CYAN };
    Part::pivot().with(
        Part::pivot()
            .tag(AnimTag::Bob)
            .with(Part::pivot().tag(AnimTag::Spin(3.0)).with(Part::ico(s, s * 1.6, s).color(color).glow(3.0))),
    )
}

/// Crystal rhino: low, wide, a big glowing horn. Reads as "it will charge".
fn charger() -> Part {
    let leg = |x: f32, z: f32, tag: AnimTag| {
        Part::pivot()
            .at(x, 0.75, z)
            .tag(tag)
            .with(Part::cylinder(0.34, 0.75).at(0.0, -0.37, 0.0).color(PLUM_DARK))
            .with(Part::cube(0.4, 0.15, 0.45).at(0.0, -0.72, -0.05).color(STONE))
    };
    let head = Part::pivot()
        .at(0.0, 0.95, -1.05)
        .tag(AnimTag::Head)
        .with(Part::cube(0.8, 0.6, 0.75).color(PLUM))
        .with(Part::cone(0.38, 1.0).at(0.0, 0.25, -0.45).rot_x(-65.0).color(CRYSTAL_CYAN).glow(3.0))
        .with(Part::cone(0.2, 0.45).at(0.0, 0.42, -0.05).rot_x(-30.0).color(CRYSTAL_CYAN).glow(3.0))
        .with(Part::ico(0.12, 0.12, 0.12).at(0.28, 0.1, -0.38).color(GLOW_ORANGE).glow(5.0))
        .with(Part::ico(0.12, 0.12, 0.12).at(-0.28, 0.1, -0.38).color(GLOW_ORANGE).glow(5.0));
    let body = Part::pivot()
        .tag(AnimTag::Body)
        .with(Part::ellipsoid(1.5, 1.1, 2.1).at(0.0, 0.95, 0.0).color(PLUM))
        .with(Part::cube(1.1, 0.25, 1.4).at(0.0, 1.5, 0.1).rot_x(-6.0).color(STONE))
        .with(Part::ico(0.4, 0.5, 0.4).at(0.3, 1.65, 0.3).color(CRYSTAL_CYAN).glow(2.0))
        .with(Part::ico(0.3, 0.45, 0.3).at(-0.3, 1.62, -0.1).color(CRYSTAL_CYAN).glow(2.0))
        .with(Part::cone(0.15, 0.4).at(0.0, 0.9, 1.1).rot_x(110.0).color(PLUM_DARK))
        .with(head);
    Part::pivot()
        .with(leg(0.5, -0.65, AnimTag::LegL))
        .with(leg(-0.5, -0.65, AnimTag::LegR))
        .with(leg(0.5, 0.65, AnimTag::LegR))
        .with(leg(-0.5, 0.65, AnimTag::LegL))
        .with(body)
}

/// Wobbly slime with a dark core and big cartoon eyes.
fn jelly() -> Part {
    let eye = |x: f32| {
        Part::pivot()
            .at(x, 0.85, -0.6)
            .with(Part::sphere(0.3).color(rgb(1.0, 1.0, 1.0)))
            .with(Part::sphere(0.14).at(0.0, 0.0, -0.12).color(PLUM_DARK))
    };
    Part::pivot().with(
        Part::pivot()
            .tag(AnimTag::Pulse)
            .with(Part::ellipsoid(1.6, 1.15, 1.6).at(0.0, 0.58, 0.0).color(rgb(0.45, 0.85, 0.35)).glow(0.35))
            .with(Part::ico(0.7, 0.6, 0.7).at(0.0, 0.55, 0.1).color(rgb(0.2, 0.45, 0.15)))
            .with(Part::sphere(0.25).at(0.45, 1.05, 0.3).color(rgb(0.6, 1.0, 0.5)).glow(0.8))
            .with(Part::sphere(0.18).at(-0.4, 1.0, 0.4).color(rgb(0.6, 1.0, 0.5)).glow(0.8))
            .with(eye(0.3))
            .with(eye(-0.3))
            .with(Part::cube(0.4, 0.06, 0.05).at(0.0, 0.55, -0.79).color(PLUM_DARK)),
    )
}

/// Floating bone mask with a glowing staff: the healer to focus first.
fn shaman() -> Part {
    let feather = |a: f32| Part::prism(0.08, 0.6, 0.25).at(0.0, 0.55, 0.05).rot_z(a).color(if a > 0.0 { GLOW_TOXIC } else { GLOW_MAGENTA }).glow(1.5);
    Part::pivot().with(
        Part::pivot()
            .tag(AnimTag::Bob)
            .with(Part::cone(0.9, 1.1).at(0.0, 0.45, 0.0).rot_x(180.0).color(PLUM).tag(AnimTag::Wing(0.0)))
            .with(
                Part::pivot()
                    .at(0.0, 1.05, 0.0)
                    .tag(AnimTag::Head)
                    .with(Part::cube(0.65, 0.85, 0.18).color(BONE))
                    .with(Part::ico(0.14, 0.18, 0.1).at(0.15, 0.12, -0.1).color(GLOW_MAGENTA).glow(5.0))
                    .with(Part::ico(0.14, 0.18, 0.1).at(-0.15, 0.12, -0.1).color(GLOW_MAGENTA).glow(5.0))
                    .with(Part::cube(0.3, 0.05, 0.04).at(0.0, -0.2, -0.1).color(PLUM_DARK))
                    .with(feather(-25.0))
                    .with(feather(0.0))
                    .with(feather(25.0)),
            )
            .with(
                Part::pivot()
                    .at(0.55, 0.9, -0.1)
                    .tag(AnimTag::ArmR)
                    .with(Part::cylinder(0.06, 1.4).at(0.0, -0.1, 0.0).color(BARK))
                    .with(Part::ico(0.28, 0.28, 0.28).at(0.0, 0.7, 0.0).color(GLOW_TOXIC).glow(5.0))
                    .with(Part::torus(0.45, 0.05).at(0.0, 0.7, 0.0).color(GOLD).tag(AnimTag::Spin(3.0))),
            ),
    )
}

/// Giant wasp queen: striped abdomen, four wings, crown.
fn hive_queen() -> Part {
    let yellow = rgb(0.95, 0.72, 0.12);
    let wing = |x: f32, z: f32, phase: f32| {
        Part::pivot()
            .at(x, 0.9, z)
            .tag(AnimTag::Wing(phase))
            .with(Part::ellipsoid(2.6, 0.08, 1.0).at(x.signum() * 1.3, 0.0, 0.0).rot_z(-x.signum() * 15.0).color(rgb(0.85, 0.9, 1.0)).glow(0.6))
    };
    let mut abdomen = Part::pivot()
        .at(0.0, 0.0, 1.9)
        .rot_x(15.0)
        .with(Part::ellipsoid(2.0, 1.9, 3.0).color(yellow))
        .with(Part::cone(0.4, 1.0).at(0.0, -0.2, 1.8).rot_x(90.0).color(PLUM_DARK));
    for z in [-0.6f32, 0.2, 1.0] {
        abdomen = abdomen.with(Part::torus(1.9 - z.abs() * 0.4, 0.28).at(0.0, 0.0, z).rot_x(90.0).color(PLUM_DARK));
    }
    let mut crown = Part::pivot().at(0.0, 0.75, 0.0);
    for i in -2..=2 {
        crown = crown.with(Part::cone(0.2, 0.6 - (i as f32).abs() * 0.12).at(i as f32 * 0.22, 0.0, 0.0).color(GOLD).glow(1.0));
    }
    let head = Part::pivot()
        .at(0.0, 0.2, -1.7)
        .tag(AnimTag::Head)
        .with(Part::sphere(1.2).color(PLUM_DARK))
        .with(Part::ico(0.45, 0.55, 0.3).at(0.38, 0.1, -0.4).color(GLOW_MAGENTA).glow(5.0))
        .with(Part::ico(0.45, 0.55, 0.3).at(-0.38, 0.1, -0.4).color(GLOW_MAGENTA).glow(5.0))
        .with(Part::cone(0.12, 0.7).at(0.3, -0.5, -0.4).rot_x(160.0).color(BONE).tag(AnimTag::Jaw))
        .with(Part::cone(0.12, 0.7).at(-0.3, -0.5, -0.4).rot_x(160.0).color(BONE))
        .with(crown);
    Part::pivot().with(
        Part::pivot()
            .at(0.0, 1.8, 0.0)
            .tag(AnimTag::Bob)
            .with(Part::ellipsoid(1.7, 1.5, 1.9).color(yellow))
            .with(Part::torus(1.5, 0.25).rot_x(90.0).color(PLUM_DARK))
            .with(abdomen)
            .with(head)
            .with(wing(0.6, -0.2, 0.0))
            .with(wing(-0.6, -0.2, 0.5))
            .with(wing(0.6, 0.5, 0.25))
            .with(wing(-0.6, 0.5, 0.75)),
    )
}

/// Storm titan: dark stone giant studded with cyan crystals, lightning crown.
fn storm_titan() -> Part {
    let leg = Part::pivot()
        .at(1.0, 2.6, 0.0)
        .tag(AnimTag::LegL)
        .with(Part::cube(1.1, 2.6, 1.1).at(0.0, -1.3, 0.0).color(STONE_DARK))
        .with(Part::cube(1.4, 0.5, 1.7).at(0.0, -2.4, -0.2).color(STONE))
        .with(Part::ico(0.4, 0.6, 0.4).at(0.4, -1.0, -0.5).color(CRYSTAL_CYAN).glow(3.0));
    let arm = Part::pivot()
        .at(2.0, 5.6, 0.0)
        .tag(AnimTag::ArmL)
        .with(Part::cube(1.0, 2.8, 1.0).at(0.0, -1.4, 0.0).rot_z(8.0).color(STONE))
        .with(Part::ico(1.4, 1.6, 1.4).at(0.2, -3.1, 0.0).color(STONE_DARK))
        .with(Part::ico(0.5, 0.9, 0.5).at(0.25, -3.1, -0.6).color(CRYSTAL_CYAN).glow(4.0));
    let mut crown = Part::pivot().at(0.0, 7.8, 0.0).tag(AnimTag::Spin(1.2));
    for i in 0..6 {
        let a = i as f32 / 6.0 * std::f32::consts::TAU;
        crown = crown.with(Part::ico(0.35, 1.0, 0.35).at(a.cos() * 1.3, (i as f32).sin() * 0.2, a.sin() * 1.3).color(CRYSTAL_CYAN).glow(5.0));
    }
    let body = Part::pivot()
        .tag(AnimTag::Body)
        .with(Part::cube(3.0, 3.2, 2.0).at(0.0, 4.3, 0.0).rot_x(8.0).color(STONE))
        .with(Part::ico(1.2, 1.5, 0.6).at(0.0, 4.5, -1.0).color(CRYSTAL_CYAN).glow(4.0))
        .with(Part::ico(0.9, 1.2, 0.9).at(1.5, 5.9, 0.0).color(CRYSTAL_CYAN).glow(3.0))
        .with(Part::ico(0.9, 1.2, 0.9).at(-1.5, 5.9, 0.0).color(CRYSTAL_CYAN).glow(3.0))
        .with(
            Part::pivot()
                .at(0.0, 6.4, -0.2)
                .tag(AnimTag::Head)
                .with(Part::cube(1.3, 1.1, 1.2).color(STONE_DARK))
                .with(Part::cube(1.0, 0.18, 0.1).at(0.0, 0.1, -0.62).color(GLOW_CYAN).glow(6.0)),
        )
        .with(arm.mirrored_x())
        .with(arm)
        .with(crown);
    Part::pivot().with(leg.mirrored_x()).with(leg).with(body)
}

/// Same model turned to stone (statues): grey colors, no glow.
pub fn petrify(mut p: Part) -> Part {
    let s = p.color.to_srgba();
    let v = (s.red + s.green + s.blue) / 3.0 * 0.5 + 0.3;
    p.color = rgb(v, v * 0.98, v * 1.04);
    p.glow = 0.0;
    p.tag = match p.tag {
        Some(AnimTag::Spin(_)) | Some(AnimTag::Bob) => p.tag,
        _ => None,
    };
    p.children = p.children.into_iter().map(petrify).collect();
    p
}

/// A boss statue: stone pedestal with a petrified, scaled-down boss on top.
pub fn statue(boss: MobKind) -> Part {
    let scale = 2.6 / boss.def().height;
    let lift = if boss.def().flying { -1.4 * scale } else { 0.0 };
    Part::pivot()
        .with(Part::cube(3.0, 0.4, 3.0).at(0.0, 0.2, 0.0).color(STONE_DARK))
        .with(Part::cube(2.4, 0.9, 2.4).at(0.0, 0.85, 0.0).color(STONE))
        .with(Part::cube(2.5, 0.12, 2.5).at(0.0, 1.32, 0.0).color(GLOW_MAGENTA).glow(1.5))
        .with(scale_part(petrify(mob(boss)), scale).at(0.0, 1.35 + lift, 0.0))
}

fn skitter() -> Part {
    let mut legs = Vec::new();
    for (i, z) in [-0.3f32, 0.0, 0.3].iter().enumerate() {
        let leg = Part::pivot()
            .at(0.26, 0.45, *z)
            .rot_y(-25.0 + i as f32 * 25.0)
            .tag(AnimTag::Leg(i as f32 / 3.0))
            .with(Part::cylinder(0.08, 0.5).at(0.2, 0.08, 0.0).rot_z(-65.0).color(PLUM_DARK))
            .with(Part::cone(0.1, 0.55).at(0.42, -0.2, 0.0).rot_z(180.0).color(BONE));
        legs.push(leg.mirrored_x());
        legs.push(leg);
    }
    let body = Part::pivot()
        .at(0.0, 0.45, 0.0)
        .tag(AnimTag::Body)
        .with(Part::ellipsoid(0.85, 0.5, 0.95).at(0.0, 0.02, 0.25).color(PLUM))
        .with(Part::ellipsoid(0.62, 0.42, 0.55).at(0.0, 0.05, -0.28).color(PLUM_DARK))
        .with(Part::cone(0.16, 0.32).at(0.0, 0.3, 0.15).rot_x(-25.0).color(BONE))
        .with(Part::cone(0.14, 0.28).at(0.0, 0.26, 0.42).rot_x(-35.0).color(BONE))
        .with(Part::ico(0.13, 0.13, 0.13).at(0.14, 0.12, -0.52).color(GLOW_MAGENTA).glow(4.0))
        .with(Part::ico(0.13, 0.13, 0.13).at(-0.14, 0.12, -0.52).color(GLOW_MAGENTA).glow(4.0))
        .with(Part::ico(0.08, 0.08, 0.08).at(0.0, 0.2, -0.52).color(GLOW_MAGENTA).glow(4.0))
        .with(
            Part::pivot()
                .at(0.0, -0.06, -0.52)
                .tag(AnimTag::Jaw)
                .with(Part::cone(0.08, 0.28).at(0.1, 0.0, -0.1).rot_x(-100.0).color(BONE))
                .with(Part::cone(0.08, 0.28).at(-0.1, 0.0, -0.1).rot_x(-100.0).color(BONE)),
        );
    Part::pivot().with_all(legs).with(body)
}

fn spitter() -> Part {
    let mut roots = Vec::new();
    for i in 0..4 {
        roots.push(
            Part::cone(0.2, 0.55)
                .at(0.0, 0.1, 0.0)
                .rot_y(45.0 + i as f32 * 90.0)
                .rot_x(70.0)
                .color(PLUM_DARK),
        );
    }
    let head = Part::pivot()
        .at(0.0, 0.95, 0.0)
        .tag(AnimTag::Head)
        .with(Part::sphere(0.78).at(0.0, 0.15, 0.0).color(PLUM))
        .with(Part::cylinder(0.34, 0.5).at(0.0, 0.1, -0.45).rot_x(90.0).color(PLUM_DARK))
        .with(Part::cylinder(0.22, 0.06).at(0.0, 0.1, -0.71).rot_x(90.0).color(GLOW_TOXIC).glow(4.0))
        .with(Part::ico(0.11, 0.11, 0.11).at(0.2, 0.38, -0.25).color(GLOW_TOXIC).glow(4.0))
        .with(Part::ico(0.11, 0.11, 0.11).at(-0.2, 0.38, -0.25).color(GLOW_TOXIC).glow(4.0))
        .with(Part::cone(0.14, 0.35).at(0.0, 0.55, 0.1).rot_x(-20.0).color(BONE))
        .with(
            Part::pivot()
                .at(0.0, 0.2, 0.32)
                .tag(AnimTag::Pulse)
                .with(Part::sphere(0.3).at(0.22, 0.0, 0.0).color(GLOW_TOXIC).glow(1.6))
                .with(Part::sphere(0.26).at(-0.22, 0.05, 0.02).color(GLOW_TOXIC).glow(1.6)),
        );
    let stalk = Part::pivot()
        .at(0.0, 0.5, 0.0)
        .tag(AnimTag::Body)
        .with(Part::cylinder(0.32, 0.95).at(0.0, 0.45, 0.0).color(PLUM_LIGHT))
        .with(Part::torus(0.44, 0.08).at(0.0, 0.25, 0.0).color(BONE))
        .with(Part::torus(0.4, 0.08).at(0.0, 0.6, 0.0).color(BONE))
        .with(head);
    Part::pivot()
        .with(Part::frustum(1.0, 0.5).at(0.0, 0.25, 0.0).color(PLUM_DARK))
        .with_all(roots)
        .with(stalk)
}

fn wisp() -> Part {
    let mut tentacles = Vec::new();
    for i in 0..3 {
        let a = i as f32 * 120.0;
        tentacles.push(
            Part::pivot()
                .at(0.0, -0.15, 0.0)
                .rot_y(a)
                .tag(AnimTag::Wing(i as f32 * 0.3))
                .with(Part::cone(0.14, 0.75).at(0.18, -0.4, 0.0).rot_z(170.0).color(PLUM)),
        );
    }
    let core = Part::pivot()
        .at(0.0, 0.5, 0.0)
        .tag(AnimTag::Bob)
        .with(Part::sphere(0.62).color(GLOW_CYAN).glow(5.0))
        .with(Part::ico(0.3, 0.3, 0.3).at(0.0, 0.0, -0.2).color(rgb(1.0, 1.0, 1.0)).glow(6.0))
        .with(Part::pivot().tag(AnimTag::Spin(2.5)).with(Part::torus(1.35, 0.12).rot_x(70.0).color(BONE)))
        .with(Part::pivot().tag(AnimTag::Spin(-3.5)).with(Part::torus(1.0, 0.1).rot_z(60.0).color(PLUM_LIGHT)))
        .with(Part::prism(0.06, 0.55, 0.45).at(0.45, 0.05, 0.1).rot_z(-70.0).color(PLUM).tag(AnimTag::Wing(0.0)))
        .with(Part::prism(0.06, 0.55, 0.45).at(-0.45, 0.05, 0.1).rot_z(70.0).color(PLUM).tag(AnimTag::Wing(0.5)))
        .with_all(tentacles);
    Part::pivot().with(core)
}

fn bomber() -> Part {
    let foot = Part::cube(0.26, 0.2, 0.34).at(0.25, 0.1, -0.03).color(PLUM_DARK).tag(AnimTag::LegL);
    let body = Part::pivot()
        .at(0.0, 0.58, 0.0)
        .tag(AnimTag::Pulse)
        .with(Part::sphere(1.0).color(BOMB_RED))
        .with(Part::torus(1.02, 0.09).color(PLUM_DARK))
        .with(Part::cube(0.2, 0.08, 0.05).at(0.18, 0.14, -0.47).rot_z(18.0).color(GLOW_ORANGE).glow(5.0))
        .with(Part::cube(0.2, 0.08, 0.05).at(-0.18, 0.14, -0.47).rot_z(-18.0).color(GLOW_ORANGE).glow(5.0))
        .with(Part::cube(0.3, 0.05, 0.05).at(0.0, -0.12, -0.48).color(PLUM_DARK))
        .with(Part::cylinder(0.1, 0.38).at(0.0, 0.62, 0.0).rot_z(12.0).color(BONE))
        .with(Part::ico(0.2, 0.2, 0.2).at(0.04, 0.84, 0.0).color(GLOW_ORANGE).glow(6.0).tag(AnimTag::Bob));
    Part::pivot().with(foot.mirrored_x()).with(foot).with(body)
}

fn brute() -> Part {
    let leg = Part::pivot()
        .at(0.55, 1.0, 0.0)
        .tag(AnimTag::LegL)
        .with(Part::cylinder(0.6, 0.9).at(0.0, -0.45, 0.0).color(PLUM_DARK))
        .with(Part::cube(0.75, 0.3, 0.95).at(0.0, -0.88, -0.1).color(STONE));
    let arm = Part::pivot()
        .at(1.25, 1.55, 0.0)
        .tag(AnimTag::ArmL)
        .with(Part::cube(0.6, 1.25, 0.6).at(0.0, -0.6, 0.0).rot_z(8.0).color(PLUM))
        .with(Part::cube(0.85, 0.75, 0.85).at(0.08, -1.42, -0.05).color(PLUM_DARK))
        .with(Part::cube(0.88, 0.12, 0.2).at(0.08, -1.3, -0.45).color(GLOW_ORANGE).glow(2.5));
    let head = Part::pivot()
        .at(0.0, 1.75, -0.55)
        .tag(AnimTag::Head)
        .with(Part::cube(0.62, 0.52, 0.62).color(PLUM_DARK))
        .with(Part::ico(0.13, 0.13, 0.13).at(0.15, 0.04, -0.32).color(GLOW_ORANGE).glow(5.0))
        .with(Part::ico(0.13, 0.13, 0.13).at(-0.15, 0.04, -0.32).color(GLOW_ORANGE).glow(5.0))
        .with(Part::cone(0.16, 0.5).at(0.32, 0.32, 0.0).rot_z(-35.0).color(BONE))
        .with(Part::cone(0.16, 0.5).at(-0.32, 0.32, 0.0).rot_z(35.0).color(BONE));
    let plate = Part::cube(0.95, 0.36, 1.1)
        .at(1.05, 1.75, 0.1)
        .rot_z(-15.0)
        .color(STONE)
        .with(Part::cone(0.28, 0.55).at(0.1, 0.4, 0.0).color(BONE));
    let body = Part::pivot()
        .at(0.0, 1.0, 0.0)
        .tag(AnimTag::Body)
        .with(Part::cube(2.1, 1.5, 1.3).at(0.0, 0.95, 0.1).rot_x(15.0).color(PLUM))
        .with(Part::cube(1.3, 0.8, 0.2).at(0.0, 0.6, -0.56).color(PLUM_LIGHT))
        // Weak point: a glowing growth on its back.
        .with(Part::ico(0.7, 0.8, 0.5).at(0.0, 1.5, 0.75).color(GLOW_ORANGE).glow(4.0).tag(AnimTag::Pulse))
        .with(plate.mirrored_x())
        .with(plate)
        .with(head)
        .with(arm.mirrored_x())
        .with(arm);
    Part::pivot().with(leg.mirrored_x()).with(leg).with(body)
}

fn colossus() -> Part {
    let leg = Part::pivot()
        .at(1.4, 3.0, 0.0)
        .tag(AnimTag::LegL)
        .with(Part::cube(1.4, 3.0, 1.4).at(0.0, -1.5, 0.0).color(STONE))
        .with(Part::cube(1.9, 0.6, 2.3).at(0.0, -2.9, -0.2).color(STONE_DARK))
        .with(Part::ico(0.5, 0.5, 0.5).at(0.0, -1.2, -0.7).color(GLOW_ORANGE).glow(4.0));
    let arm = Part::pivot()
        .at(2.5, 3.4, 0.0)
        .tag(AnimTag::ArmL)
        .with(Part::cube(1.2, 2.6, 1.2).at(0.0, -1.3, 0.0).rot_z(10.0).color(STONE))
        .with(Part::ico(1.9, 1.7, 1.9).at(0.25, -3.0, 0.0).color(STONE_DARK))
        .with(Part::ico(0.5, 0.5, 0.5).at(0.3, -2.9, -0.85).color(GLOW_ORANGE).glow(4.0));
    let mut crown = Vec::new();
    for i in 0..5 {
        let x = -0.6 + i as f32 * 0.3;
        crown.push(Part::cone(0.35, 1.0 + (i as f32 - 2.0).abs() * -0.2 + 0.4).at(x, 0.95, 0.0).color(BONE));
    }
    let head = Part::pivot()
        .at(0.0, 4.1, -0.3)
        .tag(AnimTag::Head)
        .with(Part::cube(1.5, 1.15, 1.35).color(STONE))
        .with(Part::cube(1.05, 0.2, 0.1).at(0.0, 0.1, -0.68).color(GLOW_ORANGE).glow(6.0))
        .with_all(crown);
    let mut halo = Vec::new();
    for i in 0..6 {
        let a = i as f32 / 6.0 * std::f32::consts::TAU;
        halo.push(Part::ico(0.8, 0.7, 0.8).at(a.cos() * 3.2, (i as f32).sin() * 0.4, a.sin() * 3.2).color(STONE_DARK));
    }
    let cracks = [
        Vec3::new(0.9, 2.4, -1.5),
        Vec3::new(-1.1, 1.4, -1.4),
        Vec3::new(1.6, 1.0, 0.5),
        Vec3::new(-1.5, 2.3, 0.8),
    ];
    let body = Part::pivot()
        .at(0.0, 3.0, 0.0)
        .tag(AnimTag::Body)
        .with(Part::ico(4.0, 3.6, 3.4).at(0.0, 1.9, 0.0).color(STONE_DARK))
        .with(Part::sphere(1.1).at(0.0, 2.1, -1.55).color(GLOW_MAGENTA).glow(6.0))
        .with_all(cracks.iter().map(|p| Part::ico(0.6, 0.6, 0.6).at_v(*p).color(GLOW_ORANGE).glow(4.0)))
        .with(head)
        .with(arm.mirrored_x())
        .with(arm)
        .with(Part::pivot().at(0.0, 2.6, 0.0).tag(AnimTag::Spin(0.7)).with_all(halo));
    Part::pivot().with(leg.mirrored_x()).with(leg).with(body)
}

// ---------------------------------------------------------------------------
// Chests
// ---------------------------------------------------------------------------

/// Vertical light beam showing valuable chests from far away.
fn beam(color: Color) -> Part {
    Part::pivot()
        .tag(AnimTag::Beam)
        .with(Part::cylinder(0.22, 26.0).at(0.0, 13.5, 0.0).color(color).glow(2.5))
        .with(Part::cylinder(0.5, 26.0).at(0.0, 13.5, 0.0).color(color).glow(0.6))
}

pub fn chest(kind: ChestKind) -> Part {
    let base = chest_body(kind);
    match kind {
        ChestKind::Small => base,
        ChestKind::Large => base.with(beam(GLOW_ORANGE)),
        ChestKind::Legendary => base.with(beam(GLOW_MAGENTA)),
        ChestKind::Weapon => base.with(beam(TEAL_GLOW)),
    }
}

fn chest_body(kind: ChestKind) -> Part {
    match kind {
        ChestKind::Small => {
            let lid = Part::pivot()
                .at(0.0, 0.55, 0.35)
                .tag(AnimTag::Lid)
                .with(Part::cube(1.0, 0.24, 0.7).at(0.0, 0.12, -0.35).color(WOOD_DARK))
                .with(Part::cube(1.04, 0.06, 0.74).at(0.0, 0.2, -0.35).color(GOLD))
                .with(Part::ico(0.16, 0.16, 0.1).at(0.0, 0.08, -0.72).color(GOLD).glow(3.0));
            Part::pivot()
                .with(Part::cube(1.0, 0.55, 0.7).at(0.0, 0.275, 0.0).color(WOOD))
                .with(Part::cube(1.04, 0.07, 0.74).at(0.0, 0.08, 0.0).color(GOLD))
                .with(Part::cube(1.04, 0.07, 0.74).at(0.0, 0.48, 0.0).color(GOLD))
                .with(Part::cube(0.16, 0.2, 0.05).at(0.0, 0.42, -0.37).color(GOLD))
                .with(Part::sphere(0.5).at(0.0, 0.45, 0.0).color(GOLD).glow(3.0))
                .with(lid)
        }
        ChestKind::Large => {
            let lid = Part::pivot()
                .at(0.0, 0.8, 0.5)
                .tag(AnimTag::Lid)
                .with(Part::cube(1.5, 0.32, 1.0).at(0.0, 0.16, -0.5).color(GOLD))
                .with(Part::cube(1.54, 0.08, 1.04).at(0.0, 0.3, -0.5).color(WOOD_DARK))
                .with(Part::ico(0.24, 0.24, 0.12).at(0.0, 0.12, -1.02).color(GLOW_ORANGE).glow(4.0));
            Part::pivot()
                .with(Part::cube(1.5, 0.8, 1.0).at(0.0, 0.4, 0.0).color(GOLD))
                .with(Part::cube(1.54, 0.1, 1.04).at(0.0, 0.1, 0.0).color(WOOD_DARK))
                .with(Part::cube(0.12, 0.82, 1.04).at(0.5, 0.4, 0.0).color(WOOD_DARK))
                .with(Part::cube(0.12, 0.82, 1.04).at(-0.5, 0.4, 0.0).color(WOOD_DARK))
                .with(Part::cube(0.22, 0.28, 0.06).at(0.0, 0.62, -0.52).color(WOOD_DARK))
                .with(Part::sphere(0.7).at(0.0, 0.65, 0.0).color(GLOW_ORANGE).glow(3.5))
                .with(lid)
        }
        ChestKind::Legendary => {
            let lid = Part::pivot()
                .at(0.0, 0.7, 0.45)
                .tag(AnimTag::Lid)
                .with(Part::cube(1.3, 0.28, 0.9).at(0.0, 0.14, -0.45).color(PLUM_DARK))
                .with(Part::cube(1.34, 0.06, 0.94).at(0.0, 0.26, -0.45).color(GLOW_MAGENTA).glow(3.0))
                .with(Part::ico(0.3, 0.3, 0.3).at(0.0, 0.45, -0.45).color(GLOW_MAGENTA).glow(5.0).tag(AnimTag::Bob));
            Part::pivot()
                .with(Part::cube(1.3, 0.7, 0.9).at(0.0, 0.35, 0.0).color(PLUM_DARK))
                .with(Part::cube(1.34, 0.06, 0.94).at(0.0, 0.08, 0.0).color(GLOW_MAGENTA).glow(3.0))
                .with(Part::cube(1.34, 0.06, 0.94).at(0.0, 0.6, 0.0).color(GOLD))
                .with(Part::cone(0.25, 0.5).at(0.6, 0.95, 0.4).color(GOLD))
                .with(Part::cone(0.25, 0.5).at(-0.6, 0.95, 0.4).color(GOLD))
                .with(Part::sphere(0.7).at(0.0, 0.55, 0.0).color(GLOW_MAGENTA).glow(4.0))
                .with(lid)
        }
        ChestKind::Weapon => {
            let lid = Part::pivot()
                .at(0.0, 0.55, 0.4)
                .tag(AnimTag::Lid)
                .with(Part::cube(1.6, 0.22, 0.8).at(0.0, 0.11, -0.4).color(STEEL))
                .with(Part::cube(1.2, 0.05, 0.1).at(0.0, 0.22, -0.4).color(TEAL_GLOW).glow(3.0));
            Part::pivot()
                .with(Part::cube(1.6, 0.55, 0.8).at(0.0, 0.275, 0.0).color(STEEL))
                .with(Part::cube(1.64, 0.06, 0.84).at(0.0, 0.3, 0.0).color(TEAL_GLOW).glow(2.5))
                .with(Part::cube(0.3, 0.6, 0.86).at(0.6, 0.3, 0.0).color(METAL_DARK))
                .with(Part::cube(0.3, 0.6, 0.86).at(-0.6, 0.3, 0.0).color(METAL_DARK))
                .with(Part::sphere(0.55).at(0.0, 0.45, 0.0).color(TEAL_GLOW).glow(3.0))
                .with(lid)
        }
    }
}

// ---------------------------------------------------------------------------
// Weapons (first person). Origin at the grip, barrel toward -Z.
// ---------------------------------------------------------------------------

/// Muzzle position relative to the weapon origin.
pub fn muzzle(kind: MainWeapon) -> Vec3 {
    match kind {
        MainWeapon::FireStaff => Vec3::new(0.0, 0.1, -0.72),
        MainWeapon::Bow => Vec3::new(0.0, 0.0, -0.2),
        MainWeapon::Revolver => Vec3::new(0.0, 0.05, -0.4),
        MainWeapon::Shotgun => Vec3::new(0.0, 0.04, -0.6),
        MainWeapon::Sniper => Vec3::new(0.0, 0.04, -0.92),
        MainWeapon::LightningStaff => Vec3::new(0.0, 0.1, -0.72),
        MainWeapon::Bananarang => Vec3::new(0.0, 0.05, -0.25),
        MainWeapon::HeroSword => Vec3::new(0.0, 0.05, -0.8),
    }
}

/// A magic staff held forward, with a glowing head.
fn staff(head: Part, ring: Color) -> Part {
    Part::pivot()
        .with(Part::cylinder(0.05, 0.85).at(0.0, -0.02, -0.25).rot_x(-80.0).color(WOOD_DARK))
        .with(Part::torus(0.11, 0.025).at(0.0, 0.07, -0.6).rot_x(-80.0).color(ring).glow(3.0).tag(AnimTag::Recoil))
        .with(head.at(0.0, 0.1, -0.72))
}

pub fn weapon(kind: MainWeapon, accent: Color) -> Part {
    let grip = Part::cube(0.07, 0.17, 0.09).at(0.0, -0.08, 0.03).rot_x(-15.0).color(METAL_DARK);
    match kind {
        MainWeapon::FireStaff => staff(
            Part::pivot()
                .with(Part::sphere(0.16).color(GLOW_ORANGE).glow(6.0).tag(AnimTag::Pulse))
                .with(Part::pivot().tag(AnimTag::Spin(3.0)).with_all((0..3).map(|i| {
                    let a = i as f32 * 120.0;
                    Part::cone(0.05, 0.14).at(0.0, -0.06, 0.0).rot_y(a).rot_x(25.0).color(WOOD)
                }))),
            accent,
        ),
        MainWeapon::LightningStaff => staff(
            Part::pivot()
                .with(Part::ico(0.15, 0.24, 0.15).color(GLOW_CYAN).glow(6.0).tag(AnimTag::Pulse))
                .with(Part::pivot().tag(AnimTag::Spin(-4.0)).with(Part::torus(0.22, 0.02).rot_x(70.0).color(METAL_LIGHT))),
            accent,
        ),
        MainWeapon::Bow => {
            // Limbs: segments along a curve, the string straight.
            let mut bow = Part::pivot().at(0.0, 0.0, -0.2);
            for i in -3..=3 {
                let t = i as f32 / 3.0;
                let y = t * 0.36;
                let z = 0.09 * (1.0 - t * t);
                bow = bow.with(Part::cube(0.035, 0.13, 0.04).at(0.0, y, -z).rot_x(-t * 30.0).color(WOOD));
            }
            bow.with(Part::cube(0.006, 0.72, 0.006).at(0.0, 0.0, 0.0).color(BONE))
                .with(Part::cube(0.05, 0.1, 0.05).color(accent).glow(2.0))
                .with(Part::cylinder(0.018, 0.5).at(0.0, 0.0, -0.08).rot_x(90.0).color(WOOD_DARK).tag(AnimTag::Recoil))
                .rot_z(-12.0)
        }
        MainWeapon::Revolver => Part::pivot()
            .with(Part::cube(0.075, 0.18, 0.1).at(0.0, -0.08, 0.04).rot_x(-20.0).color(WOOD))
            .with(Part::cylinder(0.12, 0.1).at(0.0, 0.04, -0.06).rot_x(90.0).color(METAL_LIGHT).tag(AnimTag::Recoil))
            .with(Part::cylinder(0.05, 0.32).at(0.0, 0.05, -0.24).rot_x(90.0).color(METAL_DARK))
            .with(Part::cube(0.03, 0.03, 0.03).at(0.0, 0.09, -0.38).color(accent).glow(4.0))
            .with(Part::cube(0.06, 0.06, 0.1).at(0.0, 0.07, 0.03).color(METAL_DARK)),
        MainWeapon::Shotgun => Part::pivot()
            .with(grip)
            .with(Part::cylinder(0.065, 0.58).at(0.036, 0.05, -0.32).rot_x(90.0).color(METAL_DARK))
            .with(Part::cylinder(0.065, 0.58).at(-0.036, 0.05, -0.32).rot_x(90.0).color(METAL_DARK))
            .with(Part::cube(0.12, 0.08, 0.2).at(0.0, -0.01, -0.32).color(WOOD).tag(AnimTag::Recoil))
            .with(Part::cube(0.12, 0.13, 0.22).at(0.0, 0.03, -0.01).color(METAL_LIGHT))
            .with(Part::cube(0.1, 0.13, 0.32).at(0.0, -0.04, 0.24).rot_x(10.0).color(WOOD))
            .with(Part::torus(0.16, 0.03).at(0.0, 0.05, -0.55).rot_x(90.0).color(accent).glow(3.0)),
        MainWeapon::Sniper => Part::pivot()
            .with(grip)
            .with(Part::cube(0.09, 0.12, 0.5).at(0.0, 0.03, -0.08).color(METAL_DARK))
            .with(Part::cylinder(0.045, 0.62).at(0.0, 0.04, -0.6).rot_x(90.0).color(METAL_DARK))
            .with(Part::cube(0.07, 0.065, 0.09).at(0.0, 0.04, -0.88).color(METAL_LIGHT))
            .with(Part::cylinder(0.085, 0.34).at(0.0, 0.15, -0.12).rot_x(90.0).color(METAL_LIGHT))
            .with(Part::cylinder(0.075, 0.01).at(0.0, 0.15, -0.295).rot_x(90.0).color(accent).glow(5.0))
            .with(Part::cube(0.08, 0.14, 0.3).at(0.0, -0.01, 0.26).rot_x(6.0).color(METAL_DARK))
            .with(Part::cube(0.092, 0.02, 0.4).at(0.0, -0.02, -0.12).color(TEAL_GLOW).glow(2.5))
            .with(Part::cylinder(0.04, 0.06).at(0.06, 0.06, 0.0).rot_z(90.0).color(METAL_LIGHT).tag(AnimTag::Recoil)),
        MainWeapon::Bananarang => Part::pivot().at(0.0, 0.05, -0.2).rot_x(70.0).with(banana(0.5)),
        MainWeapon::HeroSword => Part::pivot()
            .with(Part::cylinder(0.05, 0.16).at(0.0, -0.04, 0.04).rot_x(90.0).color(WOOD_DARK))
            .with(Part::sphere(0.06).at(0.0, -0.04, 0.13).color(GOLD))
            .with(Part::cube(0.26, 0.04, 0.05).at(0.0, -0.04, -0.05).color(GOLD).tag(AnimTag::Recoil))
            .with(Part::cube(0.075, 0.015, 0.72).at(0.0, -0.04, -0.43).color(rgb(0.85, 0.9, 1.0)).glow(0.6))
            .with(Part::prism(0.075, 0.015, 0.1).at(0.0, -0.04, -0.84).rot_x(-90.0).color(rgb(0.85, 0.9, 1.0)))
            .with(Part::cube(0.02, 0.018, 0.6).at(0.0, -0.04, -0.4).color(accent).glow(4.0)),
    }
}

/// A banana boomerang lying in the XZ plane, `size` across.
fn banana(size: f32) -> Part {
    let yellow = rgb(1.0, 0.86, 0.25);
    let k = size;
    Part::pivot()
        .with(Part::ellipsoid(0.62 * k, 0.16 * k, 0.2 * k).at(-0.22 * k, 0.0, 0.08 * k).rot_y(-28.0).color(yellow))
        .with(Part::ellipsoid(0.62 * k, 0.16 * k, 0.2 * k).at(0.22 * k, 0.0, 0.08 * k).rot_y(28.0).color(yellow))
        .with(Part::cube(0.06 * k, 0.1 * k, 0.06 * k).at(-0.5 * k, 0.0, 0.22 * k).color(WOOD_DARK))
        .with(Part::cube(0.06 * k, 0.1 * k, 0.06 * k).at(0.5 * k, 0.0, 0.22 * k).color(WOOD_DARK))
}

/// Icon-ish 3D model of an automatic weapon (floating next to the player).
pub fn auto_weapon(kind: AutoWeapon) -> Part {
    match kind {
        AutoWeapon::Axes => axe(),
        _ => Part::pivot().with(Part::ico(0.3, 0.3, 0.3).color(GLOW_CYAN).glow(4.0)),
    }
}

fn axe() -> Part {
    Part::pivot()
        .with(Part::cylinder(0.12, 1.2).color(WOOD))
        .with(Part::prism(0.7, 0.5, 0.08).at(0.28, 0.4, 0.0).rot_z(-90.0).color(METAL_LIGHT))
        .with(Part::cube(0.06, 0.5, 0.1).at(0.6, 0.4, 0.0).color(GLOW_ORANGE).glow(2.5))
}

/// A charge shrine: a ring of glowing stones around a hollow pillar.
pub fn shrine() -> Part {
    let mut p = Part::pivot()
        .with(Part::cylinder(1.6, 0.3).at(0.0, 0.15, 0.0).color(STONE))
        .with(Part::frustum(1.2, 0.5).at(0.0, 0.5, 0.0).color(STONE_DARK))
        .with(Part::torus(1.1, 0.12).at(0.0, 3.2, 0.0).color(GLOW_CYAN).glow(4.0).tag(AnimTag::Spin(1.5)))
        .with(Part::ico(0.5, 0.5, 0.5).at(0.0, 3.6, 0.0).color(CRYSTAL_CYAN).glow(4.0).tag(AnimTag::Bob));
    // Four thin posts holding the crown.
    for i in 0..4 {
        let a = i as f32 * std::f32::consts::FRAC_PI_2 + 0.4;
        p = p.with(Part::cube(0.14, 2.8, 0.14).at(a.cos() * 0.55, 1.6, a.sin() * 0.55).color(STONE));
    }
    // The circle to stand in: small glowing stones on its edge.
    for i in 0..20 {
        let a = i as f32 / 20.0 * std::f32::consts::TAU;
        let r = SHRINE_RADIUS;
        p = p.with(Part::ico(0.3, 0.22, 0.3).at(a.cos() * r, 0.1, a.sin() * r).color(GLOW_CYAN).glow(2.5));
    }
    p
}

/// Totem of challenge: a dark obelisk with burning red runes and horns.
pub fn totem() -> Part {
    let red = rgb(1.0, 0.18, 0.12);
    let mut p = Part::pivot()
        .with(Part::cylinder(1.5, 0.35).at(0.0, 0.17, 0.0).color(STONE_DARK))
        .with(Part::frustum(0.9, 0.6).at(0.0, 0.5, 0.0).color(STONE))
        .with(Part::frustum(0.55, 3.2).at(0.0, 2.3, 0.0).color(PLUM_DARK))
        // Skull-like head with glowing eyes and two horns.
        .with(Part::ico(0.75, 0.7, 0.7).at(0.0, 4.2, 0.0).color(BONE))
        .with(Part::sphere(0.14).at(0.22, 4.25, -0.58).color(red).glow(6.0))
        .with(Part::sphere(0.14).at(-0.22, 4.25, -0.58).color(red).glow(6.0))
        .with(Part::cone(0.18, 0.9).at(0.55, 4.7, 0.0).rot_z(-35.0).color(BONE))
        .with(Part::cone(0.18, 0.9).at(-0.55, 4.7, 0.0).rot_z(35.0).color(BONE))
        // Floating red flame above it.
        .with(Part::ico(0.35, 0.6, 0.35).at(0.0, 5.4, 0.0).color(red).glow(5.0).tag(AnimTag::Bob))
        .with(Part::torus(1.3, 0.08).at(0.0, 0.4, 0.0).color(red).glow(3.0).tag(AnimTag::Pulse));
    // Runes on the 4 sides of the pillar.
    for i in 0..4 {
        let a = i as f32 * std::f32::consts::FRAC_PI_2;
        for k in 0..3 {
            let y = 1.3 + k as f32 * 0.8;
            let r = 0.5 - k as f32 * 0.05;
            p = p.with(Part::cube(0.18, 0.4, 0.06).at(a.sin() * r, y, a.cos() * r).rot_y(a.to_degrees()).color(red).glow(3.5));
        }
    }
    p
}

/// A floating power-up.
pub fn powerup(kind: PowerupKind) -> Part {
    let icon = match kind {
        PowerupKind::Magnet => Part::pivot()
            .with(Part::cube(0.18, 0.5, 0.18).at(-0.22, -0.05, 0.0).color(rgb(0.9, 0.15, 0.15)))
            .with(Part::cube(0.18, 0.5, 0.18).at(0.22, -0.05, 0.0).color(rgb(0.9, 0.15, 0.15)))
            .with(Part::cube(0.62, 0.18, 0.18).at(0.0, 0.25, 0.0).color(rgb(0.9, 0.15, 0.15)))
            .with(Part::cube(0.18, 0.14, 0.18).at(-0.22, -0.36, 0.0).color(METAL_LIGHT).glow(1.5))
            .with(Part::cube(0.18, 0.14, 0.18).at(0.22, -0.36, 0.0).color(METAL_LIGHT).glow(1.5)),
        PowerupKind::Heal => Part::pivot()
            .with(Part::cube(0.2, 0.62, 0.2).color(GLOW_TOXIC).glow(3.0))
            .with(Part::cube(0.62, 0.2, 0.2).color(GLOW_TOXIC).glow(3.0)),
        PowerupKind::Bomb => Part::pivot()
            .with(Part::sphere(0.55).color(METAL_DARK))
            .with(Part::cylinder(0.12, 0.2).at(0.0, 0.32, 0.0).color(METAL_LIGHT))
            .with(Part::sphere(0.12).at(0.0, 0.45, 0.0).color(GLOW_ORANGE).glow(6.0).tag(AnimTag::Pulse)),
        PowerupKind::Haste => Part::pivot()
            .with(Part::prism(0.2, 0.5, 0.12).at(0.05, 0.18, 0.0).rot_z(-20.0).color(GOLD).glow(3.0))
            .with(Part::prism(0.2, 0.5, 0.12).at(-0.05, -0.18, 0.0).rot_z(160.0).color(GOLD).glow(3.0)),
    };
    Part::pivot()
        .with(Part::cylinder(1.0, 0.03).at(0.0, -0.55, 0.0).color(GOLD).glow(1.5))
        .with(Part::pivot().tag(AnimTag::Bob).with(Part::pivot().tag(AnimTag::Spin(2.0)).with(icon)))
}

/// Scales a whole part tree (sizes and positions).
pub fn scale_part(mut p: Part, k: f32) -> Part {
    p.size *= k;
    p.pos *= k;
    p.children = p.children.into_iter().map(|c| scale_part(c, k)).collect();
    p
}

// ---------------------------------------------------------------------------
// Projectiles (oriented along -Z). Player projectiles are modelled for a
// radius of 1 and scaled to their actual size (size upgrades show!).
// ---------------------------------------------------------------------------

pub fn projectile(kind: ProjectileKind, size: f32) -> Part {
    let unit = match kind {
        ProjectileKind::Acid => {
            return Part::pivot()
                .with(Part::sphere(0.45).color(GLOW_TOXIC).glow(3.5))
                .with(Part::ico(0.25, 0.25, 0.25).at(0.0, 0.1, 0.25).color(GLOW_TOXIC).glow(2.0));
        }
        ProjectileKind::Bolt => return Part::pivot().with(Part::ellipsoid(0.25, 0.25, 0.8).color(GLOW_CYAN).glow(5.0)),
        ProjectileKind::Thorn => return Part::pivot().with(Part::cone(0.22, 0.8).rot_x(-90.0).color(BONE).glow(1.5)),
        ProjectileKind::Shard => return Part::pivot().with(Part::ico(0.4, 0.4, 1.0).color(CRYSTAL_PINK).glow(4.0)),
        ProjectileKind::BossOrb => {
            return Part::pivot()
                .with(Part::sphere(1.0).color(GLOW_ORANGE).glow(5.0))
                .with(Part::pivot().tag(AnimTag::Spin(6.0)).with(Part::torus(1.4, 0.12).rot_x(80.0).color(STONE_DARK)));
        }
        ProjectileKind::Fireball => Part::pivot()
            .with(Part::sphere(2.0).color(GLOW_ORANGE).glow(6.0))
            .with(Part::ico(1.3, 1.3, 1.3).color(rgb(1.0, 0.9, 0.4)).glow(8.0).tag(AnimTag::Spin(8.0)))
            .with(Part::cone(1.6, 2.2).at(0.0, 0.0, 1.4).rot_x(90.0).color(rgb(1.0, 0.3, 0.1)).glow(4.0)),
        ProjectileKind::Arrow => Part::pivot()
            .with(Part::cylinder(0.25, 6.0).rot_x(90.0).color(WOOD))
            .with(Part::cone(0.8, 1.2).at(0.0, 0.0, -3.4).rot_x(-90.0).color(METAL_LIGHT).glow(1.5))
            .with(Part::cube(0.08, 1.0, 1.4).at(0.0, 0.0, 2.6).color(rgb(0.95, 0.95, 1.0)).glow(1.0))
            .with(Part::cube(1.0, 0.08, 1.4).at(0.0, 0.0, 2.6).color(rgb(0.95, 0.95, 1.0)).glow(1.0)),
        ProjectileKind::Bullet => Part::pivot().with(Part::ellipsoid(1.4, 1.4, 6.0).color(rgb(1.0, 0.9, 0.5)).glow(8.0)),
        ProjectileKind::Pellet => Part::pivot().with(Part::ellipsoid(1.6, 1.6, 3.5).color(rgb(1.0, 0.75, 0.35)).glow(7.0)),
        ProjectileKind::SwordWave => Part::pivot()
            .with(Part::ellipsoid(2.4, 0.22, 0.7).color(rgb(0.8, 0.95, 1.0)).glow(6.0))
            .with(Part::ellipsoid(1.6, 0.12, 0.5).at(0.0, 0.0, 0.35).color(GLOW_CYAN).glow(4.0)),
        ProjectileKind::Banana => Part::pivot().tag(AnimTag::Spin(16.0)).with(banana(2.0)),
        ProjectileKind::Axe => Part::pivot().with(Part::pivot().rot_z(90.0).tag(AnimTag::Spin(12.0)).with(scale_part(axe(), 0.8))),
    };
    scale_part(unit, size)
}

// ---------------------------------------------------------------------------
// World props (baked into chunk meshes).
// ---------------------------------------------------------------------------

fn tint(c: Color, v: f32) -> Color {
    let s = c.to_srgba();
    let f = 0.9 + v * 0.2;
    Color::srgb(s.red * f, s.green * f, s.blue * f)
}

/// Mixes a color toward white (crystal tips, sunlit leaves).
fn lift(c: Color, t: f32) -> Color {
    let s = c.to_srgba();
    Color::srgb(s.red + (1.0 - s.red) * t, s.green + (1.0 - s.green) * t, s.blue + (1.0 - s.blue) * t)
}

/// Cheap deterministic pseudo-random value in [0, 1) from a prop variant and a salt:
/// gives every prop its own small differences from the single `variant` number.
fn vary(v: f32, salt: f32) -> f32 {
    let x = (v * 91.7 + salt * 17.13).sin() * 43_758.547;
    x - x.floor()
}

/// One crystal: an octagonal column with a pointed, brighter tip. Base at the origin, along +Y.
fn crystal_spike(d: f32, h: f32, color: Color, glow: f32) -> Part {
    Part::pivot()
        .with(Part::cylinder(d, h * 0.7).at(0.0, h * 0.35, 0.0).color(color).glow(glow))
        .with(Part::cone(d, h * 0.3).at(0.0, h * 0.85, 0.0).color(lift(color, 0.45)).glow(glow * 1.5))
}

/// Round leafy crown: a big core and puffs around it (darker below, sunlit on top).
fn round_crown(base: Part, s: f32, c: Color, v: f32) -> Part {
    let dark = tint(c, 0.0);
    let light = lift(c, 0.18);
    let k = 0.85 + vary(v, 4.0) * 0.3;
    base.with(Part::cylinder(0.3 * s, 1.0 * s).at(0.0, 0.2 * s, 0.0).color(BARK))
        .with(Part::ico(2.3 * s, 1.6 * s * k, 2.3 * s).at(0.0, 0.8 * s, 0.0).rot_y(18.0).color(c))
        .with(Part::ico(1.5 * s, 1.2 * s, 1.5 * s).at(0.8 * s, 1.05 * s, 0.35 * s).rot_y(40.0).color(tint(c, 0.7)))
        .with(Part::ico(1.4 * s, 1.1 * s, 1.4 * s).at(-0.75 * s, 0.95 * s, -0.4 * s).rot_z(15.0).color(tint(c, 0.4)))
        .with(Part::ico(1.3 * s, 1.0 * s, 1.3 * s).at(-0.2 * s, 0.5 * s, 0.85 * s).rot_x(20.0).color(dark))
        .with(Part::ico(1.4 * s, 1.0 * s, 1.4 * s).at(0.1 * s, 1.5 * s * k, -0.1 * s).rot_y(70.0).color(light))
}

/// Model for a collision object. Visual parts stay inside the collision volume
/// (small rubble, roots and drapes may poke out a little at the foot).
pub fn solid(s: &Solid, variant: f32) -> Part {
    let r = |salt: f32| vary(variant, salt);
    match (s.kind, s.shape) {
        // Rock: a chunky faceted cluster with a dark stratum, moss on top and pebbles
        // at the foot. The ground is at y = -0.7 * half.y (the generator buries 30%).
        (SolidKind::Rock, Shape::Box { half, yaw }) => {
            let (hx, hy, hz) = (half.x, half.y, half.z);
            let ground = -0.7 * hy;
            let rock = Part::pivot()
                .at_v(s.center)
                .rot_y(yaw.to_degrees())
                .with(Part::ico(hx * 2.2, hy * 2.15, hz * 2.2).rot_y(r(1.0) * 72.0).rot_z(6.0 - r(2.0) * 12.0).color(tint(ROCK, variant)))
                // Stratum: a darker flattened layer cutting through the boulder.
                .with(Part::ico(hx * 2.35, hy * 0.6, hz * 2.3).at(0.0, -hy * 0.1, 0.0).rot_y(36.0).color(tint(ROCK_DARK, variant)))
                // Shoulder block leaning on one side, and a smaller boulder at the foot.
                .with(Part::ico(hx * 1.3, hy * 1.45, hz * 1.3).at(hx * 0.45, hy * 0.2, -hz * 0.3).rot_y(40.0).rot_x(14.0).color(tint(ROCK, 1.0 - variant)))
                .with(Part::ico(hx * 0.95, hy * 0.85, hz * 0.95).at(-hx * 0.55, ground + hy * 0.3, hz * 0.5).rot_z(22.0).color(tint(ROCK_DARK, 1.0 - variant)))
                .with(Part::ico(0.55, 0.32, 0.45).at(hx * 1.0, ground + 0.08, hz * 0.55).rot_y(30.0).color(tint(ROCK, variant)))
                .with(Part::ico(0.38, 0.24, 0.34).at(-hx * 0.2, ground + 0.06, -hz * 1.05).color(tint(ROCK_DARK, variant)));
            if variant < 0.7 {
                // Moss cap on the top facets.
                rock.with(Part::ico(hx * 1.6, hy * 0.45, hz * 1.5).at(-hx * 0.1, hy * 0.88, hz * 0.05).rot_y(r(3.0) * 72.0).color(tint(LEAF_A, variant)))
                    .with(Part::ico(hx * 0.7, hy * 0.3, hz * 0.7).at(hx * 0.5, hy * 0.85, -hz * 0.3).color(tint(LEAF_B, variant)))
            } else {
                rock
            }
        }
        // Broken wall: plinth, coursed body, a jagged crown of blocks (within 15% of
        // the full height), an arched window, ivy and fallen stones.
        (SolidKind::Ruin, Shape::Box { half, yaw }) => {
            let (hx, hy, hz) = (half.x, half.y, half.z);
            let joint = -hy + hy * 1.2;
            let mut wall = Part::pivot()
                .at_v(s.center)
                .rot_y(yaw.to_degrees())
                .with(Part::cube(hx * 2.0 + 0.3, 0.75, hz * 2.0 + 0.3).at(0.0, -hy + 0.37, 0.0).color(SANDSTONE_DARK))
                .with(Part::cube(hx * 2.0, hy * 1.2, hz * 2.0).at(0.0, -hy + hy * 0.6, 0.0).color(tint(SANDSTONE, variant)))
                // Courses: thin dark grooves around the wall.
                .with(Part::cube(hx * 2.0 + 0.04, 0.07, hz * 2.0 + 0.04).at(0.0, -hy + hy * 0.62, 0.0).color(SANDSTONE_DARK))
                .with(Part::cube(hx * 2.0 + 0.04, 0.09, hz * 2.0 + 0.04).at(0.0, joint, 0.0).color(SANDSTONE_DARK));
            // Crown: 4 blocks of uneven height, the tallest reaching the collision top.
            let tallest = (r(5.0) * 4.0) as usize % 4;
            let w = hx * 2.0 / 4.0;
            for i in 0..4 {
                let f = if i == tallest { 1.0 } else { 0.85 + r(6.0 + i as f32) * 0.13 };
                let top = -hy + hy * 2.0 * f;
                let x = -hx + w * (i as f32 + 0.5);
                let h = top - joint;
                let c = tint(SANDSTONE, r(10.0 + i as f32));
                wall = wall.with(Part::cube(w * 0.97, h, hz * 2.0 * 0.97).at(x, joint + h * 0.5, 0.0).color(c));
                // A capstone knocked askew on some blocks.
                if r(14.0 + i as f32) > 0.5 {
                    wall = wall.with(Part::cube(w * 0.8, 0.22, hz * 2.2).at(x, top - 0.1, 0.0).rot_y(r(18.0 + i as f32) * 12.0 - 6.0).color(SANDSTONE_DARK));
                }
            }
            if hx > 1.9 && hy > 1.5 {
                // Arched window: a dark recess through the wall.
                let wx = hx * (r(22.0) - 0.5) * 0.6;
                let wy = -hy * 0.15;
                wall = wall
                    .with(Part::cube(0.8, 1.0, hz * 2.0 + 0.04).at(wx, wy, 0.0).color(STONE_DARK))
                    .with(Part::cylinder(0.8, hz * 2.0 + 0.04).at(wx, wy + 0.5, 0.0).rot_x(90.0).color(STONE_DARK))
                    .with(Part::cube(1.1, 0.16, hz * 2.0 + 0.16).at(wx, wy - 0.55, 0.0).color(SANDSTONE_DARK));
            }
            // Ivy hanging from the top on both faces.
            for (i, side) in [1.0f32, -1.0].into_iter().enumerate() {
                let x = hx * (r(30.0 + i as f32) * 1.4 - 0.7);
                let len = hy * (0.5 + r(32.0 + i as f32) * 0.6);
                wall = wall
                    .with(Part::cube(0.5, len, 0.06).at(x, hy * 0.65 - len * 0.5, side * (hz + 0.03)).color(tint(LEAF_A, r(34.0))))
                    .with(Part::cube(0.35, len * 0.6, 0.06).at(x + 0.35, hy * 0.65 - len * 0.3, side * (hz + 0.03)).color(tint(LEAF_B, r(35.0))));
            }
            // Moss on top and fallen stones at the foot.
            wall.with(Part::cube(w * 1.5, 0.1, hz * 2.05).at(hx * 0.3, hy * 2.0 * 0.85 - hy + 0.02, 0.0).color(tint(LEAF_A, variant)))
                .with(Part::cube(0.7, 0.5, 0.6).at(hx * 0.6, -hy + 0.3, hz + 0.6).rot_y(25.0).rot_z(12.0).color(tint(SANDSTONE, 0.3)))
                .with(Part::cube(0.5, 0.35, 0.45).at(hx * 0.2, -hy + 0.25, hz + 0.9).rot_y(-30.0).color(SANDSTONE_DARK))
                .with(Part::cube(0.6, 0.4, 0.5).at(-hx * 0.7, -hy + 0.27, -hz - 0.5).rot_y(60.0).rot_x(10.0).color(tint(SANDSTONE, 0.8)))
        }
        // Floating slab: grassy top with drapes over the edges, a stone rim, and a
        // rocky floating-island underside with hanging crystals and roots.
        (SolidKind::Platform, Shape::Box { half, yaw }) => {
            let (hx, hy, hz) = (half.x, half.y, half.z);
            let m = hx.min(hz);
            let cr = if variant > 0.5 { CRYSTAL_CYAN } else { CRYSTAL_PINK };
            let grass = tint(LEAF_B, variant);
            let mut slab = Part::pivot()
                .at_v(s.center)
                .rot_y(yaw.to_degrees())
                .with(Part::cube(hx * 2.0, hy * 2.0, hz * 2.0).color(SANDSTONE))
                .with(Part::cube(hx * 2.0 + 0.04, 0.12, hz * 2.0 + 0.04).at(0.0, hy - 0.06, 0.0).color(grass))
                .with(Part::cube(hx * 2.0 + 0.16, hy * 0.6, hz * 2.0 + 0.16).at(0.0, -hy * 0.7, 0.0).color(SANDSTONE_DARK))
                // Floating-island underside.
                .with(Part::frustum(m * 1.8, 0.8).at(0.0, -hy - 0.4, 0.0).rot_x(180.0).rot_y(22.0).color(ROCK))
                .with(Part::cone(m * 1.0, 1.3).at(m * 0.15, -hy - 1.3, -m * 0.1).rot_x(180.0).color(ROCK_DARK))
                .with(Part::ico(m * 0.8, 0.8, m * 0.7).at(-m * 0.45, -hy - 0.55, m * 0.3).color(ROCK_DARK))
                .with(crystal_spike(0.28, 0.8, cr, 2.0).at(m * 0.45, -hy - 0.3, m * 0.25).rot_x(180.0).rot_z(15.0))
                .with(crystal_spike(0.22, 0.6, CRYSTAL_CYAN, 2.0).at(-m * 0.4, -hy - 0.3, -m * 0.35).rot_x(170.0))
                // Hanging roots.
                .with(Part::cube(0.07, 1.1, 0.07).at(hx * 0.7, -hy - 0.5, -hz * 0.5).color(LEAF_A))
                .with(Part::cube(0.07, 0.8, 0.07).at(-hx * 0.75, -hy - 0.35, hz * 0.6).color(LEAF_A));
            // Grass drapes over the edges (inverted triangles).
            for i in 0..8 {
                let t = r(40.0 + i as f32) * 1.6 - 0.8;
                let len = 0.3 + r(48.0 + i as f32) * 0.35;
                let c = tint(grass, r(56.0 + i as f32));
                let (x, z, face) = match i % 4 {
                    0 => (hx + 0.03, t * hz, 90.0),
                    1 => (-hx - 0.03, t * hz, 90.0),
                    2 => (t * hx, hz + 0.03, 0.0),
                    _ => (t * hx, -hz - 0.03, 0.0),
                };
                slab = slab.with(Part::prism(0.55, len, 0.06).at(x, hy - len * 0.5 + 0.02, z).rot_y(face).rot_z(180.0).color(c));
            }
            slab
        }
        // Ancient column: stepped plinth, fluted shaft in drums, echinus and abacus,
        // moss at the foot; the tall tower pillars carry a faint glowing rune band.
        (SolidKind::Pillar, Shape::Cylinder { radius, half_height }) => {
            let (rd, hh) = (radius, half_height);
            let body = tint(SANDSTONE, variant);
            let mut col = Part::pivot()
                .at_v(s.center)
                .with(Part::cube(rd * 3.0, 0.5, rd * 3.0).at(0.0, -hh + 0.25, 0.0).color(SANDSTONE_DARK))
                .with(Part::cube(rd * 2.6, 0.35, rd * 2.6).at(0.0, -hh + 0.67, 0.0).rot_y(45.0).color(tint(SANDSTONE_DARK, 0.8)))
                .with(Part::cylinder(rd * 2.35, 0.25).at(0.0, -hh + 0.97, 0.0).color(SANDSTONE_DARK))
                .with(Part::cylinder(rd * 2.0, hh * 2.0).rot_y(22.5).color(body))
                // Capital.
                .with(Part::frustum(rd * 3.0, 0.45).at(0.0, hh - 0.55, 0.0).rot_x(180.0).color(SANDSTONE_DARK))
                .with(Part::cube(rd * 2.9, 0.33, rd * 2.9).at(0.0, hh - 0.165, 0.0).color(tint(SANDSTONE, 1.0 - variant)))
                // Moss on the plinth.
                .with(Part::ico(rd * 1.4, 0.3, rd * 1.1).at(rd * 0.9, -hh + 0.5, rd * 0.7).color(LEAF_A))
                .with(Part::ico(rd * 0.8, 0.22, rd * 0.9).at(-rd * 0.8, -hh + 0.88, -rd * 0.6).color(LEAF_B));
            // Flutes: dark vertical strips on the shaft faces.
            let fl = hh * 2.0 - 2.0;
            if fl > 0.5 {
                for a in [0.0f32, 45.0, 90.0, 135.0] {
                    col = col.with(Part::cube(0.1, fl, rd * 2.0).at(0.0, 0.2, 0.0).rot_y(a).color(SANDSTONE_DARK));
                }
            }
            // Drum joints every ~2.4 units, a few with chipped (darker) stones.
            let drums = ((hh * 2.0 - 1.6) / 2.4) as i32;
            for i in 1..=drums.min(4) {
                let y = -hh + 1.1 + i as f32 * 2.4;
                col = col.with(Part::cylinder(rd * 2.08, 0.1).at(0.0, y, 0.0).rot_y(22.5).color(SANDSTONE_DARK));
            }
            if rd >= 0.9 {
                col = col.with(Part::cylinder(rd * 2.06, 0.14).at(0.0, hh * 0.45, 0.0).rot_y(22.5).color(TEAL_GLOW).glow(1.2));
            }
            col
        }
        // Tree trunk: root flare, three roots, a branch stub and a bracket fungus.
        (SolidKind::TreeTrunk, Shape::Cylinder { radius, half_height }) => {
            let (rd, hh) = (radius, half_height);
            let a = r(60.0) * 120.0;
            let mut trunk = Part::pivot()
                .at_v(s.center)
                .with(Part::cylinder(rd * 2.0, hh * 2.0).rot_y(a).color(BARK))
                .with(Part::frustum(rd * 3.4, 0.7).at(0.0, -hh + 0.35, 0.0).rot_y(a + 22.5).color(BARK))
                .with(Part::cube(rd * 0.35, hh * 1.2, rd * 2.05).at(0.0, -hh * 0.1, 0.0).rot_y(a + 30.0).color(WOOD_DARK))
                .with(Part::cylinder(rd * 0.6, rd * 2.6).at(rd * 1.3, hh * 0.35, 0.0).rot_z(-55.0).color(BARK))
                .with(Part::cylinder(rd * 1.1, 0.07).at(-rd * 0.95, -hh * 0.25, rd * 0.2).color(BONE))
                .with(Part::cylinder(rd * 0.75, 0.06).at(-rd * 0.85, -hh * 0.25 + 0.22, rd * 0.35).color(BONE));
            for i in 0..3 {
                let yaw = a + i as f32 * 120.0 + 40.0;
                trunk = trunk.with(
                    Part::pivot()
                        .at(0.0, -hh, 0.0)
                        .rot_y(yaw)
                        .with(Part::cube(rd * 0.7, rd * 0.6, rd * 2.4).at(0.0, 0.1, rd * 1.4).rot_x(18.0).color(tint(BARK, 0.2))),
                );
            }
            trunk
        }
        // Crystal cluster: a tall main spike, tilted satellites and a dark rock base.
        // The ground is at y = -0.8 * half_height (the generator buries the base).
        (SolidKind::Crystal, Shape::Cylinder { radius, half_height }) => {
            let (rd, hh) = (radius, half_height);
            let color = if variant > 0.5 { CRYSTAL_CYAN } else { CRYSTAL_PINK };
            let other = if variant > 0.5 { CRYSTAL_PINK } else { CRYSTAL_CYAN };
            let ground = -0.8 * hh;
            let mut cl = Part::pivot()
                .at_v(s.center)
                .with(crystal_spike(rd * 1.5, hh * 1.95, color, 1.4).at(0.0, -hh, 0.0).rot_y(r(70.0) * 45.0).rot_z(4.0));
            let sats = [(0.75, 0.55, 28.0, 0.0), (0.6, 0.45, 32.0, 130.0), (0.5, 0.32, 40.0, 230.0), (0.45, 0.25, 50.0, 300.0)];
            for (i, (dk, hk, tilt, yaw)) in sats.into_iter().enumerate() {
                let c = if i == 2 { other } else { color };
                cl = cl.with(
                    Part::pivot()
                        .at(0.0, ground, 0.0)
                        .rot_y(yaw + r(71.0 + i as f32) * 30.0)
                        .with(crystal_spike(rd * dk, hh * 2.0 * hk, c, 1.4).at(rd * 0.35, -0.2, 0.0).rot_z(-tilt)),
                );
            }
            cl.with(Part::ico(rd * 2.6, hh * 0.45, rd * 2.4).at(0.0, ground, 0.0).rot_y(20.0).color(ROCK_DARK))
                .with(Part::ico(rd * 1.2, hh * 0.3, rd * 1.0).at(rd * 1.0, ground + 0.05, -rd * 0.6).color(ROCK))
                .with(Part::ico(rd * 0.8, hh * 0.2, rd * 0.9).at(-rd * 1.1, ground, rd * 0.4).color(ROCK))
        }
        // Fallback (shape/kind combos not produced by the generator).
        (_, Shape::Box { half, yaw }) => Part::cube(half.x * 2.0, half.y * 2.0, half.z * 2.0)
            .at_v(s.center)
            .rot_y(yaw.to_degrees())
            .color(ROCK),
        (_, Shape::Cylinder { radius, half_height }) => Part::cylinder(radius * 2.0, half_height * 2.0).at_v(s.center).color(ROCK),
    }
}

/// Jump pad: a stone drum with a metal plate, a glowing ring, four rim pylons and a
/// stacked up-arrow (stays flat enough to run across).
pub fn jump_pad(pos: Vec3, radius: f32) -> Part {
    let d = radius * 2.0;
    let mut pad = Part::pivot()
        .at_v(pos)
        .with(Part::cylinder(d + 0.7, 0.3).at(0.0, 0.05, 0.0).rot_y(22.5).color(STONE))
        .with(Part::cylinder(d + 0.25, 0.12).at(0.0, 0.24, 0.0).color(METAL_DARK))
        .with(Part::torus(d, 0.18).at(0.0, 0.3, 0.0).color(TEAL_GLOW).glow(3.0))
        .with(Part::cylinder(d * 0.62, 0.06).at(0.0, 0.3, 0.0).color(METAL_LIGHT))
        .with(Part::cylinder(d * 0.45, 0.08).at(0.0, 0.32, 0.0).color(TEAL_GLOW).glow(2.0))
        .with(Part::cone(0.6, 0.3).at(0.0, 0.55, 0.0).color(TEAL_GLOW).glow(3.0))
        .with(Part::cone(0.42, 0.24).at(0.0, 0.85, 0.0).color(TEAL_GLOW).glow(3.5));
    for i in 0..4 {
        let a = (i as f32 * 90.0 + 45.0).to_radians();
        let rr = d * 0.5 + 0.22;
        pad = pad
            .with(Part::cube(0.26, 0.4, 0.26).at(a.cos() * rr, 0.3, a.sin() * rr).rot_y(-(i as f32) * 90.0 - 45.0).color(METAL_LIGHT))
            .with(Part::cube(0.14, 0.08, 0.14).at(a.cos() * rr, 0.54, a.sin() * rr).color(TEAL_GLOW).glow(2.5));
    }
    pad
}

pub fn decor(d: &Decor) -> Part {
    let v = d.variant;
    let base = Part::pivot().at_v(d.pos).rot_y(d.yaw.to_degrees());
    let s = d.scale;
    match d.kind {
        DecorKind::Grass => {
            // A tuft of flat blades fanning out, a seed stalk on some.
            let c = if v > 0.5 { LEAF_B } else { LEAF_A };
            let mut tuft = base;
            for (i, shade) in [0.6, 0.2, 1.0, 0.4, 0.8].into_iter().enumerate() {
                let h = (0.32 + vary(v, i as f32) * 0.25) * s;
                let lean = if i == 0 { 0.0 } else { 14.0 + vary(v, 9.0 + i as f32) * 14.0 };
                tuft = tuft.with(
                    Part::pivot()
                        .rot_y(i as f32 * 72.0 + 20.0)
                        .with(Part::prism(0.12 * s, h, 0.04 * s).at(0.0, h * 0.5, 0.0).rot_z(lean).color(tint(c, shade))),
                );
            }
            if vary(v, 20.0) > 0.6 {
                tuft = tuft
                    .with(Part::cube(0.025 * s, 0.6 * s, 0.025 * s).at(0.03 * s, 0.3 * s, 0.0).rot_z(-6.0).color(tint(LEAF_B, 0.9)))
                    .with(Part::ico(0.07 * s, 0.14 * s, 0.07 * s).at(0.06 * s, 0.62 * s, 0.0).color(BONE));
            }
            tuft
        }
        DecorKind::Flower => {
            let petals = [rgb(1.0, 0.55, 0.7), rgb(1.0, 0.85, 0.35), rgb(0.95, 0.95, 1.0), rgb(0.7, 0.55, 1.0)];
            let c = petals[(v * 4.0) as usize % 4];
            let bloom = |sz: f32| {
                let mut b = Part::pivot().with(Part::ico(0.08 * sz, 0.06 * sz, 0.08 * sz).at(0.0, 0.02 * sz, 0.0).color(GOLD));
                for i in 0..5 {
                    let a = (i as f32 * 72.0).to_radians();
                    b = b.with(
                        Part::ico(0.1 * sz, 0.035 * sz, 0.065 * sz)
                            .at(a.cos() * 0.07 * sz, 0.0, a.sin() * 0.07 * sz)
                            .rot_y(-(i as f32) * 72.0)
                            .rot_z(12.0)
                            .color(if i % 2 == 0 { c } else { tint(c, 0.3) }),
                    );
                }
                b
            };
            let mut f = base
                .with(Part::cube(0.03 * s, 0.42 * s, 0.03 * s).at(0.0, 0.21 * s, 0.0).color(LEAF_A))
                .with(Part::prism(0.1 * s, 0.22 * s, 0.03 * s).at(0.06 * s, 0.1 * s, 0.0).rot_z(-30.0).color(LEAF_B))
                .with(Part::prism(0.1 * s, 0.18 * s, 0.03 * s).at(-0.05 * s, 0.08 * s, 0.02 * s).rot_y(70.0).rot_z(28.0).color(tint(LEAF_A, 0.7)))
                .with(bloom(s).at(0.0, 0.43 * s, 0.0).rot_x(10.0));
            if vary(v, 3.0) > 0.35 {
                // A smaller second bloom on a shorter stem.
                f = f
                    .with(Part::cube(0.025 * s, 0.28 * s, 0.025 * s).at(0.09 * s, 0.13 * s, 0.05 * s).rot_z(-14.0).color(LEAF_A))
                    .with(bloom(0.7 * s).at(0.13 * s, 0.28 * s, 0.05 * s).rot_z(-14.0));
            }
            f
        }
        DecorKind::Pebble => base
            .with(Part::ico(0.35 * s, 0.18 * s, 0.28 * s).at(0.0, 0.05, 0.0).rot_z(8.0).color(tint(ROCK, v)))
            .with(Part::ico(0.18 * s, 0.11 * s, 0.16 * s).at(0.22 * s, 0.03, 0.1 * s).rot_y(40.0).color(tint(ROCK_DARK, v))),
        DecorKind::Mushroom => {
            // Red spotted caps, or glowing cyan ones; a smaller sibling next to it.
            let glowing = v > 0.6;
            let cap = if glowing { GLOW_CYAN } else { rgb(0.86, 0.3, 0.25) };
            let glow = if glowing { 1.5 } else { 0.0 };
            let mut m = base
                .with(Part::cylinder(0.1 * s, 0.26 * s).at(0.0, 0.13 * s, 0.0).color(BONE))
                .with(Part::cylinder(0.3 * s, 0.03 * s).at(0.0, 0.25 * s, 0.0).color(tint(BONE, 0.0)))
                .with(Part::ellipsoid(0.38 * s, 0.2 * s, 0.38 * s).at(0.0, 0.29 * s, 0.0).color(cap).glow(glow))
                .with(Part::cylinder(0.06 * s, 0.13 * s).at(0.14 * s, 0.065 * s, 0.06 * s).rot_z(-10.0).color(BONE))
                .with(Part::ellipsoid(0.18 * s, 0.1 * s, 0.18 * s).at(0.155 * s, 0.14 * s, 0.06 * s).rot_z(-10.0).color(cap).glow(glow));
            if !glowing {
                for (x, z) in [(0.08, 0.04), (-0.06, 0.08), (0.0, -0.1)] {
                    m = m.with(Part::ico(0.06 * s, 0.03 * s, 0.06 * s).at(x * s, 0.375 * s, z * s).color(BONE));
                }
            }
            m
        }
        DecorKind::Shard => {
            let c = if v > 0.5 { CRYSTAL_CYAN } else { CRYSTAL_PINK };
            base.with(crystal_spike(0.15 * s, 0.6 * s, c, 1.8).at(0.0, -0.05, 0.0).rot_z(10.0))
                .with(crystal_spike(0.1 * s, 0.35 * s, c, 1.8).at(0.08 * s, -0.03, 0.04 * s).rot_x(-14.0).rot_z(-32.0))
                .with(Part::ico(0.3 * s, 0.12 * s, 0.26 * s).at(0.0, 0.02, 0.0).color(ROCK_DARK))
        }
        DecorKind::Canopy => {
            if v < 0.45 {
                // Pine: four staggered tiers, darker skirt, lighter top.
                let c = if v < 0.22 { LEAF_A } else { tint(LEAF_A, 0.3) };
                let k = 0.9 + vary(v, 1.0) * 0.25;
                base.with(Part::cylinder(0.25 * s, 0.8 * s).at(0.0, -0.1 * s, 0.0).color(BARK))
                    .with(Part::cone(2.5 * s, 1.3 * s * k).at(0.0, 0.15 * s, 0.0).color(tint(c, 0.0)))
                    .with(Part::cone(2.0 * s, 1.2 * s * k).at(0.0, 0.75 * s * k, 0.0).rot_y(22.5).color(tint(c, 0.6)))
                    .with(Part::cone(1.5 * s, 1.1 * s * k).at(0.0, 1.3 * s * k, 0.0).color(tint(c, 0.9)))
                    .with(Part::cone(0.95 * s, 1.0 * s * k).at(0.0, 1.85 * s * k, 0.0).rot_y(22.5).color(lift(c, 0.12)))
            } else if v < 0.72 {
                // Round broadleaf, sometimes with red fruit.
                let crown = round_crown(base, s, LEAF_B, v);
                if v > 0.62 {
                    let fruit = rgb(0.9, 0.28, 0.22);
                    crown
                        .with(Part::ico(0.16 * s, 0.16 * s, 0.16 * s).at(1.0 * s, 0.5 * s, 0.6 * s).color(fruit))
                        .with(Part::ico(0.16 * s, 0.16 * s, 0.16 * s).at(-0.9 * s, 0.6 * s, 0.5 * s).color(fruit))
                        .with(Part::ico(0.16 * s, 0.16 * s, 0.16 * s).at(0.3 * s, 0.4 * s, -1.0 * s).color(fruit))
                } else {
                    crown
                }
            } else if v < 0.86 {
                // Blossom tree.
                round_crown(base, s, rgb(0.98, 0.68, 0.8), v)
            } else {
                round_crown(base, s, LEAF_AUTUMN, v)
            }
        }
    }
}
