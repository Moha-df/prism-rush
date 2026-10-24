//! Visuals of the automatic weapons that follow a player: the aura ring
//! and the orbiting orbs. They are computed on each client from the
//! replicated `Gear` + `Items` (no network traffic per frame).

use std::collections::HashSet;

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

/// A visual following a player. `index` is the orb number (0 for the aura).
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
                    // The ring breathes a little.
                    let r = p.size * (1.0 + 0.04 * (time.elapsed_secs() * 4.0).sin());
                    wanted.push((
                        AutoVisual { owner: entity, weapon: *weapon, index: 0 },
                        Transform::from_translation(pos + Vec3::Y * 0.12).with_scale(Vec3::new(r, 1.0, r)),
                    ));
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
        let part = match v.weapon {
            // Unit ring (radius 1), scaled to the aura radius.
            AutoWeapon::Aura => Part::pivot()
                .with(Part::torus(2.0, 0.07).color(palette::GLOW_MAGENTA).glow(4.0))
                .with(Part::torus(1.4, 0.03).color(palette::GLOW_MAGENTA).glow(2.0)),
            _ => Part::pivot()
                .with(Part::ico(ORB_RADIUS * 2.0, ORB_RADIUS * 2.0, ORB_RADIUS * 2.0).color(palette::GLOW_CYAN).glow(5.0))
                .with(Part::torus(ORB_RADIUS * 2.6, 0.05).rot_x(70.0).color(palette::CRYSTAL_CYAN).glow(3.0)),
        };
        let root = commands.spawn((v, tr, Visibility::default())).id();
        spawn_model(&mut commands, &mut cache, &mut meshes, &mut materials, root, root, &part, ());
    }
}
