//! Visuals of the automatic weapons that follow a player: the aura ring
//! and the orbiting orbs. They are computed on each client from the
//! replicated `Gear` + `Items` (no network traffic per frame).

use std::collections::HashSet;

use bevy::light::NotShadowCaster;
use bevy::prelude::*;

use super::model::{spawn_model, Part, PrimCache};
use super::palette;
use crate::game::autos::{orb_offset, ORB_RADIUS};
use crate::net::client::{Prediction, ServerClock};
use crate::net::protocol::{Gear, Items, Player, PlayerState};
use crate::net::LocalPlayer;
use crate::shared::build::{auto_params, AutoWeapon};
use crate::shared::items::PlayerStats;
use crate::shared::movement::TICK_HZ;

/// Number of glowing dots drawing the edge of an aura.
const AURA_DOTS: u32 = 28;

/// A visual following a player. `index` is the orb number, or for the aura:
/// 0 = the translucent disc, 1.. = the dots on its edge.
#[derive(Component, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AutoVisual {
    owner: Entity,
    weapon: AutoWeapon,
    index: u32,
}

pub struct AutoVisualsPlugin;

impl Plugin for AutoVisualsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_auto_visuals);
    }
}

#[allow(clippy::too_many_arguments)]
fn update_auto_visuals(
    mut commands: Commands,
    players: Query<(Entity, &Player, &PlayerState, &Gear, &Items, Option<&Transform>), Without<AutoVisual>>,
    mut visuals: Query<(Entity, &AutoVisual, &mut Transform)>,
    local: Res<LocalPlayer>,
    pred: Res<Prediction>,
    clock: Res<ServerClock>,
    time: Res<Time>,
    mut cache: ResMut<PrimCache>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut aura_assets: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>)>>,
) {
    let t = (clock.estimate / TICK_HZ) as f32;
    // What should exist this frame, with its transform.
    let mut wanted: Vec<(AutoVisual, Transform)> = Vec::new();
    for (entity, player, state, gear, items, transform) in &players {
        // Our own player is drawn where the prediction says.
        let pos = if Some(player.slot) == local.slot && pred.active {
            pred.sim.mv.pos + pred.smoothing
        } else {
            transform.map(|t| t.translation).unwrap_or(state.sim.mv.pos)
        };
        let stats = PlayerStats::from_inventory(&items.0);
        for (weapon, up) in &gear.0.autos {
            let p = auto_params(*weapon, up, &stats);
            match weapon {
                AutoWeapon::Aura => {
                    // The disc breathes a little; dots slowly turn on its edge.
                    let time = time.elapsed_secs();
                    let r = p.size * (1.0 + 0.04 * (time * 4.0).sin());
                    wanted.push((
                        AutoVisual { owner: entity, weapon: *weapon, index: 0 },
                        Transform::from_translation(pos + Vec3::Y * 0.1).with_scale(Vec3::new(r, 1.0, r)),
                    ));
                    for i in 0..AURA_DOTS {
                        let a = time * 0.8 + i as f32 / AURA_DOTS as f32 * std::f32::consts::TAU;
                        let at = pos + Vec3::new(a.cos() * r, 0.15, a.sin() * r);
                        wanted.push((AutoVisual { owner: entity, weapon: *weapon, index: i + 1 }, Transform::from_translation(at)));
                    }
                }
                AutoWeapon::Orbs => {
                    for i in 0..p.count {
                        let o = orb_offset(i, p.count, p.size, p.speed, t);
                        wanted.push((AutoVisual { owner: entity, weapon: *weapon, index: i }, Transform::from_translation(pos + o)));
                    }
                }
                _ => {}
            }
        }
    }

    let mut present = HashSet::new();
    for (e, v, mut tr) in &mut visuals {
        match wanted.iter().find(|(w, _)| w == v) {
            Some((_, target)) => {
                *tr = *target;
                present.insert(*v);
            }
            None => commands.entity(e).try_despawn(),
        }
    }
    for (v, tr) in wanted {
        if present.contains(&v) {
            continue;
        }
        if v.weapon == AutoWeapon::Aura && v.index == 0 {
            // Translucent unit disc (radius 1), scaled to the aura radius.
            let disc = aura_assets.get_or_insert_with(|| {
                (
                    meshes.add(Cylinder::new(1.0, 0.03)),
                    materials.add(StandardMaterial {
                        base_color: Color::srgba(1.0, 0.3, 0.8, 0.07),
                        alpha_mode: AlphaMode::Blend,
                        unlit: true,
                        ..default()
                    }),
                )
            });
            commands.spawn((v, tr, Visibility::default(), Mesh3d(disc.0.clone()), MeshMaterial3d(disc.1.clone()), NotShadowCaster));
            continue;
        }
        let part = match v.weapon {
            AutoWeapon::Aura => Part::pivot().with(Part::sphere(0.14).color(palette::GLOW_MAGENTA).glow(6.0)),
            _ => Part::pivot()
                .with(Part::ico(ORB_RADIUS * 2.0, ORB_RADIUS * 2.0, ORB_RADIUS * 2.0).color(palette::GLOW_CYAN).glow(5.0))
                .with(Part::torus(ORB_RADIUS * 2.6, 0.05).rot_x(70.0).color(palette::CRYSTAL_CYAN).glow(3.0)),
        };
        let root = commands.spawn((v, tr, Visibility::default())).id();
        spawn_model(&mut commands, &mut cache, &mut meshes, &mut materials, root, root, &part, ());
    }
}
