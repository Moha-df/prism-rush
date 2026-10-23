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
use crate::net::protocol::ProjectileKind;
use crate::shared::economy::ChestKind;
use crate::shared::mobs::MobKind;
use crate::shared::weapons::WeaponKind;
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
    }
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

pub fn chest(kind: ChestKind) -> Part {
    match kind {
        ChestKind::Item => {
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
pub fn muzzle(kind: WeaponKind) -> Vec3 {
    match kind {
        WeaponKind::Pistol => Vec3::new(0.0, 0.03, -0.32),
        WeaponKind::Smg => Vec3::new(0.0, 0.03, -0.44),
        WeaponKind::Shotgun => Vec3::new(0.0, 0.04, -0.6),
        WeaponKind::AssaultRifle => Vec3::new(0.0, 0.04, -0.62),
        WeaponKind::Sniper => Vec3::new(0.0, 0.04, -0.92),
        WeaponKind::RocketLauncher => Vec3::new(0.0, 0.07, -0.64),
    }
}

pub fn weapon(kind: WeaponKind, accent: Color) -> Part {
    let grip = Part::cube(0.07, 0.17, 0.09).at(0.0, -0.08, 0.03).rot_x(-15.0).color(METAL_DARK);
    match kind {
        WeaponKind::Pistol => Part::pivot()
            .with(grip)
            .with(Part::cube(0.08, 0.09, 0.28).at(0.0, 0.02, -0.08).color(METAL_DARK))
            .with(Part::cube(0.085, 0.05, 0.3).at(0.0, 0.075, -0.09).color(METAL_LIGHT).tag(AnimTag::Recoil))
            .with(Part::cylinder(0.045, 0.1).at(0.0, 0.03, -0.26).rot_x(90.0).color(METAL_DARK))
            .with(Part::cube(0.02, 0.02, 0.03).at(0.0, 0.11, -0.21).color(accent).glow(4.0))
            .with(Part::cube(0.086, 0.015, 0.2).at(0.0, 0.0, -0.08).color(accent).glow(2.0)),
        WeaponKind::Smg => Part::pivot()
            .with(grip)
            .with(Part::cube(0.1, 0.12, 0.38).at(0.0, 0.03, -0.1).color(METAL_DARK))
            .with(Part::cube(0.06, 0.24, 0.08).at(0.0, -0.12, -0.14).color(METAL_LIGHT))
            .with(Part::cylinder(0.05, 0.16).at(0.0, 0.03, -0.36).rot_x(90.0).color(METAL_DARK))
            .with(Part::cube(0.05, 0.05, 0.22).at(0.0, 0.0, 0.17).color(METAL_LIGHT))
            .with(Part::cube(0.102, 0.02, 0.3).at(0.0, 0.07, -0.1).color(accent).glow(2.5))
            .with(Part::cube(0.04, 0.05, 0.1).at(0.0, 0.11, -0.05).color(METAL_LIGHT).tag(AnimTag::Recoil)),
        WeaponKind::Shotgun => Part::pivot()
            .with(grip)
            .with(Part::cylinder(0.065, 0.58).at(0.036, 0.05, -0.32).rot_x(90.0).color(METAL_DARK))
            .with(Part::cylinder(0.065, 0.58).at(-0.036, 0.05, -0.32).rot_x(90.0).color(METAL_DARK))
            .with(Part::cube(0.12, 0.08, 0.2).at(0.0, -0.01, -0.32).color(WOOD).tag(AnimTag::Recoil))
            .with(Part::cube(0.12, 0.13, 0.22).at(0.0, 0.03, -0.01).color(METAL_LIGHT))
            .with(Part::cube(0.1, 0.13, 0.32).at(0.0, -0.04, 0.24).rot_x(10.0).color(WOOD))
            .with(Part::torus(0.16, 0.03).at(0.0, 0.05, -0.55).rot_x(90.0).color(accent).glow(3.0)),
        WeaponKind::AssaultRifle => Part::pivot()
            .with(grip)
            .with(Part::cube(0.09, 0.13, 0.46).at(0.0, 0.03, -0.1).color(METAL_DARK))
            .with(Part::cube(0.1, 0.1, 0.24).at(0.0, 0.04, -0.4).color(METAL_LIGHT))
            .with(Part::cylinder(0.045, 0.2).at(0.0, 0.04, -0.55).rot_x(90.0).color(METAL_DARK))
            .with(Part::cube(0.06, 0.22, 0.09).at(0.0, -0.13, -0.1).rot_x(15.0).color(METAL_LIGHT))
            .with(Part::cube(0.08, 0.13, 0.26).at(0.0, -0.0, 0.24).color(METAL_DARK))
            .with(Part::cylinder(0.055, 0.16).at(0.0, 0.14, -0.08).rot_x(90.0).color(METAL_DARK))
            .with(Part::cylinder(0.045, 0.01).at(0.0, 0.14, -0.165).rot_x(90.0).color(accent).glow(4.0))
            .with(Part::cube(0.092, 0.02, 0.3).at(0.0, -0.02, -0.12).color(accent).glow(2.5))
            .with(Part::cube(0.03, 0.04, 0.08).at(0.05, 0.06, -0.02).color(METAL_LIGHT).tag(AnimTag::Recoil)),
        WeaponKind::Sniper => Part::pivot()
            .with(grip)
            .with(Part::cube(0.09, 0.12, 0.5).at(0.0, 0.03, -0.08).color(METAL_DARK))
            .with(Part::cylinder(0.045, 0.62).at(0.0, 0.04, -0.6).rot_x(90.0).color(METAL_DARK))
            .with(Part::cube(0.07, 0.065, 0.09).at(0.0, 0.04, -0.88).color(METAL_LIGHT))
            .with(Part::cylinder(0.085, 0.34).at(0.0, 0.15, -0.12).rot_x(90.0).color(METAL_LIGHT))
            .with(Part::cylinder(0.075, 0.01).at(0.0, 0.15, -0.295).rot_x(90.0).color(accent).glow(5.0))
            .with(Part::cube(0.08, 0.14, 0.3).at(0.0, -0.01, 0.26).rot_x(6.0).color(METAL_DARK))
            .with(Part::cube(0.092, 0.02, 0.4).at(0.0, -0.02, -0.12).color(accent).glow(2.5))
            .with(Part::cylinder(0.04, 0.06).at(0.06, 0.06, 0.0).rot_z(90.0).color(METAL_LIGHT).tag(AnimTag::Recoil)),
        WeaponKind::RocketLauncher => Part::pivot()
            .with(grip)
            .with(Part::cylinder(0.2, 0.82).at(0.0, 0.07, -0.2).rot_x(90.0).color(rgb(0.24, 0.32, 0.26)))
            .with(Part::torus(0.24, 0.06).at(0.0, 0.07, -0.6).rot_x(90.0).color(METAL_DARK))
            .with(Part::frustum(0.26, 0.14).at(0.0, 0.07, 0.24).rot_x(-90.0).color(METAL_DARK))
            .with(Part::cone(0.12, 0.16).at(0.0, 0.07, -0.56).rot_x(-90.0).color(GLOW_ORANGE).glow(3.0))
            .with(Part::cube(0.05, 0.08, 0.1).at(0.0, 0.2, -0.15).color(METAL_DARK))
            .with(Part::cube(0.02, 0.02, 0.02).at(0.0, 0.25, -0.15).color(accent).glow(4.0))
            .with(Part::torus(0.21, 0.03).at(0.0, 0.07, -0.1).rot_x(90.0).color(accent).glow(2.5)),
    }
}

// ---------------------------------------------------------------------------
// Projectiles (oriented along -Z).
// ---------------------------------------------------------------------------

pub fn projectile(kind: ProjectileKind) -> Part {
    match kind {
        ProjectileKind::Rocket => Part::pivot()
            .with(Part::cylinder(0.13, 0.5).rot_x(90.0).color(METAL_LIGHT))
            .with(Part::cone(0.13, 0.2).at(0.0, 0.0, -0.34).rot_x(-90.0).color(rgb(0.9, 0.25, 0.2)))
            .with(Part::cone(0.16, 0.4).at(0.0, 0.0, 0.42).rot_x(90.0).color(GLOW_ORANGE).glow(6.0)),
        ProjectileKind::Acid => Part::pivot()
            .with(Part::sphere(0.45).color(GLOW_TOXIC).glow(3.5))
            .with(Part::ico(0.25, 0.25, 0.25).at(0.0, 0.1, 0.25).color(GLOW_TOXIC).glow(2.0)),
        ProjectileKind::Bolt => Part::pivot().with(Part::ellipsoid(0.25, 0.25, 0.8).color(GLOW_CYAN).glow(5.0)),
        ProjectileKind::BossOrb => Part::pivot()
            .with(Part::sphere(1.0).color(GLOW_ORANGE).glow(5.0))
            .with(Part::pivot().tag(AnimTag::Spin(6.0)).with(Part::torus(1.4, 0.12).rot_x(80.0).color(STONE_DARK))),
    }
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
