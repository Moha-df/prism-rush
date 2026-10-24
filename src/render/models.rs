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

/// Model for a collision object. Visual parts stay inside the collision volume.
pub fn solid(s: &Solid, variant: f32) -> Part {
    match (s.kind, s.shape) {
        (SolidKind::Rock, Shape::Box { half, yaw }) => Part::pivot()
            .at_v(s.center)
            .rot_y(yaw.to_degrees())
            // Icosahedrons read as boulders much better than boxes.
            .with(Part::ico(half.x * 2.3, half.y * 2.3, half.z * 2.3).color(tint(ROCK, variant)))
            .with(
                Part::ico(half.x * 1.4, half.y * 1.3, half.z * 1.4)
                    .at(half.x * 0.35, half.y * 0.45, -half.z * 0.2)
                    .rot_y(40.0)
                    .color(tint(ROCK_DARK, variant)),
            )
            .with(Part::ico(half.x * 1.0, half.y * 0.9, half.z * 1.0).at(-half.x * 0.45, half.y * 0.3, half.z * 0.4).rot_z(20.0).color(tint(ROCK, 1.0 - variant))),
        (SolidKind::Ruin, Shape::Box { half, yaw }) => Part::pivot()
            .at_v(s.center)
            .rot_y(yaw.to_degrees())
            .with(Part::cube(half.x * 2.0, half.y * 2.0, half.z * 2.0).color(tint(SANDSTONE, variant)))
            .with(Part::cube(half.x * 0.8, 0.35, half.z * 2.3).at(-half.x * 0.5, half.y + 0.17, 0.0).color(SANDSTONE_DARK))
            .with(Part::cube(half.x * 0.6, 0.3, half.z * 2.1).at(half.x * 0.3, half.y + 0.15, 0.0).rot_y(5.0).color(SANDSTONE))
            .with(Part::cube(half.x * 2.05, 0.15, half.z * 2.2).at(0.0, -half.y * 0.3, 0.0).color(SANDSTONE_DARK)),
        (SolidKind::Platform, Shape::Box { half, yaw }) => Part::pivot()
            .at_v(s.center)
            .rot_y(yaw.to_degrees())
            .with(Part::cube(half.x * 2.0, half.y * 2.0, half.z * 2.0).color(SANDSTONE))
            .with(Part::cube(half.x * 2.02, 0.08, half.z * 2.02).at(0.0, half.y - 0.04, 0.0).color(tint(LEAF_B, variant)))
            .with(Part::frustum(half.x.min(half.z) * 1.6, 0.9).at(0.0, -half.y - 0.45, 0.0).rot_x(180.0).color(SANDSTONE_DARK))
            .with(Part::ico(0.3, 0.7, 0.3).at(half.x * 0.5, -half.y - 0.5, half.z * 0.3).color(if variant > 0.5 { CRYSTAL_CYAN } else { CRYSTAL_PINK }).glow(2.0))
            .with(Part::ico(0.25, 0.55, 0.25).at(-half.x * 0.4, -half.y - 0.4, -half.z * 0.4).color(CRYSTAL_CYAN).glow(2.0)),
        (SolidKind::Pillar, Shape::Cylinder { radius, half_height }) => Part::pivot()
            .at_v(s.center)
            .with(Part::cylinder(radius * 2.0, half_height * 2.0).color(tint(SANDSTONE, variant)))
            .with(Part::cylinder(radius * 2.5, 0.3).at(0.0, half_height - 0.15, 0.0).color(SANDSTONE_DARK))
            .with(Part::cylinder(radius * 2.4, 0.35).at(0.0, -half_height + 0.17, 0.0).color(SANDSTONE_DARK))
            .with(Part::torus(radius * 2.15, 0.12).at(0.0, 0.0, 0.0).color(SANDSTONE_DARK)),
        (SolidKind::TreeTrunk, Shape::Cylinder { radius, half_height }) => Part::pivot()
            .at_v(s.center)
            .with(Part::cylinder(radius * 2.0, half_height * 2.0).color(BARK))
            .with(Part::cone(radius * 2.6, 0.7).at(0.0, -half_height + 0.3, 0.0).color(BARK)),
        (SolidKind::Crystal, Shape::Cylinder { radius, half_height }) => {
            let color = if variant > 0.5 { CRYSTAL_CYAN } else { CRYSTAL_PINK };
            Part::pivot()
                .at_v(s.center)
                .with(Part::ico(radius * 2.0, half_height * 2.0, radius * 2.0).color(color).glow(1.4))
                .with(Part::ico(radius, half_height, radius).at(radius * 0.8, -half_height * 0.5, 0.0).rot_z(-25.0).color(color).glow(1.4))
                .with(Part::ico(radius * 1.6, 0.5, radius * 1.6).at(0.0, -half_height * 0.9, 0.0).color(ROCK_DARK))
        }
        // Fallback (shape/kind combos not produced by the generator).
        (_, Shape::Box { half, yaw }) => Part::cube(half.x * 2.0, half.y * 2.0, half.z * 2.0)
            .at_v(s.center)
            .rot_y(yaw.to_degrees())
            .color(ROCK),
        (_, Shape::Cylinder { radius, half_height }) => Part::cylinder(radius * 2.0, half_height * 2.0).at_v(s.center).color(ROCK),
    }
}

pub fn jump_pad(pos: Vec3, radius: f32) -> Part {
    let d = radius * 2.0;
    Part::pivot()
        .at_v(pos)
        .with(Part::cylinder(d + 0.4, 0.25).at(0.0, 0.05, 0.0).color(METAL_DARK))
        .with(Part::torus(d, 0.18).at(0.0, 0.2, 0.0).color(TEAL_GLOW).glow(3.0))
        .with(Part::cylinder(d * 0.6, 0.06).at(0.0, 0.2, 0.0).color(TEAL_GLOW).glow(2.0))
        .with(Part::cone(0.35, 0.4).at(0.0, 0.45, 0.0).color(TEAL_GLOW).glow(3.0))
}

pub fn decor(d: &Decor) -> Part {
    let v = d.variant;
    let base = Part::pivot().at_v(d.pos).rot_y(d.yaw.to_degrees());
    let s = d.scale;
    match d.kind {
        DecorKind::Grass => {
            let c = if v > 0.5 { LEAF_B } else { LEAF_A };
            base.with(Part::cone(0.12 * s, 0.5 * s).at(0.0, 0.25 * s, 0.0).color(c))
                .with(Part::cone(0.1 * s, 0.4 * s).at(0.1 * s, 0.2 * s, 0.05).rot_z(-15.0).color(tint(c, 0.8)))
                .with(Part::cone(0.1 * s, 0.35 * s).at(-0.08 * s, 0.17 * s, -0.04).rot_z(18.0).color(tint(c, 0.2)))
        }
        DecorKind::Flower => {
            let petals = [rgb(1.0, 0.55, 0.7), rgb(1.0, 0.85, 0.35), rgb(0.95, 0.95, 1.0), rgb(0.7, 0.55, 1.0)];
            let c = petals[(v * 4.0) as usize % 4];
            base.with(Part::cylinder(0.03, 0.4 * s).at(0.0, 0.2 * s, 0.0).color(LEAF_A))
                .with(Part::ico(0.16 * s, 0.1 * s, 0.16 * s).at(0.0, 0.42 * s, 0.0).color(c))
                .with(Part::ico(0.06 * s, 0.06 * s, 0.06 * s).at(0.0, 0.46 * s, 0.0).color(GOLD))
        }
        DecorKind::Pebble => base.with(Part::ico(0.35 * s, 0.18 * s, 0.28 * s).at(0.0, 0.05, 0.0).color(tint(ROCK, v))),
        DecorKind::Mushroom => {
            let cap = if v > 0.6 { GLOW_CYAN } else { rgb(0.86, 0.3, 0.25) };
            let glow = if v > 0.6 { 1.5 } else { 0.0 };
            base.with(Part::cylinder(0.1 * s, 0.25 * s).at(0.0, 0.12 * s, 0.0).color(BONE))
                .with(Part::ellipsoid(0.36 * s, 0.2 * s, 0.36 * s).at(0.0, 0.27 * s, 0.0).color(cap).glow(glow))
        }
        DecorKind::Shard => {
            let c = if v > 0.5 { CRYSTAL_CYAN } else { CRYSTAL_PINK };
            base.with(Part::ico(0.18 * s, 0.6 * s, 0.18 * s).at(0.0, 0.25 * s, 0.0).rot_z(12.0).color(c).glow(1.8))
        }
        DecorKind::Canopy => {
            if v < 0.55 {
                // Pine: stacked cones.
                let c = if v < 0.25 { LEAF_A } else { tint(LEAF_A, 0.3) };
                base.with(Part::cone(2.2 * s, 1.6 * s).at(0.0, 0.3 * s, 0.0).color(c))
                    .with(Part::cone(1.7 * s, 1.4 * s).at(0.0, 1.1 * s, 0.0).color(tint(c, 0.7)))
                    .with(Part::cone(1.1 * s, 1.2 * s).at(0.0, 1.8 * s, 0.0).color(tint(c, 1.0)))
            } else {
                // Round tree, sometimes autumn colored.
                let c = if v > 0.85 { LEAF_AUTUMN } else { LEAF_B };
                base.with(Part::ico(2.2 * s, 1.8 * s, 2.2 * s).at(0.0, 0.6 * s, 0.0).color(c))
                    .with(Part::ico(1.4 * s, 1.2 * s, 1.4 * s).at(0.6 * s, 1.2 * s, 0.3 * s).color(tint(c, 0.8)))
                    .with(Part::ico(1.2 * s, 1.0 * s, 1.2 * s).at(-0.6 * s, 0.9 * s, -0.4 * s).color(tint(c, 0.2)))
            }
        }
    }
}
