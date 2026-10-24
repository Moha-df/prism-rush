//! Charge shrines and power-ups (server).
//!
//! - **Charge shrine**: stand inside its circle; when it is full, every
//!   player inside gets a free level-up. More players inside = faster.
//! - **Power-ups** sometimes drop from monsters (often from elites): a
//!   magnet that pulls every XP gem, a team heal, a bomb and a haste boost.

use bevy::prelude::*;
use bevy_replicon::prelude::*;

use super::combat::DamageMob;
use super::progress::LevelUps;
use super::{ServerSet, SimTick};
use crate::net::protocol::*;
use crate::shared::movement::DT;
use crate::shared::rng::Rng;

/// Walk this close to a power-up to take it.
pub const POWERUP_RANGE: f32 = 2.2;
/// Power-ups vanish after this long.
const POWERUP_LIFE: f32 = 40.0;
const BOMB_RADIUS: f32 = 30.0;

/// Server-side lifetime of a power-up.
#[derive(Component)]
pub struct PowerupLife(pub f32);

pub struct PickupsPlugin;

impl Plugin for PickupsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, (charge_shrines, collect_powerups).chain().in_set(ServerSet::World));
    }
}

/// Maybe drops a power-up where a monster died.
pub fn maybe_drop_powerup(commands: &mut Commands, pos: Vec3, elite: bool, boss: bool, rng: &mut Rng) {
    let chance = if boss { 1.0 } else if elite { 0.3 } else { 0.012 };
    if !rng.chance(chance) {
        return;
    }
    let kind = *rng.pick(&PowerupKind::ALL);
    commands.spawn((Replicated, Powerup { kind, pos: pos + Vec3::Y * 0.6 }, PowerupLife(POWERUP_LIFE)));
}

fn charge_shrines(
    mut shrines: Query<(&Shrine, &mut ShrineCharge)>,
    mut players: Query<(&PlayerState, &Health, &mut LevelUps)>,
    mut events: MessageWriter<ToClients<GameEvent>>,
) {
    for (shrine, mut charge) in &mut shrines {
        if charge.used {
            continue;
        }
        let inside = |s: &PlayerState, h: &Health| !h.downed && s.sim.mv.pos.distance(shrine.pos) < SHRINE_RADIUS;
        let n = players.iter().filter(|(s, h, _)| inside(s, h)).count();
        if n == 0 {
            // Leaving the circle slowly loses the charge.
            charge.charge = (charge.charge - DT / SHRINE_TIME * 0.5).max(0.0);
            continue;
        }
        charge.charge += DT / SHRINE_TIME * (n as f32).sqrt();
        if charge.charge >= 1.0 {
            charge.charge = 1.0;
            charge.used = true;
            for (s, h, mut level_ups) in &mut players {
                if inside(s, h) {
                    level_ups.0 += 1;
                }
            }
            info!("charge shrine completed ({n} players inside)");
            events.write(ToClients { targets: SendTargets::All, message: GameEvent::ShrineCharged { pos: shrine.pos } });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_powerups(
    mut commands: Commands,
    mut powerups: Query<(Entity, &Powerup, &mut PowerupLife)>,
    mut players: Query<(&Player, &mut PlayerState, &mut Health)>,
    gems: Query<(Entity, &XpGem)>,
    mobs: Query<(Entity, &Mob, &MobState, &Health), Without<PlayerState>>,
    mut info: Single<&mut MatchInfo>,
    tick: Res<SimTick>,
    mut damage: MessageWriter<DamageMob>,
    mut fx: MessageWriter<ToClients<Fx>>,
    mut events: MessageWriter<ToClients<GameEvent>>,
) {
    for (entity, powerup, mut life) in &mut powerups {
        life.0 -= DT;
        if life.0 <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }
        let taker = players
            .iter()
            .find(|(_, s, h)| !h.downed && (s.sim.mv.pos + Vec3::Y * 0.8).distance(powerup.pos) < POWERUP_RANGE)
            .map(|(p, ..)| p.slot);
        let Some(slot) = taker else { continue };
        commands.entity(entity).try_despawn();
        match powerup.kind {
            PowerupKind::Magnet => {
                for (i, (gem_entity, gem)) in gems.iter().enumerate() {
                    info.xp += gem.value;
                    commands.entity(gem_entity).try_despawn();
                    // A few flying gems for the show (not hundreds of messages).
                    if i < 30 {
                        fx.write(ToClients { targets: SendTargets::All, message: Fx::XpCollected { from: gem.pos, slot } });
                    }
                }
            }
            PowerupKind::Heal => {
                for (_, _, mut h) in &mut players {
                    if !h.downed {
                        h.hp = (h.hp + h.max * 0.4).min(h.max);
                    }
                }
            }
            PowerupKind::Bomb => {
                for (e, mob, state, health) in &mobs {
                    if state.pos.distance(powerup.pos) < BOMB_RADIUS {
                        // Bosses only lose a slice of their health.
                        let amount = if mob.kind.is_boss() { health.max * 0.1 } else { health.hp + 1.0 };
                        let pos = state.pos + Vec3::Y * mob.kind.def().height * 0.5;
                        damage.write(DamageMob { target: e, amount, crit: false, pos, by: None });
                    }
                }
                fx.write(ToClients { targets: SendTargets::All, message: Fx::Explosion { pos: powerup.pos, radius: 6.0 } });
            }
            PowerupKind::Haste => {
                if let Some((_, mut s, _)) = players.iter_mut().find(|(p, ..)| p.slot == slot) {
                    s.sim.mv.haste = s.sim.mv.haste.max(8.0);
                }
            }
        }
        info!("player slot {slot} took {:?} (tick {})", powerup.kind, tick.0);
        events.write(ToClients {
            targets: SendTargets::All,
            message: GameEvent::PowerupTaken { slot, kind: powerup.kind, pos: powerup.pos },
        });
    }
}
