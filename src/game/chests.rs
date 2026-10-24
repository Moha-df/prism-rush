//! Chests (server): spawned from the deterministic chest spots of the chunks
//! around players, opened with gold. Item chests offer item cards; weapon
//! shrines (`ChestKind::Weapon`) offer a free level-up (weapon upgrades).

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy_replicon::prelude::*;

use super::players::{Intent, PlayerScore};
use crate::net::protocol::PendingChoice;
use super::world::TerrainRes;
use super::{ServerSet, SimTick};
use crate::net::protocol::*;
use crate::shared::economy::{self, ChestKind};
use crate::shared::items::PlayerStats;
use crate::shared::mobs::MobKind;
use crate::shared::rng::{hash_combine, Rng};
use crate::shared::worldgen::{chunk_of, CHUNK_SIZE};

pub const INTERACT_RANGE: f32 = 2.8;
const CHEST_STREAM_RADIUS: i32 = 3;
const CHEST_FORGET_DISTANCE: f32 = CHUNK_SIZE * 6.0;

#[derive(Resource, Default)]
pub struct ChestRegistry {
    pub spawned: HashMap<u64, Entity>,
    pub opened: HashSet<u64>,
    /// Statues: spawned ones, and those already awakened this run.
    pub statues: HashMap<u64, Entity>,
    pub statues_used: HashSet<u64>,
}

/// Despawns the entity after a delay (opened chests linger a bit).
#[derive(Component)]
pub struct DespawnAfter(pub f32);

pub struct ChestsPlugin;

impl Plugin for ChestsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChestRegistry>().add_systems(
            FixedUpdate,
            (stream_chests, awaken_statues, open_chests, despawn_after).chain().in_set(ServerSet::World),
        )
        .add_systems(PreUpdate, receive_choices.after(ServerSystems::Receive).run_if(in_state(ServerState::Running)));
    }
}

fn stream_chests(
    mut commands: Commands,
    mut registry: ResMut<ChestRegistry>,
    mut terrain: ResMut<TerrainRes>,
    players: Query<&PlayerState>,
    chests: Query<(&Chest, &ChestOpen)>,
    tick: Res<SimTick>,
) {
    // Twice per second is plenty.
    if !tick.0.is_multiple_of(30) {
        return;
    }
    let positions: Vec<Vec3> = players.iter().map(|p| p.sim.mv.pos).collect();
    let TerrainRes(t) = &mut *terrain;
    for pos in &positions {
        let c = chunk_of(pos.x, pos.z);
        for dz in -CHEST_STREAM_RADIUS..=CHEST_STREAM_RADIUS {
            for dx in -CHEST_STREAM_RADIUS..=CHEST_STREAM_RADIUS {
                let chunk = t.cache.get_or_generate(&t.wgen, c + IVec2::new(dx, dz));
                for st in &chunk.statues {
                    if registry.statues.contains_key(&st.id) {
                        continue;
                    }
                    let used = registry.statues_used.contains(&st.id);
                    let boss = MobKind::BOSSES[st.boss % MobKind::BOSSES.len()];
                    let e = commands
                        .spawn((Replicated, Statue { spot_id: st.id, pos: st.pos, boss }, StatueUsed(used)))
                        .id();
                    registry.statues.insert(st.id, e);
                }
                for spot in &chunk.chests {
                    if registry.spawned.contains_key(&spot.id) || registry.opened.contains(&spot.id) {
                        continue;
                    }
                    let entity = commands
                        .spawn((
                            Replicated,
                            Chest { spot_id: spot.id, kind: spot.kind, pos: spot.pos, yaw: spot.yaw },
                            ChestOpen::default(),
                        ))
                        .id();
                    registry.spawned.insert(spot.id, entity);
                }
            }
        }
    }
    // Forget unopened chests far from everyone (they respawn when you come back).
    for (chest, open) in &chests {
        if open.opened {
            continue;
        }
        if !positions.iter().any(|p| p.distance(chest.pos) < CHEST_FORGET_DISTANCE)
            && let Some(e) = registry.spawned.remove(&chest.spot_id)
        {
            commands.entity(e).try_despawn();
        }
    }
}

/// Spawns a chest that does not come from the world generator (boss loot).
pub fn spawn_loot_chest(commands: &mut Commands, kind: ChestKind, pos: Vec3, id_seed: u64) -> Entity {
    commands
        .spawn((
            Replicated,
            Chest { spot_id: hash_combine(&[id_seed, 0x1007]), kind, pos, yaw: 0.0 },
            ChestOpen::default(),
        ))
        .id()
}

type OpenerQuery<'a> = (
    Entity,
    &'a Player,
    &'a PlayerState,
    &'a Intent,
    &'a Health,
    &'a mut Wallet,
    &'a Items,
    &'a Gear,
    &'a super::progress::AutoPool,
    &'a mut PlayerScore,
    Has<PendingChoice>,
);

/// Opening a chest: pay, then draw cards (items, or weapon upgrades).
fn open_chests(
    mut commands: Commands,
    mut players: Query<OpenerQuery>,
    mut chests: Query<(Entity, &Chest, &mut ChestOpen)>,
    mut registry: ResMut<ChestRegistry>,
    info: Single<&MatchInfo>,
    tick: Res<SimTick>,
    mut events: MessageWriter<ToClients<GameEvent>>,
) {
    for (player_entity, player, state, intent, health, mut wallet, items, gear, pool, mut score, pending) in &mut players {
        // One choice at a time: pick your card before opening another chest.
        if !intent.interact_pressed || health.downed || pending {
            continue;
        }
        let pos = state.sim.mv.pos;
        let nearest = chests
            .iter_mut()
            .filter(|(_, c, o)| !o.opened && c.pos.distance(pos + Vec3::Y * 0.5) < INTERACT_RANGE)
            .min_by(|a, b| a.1.pos.distance(pos).total_cmp(&b.1.pos.distance(pos)));
        let Some((entity, chest, mut open)) = nearest else { continue };

        let stats = PlayerStats::from_inventory(&items.0);
        let price = player_price(chest.kind, wallet.chests_opened, info.elapsed, &stats);
        if wallet.gold < price {
            continue;
        }
        wallet.gold -= price;
        wallet.chests_opened += 1;
        open.opened = true;
        registry.opened.insert(chest.spot_id);
        registry.spawned.remove(&chest.spot_id);
        commands.entity(entity).try_insert(DespawnAfter(10.0));
        score.chests += 1;
        events.write(ToClients {
            targets: SendTargets::All,
            message: GameEvent::ChestOpened { pos: chest.pos, slot: player.slot, kind: chest.kind },
        });

        let mut rng = Rng::new(hash_combine(&[chest.spot_id, u64::from(tick.0), u64::from(player.slot)]));
        let options: Vec<CardOption> = if chest.kind == ChestKind::Weapon {
            // Weapon shrine: a free level-up, with a luck bonus.
            crate::shared::build::roll_offers(&gear.0, &items.0, &pool.0, stats.luck + 1.0, stats.cards, &mut rng)
                .into_iter()
                .map(CardOption::Offer)
                .collect()
        } else {
            crate::shared::items::roll_cards(chest.kind, stats.luck, stats.cards, &mut rng)
                .into_iter()
                .map(CardOption::Item)
                .collect()
        };
        info!("{} opened a {:?} chest for {price} gold", player.name, chest.kind);
        commands.entity(player_entity).try_insert(PendingChoice { chest: Some(chest.kind), options });
    }
}

/// Price of a chest for a given player: goes up with every chest they opened
/// (Treasure Map discount).
pub fn player_price(kind: ChestKind, opened: u32, elapsed: f32, stats: &PlayerStats) -> u32 {
    ((economy::chest_price_v3(kind, opened, elapsed) as f32) * stats.price_mult).round() as u32
}

pub const STATUE_RANGE: f32 = 3.5;

/// Pressing E at a statue summons its boss right now (one boss at a time).
/// It's a gamble: a tough fight, but every boss drops a legendary chest.
#[allow(clippy::too_many_arguments)]
fn awaken_statues(
    mut commands: Commands,
    mut players: Query<(&Player, &PlayerState, &mut Intent, &Health)>,
    mut statues: Query<(&Statue, &mut StatueUsed)>,
    mut registry: ResMut<ChestRegistry>,
    mut info: Single<&mut MatchInfo>,
    terrain: Res<TerrainRes>,
    tick: Res<SimTick>,
    mut events: MessageWriter<ToClients<GameEvent>>,
) {
    for (player, state, mut intent, health) in &mut players {
        if !intent.interact_pressed || health.downed {
            continue;
        }
        let pos = state.sim.mv.pos;
        let Some((statue, mut used)) = statues.iter_mut().find(|(s, u)| !u.0 && s.pos.distance(pos) < STATUE_RANGE) else {
            continue;
        };
        intent.interact_pressed = false;
        if info.boss_alive {
            continue; // one boss at a time
        }
        used.0 = true;
        registry.statues_used.insert(statue.spot_id);
        let angle = (pos - statue.pos).with_y(0.0).normalize_or_zero();
        let x = statue.pos.x - angle.x * 25.0;
        let z = statue.pos.z - angle.z * 25.0;
        let spawn = Vec3::new(x, terrain.wgen.ground_height(x, z) + if statue.boss.def().flying { 8.0 } else { 0.2 }, z);
        let level = info.level;
        crate::game::mobs::spawn_mob(&mut commands, statue.boss, spawn, level, tick.0, 0.0, false);
        info.boss_alive = true;
        info!("{} awakened a statue: {:?}", player.name, statue.boss);
        events.write(ToClients { targets: SendTargets::All, message: GameEvent::StatueAwakened { pos: statue.pos } });
        events.write(ToClients { targets: SendTargets::All, message: GameEvent::BossSpawned });
    }
}

/// Applies the card chosen by a player.
#[allow(clippy::too_many_arguments)]
pub fn apply_choice(
    commands: &mut Commands,
    entity: Entity,
    player: &Player,
    choice: &PendingChoice,
    index: usize,
    items: &mut Items,
    gear: &mut Gear,
    events: &mut MessageWriter<ToClients<GameEvent>>,
) {
    let Some(option) = choice.options.get(index) else { return };
    let reward = match option {
        CardOption::Item(item) => {
            items.0.add(*item);
            Reward::Item(*item)
        }
        CardOption::Offer(card) => {
            crate::shared::build::apply_offer(card, &mut gear.0, &mut items.0);
            Reward::Offer(card.clone())
        }
    };
    commands.entity(entity).remove::<PendingChoice>();
    info!("{} chose {reward:?}", player.name);
    events.write(ToClients { targets: SendTargets::All, message: GameEvent::Gained { slot: player.slot, reward } });
}

fn receive_choices(
    mut commands: Commands,
    mut choices: MessageReader<FromClient<ChooseCard>>,
    mut players: Query<(Entity, &Player, &super::players::Owner, &PendingChoice, &mut Items, &mut Gear)>,
    mut events: MessageWriter<ToClients<GameEvent>>,
) {
    for msg in choices.read() {
        let Some((entity, player, _, choice, mut items, mut gear)) =
            players.iter_mut().find(|(_, _, o, ..)| o.client == Some(msg.client_id))
        else {
            continue;
        };
        apply_choice(&mut commands, entity, player, choice, msg.index as usize, &mut items, &mut gear, &mut events);
    }
}

fn despawn_after(mut commands: Commands, mut q: Query<(Entity, &mut DespawnAfter)>, time: Res<Time>) {
    for (e, mut d) in &mut q {
        d.0 -= time.delta_secs();
        if d.0 <= 0.0 {
            commands.entity(e).try_despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::items::{Inventory, ItemKind};

    #[test]
    fn treasure_map_lowers_price() {
        let base = PlayerStats::from_inventory(&Inventory::default());
        let mut inv = Inventory::default();
        inv.add(ItemKind::TreasureMap);
        let map = PlayerStats::from_inventory(&inv);
        assert!(player_price(ChestKind::Large, 3, 60.0, &map) < player_price(ChestKind::Large, 3, 60.0, &base));
        assert_eq!(player_price(ChestKind::Small, 0, 0.0, &base), economy::chest_price_v3(ChestKind::Small, 0, 0.0));
    }
}
