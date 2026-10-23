//! Chests (server): spawned from the deterministic chest spots of the chunks
//! around players, opened with gold, giving items or weapons.

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
use crate::shared::rng::{hash_combine, Rng};
use crate::shared::weapons::WeaponKind;
use crate::shared::worldgen::{chunk_of, CHUNK_SIZE};

pub const INTERACT_RANGE: f32 = 2.8;
const CHEST_STREAM_RADIUS: i32 = 3;
const CHEST_FORGET_DISTANCE: f32 = CHUNK_SIZE * 6.0;

#[derive(Resource, Default)]
pub struct ChestRegistry {
    pub spawned: HashMap<u64, Entity>,
    pub opened: HashSet<u64>,
}

/// Despawns the entity after a delay (opened chests linger a bit).
#[derive(Component)]
pub struct DespawnAfter(pub f32);

pub struct ChestsPlugin;

impl Plugin for ChestsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChestRegistry>().add_systems(
            FixedUpdate,
            (stream_chests, pick_up_weapons, open_chests, despawn_after).chain().in_set(ServerSet::World),
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

/// Weapon from a weapon crate: rarity odds, never one you already carry.
pub fn roll_weapon(owned: &[WeaponKind], rng: &mut Rng) -> WeaponKind {
    let mut exclude = owned.to_vec();
    exclude.push(WeaponKind::Pistol);
    WeaponKind::roll([50.0, 32.0, 14.0, 4.0], &exclude, rng)
}

pub const PICKUP_RANGE: f32 = 2.5;

/// Pressing E near a weapon on the ground takes it. With 2 weapons already,
/// the weapon in hand is dropped in its place.
fn pick_up_weapons(
    mut commands: Commands,
    mut players: Query<(&Player, &mut PlayerState, &mut Intent, &Health, &Items)>,
    pickups: Query<(Entity, &WeaponPickup)>,
    mut taken: Local<Vec<Entity>>,
    mut events: MessageWriter<ToClients<GameEvent>>,
) {
    taken.clear();
    for (player, mut state, mut intent, health, items) in &mut players {
        if !intent.interact_pressed || health.downed {
            continue;
        }
        let pos = state.sim.mv.pos + Vec3::Y * 0.9;
        let nearest = pickups
            .iter()
            .filter(|(e, p)| !taken.contains(e) && p.pos.distance(pos) < PICKUP_RANGE)
            .min_by(|a, b| a.1.pos.distance(pos).total_cmp(&b.1.pos.distance(pos)));
        let Some((entity, pickup)) = nearest else { continue };
        // This press is used: don't also open a chest with it.
        intent.interact_pressed = false;
        taken.push(entity);
        commands.entity(entity).try_despawn();
        let stats = PlayerStats::from_inventory(&items.0);
        if let Some(dropped) = state.sim.arsenal.give(pickup.kind, &stats) {
            let forward = Vec3::new(-state.yaw.sin(), 0.0, -state.yaw.cos());
            commands.spawn((Replicated, WeaponPickup { kind: dropped, pos: pos + forward * 0.8 }));
        }
        info!("{} took {:?}", player.name, pickup.kind);
        events.write(ToClients { targets: SendTargets::All, message: GameEvent::Gained { slot: player.slot, reward: Reward::Weapon(pickup.kind) } });
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
    &'a mut PlayerScore,
    Has<PendingChoice>,
);

/// Opening a chest: pay, then either draw cards (item chests) or get a weapon.
fn open_chests(
    mut commands: Commands,
    mut players: Query<OpenerQuery>,
    mut chests: Query<(Entity, &Chest, &mut ChestOpen)>,
    mut registry: ResMut<ChestRegistry>,
    info: Single<&MatchInfo>,
    tick: Res<SimTick>,
    mut events: MessageWriter<ToClients<GameEvent>>,
) {
    for (player_entity, player, state, intent, health, mut wallet, items, mut score, pending) in &mut players {
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
        let price = player_price(chest.kind, info.wave, &stats);
        if wallet.gold < price {
            continue;
        }
        wallet.gold -= price;
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
        if chest.kind == ChestKind::Weapon {
            // The weapon pops out of the crate onto the ground: walk to it and press E.
            let owned: Vec<WeaponKind> = state.sim.arsenal.weapons.iter().map(|w| w.kind).collect();
            let weapon = roll_weapon(&owned, &mut rng);
            info!("{} opened a weapon crate for {price} gold: {weapon:?}", player.name);
            let toward_player = (pos - chest.pos).with_y(0.0).normalize_or_zero();
            commands.spawn((Replicated, WeaponPickup { kind: weapon, pos: chest.pos + toward_player * 1.2 + Vec3::Y * 0.9 }));
        } else {
            let options = crate::shared::items::roll_cards(chest.kind, stats.luck, stats.cards, &mut rng);
            info!("{} opened a {:?} chest for {price} gold: cards {options:?}", player.name, chest.kind);
            commands.entity(player_entity).try_insert(PendingChoice { chest: chest.kind, options });
        }
    }
}

/// Price of a chest for a given player (Treasure Map discount).
pub fn player_price(kind: ChestKind, wave: u32, stats: &PlayerStats) -> u32 {
    ((economy::chest_price(kind, wave) as f32) * stats.price_mult).round() as u32
}

/// Applies the card chosen by a player.
pub fn apply_choice(
    commands: &mut Commands,
    entity: Entity,
    player: &Player,
    choice: &PendingChoice,
    index: usize,
    items: &mut Items,
    events: &mut MessageWriter<ToClients<GameEvent>>,
) {
    let Some(item) = choice.options.get(index).copied() else { return };
    items.0.add(item);
    commands.entity(entity).remove::<PendingChoice>();
    info!("{} chose {item:?}", player.name);
    events.write(ToClients { targets: SendTargets::All, message: GameEvent::Gained { slot: player.slot, reward: Reward::Item(item) } });
}

fn receive_choices(
    mut commands: Commands,
    mut choices: MessageReader<FromClient<ChooseCard>>,
    mut players: Query<(Entity, &Player, &super::players::Owner, &PendingChoice, &mut Items)>,
    mut events: MessageWriter<ToClients<GameEvent>>,
) {
    for msg in choices.read() {
        let Some((entity, player, _, choice, mut items)) = players.iter_mut().find(|(_, _, o, ..)| o.client == Some(msg.client_id))
        else {
            continue;
        };
        apply_choice(&mut commands, entity, player, choice, msg.index as usize, &mut items, &mut events);
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
    fn weapon_crate_gives_new_weapons_first() {
        let mut rng = Rng::new(1);
        let owned = vec![WeaponKind::Pistol, WeaponKind::Shotgun];
        for _ in 0..200 {
            let w = roll_weapon(&owned, &mut rng);
            assert!(!owned.contains(&w));
        }
    }

    #[test]
    fn treasure_map_lowers_price() {
        let base = PlayerStats::from_inventory(&Inventory::default());
        let mut inv = Inventory::default();
        inv.add(ItemKind::TreasureMap);
        let map = PlayerStats::from_inventory(&inv);
        assert!(player_price(ChestKind::Large, 3, &map) < player_price(ChestKind::Large, 3, &base));
        assert_eq!(player_price(ChestKind::Small, 1, &base), economy::chest_price(ChestKind::Small, 1));
    }
}
