//! Chests (server): spawned from the deterministic chest spots of the chunks
//! around players, opened with gold, giving items or weapons.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy_replicon::prelude::*;

use super::players::{Intent, PlayerScore};
use super::world::TerrainRes;
use super::{ServerSet, SimTick};
use crate::net::protocol::*;
use crate::shared::economy::{self, ChestKind};
use crate::shared::items::{ItemKind, PlayerStats};
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
            (stream_chests, open_chests, despawn_after).chain().in_set(ServerSet::World),
        );
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

/// Picks the reward of a chest. Weapon chests prefer weapons you don't own.
pub fn roll_reward(kind: ChestKind, owned: &[WeaponKind], rng: &mut Rng) -> Reward {
    match kind {
        ChestKind::Item => Reward::Item(ItemKind::roll(rng)),
        ChestKind::Weapon => {
            let candidates: Vec<WeaponKind> = WeaponKind::ALL
                .into_iter()
                .filter(|w| *w != WeaponKind::Pistol && !owned.contains(w))
                .collect();
            if candidates.is_empty() {
                // Everything owned: a weapon chest still gives something useful.
                Reward::Item(ItemKind::roll(rng))
            } else {
                Reward::Weapon(*rng.pick(&candidates))
            }
        }
    }
}

type OpenerQuery<'a> = (
    &'a Player,
    &'a mut PlayerState,
    &'a Intent,
    &'a Health,
    &'a mut Wallet,
    &'a mut Items,
    &'a mut PlayerScore,
);

fn open_chests(
    mut commands: Commands,
    mut players: Query<OpenerQuery>,
    mut chests: Query<(Entity, &Chest, &mut ChestOpen)>,
    mut registry: ResMut<ChestRegistry>,
    info: Single<&MatchInfo>,
    tick: Res<SimTick>,
    mut events: MessageWriter<ToClients<GameEvent>>,
) {
    for (player, mut state, intent, health, mut wallet, mut items, mut score) in &mut players {
        if !intent.interact_pressed || health.downed {
            continue;
        }
        let pos = state.sim.mv.pos;
        let nearest = chests
            .iter_mut()
            .filter(|(_, c, o)| !o.opened && c.pos.distance(pos + Vec3::Y * 0.5) < INTERACT_RANGE)
            .min_by(|a, b| a.1.pos.distance(pos).total_cmp(&b.1.pos.distance(pos)));
        let Some((entity, chest, mut open)) = nearest else { continue };

        let price = economy::chest_price(chest.kind, info.difficulty);
        if wallet.gold < price {
            continue;
        }
        wallet.gold -= price;
        open.opened = true;
        registry.opened.insert(chest.spot_id);
        registry.spawned.remove(&chest.spot_id);
        commands.entity(entity).try_insert(DespawnAfter(10.0));
        score.chests += 1;

        let owned: Vec<WeaponKind> = state.sim.arsenal.weapons.iter().map(|w| w.kind).collect();
        let mut rng = Rng::new(hash_combine(&[chest.spot_id, u64::from(tick.0), u64::from(player.slot)]));
        let reward = roll_reward(chest.kind, &owned, &mut rng);
        match reward {
            Reward::Item(item) => items.0.add(item),
            Reward::Weapon(w) => {
                let stats = PlayerStats::from_inventory(&items.0);
                state.sim.arsenal.give(w, &stats);
            }
        }
        info!("{} opened a {:?} chest for {price} gold: {reward:?}", player.name, chest.kind);
        events.write(ToClients {
            targets: SendTargets::All,
            message: GameEvent::ChestOpened { pos: chest.pos, slot: player.slot, reward },
        });
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

    #[test]
    fn weapon_chest_gives_new_weapons_first() {
        let mut rng = Rng::new(1);
        let owned = vec![WeaponKind::Pistol, WeaponKind::Shotgun];
        for _ in 0..200 {
            match roll_reward(ChestKind::Weapon, &owned, &mut rng) {
                Reward::Weapon(w) => assert!(!owned.contains(&w)),
                Reward::Item(_) => panic!("should give a weapon"),
            }
        }
    }

    #[test]
    fn weapon_chest_with_everything_gives_item() {
        let mut rng = Rng::new(2);
        let owned = WeaponKind::ALL.to_vec();
        assert!(matches!(roll_reward(ChestKind::Weapon, &owned, &mut rng), Reward::Item(_)));
    }

    #[test]
    fn item_chest_gives_items() {
        let mut rng = Rng::new(3);
        assert!(matches!(roll_reward(ChestKind::Item, &[], &mut rng), Reward::Item(_)));
    }
}
