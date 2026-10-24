//! XP and levels (server).
//!
//! Monsters drop XP gems. Walking near one collects it, and the XP goes to
//! the **whole team**: everyone levels up together (coop-friendly: nobody
//! falls behind), but each player picks their own upgrades.

use bevy::prelude::*;
use bevy_replicon::prelude::*;

use super::{ServerSet, SimTick};
use crate::net::protocol::*;
use crate::shared::economy;
use crate::shared::items::PlayerStats;

/// Base radius in which gems are collected (Attraction tome increases it).
pub const BASE_PICKUP_RADIUS: f32 = 3.5;
/// Above this many gems on the ground, XP is given directly (performance).
pub const MAX_GEMS: usize = 300;

/// Level-ups a player still has to spend on upgrade choices.
#[derive(Component, Default, Debug)]
pub struct LevelUps(pub u32);

pub struct ProgressPlugin;

impl Plugin for ProgressPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, (collect_gems, level_up).chain().in_set(ServerSet::World));
    }
}

/// Spawns an XP gem (or gives the XP directly when there are too many gems).
pub fn drop_gem(commands: &mut Commands, gem_count: usize, pos: Vec3, value: f32, info: &mut MatchInfo) {
    if gem_count >= MAX_GEMS {
        info.xp += value;
    } else {
        commands.spawn((Replicated, XpGem { pos: pos + Vec3::Y * 0.5, value }));
    }
}

fn collect_gems(
    mut commands: Commands,
    gems: Query<(Entity, &XpGem)>,
    players: Query<(&Player, &PlayerState, &Health, &Items)>,
    mut info: Single<&mut MatchInfo>,
    tick: Res<SimTick>,
    mut fx: MessageWriter<ToClients<Fx>>,
) {
    // Twice per tick would be overkill: every 2 ticks is plenty.
    if !tick.0.is_multiple_of(2) {
        return;
    }
    let collectors: Vec<(u8, Vec3, f32, f32)> = players
        .iter()
        .filter(|(_, _, h, _)| !h.downed)
        .map(|(p, s, _, i)| {
            let stats = PlayerStats::from_inventory(&i.0);
            (p.slot, s.sim.mv.pos + Vec3::Y * 0.8, BASE_PICKUP_RADIUS * stats.attraction, stats.xp_mult)
        })
        .collect();
    for (entity, gem) in &gems {
        if let Some((slot, _, _, mult)) = collectors.iter().find(|(_, p, r, _)| p.distance(gem.pos) < *r) {
            commands.entity(entity).try_despawn();
            info.xp += gem.value * mult;
            fx.write(ToClients { targets: SendTargets::All, message: Fx::XpCollected { from: gem.pos, slot: *slot } });
        }
    }
}

fn level_up(
    mut info: Single<&mut MatchInfo>,
    mut players: Query<&mut LevelUps>,
    mut events: MessageWriter<ToClients<GameEvent>>,
) {
    while info.xp >= info.xp_next {
        info.xp -= info.xp_next;
        info.team_level += 1;
        info.xp_next = economy::xp_to_next(info.team_level);
        for mut l in &mut players {
            l.0 += 1;
        }
        info!("team level {}", info.team_level);
        events.write(ToClients { targets: SendTargets::All, message: GameEvent::LevelUp { level: info.team_level } });
    }
}
