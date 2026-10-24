//! Models of the V4 monsters, built from primitives like the others
//! (see `models.rs`). They face -Z.
//!
//! Convention: **what glows orange/pink/cyan on a big monster is a weak
//! point** (or a leg to break). The positions match the hit zones of
//! `shared::mobs::MobKind::zones`.

use bevy::prelude::*;

use super::model::{AnimTag, Part};
use super::models::scale_part;
use super::palette::*;
use crate::shared::mobs::BROOD_LEGS;

/// A cylinder going from `from` to `to` (limbs at any angle).
fn limb(from: Vec3, to: Vec3, d: f32, color: Color) -> Part {
    let v = to - from;
    let mut p = Part::cylinder(d, v.length()).at_v((from + to) * 0.5).color(color);
    p.rot = Quat::from_rotation_arc(Vec3::Y, v.normalize_or(Vec3::Y));
    p
}

fn glowing_eyes(x: f32, y: f32, z: f32, size: f32, color: Color) -> [Part; 2] {
    [
        Part::ico(size, size, size).at(x, y, z).color(color).glow(5.0),
        Part::ico(size, size, size).at(-x, y, z).color(color).glow(5.0),
    ]
}

// ---------------------------------------------------------------------------
// Swarmers
// ---------------------------------------------------------------------------

/// Flea: hunched brown body on huge folded back legs.
pub fn flea() -> Part {
    let brown = rgb(0.55, 0.32, 0.18);
    let hind = Part::pivot()
        .at(0.24, 0.38, 0.12)
        .tag(AnimTag::LegL)
        .with(limb(Vec3::ZERO, Vec3::new(0.12, 0.2, 0.35), 0.13, brown))
        .with(limb(Vec3::new(0.12, 0.2, 0.35), Vec3::new(0.14, -0.36, 0.3), 0.09, PLUM_DARK));
    let front = Part::pivot().at(0.16, 0.34, -0.22).tag(AnimTag::LegR).with(limb(Vec3::ZERO, Vec3::new(0.08, -0.32, -0.08), 0.06, PLUM_DARK));
    let body = Part::pivot()
        .at(0.0, 0.48, 0.0)
        .tag(AnimTag::Body)
        .with(Part::ellipsoid(0.5, 0.58, 0.78).rot_x(-20.0).color(brown))
        .with(Part::ellipsoid(0.42, 0.2, 0.6).at(0.0, 0.22, 0.05).rot_x(-20.0).color(rgb(0.7, 0.45, 0.25)))
        .with(Part::ellipsoid(0.32, 0.28, 0.32).at(0.0, -0.02, -0.4).color(PLUM_DARK))
        .with_all(glowing_eyes(0.1, 0.04, -0.52, 0.1, rgb(1.0, 0.2, 0.15)))
        .with(Part::cone(0.06, 0.3).at(0.0, -0.12, -0.56).rot_x(-120.0).color(BONE));
    Part::pivot().with(hind.mirrored_x()).with(hind).with(front.mirrored_x()).with(front).with(body)
}

/// Burrower: a mole-worm with a drill nose and digging claws.
pub fn burrower() -> Part {
    let skin = rgb(0.62, 0.44, 0.40);
    let claw = Part::pivot()
        .at(0.32, 0.25, -0.25)
        .tag(AnimTag::LegL)
        .with(Part::cube(0.18, 0.1, 0.28).at(0.05, -0.12, -0.05).color(BONE))
        .with(Part::cone(0.08, 0.2).at(0.08, -0.15, -0.22).rot_x(-90.0).color(BONE));
    let mut drill = Part::pivot().at(0.0, 0.0, -0.55).tag(AnimTag::Spin(10.0)).with(Part::cone(0.42, 0.6).rot_x(-90.0).color(METAL_LIGHT));
    for i in 0..3 {
        drill = drill.with(Part::torus(0.36 - i as f32 * 0.1, 0.05).at(0.0, 0.0, -0.08 - i as f32 * 0.14).rot_x(90.0).color(GLOW_ORANGE).glow(2.0));
    }
    let body = Part::pivot()
        .at(0.0, 0.42, 0.0)
        .tag(AnimTag::Body)
        .with(Part::capsule(0.62, 1.2).rot_x(90.0).color(skin))
        .with(Part::ellipsoid(0.5, 0.2, 0.7).at(0.0, 0.22, 0.1).color(rgb(0.48, 0.32, 0.3)))
        .with_all(glowing_eyes(0.16, 0.12, -0.42, 0.08, GLOW_ORANGE))
        .with(drill);
    Part::pivot().with(claw.mirrored_x()).with(claw).with(body)
}

/// Scarab: metal-blue armored dome, a grey shell plate in front (armor) and
/// a soft glowing abdomen at the back (weak point ×2).
pub fn scarab() -> Part {
    let shell = rgb(0.18, 0.32, 0.42);
    let mut legs = Vec::new();
    for (i, z) in [-0.3f32, 0.05, 0.35].iter().enumerate() {
        let leg = Part::pivot()
            .at(0.34, 0.32, *z)
            .rot_y(-20.0 + i as f32 * 20.0)
            .tag(AnimTag::Leg(i as f32 / 3.0))
            .with(Part::cylinder(0.09, 0.45).at(0.2, 0.0, 0.0).rot_z(-70.0).color(PLUM_DARK))
            .with(Part::cone(0.09, 0.4).at(0.4, -0.2, 0.0).rot_z(180.0).color(shell));
        legs.push(leg.mirrored_x());
        legs.push(leg);
    }
    let body = Part::pivot()
        .at(0.0, 0.0, 0.0)
        .tag(AnimTag::Body)
        .with(Part::ellipsoid(1.15, 0.7, 1.3).at(0.0, 0.5, 0.0).color(shell))
        .with(Part::cube(0.06, 0.1, 1.2).at(0.0, 0.84, 0.0).color(GOLD))
        // Front shell: bullets bounce off it.
        .with(Part::ellipsoid(1.05, 0.75, 0.4).at(0.0, 0.5, -0.62).color(METAL_LIGHT))
        .with(Part::cone(0.22, 0.6).at(0.0, 0.85, -0.75).rot_x(-35.0).color(METAL_DARK))
        .with_all(glowing_eyes(0.22, 0.42, -0.82, 0.1, GLOW_CYAN))
        // Soft back: the weak point.
        .with(Part::ellipsoid(0.7, 0.55, 0.45).at(0.0, 0.55, 0.62).color(CRYSTAL_PINK).glow(2.5).tag(AnimTag::Pulse));
    Part::pivot().with_all(legs).with(body)
}

// ---------------------------------------------------------------------------
// Blobs
// ---------------------------------------------------------------------------

/// A slime with a core, bubbles and cartoon eyes, in any colors.
fn slime(body: Color, core: Color, bubble: Color, glow: f32) -> Part {
    let eye = |x: f32| {
        Part::pivot()
            .at(x, 0.85, -0.6)
            .with(Part::sphere(0.3).color(rgb(1.0, 1.0, 1.0)))
            .with(Part::sphere(0.14).at(0.0, 0.0, -0.12).color(PLUM_DARK))
    };
    Part::pivot().with(
        Part::pivot()
            .tag(AnimTag::Pulse)
            .with(Part::ellipsoid(1.6, 1.15, 1.6).at(0.0, 0.58, 0.0).color(body).glow(glow))
            .with(Part::ico(0.7, 0.6, 0.7).at(0.0, 0.55, 0.1).color(core))
            .with(Part::sphere(0.25).at(0.45, 1.05, 0.3).color(bubble).glow(1.5))
            .with(Part::sphere(0.18).at(-0.4, 1.0, 0.4).color(bubble).glow(1.5))
            .with(eye(0.3))
            .with(eye(-0.3))
            .with(Part::cube(0.4, 0.06, 0.05).at(0.0, 0.55, -0.79).color(PLUM_DARK)),
    )
}

pub fn toxic_jelly() -> Part {
    slime(rgb(0.62, 0.95, 0.15), rgb(0.25, 0.4, 0.05), GLOW_TOXIC, 0.6)
        .with(Part::sphere(0.2).at(0.1, 1.25, -0.1).color(GLOW_TOXIC).glow(3.0).tag(AnimTag::Bob))
}

/// Magma jelly: glowing orange with dark rock crust on top.
pub fn magma_jelly() -> Part {
    let mut p = slime(rgb(1.0, 0.42, 0.1), rgb(1.0, 0.85, 0.3), rgb(1.0, 0.9, 0.4), 1.6);
    for i in 0..4 {
        let a = i as f32 * 1.6;
        p = p.with(Part::ico(0.45, 0.25, 0.4).at(a.cos() * 0.4, 1.08, a.sin() * 0.4).color(STONE_DARK));
    }
    p
}

/// Giant jelly: a big purple slime wearing a little crown.
pub fn mega_jelly() -> Part {
    let mut crown = Part::pivot().at(0.0, 1.2, 0.0);
    for i in 0..5 {
        let a = i as f32 / 5.0 * std::f32::consts::TAU;
        crown = crown.with(Part::cone(0.14, 0.3).at(a.cos() * 0.22, 0.0, a.sin() * 0.22).color(GOLD).glow(1.0));
    }
    scale_part(slime(rgb(0.55, 0.4, 0.95), rgb(0.3, 0.2, 0.6), rgb(0.8, 0.7, 1.0), 0.5).with(crown), 2.0)
}

// ---------------------------------------------------------------------------
// Ranged
// ---------------------------------------------------------------------------

/// Mortar crab: wide shell, claws, and a cannon on its back.
pub fn mortar() -> Part {
    let red = rgb(0.75, 0.28, 0.2);
    let mut legs = Vec::new();
    for (i, z) in [-0.25f32, 0.15, 0.5].iter().enumerate() {
        let leg = Part::pivot()
            .at(0.6, 0.45, *z)
            .tag(AnimTag::Leg(i as f32 / 3.0))
            .with(limb(Vec3::ZERO, Vec3::new(0.45, 0.25, 0.0), 0.12, red))
            .with(limb(Vec3::new(0.45, 0.25, 0.0), Vec3::new(0.65, -0.45, 0.0), 0.09, PLUM_DARK));
        legs.push(leg.mirrored_x());
        legs.push(leg);
    }
    let claw = Part::pivot()
        .at(0.55, 0.55, -0.6)
        .tag(AnimTag::ArmL)
        .with(Part::ellipsoid(0.35, 0.3, 0.5).at(0.1, 0.0, -0.2).color(red))
        .with(Part::cone(0.18, 0.35).at(0.18, 0.08, -0.5).rot_x(-90.0).color(BONE));
    let body = Part::pivot()
        .tag(AnimTag::Body)
        .with(Part::ellipsoid(1.6, 0.6, 1.3).at(0.0, 0.6, 0.1).color(red))
        .with(Part::ellipsoid(1.2, 0.3, 1.0).at(0.0, 0.85, 0.1).color(rgb(0.88, 0.45, 0.3)))
        .with(Part::cylinder(0.12, 0.3).at(0.2, 0.85, -0.55).color(PLUM_DARK))
        .with(Part::cylinder(0.12, 0.3).at(-0.2, 0.85, -0.55).color(PLUM_DARK))
        .with_all(glowing_eyes(0.2, 1.02, -0.55, 0.12, GOLD))
        .with(
            Part::pivot()
                .at(0.0, 1.0, 0.3)
                .rot_x(-30.0)
                .tag(AnimTag::Pulse)
                .with(Part::cylinder(0.5, 0.25).color(METAL_DARK))
                .with(Part::cylinder(0.36, 0.8).at(0.0, 0.45, 0.0).color(METAL_LIGHT))
                .with(Part::torus(0.42, 0.1).at(0.0, 0.85, 0.0).color(GLOW_ORANGE).glow(4.0)),
        );
    Part::pivot().with_all(legs).with(claw.mirrored_x()).with(claw).with(body)
}

/// Sentinel eye: a floating eyeball (pupil = weak point), tentacles below.
pub fn eye() -> Part {
    let mut tentacles = Vec::new();
    for i in 0..5 {
        let a = i as f32 * 72.0;
        tentacles.push(
            Part::pivot()
                .at(0.0, 0.35, 0.0)
                .rot_y(a)
                .tag(AnimTag::Wing(i as f32 * 0.2))
                .with(Part::cone(0.14, 0.8).at(0.2, -0.4, 0.0).rot_z(165.0).color(PLUM)),
        );
    }
    let ball = Part::pivot()
        .at(0.0, 0.7, 0.0)
        .with(Part::sphere(1.1).color(rgb(0.95, 0.93, 0.9)))
        .with(Part::cylinder(0.55, 0.06).at(0.0, 0.0, -0.53).rot_x(90.0).color(rgb(0.25, 0.75, 0.55)))
        .with(Part::sphere(0.3).at(0.0, 0.0, -0.5).color(GLOW_MAGENTA).glow(6.0).tag(AnimTag::Pulse))
        .with(Part::ellipsoid(1.2, 0.5, 1.15).at(0.0, 0.42, 0.05).color(PLUM))
        .with(Part::ellipsoid(1.2, 0.4, 1.15).at(0.0, -0.45, 0.05).color(PLUM))
        .with(Part::pivot().tag(AnimTag::Spin(1.8)).with(Part::torus(1.7, 0.08).rot_x(80.0).color(GOLD).glow(1.0)));
    Part::pivot().with(Part::pivot().tag(AnimTag::Bob).with(ball).with_all(tentacles))
}

/// Thornback: a hedgehog covered in bone spikes.
pub fn thornback() -> Part {
    let brown = rgb(0.45, 0.3, 0.22);
    let mut spikes = Vec::new();
    for i in 0..18 {
        let a = i as f32 * 2.4;
        let up = 0.35 + (i % 3) as f32 * 0.22;
        let dir = Vec3::new(a.cos() * (1.0 - up * 0.5), up, a.sin() * (1.0 - up * 0.5) + 0.15).normalize();
        let mut s = Part::cone(0.16, 0.55).at_v(Vec3::new(0.0, 0.45, 0.15) + dir * 0.55).color(BONE);
        s.rot = Quat::from_rotation_arc(Vec3::Y, dir);
        spikes.push(s);
    }
    let feet = Part::cube(0.2, 0.15, 0.25).at(0.32, 0.08, -0.2).color(PLUM_DARK).tag(AnimTag::LegL);
    let body = Part::pivot()
        .tag(AnimTag::Pulse)
        .with(Part::ellipsoid(1.2, 0.85, 1.35).at(0.0, 0.45, 0.15).color(brown))
        .with(Part::ellipsoid(0.55, 0.45, 0.6).at(0.0, 0.35, -0.55).color(rgb(0.85, 0.7, 0.55)))
        .with(Part::sphere(0.12).at(0.0, 0.35, -0.86).color(PLUM_DARK))
        .with_all(glowing_eyes(0.16, 0.48, -0.75, 0.09, GLOW_TOXIC))
        .with_all(spikes);
    Part::pivot().with(feet.mirrored_x()).with(feet).with(body)
}

/// Sky jelly: a glowing jellyfish bell with long tentacles.
pub fn sky_jelly() -> Part {
    let bell = rgb(0.55, 0.75, 1.0);
    let mut tentacles = Vec::new();
    for i in 0..7 {
        let a = i as f32 / 7.0 * std::f32::consts::TAU;
        tentacles.push(
            Part::pivot()
                .at(a.cos() * 0.45, 0.9, a.sin() * 0.45)
                .tag(AnimTag::Wing(i as f32 / 7.0))
                .with(Part::cylinder(0.07, 0.9).at(0.0, -0.45, 0.0).color(bell).glow(1.0))
                .with(Part::sphere(0.12).at(0.0, -0.9, 0.0).color(GLOW_TOXIC).glow(3.0)),
        );
    }
    Part::pivot().with(
        Part::pivot()
            .tag(AnimTag::Bob)
            .with(
                Part::pivot()
                    .tag(AnimTag::Pulse)
                    .with(Part::ellipsoid(1.5, 0.9, 1.5).at(0.0, 1.25, 0.0).color(bell).glow(1.2))
                    .with(Part::ellipsoid(0.8, 0.45, 0.8).at(0.0, 1.2, 0.0).color(CRYSTAL_PINK).glow(2.5))
                    .with(Part::torus(1.5, 0.1).at(0.0, 0.95, 0.0).color(GLOW_CYAN).glow(3.0)),
            )
            .with_all(tentacles),
    )
}

// ---------------------------------------------------------------------------
// Bombers
// ---------------------------------------------------------------------------

/// Sporeling: a walking red mushroom (it bursts into toxic spores).
pub fn sporeling() -> Part {
    let foot = Part::cube(0.18, 0.15, 0.26).at(0.15, 0.08, 0.0).color(PLUM_DARK).tag(AnimTag::LegL);
    let mut cap = Part::pivot()
        .at(0.0, 0.62, 0.0)
        .tag(AnimTag::Pulse)
        .with(Part::ellipsoid(1.1, 0.55, 1.1).at(0.0, 0.1, 0.0).color(rgb(0.85, 0.15, 0.15)))
        .with(Part::cylinder(1.0, 0.06).at(0.0, -0.12, 0.0).color(rgb(0.95, 0.85, 0.7)));
    for i in 0..6 {
        let a = i as f32 * 1.05;
        let r = if i == 0 { 0.0 } else { 0.32 };
        cap = cap.with(Part::sphere(0.14).at(a.cos() * r, 0.32 - r * 0.25, a.sin() * r).color(rgb(1.0, 0.98, 0.9)));
    }
    let stem = Part::pivot()
        .tag(AnimTag::Body)
        .with(Part::cylinder(0.42, 0.55).at(0.0, 0.36, 0.0).color(rgb(0.95, 0.88, 0.75)))
        .with_all(glowing_eyes(0.09, 0.45, -0.2, 0.08, GLOW_TOXIC))
        .with(cap);
    Part::pivot().with(foot.mirrored_x()).with(foot).with(stem)
}

/// Fire bat: a burning bat with flapping wings.
pub fn fire_bat() -> Part {
    let wing = Part::pivot()
        .at(0.2, 0.4, 0.0)
        .tag(AnimTag::Wing(0.0))
        .with(Part::prism(0.9, 0.06, 0.55).at(0.45, 0.0, 0.05).rot_z(-10.0).color(PLUM_DARK))
        .with(Part::cone(0.12, 0.4).at(0.85, 0.05, -0.2).rot_x(-90.0).color(GLOW_ORANGE).glow(3.0));
    let body = Part::pivot()
        .tag(AnimTag::Bob)
        .with(Part::sphere(0.5).at(0.0, 0.4, 0.0).color(rgb(0.95, 0.4, 0.1)).glow(2.5))
        .with(Part::cone(0.12, 0.25).at(0.12, 0.7, 0.0).color(PLUM_DARK))
        .with(Part::cone(0.12, 0.25).at(-0.12, 0.7, 0.0).color(PLUM_DARK))
        .with_all(glowing_eyes(0.1, 0.45, -0.22, 0.08, rgb(1.0, 1.0, 0.6)))
        .with(Part::cone(0.3, 0.6).at(0.0, 0.4, 0.4).rot_x(100.0).color(GLOW_ORANGE).glow(5.0).tag(AnimTag::Pulse))
        .with(wing.mirrored_x())
        .with(wing);
    Part::pivot().with(body)
}

// ---------------------------------------------------------------------------
// Tanks
// ---------------------------------------------------------------------------

/// Stone golem: blocky rocks, a glowing core in its back (weak point ×3).
pub fn stone_golem() -> Part {
    let leg = Part::pivot()
        .at(0.6, 1.1, 0.0)
        .tag(AnimTag::LegL)
        .with(Part::cube(0.7, 1.0, 0.7).at(0.0, -0.5, 0.0).color(ROCK_DARK))
        .with(Part::cube(0.85, 0.3, 0.95).at(0.0, -0.95, -0.1).color(ROCK));
    let arm = Part::pivot()
        .at(1.35, 2.5, 0.0)
        .tag(AnimTag::ArmL)
        .with(Part::cube(0.65, 1.3, 0.65).at(0.0, -0.6, 0.0).rot_z(6.0).color(ROCK))
        .with(Part::ico(1.0, 0.9, 1.0).at(0.05, -1.4, 0.0).color(ROCK_DARK));
    let body = Part::pivot()
        .at(0.0, 1.1, 0.0)
        .tag(AnimTag::Body)
        .with(Part::ico(2.3, 1.9, 1.6).at(0.0, 1.0, 0.0).color(ROCK))
        .with(Part::ico(1.2, 0.7, 1.0).at(0.0, 1.95, 0.0).color(LEAF_A))
        .with(
            Part::pivot()
                .at(0.0, 2.0, -0.55)
                .tag(AnimTag::Head)
                .with(Part::cube(0.7, 0.55, 0.6).color(ROCK_DARK))
                .with(Part::cube(0.5, 0.1, 0.05).at(0.0, 0.05, -0.31).color(GLOW_ORANGE).glow(5.0)),
        )
        // The core in the back (weak point).
        .with(Part::cube(0.9, 0.9, 0.3).at(0.0, 1.2, 0.72).color(STONE_DARK))
        .with(Part::ico(0.75, 0.75, 0.5).at(0.0, 1.2, 0.85).color(GLOW_ORANGE).glow(6.0).tag(AnimTag::Pulse))
        .with(arm.mirrored_x())
        .with(arm);
    Part::pivot().with(leg.mirrored_x()).with(leg).with(body)
}

/// Crystal golem: dark body bristling with pink crystals, crystal head (×2.5).
pub fn crystal_golem() -> Part {
    let leg = Part::pivot()
        .at(0.55, 1.2, 0.0)
        .tag(AnimTag::LegL)
        .with(Part::cube(0.6, 1.1, 0.6).at(0.0, -0.55, 0.0).color(STONE_DARK))
        .with(Part::ico(0.4, 0.6, 0.4).at(0.25, -0.4, -0.2).color(CRYSTAL_PINK).glow(1.5));
    let arm = Part::pivot()
        .at(1.2, 2.4, 0.0)
        .tag(AnimTag::ArmL)
        .with(Part::cube(0.5, 1.2, 0.5).at(0.0, -0.55, 0.0).color(STONE))
        .with(Part::ico(0.5, 1.1, 0.5).at(0.0, -1.4, -0.1).rot_x(-20.0).color(CRYSTAL_PINK).glow(2.5));
    let mut shards = Vec::new();
    for i in 0..5 {
        let a = i as f32 * 1.25;
        shards.push(Part::ico(0.35, 0.8, 0.35).at(a.cos() * 0.6, 1.5, 0.3 + a.sin() * 0.3).rot_z(a.cos() * 25.0).color(CRYSTAL_PINK).glow(2.0));
    }
    let body = Part::pivot()
        .at(0.0, 1.2, 0.0)
        .tag(AnimTag::Body)
        .with(Part::cube(1.9, 1.5, 1.2).at(0.0, 0.9, 0.0).color(STONE))
        .with_all(shards)
        // Head: one big crystal (weak point).
        .with(Part::pivot().at(0.0, 2.05, -0.1).tag(AnimTag::Head).with(Part::ico(0.9, 1.1, 0.9).color(CRYSTAL_PINK).glow(5.0)))
        .with(arm.mirrored_x())
        .with(arm);
    Part::pivot().with(leg.mirrored_x()).with(leg).with(body)
}

/// Shield knight: heavy armor, a huge shield in front (armor) and a sword;
/// the helmet sticks out above the shield (weak point ×2).
pub fn shield_knight() -> Part {
    let leg = Part::pivot()
        .at(0.3, 0.9, 0.0)
        .tag(AnimTag::LegL)
        .with(Part::cube(0.35, 0.85, 0.4).at(0.0, -0.45, 0.0).color(STEEL))
        .with(Part::cube(0.4, 0.15, 0.55).at(0.0, -0.85, -0.08).color(METAL_DARK));
    let sword_arm = Part::pivot()
        .at(-0.65, 1.75, 0.0)
        .tag(AnimTag::ArmR)
        .with(Part::cube(0.3, 0.8, 0.3).at(0.0, -0.35, 0.0).color(STEEL))
        .with(Part::cube(0.12, 1.4, 0.06).at(0.0, -0.8, -0.65).rot_x(-80.0).color(METAL_LIGHT).glow(0.8))
        .with(Part::cube(0.4, 0.08, 0.12).at(0.0, -0.75, -0.05).color(GOLD));
    let body = Part::pivot()
        .at(0.0, 0.9, 0.0)
        .tag(AnimTag::Body)
        .with(Part::cube(1.0, 1.0, 0.6).at(0.0, 0.55, 0.0).color(STEEL))
        .with(Part::cube(1.2, 0.3, 0.7).at(0.0, 1.05, 0.0).color(METAL_DARK))
        .with(Part::cube(0.9, 0.5, 0.15).at(0.0, 0.2, 0.32).color(rgb(0.6, 0.12, 0.15)))
        .with(
            Part::pivot()
                .at(0.0, 1.3, -0.1)
                .tag(AnimTag::Head)
                .with(Part::cube(0.55, 0.6, 0.55).color(METAL_LIGHT))
                .with(Part::cube(0.45, 0.08, 0.05).at(0.0, 0.05, -0.29).color(GLOW_ORANGE).glow(5.0))
                .with(Part::prism(0.1, 0.5, 0.5).at(0.0, 0.45, 0.05).color(rgb(0.85, 0.15, 0.2))),
        )
        .with(sword_arm);
    // The shield stays in front of it (matches its armor zone).
    let shield = Part::pivot()
        .at(0.0, 1.2, -0.85)
        .with(Part::cube(1.3, 1.6, 0.15).color(METAL_DARK))
        .with(Part::cube(1.15, 1.45, 0.05).at(0.0, 0.0, -0.09).color(rgb(0.25, 0.35, 0.7)))
        .with(Part::cube(0.18, 1.2, 0.04).at(0.0, 0.0, -0.13).color(GOLD))
        .with(Part::cube(0.9, 0.18, 0.04).at(0.0, 0.15, -0.13).color(GOLD));
    Part::pivot().with(leg.mirrored_x()).with(leg).with(body).with(shield)
}

// ---------------------------------------------------------------------------
// Support / charger
// ---------------------------------------------------------------------------

/// Summoner: a floating hooded robe holding a glowing lantern (×2).
pub fn summoner() -> Part {
    let robe = rgb(0.22, 0.16, 0.38);
    Part::pivot().with(
        Part::pivot()
            .tag(AnimTag::Bob)
            .with(Part::cone(1.1, 1.6).at(0.0, 0.8, 0.0).color(robe).tag(AnimTag::Wing(0.0)))
            .with(Part::sphere(0.65).at(0.0, 1.6, 0.0).color(robe))
            .with(Part::sphere(0.45).at(0.0, 1.55, -0.12).color(PLUM_DARK))
            .with_all(glowing_eyes(0.1, 1.58, -0.33, 0.08, GLOW_CYAN))
            .with(
                Part::pivot()
                    .at(0.0, 1.5, -0.3)
                    .tag(AnimTag::Pulse)
                    .with(Part::cylinder(0.04, 0.3).at(0.0, 0.25, 0.0).color(METAL_DARK))
                    .with(Part::cube(0.3, 0.36, 0.3).color(METAL_DARK))
                    .with(Part::ico(0.26, 0.3, 0.26).color(GLOW_CYAN).glow(7.0)),
            )
            .with(Part::cylinder(0.06, 1.8).at(0.5, 1.0, -0.1).rot_z(-8.0).color(BARK))
            .with(Part::ico(0.22, 0.22, 0.22).at(0.62, 1.95, -0.1).color(GLOW_MAGENTA).glow(4.0)),
    )
}

/// Lava boar: black rock body with glowing lava cracks and tusks.
pub fn lava_boar() -> Part {
    let leg = |x: f32, z: f32, tag: AnimTag| {
        Part::pivot().at(x, 0.65, z).tag(tag).with(Part::cylinder(0.3, 0.65).at(0.0, -0.32, 0.0).color(STONE_DARK))
    };
    let head = Part::pivot()
        .at(0.0, 0.85, -0.95)
        .tag(AnimTag::Head)
        .with(Part::cube(0.7, 0.6, 0.7).color(STONE_DARK))
        .with(Part::cylinder(0.35, 0.2).at(0.0, -0.1, -0.4).rot_x(90.0).color(PLUM_DARK))
        .with(Part::cone(0.12, 0.45).at(0.25, -0.1, -0.35).rot_x(-60.0).color(BONE))
        .with(Part::cone(0.12, 0.45).at(-0.25, -0.1, -0.35).rot_x(-60.0).color(BONE))
        .with_all(glowing_eyes(0.2, 0.12, -0.36, 0.1, GLOW_ORANGE));
    let mut body = Part::pivot()
        .tag(AnimTag::Body)
        .with(Part::ellipsoid(1.3, 1.0, 1.9).at(0.0, 0.9, 0.0).color(STONE_DARK))
        .with(head);
    for i in 0..5 {
        let z = -0.6 + i as f32 * 0.3;
        body = body.with(Part::cone(0.25, 0.5).at(0.0, 1.45, z).color(GLOW_ORANGE).glow(3.0));
    }
    for (x, z) in [(0.6f32, -0.2f32), (-0.6, 0.3), (0.55, 0.5)] {
        body = body.with(Part::cube(0.05, 0.4, 0.5).at(x, 0.9, z).rot_y(30.0).color(GLOW_ORANGE).glow(4.0));
    }
    Part::pivot()
        .with(leg(0.4, -0.55, AnimTag::LegL))
        .with(leg(-0.4, -0.55, AnimTag::LegR))
        .with(leg(0.4, 0.55, AnimTag::LegR))
        .with(leg(-0.4, 0.55, AnimTag::LegL))
        .with(body)
}

// ---------------------------------------------------------------------------
// Bosses
// ---------------------------------------------------------------------------

/// Broodmother: giant spider. Its 4 legs have glowing knees: break them
/// all and it collapses (no resistance, ×2 damage).
pub fn broodmother() -> Part {
    let mut legs = Vec::new();
    for (i, knee) in BROOD_LEGS.iter().enumerate() {
        let sx = knee.x.signum();
        let sz = knee.z.signum();
        let hip = Vec3::new(sx * 1.1, 2.2, sz * 0.7);
        let foot = Vec3::new(sx * 4.6, 0.0, sz * 2.8);
        legs.push(
            Part::pivot()
                .at_v(*knee)
                .tag(AnimTag::Breakable(i as u8))
                .with(limb(Vec3::ZERO, hip - *knee, 0.5, PLUM))
                .with(limb(Vec3::ZERO, foot - *knee, 0.4, PLUM_DARK))
                .with(Part::cone(0.3, 0.5).at_v((foot - *knee) * 0.98).rot_x(180.0).color(BONE))
                .with(Part::sphere(1.0).color(GLOW_ORANGE).glow(5.0).tag(AnimTag::Pulse)),
        );
    }
    // Small extra legs (not breakable) for the spider look.
    for (x, z) in [(1.0f32, -0.2f32), (-1.0, -0.2)] {
        legs.push(
            Part::pivot()
                .at(x, 2.0, z)
                .tag(AnimTag::Leg(0.5))
                .with(limb(Vec3::ZERO, Vec3::new(x.signum() * 1.4, 0.8, 0.0), 0.3, PLUM_DARK))
                .with(limb(Vec3::new(x.signum() * 1.4, 0.8, 0.0), Vec3::new(x.signum() * 2.4, -2.0, 0.0), 0.25, PLUM_DARK)),
        );
    }
    let head = Part::pivot()
        .at(0.0, 0.0, -1.6)
        .tag(AnimTag::Head)
        .with(Part::sphere(1.5).color(PLUM_DARK))
        .with_all(glowing_eyes(0.3, 0.25, -0.62, 0.22, GLOW_MAGENTA))
        .with_all(glowing_eyes(0.55, 0.45, -0.45, 0.15, GLOW_MAGENTA))
        .with(
            Part::pivot()
                .at(0.0, -0.45, -0.6)
                .tag(AnimTag::Jaw)
                .with(Part::cone(0.22, 0.7).at(0.25, -0.2, 0.0).rot_x(170.0).color(BONE))
                .with(Part::cone(0.22, 0.7).at(-0.25, -0.2, 0.0).rot_x(170.0).color(BONE)),
        );
    let body = Part::pivot()
        .at(0.0, 2.2, 0.0)
        .tag(AnimTag::Body)
        .with(Part::ellipsoid(2.3, 1.5, 2.2).color(PLUM))
        .with(Part::ellipsoid(3.2, 2.6, 3.4).at(0.0, 0.5, 2.3).color(PLUM_DARK))
        .with(Part::ellipsoid(1.4, 0.4, 2.0).at(0.0, 1.7, 2.3).color(rgb(0.85, 0.15, 0.25)))
        .with(Part::cube(0.25, 0.1, 2.4).at(0.0, 1.85, 2.3).color(GLOW_MAGENTA).glow(2.0))
        .with(head);
    Part::pivot().with_all(legs).with(body)
}

/// Ancient golem: huge mossy stone giant, a cyan core in its back (×3).
pub fn ancient_golem() -> Part {
    let leg = Part::pivot()
        .at(1.3, 3.0, 0.0)
        .tag(AnimTag::LegL)
        .with(Part::cube(1.3, 3.0, 1.3).at(0.0, -1.5, 0.0).color(ROCK_DARK))
        .with(Part::cube(1.8, 0.6, 2.1).at(0.0, -2.8, -0.2).color(ROCK))
        .with(Part::ico(0.8, 0.5, 0.8).at(0.3, -0.6, 0.3).color(LEAF_A));
    let arm = Part::pivot()
        .at(2.6, 5.8, 0.0)
        .tag(AnimTag::ArmL)
        .with(Part::cube(1.1, 2.6, 1.1).at(0.0, -1.3, 0.0).rot_z(8.0).color(ROCK))
        .with(Part::ico(1.8, 1.7, 1.8).at(0.2, -3.0, 0.0).color(ROCK_DARK))
        .with(Part::ico(0.9, 0.4, 0.9).at(0.0, -0.2, 0.0).color(LEAF_B));
    let mut runes = Vec::new();
    for (x, y) in [(-0.8f32, 1.0f32), (0.8, 1.6), (0.0, 0.5)] {
        runes.push(Part::cube(0.4, 0.4, 0.06).at(x, y, -1.42).rot_z(45.0).color(GLOW_CYAN).glow(3.0));
    }
    let body = Part::pivot()
        .at(0.0, 3.0, 0.0)
        .tag(AnimTag::Body)
        .with(Part::cube(3.4, 3.2, 2.8).at(0.0, 1.4, 0.0).rot_x(6.0).color(ROCK))
        .with(Part::ico(3.0, 1.0, 2.4).at(0.0, 3.1, 0.0).color(LEAF_A))
        .with_all(runes)
        .with(
            Part::pivot()
                .at(0.0, 3.4, -0.6)
                .tag(AnimTag::Head)
                .with(Part::cube(1.4, 1.1, 1.3).color(ROCK_DARK))
                .with(Part::cube(1.0, 0.18, 0.1).at(0.0, 0.1, -0.66).color(GLOW_CYAN).glow(6.0)),
        )
        // The core in its back: the weak point.
        .with(Part::cube(1.6, 1.6, 0.5).at(0.0, 2.0, 1.35).color(STONE_DARK))
        .with(Part::ico(1.3, 1.3, 0.8).at(0.0, 2.0, 1.55).color(GLOW_CYAN).glow(7.0).tag(AnimTag::Pulse))
        .with(arm.mirrored_x())
        .with(arm);
    Part::pivot().with(leg.mirrored_x()).with(leg).with(body)
}

/// Sandworm: a column of armored rings, a round mouth full of teeth (×2.5).
pub fn sandworm() -> Part {
    let skin = rgb(0.78, 0.62, 0.42);
    let mut body = Part::pivot().tag(AnimTag::Body);
    for i in 0..7 {
        let y = i as f32 * 0.9;
        let d = 2.6 - i as f32 * 0.12;
        let z = (i as f32 * 0.5).sin() * 0.25;
        body = body
            .with(Part::cylinder(d, 0.85).at(0.0, y + 0.45, z).color(if i % 2 == 0 { skin } else { SANDSTONE_DARK }))
            .with(Part::torus(d * 1.02, 0.25).at(0.0, y + 0.85, z).color(rgb(0.5, 0.38, 0.28)));
    }
    let mut mouth = Part::pivot()
        .at(0.0, 6.4, -0.4)
        .tag(AnimTag::Head)
        .with(Part::sphere(2.3).color(skin))
        .with(Part::cylinder(1.6, 0.3).at(0.0, 0.0, -0.95).rot_x(90.0).color(rgb(0.45, 0.05, 0.1)))
        .with(Part::sphere(1.0).at(0.0, 0.0, -0.75).color(GLOW_ORANGE).glow(5.0).tag(AnimTag::Pulse));
    for i in 0..10 {
        let a = i as f32 / 10.0 * std::f32::consts::TAU;
        let mut tooth = Part::cone(0.22, 0.45).at(a.cos() * 0.75, a.sin() * 0.75, -1.05).color(BONE);
        tooth.rot = Quat::from_rotation_arc(Vec3::Y, Vec3::new(-a.cos(), -a.sin(), -0.6).normalize());
        mouth = mouth.with(tooth);
    }
    Part::pivot().with(body.with(mouth))
}

/// Dirt mound over a burrowing monster.
pub fn mound(radius: f32) -> Part {
    let mut p = Part::pivot();
    for i in 0..7 {
        let a = i as f32 / 7.0 * std::f32::consts::TAU;
        let r = radius * 0.8;
        p = p.with(Part::ico(radius * 0.9, radius * 0.5, radius * 0.9).at(a.cos() * r, 0.1, a.sin() * r).color(if i % 2 == 0 { SANDSTONE_DARK } else { BARK }));
    }
    p.with(Part::ico(radius * 1.2, radius * 0.7, radius * 1.2).at(0.0, 0.15, 0.0).color(SANDSTONE_DARK).tag(AnimTag::Pulse))
}
